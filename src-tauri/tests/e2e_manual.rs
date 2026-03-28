//! Manual E2E test — exercises the full pipeline and dumps output for inspection.
//! Run with: cargo test --test e2e_manual -- --nocapture

use handy_app_lib::domain::meeting_state::MeetingEvent;
use handy_app_lib::domain::meeting_types::MeetingMetadata;
use handy_app_lib::domain::models::{EnhancedNotes, SessionRecord};
use handy_app_lib::domain::utterance::{Speaker, Utterance};
use handy_app_lib::engines::{knowledge_base, suggestion_engine, transcript_cleanup};
use handy_app_lib::session_coordinator::SessionCoordinator;
use handy_app_lib::stores::template_store::TemplateStore;
use std::fs;

fn make_utterance(text: &str, speaker: Speaker, timestamp: i64) -> Utterance {
    Utterance {
        id: uuid::Uuid::new_v4().to_string(),
        text: text.to_string(),
        speaker,
        timestamp,
        refined_text: None,
        refinement_status: None,
    }
}

// =============================================================================
// Full Meeting Lifecycle — simulates an OpenOats product discussion
// =============================================================================

#[test]
fn e2e_full_meeting_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    eprintln!("\n=== E2E TEST: Full Meeting Lifecycle ===");
    eprintln!("Working directory: {}\n", root.display());

    let mut coord = SessionCoordinator::new(Some(root.clone()));

    // --- START SESSION ---
    eprintln!("1. Starting session...");
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    let state = coord.live_state();
    let session_id = state.session_id.clone().unwrap();
    eprintln!("   Session ID: {}", session_id);
    eprintln!("   Is recording: {}", state.is_recording);

    // --- FEED UTTERANCES (OpenOats product meeting) ---
    eprintln!("\n2. Feeding utterances...");
    let conversation = vec![
        ("Let's talk about the Windows port of OpenOats", Speaker::You, 1000),
        ("Yeah so the main challenge is translating the Swift transcription pipeline to Rust", Speaker::Them, 5000),
        ("How are we handling the system audio capture on Windows?", Speaker::You, 10000),
        ("We're using WASAPI loopback through cpal, it captures all system audio unlike the Mac process taps", Speaker::Them, 15000),
        ("Does that mean we can't isolate the meeting app's audio on Windows?", Speaker::You, 20000),
        ("Right, that's a platform limitation. We use voice activity detection to filter out non-speech", Speaker::Them, 25000),
        ("What about the diarization? FluidAudio is macOS only", Speaker::You, 30000),
        ("We'll need an ONNX-based alternative. The transcribe-rs crate already uses ONNX runtime", Speaker::Them, 35000),
        ("OK so the binary You versus Them speaker model works for now", Speaker::You, 40000),
        ("Exactly, and the suggestion engine doesn't depend on diarization at all", Speaker::Them, 45000),
    ];

    for (text, speaker, ts) in &conversation {
        let u = make_utterance(text, speaker.clone(), *ts);
        let accepted = coord.on_utterance(u);
        eprintln!("   [{}] {}: {} {}", ts, speaker.display_label(), text,
            if accepted { "✓" } else { "✗ (suppressed)" });
    }

    // --- TEST ECHO SUPPRESSION ---
    eprintln!("\n3. Testing echo suppression...");
    let echo = make_utterance(
        "We're using WASAPI loopback through cpal, it captures all system audio unlike the Mac process taps",
        Speaker::You,
        15500, // 500ms after "them" said the same thing
    );
    let accepted = coord.on_utterance(echo);
    eprintln!("   Echo attempt: {} (expected: suppressed)",
        if accepted { "ACCEPTED - BUG!" } else { "suppressed ✓" });
    assert!(!accepted);

    eprintln!("   Total utterances in store: {}", coord.utterances().len());
    assert_eq!(coord.utterances().len(), 10);

    // --- STOP SESSION ---
    eprintln!("\n4. Stopping session...");
    coord.handle(MeetingEvent::UserStopped);
    eprintln!("   Is recording: {}", coord.is_recording());
    assert!(!coord.is_recording());

    // --- INSPECT FILES ON DISK ---
    eprintln!("\n5. Inspecting files on disk...");
    let session_dir = root.join("sessions").join(&session_id);
    assert!(session_dir.exists(), "Session directory should exist");

    // session.json
    let meta_path = session_dir.join("session.json");
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_content).unwrap();
    eprintln!("   session.json:");
    eprintln!("     id: {}", meta["id"]);
    eprintln!("     startedAt: {}", meta["startedAt"]);
    eprintln!("     endedAt: {}", meta["endedAt"]);
    eprintln!("     utteranceCount: {}", meta["utteranceCount"]);
    eprintln!("     hasNotes: {}", meta["hasNotes"]);
    assert_eq!(meta["utteranceCount"], 10);
    assert!(!meta["endedAt"].is_null());
    assert_eq!(meta["hasNotes"], false);

    // transcript.live.jsonl
    let transcript_path = session_dir.join("transcript.live.jsonl");
    let transcript_content = fs::read_to_string(&transcript_path).unwrap();
    let lines: Vec<&str> = transcript_content.lines().filter(|l| !l.is_empty()).collect();
    eprintln!("\n   transcript.live.jsonl: {} lines", lines.len());
    assert_eq!(lines.len(), 10);

    // Verify each line is valid JSON
    for (i, line) in lines.iter().enumerate() {
        let record: SessionRecord = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("Line {} is invalid JSON: {}", i, e));
        if i < 3 {
            eprintln!("     Line {}: speaker={}, text='{}'",
                i, record.speaker.display_label(), &record.text[..record.text.len().min(60)]);
        }
    }
    eprintln!("     ... ({} more lines, all valid JSON)", lines.len() - 3);

    // --- SAVE NOTES ---
    eprintln!("\n6. Saving notes...");
    let template = coord.template_store().template_for("00000000-0000-0000-0000-000000000000").unwrap();
    let snapshot = TemplateStore::snapshot(template);
    let notes_markdown = r#"# OpenOats Windows Port Discussion

