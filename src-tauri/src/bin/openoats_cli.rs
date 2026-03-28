//! OpenOats CLI — record from mic, transcribe in real-time, persist sessions.
//!
//! Usage:
//!   cargo run --bin openoats_cli -- [OPTIONS]
//!
//! Options:
//!   --model <path>    Path to transcription model directory (default: test-fixtures/moonshine-tiny-streaming-en)
//!   --device <index>  Audio input device index (default: system default)
//!   --list-devices    List available audio input devices and exit
//!   --output <dir>    Session output directory (default: ./openoats-sessions)

use handy_app_lib::audio_toolkit::{list_input_devices, AudioRecorder, SileroVad};
use handy_app_lib::audio_toolkit::vad::SmoothedVad;
use handy_app_lib::domain::meeting_state::MeetingEvent;
use handy_app_lib::domain::meeting_types::MeetingMetadata;
use handy_app_lib::domain::utterance::{Speaker, Utterance};
use handy_app_lib::session_coordinator::SessionCoordinator;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use transcribe_rs::onnx::moonshine::StreamingModel;
use transcribe_rs::onnx::Quantization;
use transcribe_rs::SpeechModel;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();

    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return Ok(());
    }

    if args.contains(&"--list-devices".to_string()) {
        let devices = list_input_devices()?;
        println!("Audio input devices:");
        for (i, d) in devices.iter().enumerate() {
            println!("  {}: {}", i, d.name);
        }
        return Ok(());
    }

    let model_path = get_arg(&args, "--model")
        .unwrap_or_else(|| "test-fixtures/moonshine-tiny-streaming-en".to_string());
    let output_dir = get_arg(&args, "--output")
        .unwrap_or_else(|| "./openoats-sessions".to_string());
    let device_idx: Option<usize> = get_arg(&args, "--device")
        .and_then(|s| s.parse().ok());
    let file_path = get_arg(&args, "--file");

    println!("╔══════════════════════════════════════╗");
    println!("║       OpenOats CLI Transcriber        ║");
    println!("╚══════════════════════════════════════╝");
    println!();

    // Load model
    let model_dir = PathBuf::from(&model_path);
    if !model_dir.exists() {
        eprintln!("Model not found at: {}", model_path);
        eprintln!("Download with:");
        eprintln!("  curl -L -o moonshine-tiny-streaming-en.tar.gz https://blob.handy.computer/moonshine-tiny-streaming-en.tar.gz");
        eprintln!("  tar xzf moonshine-tiny-streaming-en.tar.gz");
        return Err("Model not found".into());
    }

    print!("Loading transcription model... ");
    io::stdout().flush()?;
    let load_start = Instant::now();
    let mut model = StreamingModel::load(&model_dir, 0, &Quantization::default())
        .map_err(|e| format!("Failed to load model: {}", e))?;
    println!("done ({:.1}s)", load_start.elapsed().as_secs_f64());

    // Set up coordinator
    let output = PathBuf::from(&output_dir);
    let mut coord = SessionCoordinator::new(Some(output.clone()));

    // === FILE MODE: transcribe a WAV and exit ===
    if let Some(ref wav_file) = file_path {
        let wav_path = PathBuf::from(wav_file);
        if !wav_path.exists() {
            return Err(format!("WAV file not found: {}", wav_file).into());
        }

        println!("Mode: file transcription");
        println!("Input: {}", wav_file);
        println!();

        let audio = handy_app_lib::audio_toolkit::load_wav_file(&wav_path)
            .map_err(|e| format!("Failed to load WAV: {}", e))?;
        println!("Audio: {:.1}s at {}Hz", audio.duration_secs, audio.sample_rate);

        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        let session_id = coord.live_state().session_id.clone().unwrap();
        println!("Session: {}\n", session_id);

        let mut chunk_count = 0u32;
        transcribe_and_ingest(&mut model, &audio.samples, &mut coord, &mut chunk_count);

        coord.handle(MeetingEvent::UserStopped);

        let transcript = coord.load_transcript(&session_id);
        println!("\n--- Session complete ---");
        println!("Utterances: {}", transcript.len());
        println!("Saved to: {}/sessions/{}/", output.display(), session_id);

        return Ok(());
    }

    // === LIVE MODE: record from mic ===

    // Set up VAD
    let vad_path = PathBuf::from("resources/models/silero_vad_v4.onnx");
    let has_vad = vad_path.exists();
    if !has_vad {
        println!("VAD model not found — recording without voice activity detection");
    }

    // Set up audio recorder
    let devices = list_input_devices()?;
    let device = device_idx.and_then(|i| devices.get(i).map(|d| d.device.clone()));
    let device_name = device_idx
        .and_then(|i| devices.get(i))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let mut recorder = AudioRecorder::new()?;
    if has_vad {
        let silero = SileroVad::new(vad_path.to_str().unwrap(), 0.5)?;
        let smoothed = SmoothedVad::new(Box::new(silero), 15, 15, 3);
        recorder = recorder.with_vad(Box::new(smoothed));
    }

    println!("Audio device: {}", device_name);
    println!("Output: {}", output.display());
    println!();
    println!("Commands:");
    println!("  [Enter]  Start/stop recording");
    println!("  q        Quit");
    println!("  d        List devices");
    println!("  s        Show session status");
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).ok();

    let mut is_recording = false;
    let mut chunk_count = 0u32;

    while running.load(Ordering::SeqCst) {
        if is_recording {
            print!("[recording] > ");
        } else {
            print!("[idle] > ");
        }
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        let cmd = input.trim();

        match cmd {
            "q" | "quit" | "exit" => {
                if is_recording {
                    println!("Stopping recording...");
                    let samples = recorder.stop()?;
                    recorder.close()?;
                    is_recording = false;

                    if !samples.is_empty() {
                        transcribe_and_ingest(&mut model, &samples, &mut coord, &mut chunk_count);
                    }

                    coord.handle(MeetingEvent::UserStopped);
                    let sessions = coord.list_sessions();
                    if let Some(last) = sessions.first() {
                        println!("\nSession saved: {} ({} utterances)",
                            last.id, last.utterance_count);
                    }
                }
                println!("Goodbye!");
                break;
            }

            "d" | "devices" => {
                let devices = list_input_devices()?;
                println!("Audio input devices:");
                for (i, d) in devices.iter().enumerate() {
                    println!("  {}: {}", i, d.name);
                }
            }

            "s" | "status" => {
                let state = coord.live_state();
                println!("Recording: {}", state.is_recording);
                println!("Session: {}", state.session_id.as_deref().unwrap_or("none"));
                println!("Utterances: {}", state.utterance_count);
                let sessions = coord.list_sessions();
                println!("Past sessions: {}", sessions.len());
            }

            "" => {
                // Toggle recording
                if is_recording {
                    // Stop
                    println!("Stopping...");
                    let samples = recorder.stop()?;
                    recorder.close()?;
                    is_recording = false;

                    if !samples.is_empty() {
                        transcribe_and_ingest(&mut model, &samples, &mut coord, &mut chunk_count);
                    }

                    coord.handle(MeetingEvent::UserStopped);

                    let sessions = coord.list_sessions();
                    if let Some(last) = sessions.first() {
                        println!("\n--- Session complete ---");
                        println!("ID: {}", last.id);
                        println!("Utterances: {}", last.utterance_count);

                        // Print transcript
                        let transcript = coord.load_transcript(&last.id);
                        if !transcript.is_empty() {
                            println!("\nTranscript:");
                            for record in &transcript {
                                println!("  [{}] {}", record.speaker.display_label(), record.text);
                            }
                        }

                        println!("Saved to: {}/sessions/{}/", output.display(), last.id);
                    }
                    println!();
                    chunk_count = 0;
                } else {
                    // Start
                    recorder.open(device.clone())?;
                    recorder.start()?;
                    is_recording = true;
                    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
                    println!("Recording... (press Enter to stop)");
                }
            }

            _ => {
                println!("Unknown command. Press Enter to start/stop, 'q' to quit.");
            }
        }
    }

    Ok(())
}

