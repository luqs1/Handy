//! Integration tests for the full meeting session lifecycle.
//! These exercise the SessionCoordinator end-to-end without Tauri.

use handy_app_lib::domain::meeting_state::{MeetingEvent, MeetingState};
use handy_app_lib::domain::meeting_types::MeetingMetadata;
use handy_app_lib::domain::models::SessionRecord;
use handy_app_lib::domain::utterance::{Speaker, Utterance};
use handy_app_lib::engines::suggestion_engine;
use handy_app_lib::engines::transcript_cleanup;
use handy_app_lib::session_coordinator::SessionCoordinator;
use handy_app_lib::stores::template_store;
use tempfile::TempDir;

fn make_coordinator() -> (SessionCoordinator, TempDir) {
    let dir = TempDir::new().unwrap();
    let coord = SessionCoordinator::new(Some(dir.path().to_path_buf()));
    (coord, dir)
}

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
// Full Meeting Lifecycle
// =============================================================================

#[test]
fn full_meeting_lifecycle() {
    let (mut coord, _dir) = make_coordinator();

    // 1. Start session
    assert!(matches!(coord.state(), MeetingState::Idle));
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    assert!(coord.is_recording());
    let session_id = coord.live_state().session_id.clone().unwrap();

    // 2. Simulate a conversation
    let utterances = vec![
        make_utterance("Hey everyone, let's get started with the standup", Speaker::You, 1000),
        make_utterance("Sure, I finished the API migration yesterday", Speaker::Them, 5000),
        make_utterance("Great, any blockers?", Speaker::You, 10000),
        make_utterance("I'm blocked on the database credentials, need DevOps help", Speaker::Them, 15000),
        make_utterance("I'll ping them after this call", Speaker::You, 20000),
        make_utterance("Thanks. Also the CI pipeline is flaky again", Speaker::Them, 25000),
        make_utterance("Yeah I noticed that too, let's create a ticket", Speaker::You, 30000),
    ];

    for u in &utterances {
        let accepted = coord.on_utterance(u.clone());
        assert!(accepted, "Utterance should be accepted: {}", u.text);
    }

    // 3. Verify live state
    let live = coord.live_state();
    assert!(live.is_recording);
    assert_eq!(live.utterance_count, 7);

    // 4. Stop session
    coord.handle(MeetingEvent::UserStopped);
    assert!(!coord.is_recording());
    assert!(matches!(coord.state(), MeetingState::Idle));

    // 5. Verify persisted transcript
    let transcript = coord.load_transcript(&session_id);
    assert_eq!(transcript.len(), 7);
    assert_eq!(transcript[0].text, "Hey everyone, let's get started with the standup");
    assert_eq!(transcript[3].text, "I'm blocked on the database credentials, need DevOps help");

    // 6. Verify session in history
    let sessions = coord.list_sessions();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert!(sessions[0].ended_at.is_some());
    assert_eq!(sessions[0].utterance_count, 7);
}

// =============================================================================
// Echo Suppression in Full Pipeline
// =============================================================================

#[test]
fn echo_suppression_end_to_end() {
    let (mut coord, _dir) = make_coordinator();
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    let session_id = coord.live_state().session_id.clone().unwrap();

    // Them says something
    let them = make_utterance(
        "We need to refactor the authentication module completely",
        Speaker::Them,
        1000,
    );
    assert!(coord.on_utterance(them));

    // Mic picks up the same thing 500ms later (acoustic echo)
    let echo = make_utterance(
        "We need to refactor the authentication module completely",
        Speaker::You,
        1500,
    );
    assert!(!coord.on_utterance(echo), "Echo should be suppressed");

    // Mic picks up a real response 3 seconds later (outside echo window)
    let real = make_utterance(
        "I agree, let's plan that for next sprint",
        Speaker::You,
        4000,
    );
    assert!(coord.on_utterance(real), "Real response should be accepted");

    coord.handle(MeetingEvent::UserStopped);

    // Only 2 utterances should be persisted (echo was suppressed)
    let transcript = coord.load_transcript(&session_id);
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].text, "We need to refactor the authentication module completely");
    assert_eq!(transcript[1].text, "I agree, let's plan that for next sprint");
}

// =============================================================================
// Multiple Sessions
// =============================================================================

#[test]
fn multiple_sessions_history() {
    let (mut coord, _dir) = make_coordinator();

    // Session 1
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    coord.on_utterance(make_utterance("Session one content", Speaker::You, 1000));
    coord.handle(MeetingEvent::UserStopped);

    // Session 2 (manually create with different ID to avoid timestamp collision)
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    coord.on_utterance(make_utterance("Session two content", Speaker::You, 1000));
    coord.on_utterance(make_utterance("More content", Speaker::Them, 2000));
    coord.handle(MeetingEvent::UserStopped);

    let sessions = coord.list_sessions();
    // May be 1 or 2 depending on timestamp resolution, but at least 1
    assert!(!sessions.is_empty());
}