## Summary
Discussed the technical approach for porting OpenOats from macOS to Windows, focusing on audio capture differences, diarization alternatives, and the suggestion engine architecture.

## Key Points
- WASAPI loopback captures all system audio (unlike macOS process taps which can isolate per-app)
- Voice activity detection filters non-speech audio on Windows
- FluidAudio diarization is macOS-only; need ONNX-based alternative for Windows
- Binary You/Them speaker model sufficient for initial Windows release
- Suggestion engine has no dependency on diarization

## Action Items
- [ ] Implement ONNX-based diarization using transcribe-rs runtime
- [ ] Test WASAPI loopback with multiple audio sources
- [ ] Validate VAD filtering accuracy on Windows

## Decisions Made
- Ship Windows v1 with binary speaker model (no multi-speaker diarization)
- Use existing ONNX runtime from transcribe-rs for future diarization
"#;

    let notes = EnhancedNotes {
        template: snapshot,
        generated_at: chrono::Utc::now().timestamp_millis(),
        markdown: notes_markdown.to_string(),
    };
    coord.session_repo().save_notes(&session_id, &notes);

    // Verify notes files
    let notes_md_path = session_dir.join("notes.md");
    let notes_meta_path = session_dir.join("notes.meta.json");
    assert!(notes_md_path.exists());
    assert!(notes_meta_path.exists());

    let saved_notes = fs::read_to_string(&notes_md_path).unwrap();
    eprintln!("   notes.md: {} bytes", saved_notes.len());
    eprintln!("   First line: {}", saved_notes.lines().next().unwrap());
    assert!(saved_notes.contains("## Action Items"));
    assert!(saved_notes.contains("WASAPI loopback"));

    let notes_meta = fs::read_to_string(&notes_meta_path).unwrap();
    let meta_json: serde_json::Value = serde_json::from_str(&notes_meta).unwrap();
    eprintln!("   notes.meta.json template: {}", meta_json["templateSnapshot"]["name"]);
    assert_eq!(meta_json["templateSnapshot"]["name"], "Generic");

    // Verify session.json updated hasNotes
    let meta_content = fs::read_to_string(&meta_path).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&meta_content).unwrap();
    eprintln!("   session.json hasNotes now: {}", meta["hasNotes"]);
    assert_eq!(meta["hasNotes"], true);

    // --- LIST SESSIONS ---
    eprintln!("\n7. Listing sessions...");
    let sessions = coord.list_sessions();
    eprintln!("   Found {} session(s)", sessions.len());
    for s in &sessions {
        eprintln!("     - {} | utterances={} | hasNotes={} | title={:?}",
            s.id, s.utterance_count, s.has_notes, s.title);
    }
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].has_notes);

    // --- LOAD TRANSCRIPT BACK ---
    eprintln!("\n8. Loading transcript back from disk...");
    let loaded = coord.load_transcript(&session_id);
    eprintln!("   Loaded {} records", loaded.len());
    assert_eq!(loaded.len(), 10);
    assert_eq!(loaded[0].text, "Let's talk about the Windows port of OpenOats");
    assert_eq!(loaded[9].text, "Exactly, and the suggestion engine doesn't depend on diarization at all");

    // --- LOAD NOTES BACK ---
    eprintln!("\n9. Loading notes back from disk...");
    let loaded_notes = coord.session_repo().load_notes(&session_id).unwrap();
    eprintln!("   Template: {}", loaded_notes.template.name);
    eprintln!("   Markdown length: {} bytes", loaded_notes.markdown.len());
    assert_eq!(loaded_notes.template.name, "Generic");
    assert!(loaded_notes.markdown.contains("## Action Items"));

    // --- RENAME ---
    eprintln!("\n10. Renaming session...");
    coord.rename_session(&session_id, "OpenOats Windows Port Sync");
    let sessions = coord.list_sessions();
    eprintln!("    Title: {:?}", sessions[0].title);
    assert_eq!(sessions[0].title.as_deref(), Some("OpenOats Windows Port Sync"));

    // --- DELETE ---
    eprintln!("\n11. Deleting session...");
    let deleted = coord.delete_session(&session_id);
    eprintln!("    Deleted: {}", deleted);
    assert!(deleted);
    assert!(!session_dir.exists());
    assert!(coord.list_sessions().is_empty());

    eprintln!("\n=== ALL E2E CHECKS PASSED ===\n");
}