fn transcribe_and_ingest(
    model: &mut StreamingModel,
    samples: &[f32],
    coord: &mut SessionCoordinator,
    chunk_count: &mut u32,
) {
    let chunk_size = 48000; // 3 seconds at 16kHz
    let chunks: Vec<&[f32]> = samples.chunks(chunk_size).collect();

    println!("\nTranscribing {} chunks ({:.1}s of audio)...\n",
        chunks.len(), samples.len() as f64 / 16000.0);

    for chunk in &chunks {
        if chunk.len() < 8000 {
            continue;
        }

        match model.transcribe(chunk, &transcribe_rs::TranscribeOptions::default()) {
            Ok(result) => {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    *chunk_count += 1;
                    let speaker = Speaker::You;
                    let ts = chrono::Utc::now().timestamp_millis();

                    let utterance = Utterance {
                        id: uuid::Uuid::new_v4().to_string(),
                        text: text.clone(),
                        speaker: speaker.clone(),
                        timestamp: ts,
                        refined_text: None,
                        refinement_status: None,
                    };

                    let accepted = coord.on_utterance(utterance);
                    if accepted {
                        println!("  [{}] {}: {}", chunk_count, speaker.display_label(), text);
                    }
                }
            }
            Err(e) => {
                eprintln!("  Transcription error: {}", e);
            }
        }
    }
}

fn get_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn print_usage() {
    println!("OpenOats CLI — Record from mic or transcribe WAV files");
    println!();
    println!("Usage:");
    println!("  openoats_cli                         Interactive mic recording");
    println!("  openoats_cli --file input.wav         Transcribe a WAV file");
    println!();
    println!("Options:");
    println!("  --model <path>    Transcription model directory");
    println!("                    (default: test-fixtures/moonshine-tiny-streaming-en)");
    println!("  --file <path>     Transcribe a WAV file instead of recording from mic");
    println!("  --device <index>  Audio input device index (live mode only)");
    println!("  --list-devices    List audio devices and exit");
    println!("  --output <dir>    Session output directory (default: ./openoats-sessions)");
    println!("  -h, --help        Show this help");
}
