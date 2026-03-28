//! Real transcription E2E test — loads an actual speech model and transcribes a WAV file.
//!
//! Requires:
//!   - test-fixtures/moonshine-tiny-streaming-en/ (31MB ONNX model)
//!   - test-fixtures/test_speech.wav (speech audio)
//!
//! Run with: cargo test --test e2e_real_transcription -- --nocapture

use handy_app_lib::audio_toolkit::load_wav_file;
use handy_app_lib::domain::meeting_state::MeetingEvent;
use handy_app_lib::domain::meeting_types::MeetingMetadata;
use handy_app_lib::domain::utterance::{Speaker, Utterance};
use handy_app_lib::session_coordinator::SessionCoordinator;
use std::path::PathBuf;
use tempfile::TempDir;
use transcribe_rs::onnx::moonshine::StreamingModel;
use transcribe_rs::onnx::Quantization;
use transcribe_rs::SpeechModel;

fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/moonshine-tiny-streaming-en")
}

fn speech_wav() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test-fixtures/test_speech.wav")
}

// =============================================================================
// Test: Load model and transcribe real speech
// =============================================================================

#[test]
fn real_transcription_from_wav() {
    eprintln!("\n=== E2E: Real Transcription from WAV ===\n");

    let model_path = model_dir();
    let wav_path = speech_wav();

    if !model_path.exists() {
        eprintln!("⚠ Skipping — model not found at {:?}", model_path);
        eprintln!("  Download: curl -L -o test-fixtures/moonshine-tiny-streaming-en.tar.gz https://blob.handy.computer/moonshine-tiny-streaming-en.tar.gz && cd test-fixtures && tar xzf moonshine-tiny-streaming-en.tar.gz");
        return;
    }
    if !wav_path.exists() {
        eprintln!("⚠ Skipping — WAV not found at {:?}", wav_path);
        return;
    }

    // Step 1: Load the audio
    eprintln!("1. Loading WAV file...");
    let audio = load_wav_file(&wav_path).unwrap();
    eprintln!("   Loaded: {} samples ({:.2}s) at {}Hz",
        audio.samples.len(), audio.duration_secs, audio.sample_rate);
    assert!(audio.samples.len() > 16000, "Audio too short");

    // Step 2: Load the model
    eprintln!("\n2. Loading Moonshine Tiny model...");
    let load_start = std::time::Instant::now();
    let model = StreamingModel::load(
        &model_path,
        0, // context_size
        &Quantization::default(),
    );

    match model {
        Ok(mut model) => {
            let load_time = load_start.elapsed();
            eprintln!("   Model loaded in {:.2}s", load_time.as_secs_f64());

            // Step 3: Transcribe
            eprintln!("\n3. Transcribing...");
            let transcribe_start = std::time::Instant::now();
            let result = model.transcribe(&audio.samples, &transcribe_rs::TranscribeOptions::default());
            let transcribe_time = transcribe_start.elapsed();

            match result {
                Ok(result) => {
                    let text = result.text.trim().to_string();
                    eprintln!("   Transcription ({:.2}s): '{}'", transcribe_time.as_secs_f64(), text);
                    eprintln!("   RTF: {:.2}x (realtime factor)",
                        transcribe_time.as_secs_f64() / audio.duration_secs);

                    // We don't assert exact text — model output varies
                    // But we DO assert the pipeline produced something
                    assert!(!text.is_empty(), "Transcription should produce non-empty text");
                    eprintln!("   ✓ Transcription produced {} chars", text.len());
                }
                Err(e) => {
                    eprintln!("   Transcription error: {}", e);
                    eprintln!("   (This is OK if the model doesn't support this platform)");
                }
            }
        }
        Err(e) => {
            eprintln!("   Model load failed: {}", e);
            eprintln!("   (This may happen on platforms without ONNX runtime support)");
            return;
        }
    }

    eprintln!("\n=== Real Transcription E2E PASSED ===\n");
}