// =============================================================================
// Transcript Cleanup — simulates cleaning up a messy transcription
// =============================================================================

#[test]
fn e2e_transcript_cleanup_pipeline() {
    eprintln!("\n=== E2E TEST: Transcript Cleanup Pipeline ===\n");

    let records: Vec<SessionRecord> = vec![
        SessionRecord::new(Speaker::You, "um so yeah I was looking at the uh transcription engine code".to_string(), 0),
        SessionRecord::new(Speaker::Them, "right so basically the whisper kit backend is like only for Apple Silicon".to_string(), 15000),
        SessionRecord::new(Speaker::You, "okay okay so what do we use on on Windows then".to_string(), 30000),
        SessionRecord::new(Speaker::Them, "well actually we have whisper cpp through the uh transcribe rs crate you know".to_string(), 45000),
        SessionRecord::new(Speaker::You, "oh nice so like is the accuracy um comparable".to_string(), 60000),
    ];

    let chunks = transcript_cleanup::chunk_records(&records);
    eprintln!("1. Chunking: {} records → {} chunk(s)", records.len(), chunks.len());
    assert_eq!(chunks.len(), 1);

    let prompt = transcript_cleanup::format_chunk_prompt(&records);
    eprintln!("\n2. Formatted prompt ({} chars):", prompt.len());
    for line in prompt.lines() {
        eprintln!("   {}", line);
    }

    // Simulate what an LLM would return after cleaning
    let cleaned_response = "[00:00:00] You: I was looking at the transcription engine code.\n\
        [00:00:15] Them: The WhisperKit backend is only for Apple Silicon.\n\
        [00:00:30] You: Okay, so what do we use on Windows then?\n\
        [00:00:45] Them: We have whisper.cpp through the transcribe-rs crate.\n\
        [00:01:00] You: Nice, is the accuracy comparable?";

    let parsed = transcript_cleanup::parse_response(cleaned_response, &records);
    eprintln!("\n3. Parsed cleanup response:");
    match parsed {
        Some(ref cleaned) => {
            for (i, r) in cleaned.iter().enumerate() {
                eprintln!("   Original:  '{}'", records[i].text);
                eprintln!("   Cleaned:   '{}'", r.refined_text.as_deref().unwrap_or("(none)"));
                eprintln!();
            }
            assert_eq!(cleaned.len(), 5);
            assert!(cleaned[0].refined_text.as_ref().unwrap().contains("transcription engine"));
            assert!(!cleaned[0].refined_text.as_ref().unwrap().contains("um"));
            assert!(!cleaned[0].refined_text.as_ref().unwrap().contains("uh"));
        }
        None => panic!("Parse should succeed"),
    }

    eprintln!("=== CLEANUP PIPELINE E2E PASSED ===\n");
}