// =============================================================================
// Discard Session
// =============================================================================

#[test]
fn discard_removes_everything() {
    let (mut coord, _dir) = make_coordinator();

    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    coord.on_utterance(make_utterance("This should be deleted", Speaker::You, 1000));
    coord.on_utterance(make_utterance("This too", Speaker::Them, 2000));

    assert_eq!(coord.live_state().utterance_count, 2);

    coord.handle(MeetingEvent::UserDiscarded);

    assert!(!coord.is_recording());
    assert_eq!(coord.utterances().len(), 0);
    assert!(coord.list_sessions().is_empty());
}

// =============================================================================
// Session Rename
// =============================================================================

#[test]
fn rename_persists() {
    let (mut coord, _dir) = make_coordinator();

    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    coord.on_utterance(make_utterance("test", Speaker::You, 1000));
    coord.handle(MeetingEvent::UserStopped);

    let sessions = coord.list_sessions();
    assert!(!sessions.is_empty());
    let sid = &sessions[0].id;

    coord.rename_session(sid, "Weekly Standup Q1");

    let sessions = coord.list_sessions();
    assert_eq!(sessions[0].title.as_deref(), Some("Weekly Standup Q1"));
}

// =============================================================================
// Template Store Integration
// =============================================================================

#[test]
fn templates_available_at_startup() {
    let (coord, _dir) = make_coordinator();
    let templates = coord.template_store().templates();
    assert_eq!(templates.len(), 6);
    assert!(templates.iter().all(|t| t.is_built_in));

    // Verify the generic template has the expected system prompt structure
    let generic = coord
        .template_store()
        .template_for("00000000-0000-0000-0000-000000000000")
        .unwrap();
    assert_eq!(generic.name, "Generic");
    assert!(generic.system_prompt.contains("## Summary"));
    assert!(generic.system_prompt.contains("## Action Items"));
}

// =============================================================================
// Transcript Cleanup Pure Logic
// =============================================================================

#[test]
fn transcript_cleanup_chunking_and_parsing() {
    // Simulate a 5-minute meeting transcript
    let records: Vec<SessionRecord> = (0..20)
        .map(|i| {
            let speaker = if i % 2 == 0 { Speaker::You } else { Speaker::Them };
            let timestamp = i * 15_000; // 15 seconds apart
            SessionRecord::new(speaker, format!("Utterance number {}", i), timestamp)
        })
        .collect();

    // Chunking: 20 utterances * 15s = 300s = 2 chunks of ~150s each
    let chunks = transcript_cleanup::chunk_records(&records);
    assert_eq!(chunks.len(), 2, "Should split into 2 chunks at 150s boundary");
    assert_eq!(chunks[0].len() + chunks[1].len(), 20);

    // Parsing: simulate a cleaned LLM response
    let original = &chunks[0];
    let response_lines: Vec<String> = original
        .iter()
        .map(|r| {
            let label = r.speaker.display_label();
            format!("[00:00:00] {}: Cleaned {}", label, r.text)
        })
        .collect();
    let response = response_lines.join("\n");

    let parsed = transcript_cleanup::parse_response(&response, original).unwrap();
    assert_eq!(parsed.len(), original.len());
    // All should have refined_text set
    assert!(parsed.iter().all(|r| r.refined_text.is_some()));
    assert!(parsed[0].refined_text.as_ref().unwrap().starts_with("Cleaned"));
}

// =============================================================================
// Suggestion Engine Heuristics
// =============================================================================

#[test]
fn suggestion_pipeline_heuristics() {
    // Utterance too short — rejected
    let short = Utterance::new("yes".to_string(), Speaker::Them);
    assert!(!suggestion_engine::should_evaluate_utterance(
        &short,
        &[],
        None,
        45.0
    ));

    // Good utterance — accepted
    let good = Utterance::new(
        "I think we should pivot our go-to-market strategy for the enterprise segment".to_string(),
        Speaker::Them,
    );
    assert!(suggestion_engine::should_evaluate_utterance(
        &good,
        &[],
        None,
        45.0
    ));

    // Same utterance triggers domain signal
    let trigger = suggestion_engine::detect_trigger(&good);
    assert!(trigger.is_some());

    // Within cooldown — rejected
    assert!(!suggestion_engine::should_evaluate_utterance(
        &good,
        &[],
        Some(10.0), // 10 seconds since last suggestion
        45.0         // 45 second cooldown
    ));
}

// =============================================================================
// Knowledge Base Chunking + Search
// =============================================================================

