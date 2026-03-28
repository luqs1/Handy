//! E2E audio pipeline tests.
//! Tests the full flow: WAV file → load → audio processing → utterance → coordinator → disk.
//!
//! Run with: cargo test --test e2e_audio_pipeline -- --nocapture

use handy_app_lib::audio_toolkit::{load_wav_file, SileroVad, VoiceActivityDetector, WavAudio};
use handy_app_lib::domain::meeting_state::MeetingEvent;
use handy_app_lib::domain::meeting_types::MeetingMetadata;
use handy_app_lib::domain::utterance::{Speaker, Utterance};
use handy_app_lib::session_coordinator::SessionCoordinator;
use std::f32::consts::PI;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// Helpers: Generate synthetic audio
// =============================================================================

/// Generate a sine wave at the given frequency, 16kHz mono f32.
fn generate_sine(frequency_hz: f32, duration_secs: f32) -> Vec<f32> {
    let sample_rate = 16000.0;
    let num_samples = (sample_rate * duration_secs) as usize;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (2.0 * PI * frequency_hz * t).sin() * 0.5 // amplitude 0.5
        })
        .collect()
}

/// Generate silence (zeros).
fn generate_silence(duration_secs: f32) -> Vec<f32> {
    let num_samples = (16000.0 * duration_secs) as usize;
    vec![0.0; num_samples]
}

/// Generate white noise (random values).
fn generate_noise(duration_secs: f32, amplitude: f32) -> Vec<f32> {
    let num_samples = (16000.0 * duration_secs) as usize;
    (0..num_samples)
        .map(|i| {
            // Deterministic pseudo-random based on index
            let x = (i as f32 * 0.123456).sin() * 43758.5453;
            (x - x.floor()) * 2.0 * amplitude - amplitude
        })
        .collect()
}

/// Save f32 samples as a 16kHz mono WAV file (sync version for tests).
fn save_test_wav(path: &std::path::Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for &s in samples {
        let s_i16 = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(s_i16).unwrap();
    }
    writer.finalize().unwrap();
}