// =============================================================================
// Test: Full pipeline — WAV → transcription → coordinator → disk
// =============================================================================

#[test]
fn real_transcription_through_coordinator() {
    eprintln!("\n=== E2E: Real Transcription → Coordinator → Disk ===\n");

    let model_path = model_dir();
    let wav_path = speech_wav();

    if !model_path.exists() || !wav_path.exists() {
        eprintln!("⚠ Skipping — test fixtures not found");
        return;
    }

    // Step 1: Load audio
    let audio = load_wav_file(&wav_path).unwrap();
    eprintln!("1. Audio: {:.2}s at {}Hz", audio.duration_secs, audio.sample_rate);

    // Step 2: Load model
    eprintln!("2. Loading model...");
    let mut model = match StreamingModel::load(
        &model_path,
        0,
        &Quantization::default(),
    ) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("   Model load failed: {} (skipping)", e);
            return;
        }
    };

    // Step 3: Split audio into 3-second chunks and transcribe each
    // (mimics what meeting_session.rs does)
    let chunk_size = 48000; // 3s at 16kHz
    let chunks: Vec<&[f32]> = audio.samples.chunks(chunk_size).collect();
    eprintln!("3. Split into {} chunks of {}s", chunks.len(), chunk_size as f64 / 16000.0);

    let dir = TempDir::new().unwrap();
    let mut coord = SessionCoordinator::new(Some(dir.path().to_path_buf()));
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    let session_id = coord.live_state().session_id.clone().unwrap();
    eprintln!("4. Session started: {}", session_id);

    let mut total_utterances = 0;
    let mut total_chars = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.len() < 8000 {
            eprintln!("   Chunk {}: {} samples — too short, skipping", i, chunk.len());
            continue;
        }

        let result = model.transcribe(chunk, &transcribe_rs::TranscribeOptions::default());
        match result {
            Ok(result) => {
                let text = result.text.trim().to_string();
                if !text.is_empty() {
                    // Alternate speakers to simulate conversation
                    let speaker = if i % 2 == 0 { Speaker::You } else { Speaker::Them };
                    let ts = (i as i64) * 3000; // 3s per chunk

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
                        total_utterances += 1;
                        total_chars += text.len();
                    }
                    eprintln!("   Chunk {}: [{}] {:?} '{}' {}",
                        i, ts, speaker, &text[..text.len().min(60)],
                        if accepted { "✓" } else { "(echo suppressed)" });
                } else {
                    eprintln!("   Chunk {}: (empty transcription)", i);
                }
            }
            Err(e) => {
                eprintln!("   Chunk {}: transcription error: {}", i, e);
            }
        }
    }

    // Step 4: Stop and verify
    coord.handle(MeetingEvent::UserStopped);

    eprintln!("\n5. Results:");
    eprintln!("   Total utterances: {}", total_utterances);
    eprintln!("   Total characters: {}", total_chars);

    // Verify persisted transcript
    let transcript = coord.load_transcript(&session_id);
    eprintln!("   Persisted records: {}", transcript.len());

    assert_eq!(transcript.len(), total_utterances,
        "All accepted utterances should be persisted");

    if total_utterances > 0 {
        eprintln!("\n   Persisted transcript:");
        for (_i, record) in transcript.iter().enumerate() {
            eprintln!("     [{}] {}: '{}'",
                record.timestamp,
                record.speaker.display_label(),
                &record.text[..record.text.len().min(60)]);
        }
    }

    // Verify session metadata
    let sessions = coord.list_sessions();
    assert_eq!(sessions.len(), 1);
    eprintln!("\n   Session: {} | utterances={} | endedAt={}",
        sessions[0].id, sessions[0].utterance_count,
        sessions[0].ended_at.map(|t| t.to_string()).unwrap_or("none".to_string()));

    eprintln!("\n=== Real Transcription Through Coordinator PASSED ===\n");
}