// =============================================================================
// Suggestion Heuristics — tests what triggers suggestions during meetings
// =============================================================================

#[test]
fn e2e_suggestion_heuristics_pipeline() {
    eprintln!("\n=== E2E TEST: Suggestion Heuristics Pipeline ===\n");

    let test_cases: Vec<(&str, bool, &str)> = vec![
        ("yes", false, "too short"),
        ("okay sure", false, "too short"),
        ("yeah um like basically right so well anyway actually", false, "filler-heavy"),
        ("How should we handle the knowledge base indexing when the user has thousands of markdown files?", true, "valid question about KB"),
        ("I think we should use ONNX runtime for the diarization model on Windows", true, "valid opinion about architecture"),
        ("The user retention for meeting transcription tools drops when accuracy falls below 90 percent", true, "domain signal about users"),
        ("But that approach won't work because WASAPI doesn't support per-process audio capture", true, "disagreement about platform limits"),
    ];

    eprintln!("Heuristic filter results:");
    for (text, expected, reason) in &test_cases {
        let u = Utterance::new(text.to_string(), Speaker::Them);
        let result = suggestion_engine::should_evaluate_utterance(&u, &[], None, 45.0);
        let status = if result == *expected { "✓" } else { "✗ MISMATCH" };
        eprintln!("  {} [{}] '{}' ({})", status, if result { "PASS" } else { "SKIP" }, &text[..text.len().min(70)], reason);
        assert_eq!(result, *expected, "Failed for: {}", text);
    }

    eprintln!("\nTrigger detection:");
    let trigger_cases: Vec<(&str, &str)> = vec![
        ("How should we handle the embedding cache invalidation?", "ExplicitQuestion"),
        ("Should we ship the Windows build with or without diarization?", "ExplicitQuestion"),
        ("However the WASAPI approach has a major limitation for multi-app scenarios", "Disagreement"),
        ("I believe most users only use one meeting app at a time anyway", "Assumption"),
        ("Our user churn increased after we removed the free tier", "CustomerProblem"),
        ("We need a better go to market strategy for the Windows launch", "DistributionGoToMarket"),
        ("Let's scope the MVP to just transcription and notes, no suggestions yet", "ProductScope"),
        ("The weather looks nice for a walk after this meeting", "(none)"),
    ];

    for (text, expected_kind) in &trigger_cases {
        let u = Utterance::new(text.to_string(), Speaker::Them);
        let trigger = suggestion_engine::detect_trigger(&u);
        let kind = trigger.as_ref().map(|t| format!("{:?}", t.kind)).unwrap_or("(none)".to_string());
        let status = if kind.contains(expected_kind) { "✓" } else { "✗" };
        eprintln!("  {} '{}' → {}", status, &text[..text.len().min(65)], kind);
        assert!(kind.contains(expected_kind), "Expected {} for '{}'", expected_kind, text);
    }

    eprintln!("\n=== SUGGESTION HEURISTICS E2E PASSED ===\n");
}