/// Save f32 samples as a stereo 44.1kHz WAV (to test format conversion).
fn save_test_wav_stereo_44k(path: &std::path::Path, mono_samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    // Upsample 16k → 44.1k naively (repeat samples) and write stereo
    let ratio = 44100.0 / 16000.0;
    let num_out = (mono_samples.len() as f64 * ratio) as usize;
    for i in 0..num_out {
        let src_idx = (i as f64 / ratio) as usize;
        let s = mono_samples.get(src_idx).copied().unwrap_or(0.0);
        let s_i16 = (s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        writer.write_sample(s_i16).unwrap(); // left
        writer.write_sample(s_i16).unwrap(); // right
    }
    writer.finalize().unwrap();
}

// =============================================================================
// Test: WAV round-trip (write → load → verify samples match)
// =============================================================================

#[test]
fn wav_roundtrip_16k_mono() {
    let dir = TempDir::new().unwrap();
    let wav_path = dir.path().join("test_16k_mono.wav");

    eprintln!("\n=== E2E: WAV Round-Trip (16kHz Mono) ===\n");

    // Generate 2 seconds of 440Hz sine
    let original = generate_sine(440.0, 2.0);
    eprintln!("Generated: {} samples ({:.2}s)", original.len(), original.len() as f64 / 16000.0);
    save_test_wav(&wav_path, &original);

    let file_size = fs::metadata(&wav_path).unwrap().len();
    eprintln!("WAV file: {} bytes", file_size);

    // Load it back
    let loaded = load_wav_file(&wav_path).unwrap();
    eprintln!("Loaded: {} samples ({:.2}s) at {}Hz, {} channels",
        loaded.samples.len(), loaded.duration_secs, loaded.sample_rate, loaded.channels);

    assert_eq!(loaded.sample_rate, 16000);
    assert_eq!(loaded.channels, 1);
    assert_eq!(loaded.samples.len(), original.len());

    // Verify samples match within quantization error (f32 → i16 → f32)
    let max_error: f32 = loaded.samples.iter().zip(original.iter())
        .map(|(a, b): (&f32, &f32)| (a - b).abs())
        .fold(0.0f32, f32::max);
    eprintln!("Max quantization error: {:.6} (expected < 0.001)", max_error);
    assert!(max_error < 0.001, "Quantization error too high: {}", max_error);

    eprintln!("\n=== WAV Round-Trip PASSED ===\n");
}

// =============================================================================
// Test: WAV format conversion (stereo 44.1kHz → mono 16kHz)
// =============================================================================

#[test]
fn wav_format_conversion() {
    let dir = TempDir::new().unwrap();
    let wav_path = dir.path().join("test_44k_stereo.wav");

    eprintln!("\n=== E2E: WAV Format Conversion (44.1kHz Stereo → 16kHz Mono) ===\n");

    let mono_16k = generate_sine(440.0, 1.0);
    save_test_wav_stereo_44k(&wav_path, &mono_16k);

    let loaded = load_wav_file(&wav_path).unwrap();
    eprintln!("Input: 44100Hz stereo → Output: {}Hz {} channel(s), {} samples ({:.2}s)",
        loaded.sample_rate, loaded.channels, loaded.samples.len(), loaded.duration_secs);

    assert_eq!(loaded.sample_rate, 16000);
    assert_eq!(loaded.channels, 1);
    // Duration should be approximately the same (within 5%)
    let expected_duration = mono_16k.len() as f64 / 16000.0;
    assert!((loaded.duration_secs - expected_duration).abs() < expected_duration * 0.05,
        "Duration mismatch: expected ~{:.2}s, got {:.2}s", expected_duration, loaded.duration_secs);

    eprintln!("\n=== Format Conversion PASSED ===\n");
}

// =============================================================================
// Test: VAD correctly detects speech vs silence
// =============================================================================

#[test]
fn vad_detects_speech_vs_silence() {
    eprintln!("\n=== E2E: Voice Activity Detection ===\n");

    // We need the Silero VAD model file. Check if it exists.
    let model_path = std::path::PathBuf::from("resources/models/silero_vad_v4.onnx");
    if !model_path.exists() {
        eprintln!("⚠ Skipping VAD test — model not found at {:?}", model_path);
        eprintln!("  Download with: curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx");
        return;
    }

    let mut vad = SileroVad::new(model_path.to_str().unwrap(), 0.5).unwrap();

    // Test 1: Pure silence should not be detected as speech
    let silence = generate_silence(1.0);
    let silence_frames: Vec<&[f32]> = silence.chunks(480).collect(); // 30ms frames
    let mut speech_count = 0;
    for frame in &silence_frames {
        if vad.is_voice(frame).unwrap_or(false) {
            speech_count += 1;
        }
    }
    let silence_speech_ratio = speech_count as f64 / silence_frames.len() as f64;
    eprintln!("Silence: {}/{} frames detected as speech ({:.0}%)",
        speech_count, silence_frames.len(), silence_speech_ratio * 100.0);
    assert!(silence_speech_ratio < 0.1, "Too many silence frames classified as speech");

    vad.reset();

    // Test 2: Loud noise should trigger some activity
    let noise = generate_noise(1.0, 0.8);
    let noise_frames: Vec<&[f32]> = noise.chunks(480).collect();
    let mut noise_speech_count = 0;
    for frame in &noise_frames {
        if vad.is_voice(frame).unwrap_or(false) {
            noise_speech_count += 1;
        }
    }
    eprintln!("Noise: {}/{} frames detected as speech",
        noise_speech_count, noise_frames.len());
    // We don't assert on noise — VAD behavior on pure noise is model-dependent

    eprintln!("\n=== VAD Test PASSED ===\n");
}

// =============================================================================
// Test: Audio → simulated transcription → coordinator → persisted JSONL
// =============================================================================

#[test]
fn audio_to_disk_full_pipeline() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    eprintln!("\n=== E2E: Audio → Coordinator → Disk Pipeline ===\n");

    // Step 1: Create WAV files simulating mic and system audio
    let mic_wav = dir.path().join("mic_input.wav");
    let sys_wav = dir.path().join("system_input.wav");

    let mic_audio = generate_sine(300.0, 3.0); // 3s of 300Hz (simulated speech pitch)
    let sys_audio = generate_sine(200.0, 3.0); // 3s of 200Hz
    save_test_wav(&mic_wav, &mic_audio);
    save_test_wav(&sys_wav, &sys_audio);

    eprintln!("1. Created test WAV files:");
    eprintln!("   mic_input.wav: {} samples ({:.1}s)", mic_audio.len(), mic_audio.len() as f64 / 16000.0);
    eprintln!("   system_input.wav: {} samples ({:.1}s)", sys_audio.len(), sys_audio.len() as f64 / 16000.0);

    // Step 2: Load and verify
    let mic_loaded = load_wav_file(&mic_wav).unwrap();
    let sys_loaded = load_wav_file(&sys_wav).unwrap();
    eprintln!("\n2. Loaded WAV files:");
    eprintln!("   mic: {} samples at {}Hz ({:.2}s)", mic_loaded.samples.len(), mic_loaded.sample_rate, mic_loaded.duration_secs);
    eprintln!("   sys: {} samples at {}Hz ({:.2}s)", sys_loaded.samples.len(), sys_loaded.sample_rate, sys_loaded.duration_secs);
    assert_eq!(mic_loaded.sample_rate, 16000);
    assert_eq!(sys_loaded.sample_rate, 16000);

    // Step 3: Verify audio properties (the pipeline would feed these to transcription)
    let mic_rms = rms(&mic_loaded.samples);
    let sys_rms = rms(&sys_loaded.samples);
    let mic_peak = mic_loaded.samples.iter().fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
    let sys_peak = sys_loaded.samples.iter().fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
    eprintln!("\n3. Audio properties:");
    eprintln!("   mic: RMS={:.4}, peak={:.4}", mic_rms, mic_peak);
    eprintln!("   sys: RMS={:.4}, peak={:.4}", sys_rms, sys_peak);
    assert!(mic_rms > 0.1, "Mic audio too quiet");
    assert!(sys_rms > 0.1, "System audio too quiet");
    assert!(mic_loaded.samples.len() >= 8000, "Mic audio too short for transcription (need >= 8000 samples)");

    // Step 4: Simulate what the transcription engine would do
    // (We can't load a real model in unit tests, so simulate the output)
    // In a real E2E test with a model loaded, this would be:
    //   let text = transcription_manager.transcribe(mic_loaded.samples)?;
    let simulated_transcriptions: Vec<(&str, Speaker, i64)> = vec![
        ("Let me show you how the transcription pipeline works on Windows", Speaker::You, 0),
        ("The WASAPI loopback captures all system audio at once", Speaker::Them, 3000),
        ("Can we filter that to just the meeting app?", Speaker::You, 6000),
        ("Not directly, but VAD helps separate speech from noise", Speaker::Them, 9000),
    ];

    eprintln!("\n4. Simulated transcription output (would come from whisper-rs in production):");
    for (text, speaker, ts) in &simulated_transcriptions {
        eprintln!("   [{}ms] {:?}: {}", ts, speaker, text);
    }

    // Step 5: Feed through coordinator (the real integration point)
    let mut coord = SessionCoordinator::new(Some(root.clone()));
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    let session_id = coord.live_state().session_id.clone().unwrap();

    eprintln!("\n5. Session started: {}", session_id);

    for (text, speaker, ts) in &simulated_transcriptions {
        let utterance = Utterance {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
            speaker: speaker.clone(),
            timestamp: *ts,
            refined_text: None,
            refinement_status: None,
        };
        let accepted = coord.on_utterance(utterance);
        eprintln!("   Utterance '{}' → {}", &text[..text.len().min(50)], if accepted { "accepted" } else { "suppressed" });
        assert!(accepted);
    }

    coord.handle(MeetingEvent::UserStopped);

    // Step 6: Verify everything on disk
    let session_dir = root.join("sessions").join(&session_id);
    eprintln!("\n6. Verifying on-disk artifacts:");

    // Transcript JSONL
    let transcript_path = session_dir.join("transcript.live.jsonl");
    let transcript_content = fs::read_to_string(&transcript_path).unwrap();
    let lines: Vec<&str> = transcript_content.lines().filter(|l| !l.is_empty()).collect();
    eprintln!("   transcript.live.jsonl: {} lines", lines.len());
    assert_eq!(lines.len(), 4);

    // Verify each line is valid JSON with correct speaker labels
    for (i, line) in lines.iter().enumerate() {
        let record: serde_json::Value = serde_json::from_str(line).unwrap();
        let speaker = record["speaker"].as_str().unwrap();
        let text = record["text"].as_str().unwrap();
        eprintln!("   Line {}: speaker='{}', text='{}'", i, speaker, &text[..text.len().min(50)]);

        match i {
            0 | 2 => assert_eq!(speaker, "you"),
            1 | 3 => assert_eq!(speaker, "them"),
            _ => {}
        }
    }

    // Session metadata
    let meta: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(session_dir.join("session.json")).unwrap()
    ).unwrap();
    eprintln!("   session.json: utteranceCount={}, endedAt={}",
        meta["utteranceCount"], meta["endedAt"]);
    assert_eq!(meta["utteranceCount"], 4);
    assert!(!meta["endedAt"].is_null());

    // WAV files still exist (audio preserved for batch re-transcription)
    assert!(mic_wav.exists(), "Mic WAV should still exist");
    assert!(sys_wav.exists(), "System WAV should still exist");

    eprintln!("\n=== Audio Pipeline E2E PASSED ===\n");
}