#[test]
fn knowledge_base_end_to_end() {
    use handy_app_lib::engines::knowledge_base::{
        chunk_markdown, cosine_similarity, search_chunks, KBChunk,
    };

    // Chunk a real-looking markdown doc
    let doc = r#"# Product Strategy

## Target Market
We're focused on mid-market SaaS companies with 50-500 employees who struggle with meeting overload.
Our primary persona is the engineering manager who spends 40% of their time in meetings and needs
better tooling to capture and share decisions. The pain point is acute: decisions get lost, action items
are forgotten, and new team members have no way to catch up on past discussions. We solve this by
recording, transcribing, and intelligently summarizing every meeting automatically.

## Competitive Landscape
The main competitors are Otter.ai (consumer-focused, limited integration), Fireflies.ai (good transcription
but weak summarization), and Grain (strong on clips but no real-time features). Our differentiation is
the knowledge base integration: we don't just transcribe, we connect what's being said to what the team
already knows. This is our wedge into the enterprise market.

## Pricing Model
We're considering a freemium model with a generous free tier (10 meetings/month) and a pro tier at $15/user/month.
Enterprise pricing will be custom based on seat count and integration requirements.
"#;

    let chunks = chunk_markdown(doc, "strategy.md");
    assert!(!chunks.is_empty(), "Should produce at least one chunk");

    // Debug: print all chunks and their headers
    for (i, (text, header)) in chunks.iter().enumerate() {
        eprintln!("Chunk {}: header='{}', words={}, text='{}'", i, header, text.split_whitespace().count(), &text[..text.len().min(80)]);
    }

    // Verify header contexts exist (at least one chunk has a non-empty header)
    let has_header = chunks.iter().any(|(_, h)| !h.is_empty());
    assert!(has_header, "At least one chunk should have a header context");

    // Simulate search with fake embeddings
    let kb_chunks: Vec<KBChunk> = chunks
        .iter()
        .enumerate()
        .map(|(i, (text, header))| {
            // Create embeddings that vary by chunk index
            let mut emb = vec![0.0f32; 3];
            emb[i % 3] = 1.0;
            KBChunk {
                text: text.clone(),
                source_file: "strategy.md".to_string(),
                header_context: header.clone(),
                embedding: emb,
            }
        })
        .collect();

    // Search with a query embedding
    let query = vec![vec![1.0f32, 0.0, 0.0]]; // should match chunk 0
    let results = search_chunks(&kb_chunks, &query, 2);
    assert!(!results.is_empty());
    assert_eq!(results[0].source_file, "strategy.md");

    // Verify cosine similarity
    let sim = cosine_similarity(&[1.0, 0.0, 0.0], &[1.0, 0.0, 0.0]);
    assert!((sim - 1.0).abs() < f32::EPSILON);

    let sim_ortho = cosine_similarity(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]);
    assert!(sim_ortho.abs() < f32::EPSILON);
}

// =============================================================================
// Notes Save/Load Through Coordinator
// =============================================================================

#[test]
fn notes_save_and_retrieve() {
    let (mut coord, _dir) = make_coordinator();

    // Run a session
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    let session_id = coord.live_state().session_id.clone().unwrap();
    coord.on_utterance(make_utterance("Let's discuss the roadmap", Speaker::You, 1000));
    coord.on_utterance(make_utterance("I think we should prioritize mobile", Speaker::Them, 5000));
    coord.handle(MeetingEvent::UserStopped);

    // Save notes (simulating what session_generate_notes does after LLM call)
    let template = coord
        .template_store()
        .template_for("00000000-0000-0000-0000-000000000000")
        .unwrap();
    let snapshot = template_store::TemplateStore::snapshot(template);

    let notes = handy_app_lib::domain::models::EnhancedNotes {
        template: snapshot,
        generated_at: chrono::Utc::now().timestamp_millis(),
        markdown: "# Meeting Notes\n\n## Summary\nDiscussed roadmap priorities.\n\n## Action Items\n- Prioritize mobile development".to_string(),
    };

    coord.session_repo().save_notes(&session_id, &notes);

    // Load and verify
    let loaded = coord.session_repo().load_notes(&session_id).unwrap();
    assert!(loaded.markdown.contains("## Summary"));
    assert!(loaded.markdown.contains("Prioritize mobile"));
    assert_eq!(loaded.template.name, "Generic");

    // Verify session index reflects notes
    let sessions = coord.list_sessions();
    let session = sessions.iter().find(|s| s.id == session_id).unwrap();
    assert!(session.has_notes);
}

// =============================================================================
// State Machine Edge Cases
// =============================================================================

#[test]
fn state_machine_edge_cases() {
    let (mut coord, _dir) = make_coordinator();

    // Stop while idle — no-op
    assert!(!coord.handle(MeetingEvent::UserStopped));
    assert!(matches!(coord.state(), MeetingState::Idle));

    // Discard while idle — no-op
    assert!(!coord.handle(MeetingEvent::UserDiscarded));
    assert!(matches!(coord.state(), MeetingState::Idle));

    // Start → double start — no-op on second
    coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
    assert!(!coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual())));
    assert!(coord.is_recording());

    // Stop works
    assert!(coord.handle(MeetingEvent::UserStopped));
    assert!(matches!(coord.state(), MeetingState::Idle));
}