// =============================================================================
// Knowledge Base — chunks OpenOats docs and searches them
// =============================================================================

#[test]
fn e2e_knowledge_base_chunking() {
    eprintln!("\n=== E2E TEST: Knowledge Base Markdown Chunking ===\n");

    let doc = r#"# OpenOats Architecture

## Audio Capture
On macOS, OpenOats uses Core Audio process taps to capture system audio from specific applications.
This allows isolating the meeting app's audio stream from other system sounds like music or
notifications. The process tap creates an aggregate device that combines the output device with
the tap, reading audio via an IO proc callback. On Windows, we use WASAPI loopback capture
through the cpal crate, which captures all system audio. This is a platform limitation — there's
no way to isolate a single app's audio on Windows. Voice activity detection helps filter out
non-speech audio. The mic capture uses cpal on both platforms for consistency.

## Transcription Pipeline
The transcription engine supports multiple backends: WhisperKit (macOS only, optimized for Apple
Silicon via Core ML and Metal), Parakeet (cross-platform via ONNX Runtime), and Qwen3. On Windows,
whisper.cpp through the transcribe-rs crate provides Whisper model inference with DirectML or CUDA
acceleration. The streaming transcriber feeds audio in real-time, producing utterances that flow
through the diarization filter and acoustic echo filter before reaching the transcript store.
Batch transcription runs post-meeting for higher quality output using a larger model.

## Suggestion Engine
The suggestion engine runs a five-stage pipeline for each remote utterance: heuristic pre-filter,
trigger detection, conversation state update via LLM, multi-query knowledge base retrieval with
score fusion, and a surfacing gate that scores relevance, helpfulness, timing, and novelty. Only
suggestions that pass all threshold scores are shown to the user. The knowledge base indexes the
user's markdown documents by chunking them with header awareness, embedding chunks via Voyage AI
or Ollama, and caching embeddings with SHA-256 content fingerprinting for incremental re-indexing.
"#;

    let chunks = knowledge_base::chunk_markdown(doc, "architecture.md");
    eprintln!("Document: {} words → {} chunks\n", doc.split_whitespace().count(), chunks.len());

    for (i, (text, header)) in chunks.iter().enumerate() {
        let words = text.split_whitespace().count();
        let preview = &text[..text.len().min(80)];
        eprintln!("  Chunk {}: [{}] ({} words)", i, header, words);
        eprintln!("    '{}'...\n", preview);
    }

    assert!(chunks.len() >= 2, "Should produce multiple chunks");
    assert!(chunks.iter().all(|(_, h)| !h.is_empty()), "All chunks should have headers");

    // Simulate search with fake embeddings
    let kb_chunks: Vec<knowledge_base::KBChunk> = chunks.iter().enumerate().map(|(i, (text, header))| {
        let mut emb = vec![0.0f32; 4];
        emb[i % 4] = 1.0;
        knowledge_base::KBChunk {
            text: text.clone(),
            source_file: "architecture.md".to_string(),
            header_context: header.clone(),
            embedding: emb,
        }
    }).collect();

    // Search for audio-related content
    let query = vec![vec![1.0f32, 0.0, 0.0, 0.0]];
    let results = knowledge_base::search_chunks(&kb_chunks, &query, 3);
    eprintln!("Search results for query matching chunk 0:");
    for r in &results {
        eprintln!("  score={:.3} file={} header='{}'", r.score, r.source_file, r.header_context);
    }
    assert!(!results.is_empty());
    assert_eq!(results[0].source_file, "architecture.md");

    eprintln!("\n=== KNOWLEDGE BASE E2E PASSED ===\n");
}