// =============================================================================
// Test: Echo suppression with audio timestamps
// =============================================================================

#[test]
fn audio_echo_suppression_realistic() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    eprintln!("\n=== E2E: Realistic Echo Suppression with Audio Timestamps ===\n");

    let mut coord = SessionCoordinator::new(Some(root));
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));

    // Simulate a real meeting where system audio bleeds into the mic.
    //
    // Timeline:
    //   0ms   - System audio captures "Them" saying something
    //   200ms - Mic picks up the same audio (acoustic echo, ~200ms delay)
    //   3000ms - "You" actually speaks (real response)
    //   3200ms - System picks up faint echo of "You" (but should be suppressed)

    let them_text = "We need to redesign the transcription pipeline for better accuracy on Windows";
    let you_text = "I agree, the whisper cpp backend gives us good Windows support through DirectML";

    // System captures "them" at t=0
    let them_utterance = Utterance {
        id: "u_0".to_string(),
        text: them_text.to_string(),
        speaker: Speaker::Them,
        timestamp: 0,
        refined_text: None,
        refinement_status: None,
    };
    assert!(coord.on_utterance(them_utterance));
    eprintln!("  [0ms] Them: '{}' → accepted", &them_text[..50]);

    // Mic picks up echo at t=200ms (should be suppressed)
    let echo_1 = Utterance {
        id: "u_200".to_string(),
        text: them_text.to_string(), // same text!
        speaker: Speaker::You,
        timestamp: 200,
        refined_text: None,
        refinement_status: None,
    };
    let echo_accepted = coord.on_utterance(echo_1);
    eprintln!("  [200ms] You (echo): '{}' → {}",
        &them_text[..50], if echo_accepted { "ACCEPTED (bug!)" } else { "suppressed ✓" });
    assert!(!echo_accepted, "Echo should be suppressed");

    // Real "You" response at t=3000ms (outside echo window)
    let you_utterance = Utterance {
        id: "u_3000".to_string(),
        text: you_text.to_string(),
        speaker: Speaker::You,
        timestamp: 3000,
        refined_text: None,
        refinement_status: None,
    };
    assert!(coord.on_utterance(you_utterance));
    eprintln!("  [3000ms] You: '{}' → accepted", &you_text[..50]);

    // System picks up faint echo of "You" at t=3200ms (should be suppressed)
    // Note: system audio → Speaker::Them, but the text matches "You"'s utterance
    // However, echo suppression only checks You→Them direction, not Them→You
    // This is by design: the mic is more likely to echo system audio than vice versa

    coord.handle(MeetingEvent::UserStopped);

    assert_eq!(coord.utterances().len(), 2, "Should have exactly 2 utterances (echo suppressed)");
    eprintln!("\n  Final: {} utterances persisted (1 echo suppressed)", coord.utterances().len());

    eprintln!("\n=== Echo Suppression E2E PASSED ===\n");
}

// =============================================================================
// Test: Audio chunking for batch transcription
// =============================================================================

#[test]
fn audio_chunking_for_batch_transcription() {
    eprintln!("\n=== E2E: Audio Chunking for Batch Transcription ===\n");

    // Simulate a 30-second meeting recording
    let total_duration = 30.0;
    let chunk_size = 48000; // 3 seconds at 16kHz (matches meeting_session.rs threshold)

    let full_audio = generate_sine(300.0, total_duration);
    eprintln!("Full recording: {} samples ({:.0}s)", full_audio.len(), total_duration);

    // Split into chunks like meeting_session does
    let chunks: Vec<&[f32]> = full_audio.chunks(chunk_size).collect();
    eprintln!("Split into {} chunks of {} samples ({}s each)",
        chunks.len(), chunk_size, chunk_size as f64 / 16000.0);

    // Verify each chunk meets minimum transcription length (8000 samples = 0.5s)
    let min_transcription_samples = 8000;
    for (i, chunk) in chunks.iter().enumerate() {
        let meets_min = chunk.len() >= min_transcription_samples;
        if i < 3 || i == chunks.len() - 1 {
            eprintln!("  Chunk {}: {} samples ({:.2}s) {}",
                i, chunk.len(), chunk.len() as f64 / 16000.0,
                if meets_min { "✓" } else { "✗ too short" });
        }
    }

    let valid_chunks = chunks.iter().filter(|c| c.len() >= min_transcription_samples).count();
    eprintln!("\n{}/{} chunks meet minimum transcription length", valid_chunks, chunks.len());
    assert!(valid_chunks >= chunks.len() - 1, "Most chunks should be transcribable");

    // Verify no audio lost in chunking
    let total_chunked_samples: usize = chunks.iter().map(|c| c.len()).sum();
    assert_eq!(total_chunked_samples, full_audio.len(), "No samples lost in chunking");

    eprintln!("\n=== Audio Chunking E2E PASSED ===\n");
}

// =============================================================================
// Test: Full pipeline with WAV file I/O verification
// =============================================================================

#[test]
fn wav_save_load_transcode_verify() {
    let dir = TempDir::new().unwrap();

    eprintln!("\n=== E2E: WAV Save → Load → Verify Audio Integrity ===\n");

    // Step 1: Create audio with known properties
    let freq = 1000.0; // 1kHz tone
    let duration = 1.0;
    let original = generate_sine(freq, duration);

    // Step 2: Save as WAV
    let wav_path = dir.path().join("tone_1khz.wav");
    save_test_wav(&wav_path, &original);

    // Step 3: Load back
    let loaded = load_wav_file(&wav_path).unwrap();

    // Step 4: Verify audio properties are preserved
    let orig_rms = rms(&original);
    let loaded_rms = rms(&loaded.samples);
    let rms_error = (orig_rms - loaded_rms).abs() / orig_rms;
    eprintln!("RMS: original={:.6}, loaded={:.6}, error={:.4}%", orig_rms, loaded_rms, rms_error * 100.0);
    assert!(rms_error < 0.01, "RMS error too high: {:.4}%", rms_error * 100.0);

    // Step 5: Verify zero-crossings match (frequency preserved)
    let orig_crossings = count_zero_crossings(&original);
    let loaded_crossings = count_zero_crossings(&loaded.samples);
    let crossing_error = (orig_crossings as f64 - loaded_crossings as f64).abs() / orig_crossings as f64;
    eprintln!("Zero crossings: original={}, loaded={}, error={:.4}%",
        orig_crossings, loaded_crossings, crossing_error * 100.0);
    assert!(crossing_error < 0.01, "Frequency changed during WAV round-trip");

    // Step 6: Verify peak amplitude
    let orig_peak = original.iter().fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
    let loaded_peak = loaded.samples.iter().fold(0.0f32, |a: f32, &b: &f32| a.max(b.abs()));
    eprintln!("Peak: original={:.6}, loaded={:.6}", orig_peak, loaded_peak);
    assert!((orig_peak - loaded_peak).abs() < 0.002, "Peak amplitude changed");

    eprintln!("\n=== WAV Integrity E2E PASSED ===\n");
}

// =============================================================================
// Helpers
// =============================================================================

fn rms(samples: &[f32]) -> f32 {
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

fn count_zero_crossings(samples: &[f32]) -> usize {
    samples.windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count()
}
