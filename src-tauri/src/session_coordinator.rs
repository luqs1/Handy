//! Session coordinator: wires stores and engines together into a meeting lifecycle.
//! Translated from: OpenOats/Sources/OpenOats/App/AppCoordinator.swift +
//!                   OpenOats/Sources/OpenOats/App/LiveSessionController.swift
//!
//! This is the central orchestration layer. It owns the state machine, stores,
//! and engines. Tauri commands delegate to this coordinator.

use crate::domain::meeting_state::{transition, MeetingEvent, MeetingState};
use crate::domain::meeting_types::MeetingMetadata;
use crate::domain::models::{SessionIndex, SessionRecord};
use crate::domain::utterance::{Speaker, Utterance};
use crate::stores::session_repository::{
    SessionFinalizeMetadata, SessionHandle, SessionRepository, SessionStartConfig,
};
use crate::stores::template_store::TemplateStore;
use crate::stores::transcript_store::TranscriptStore;
use log::info;
use std::path::PathBuf;

/// Published state for the live session, consumed by the frontend.
#[derive(Clone, Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveSessionState {
    pub is_recording: bool,
    pub session_id: Option<String>,
    pub utterance_count: usize,
    pub volatile_you_text: String,
    pub volatile_them_text: String,
}

/// The session coordinator owns the meeting lifecycle and all stores/engines.
pub struct SessionCoordinator {
    state: MeetingState,
    transcript_store: TranscriptStore,
    session_repo: SessionRepository,
    template_store: TemplateStore,
    current_handle: Option<SessionHandle>,
}

impl SessionCoordinator {
    // Swift: N/A (no direct Swift equivalent — combines AppCoordinator.init and LiveSessionController.init)
    pub fn new(root_dir: Option<PathBuf>) -> Self {
        let dir = root_dir.clone();
        Self {
            state: MeetingState::Idle,
            transcript_store: TranscriptStore::new(),
            session_repo: SessionRepository::new(root_dir),
            template_store: TemplateStore::new(dir),
            current_handle: None,
        }
    }

    // MARK: - State Machine

    pub fn state(&self) -> &MeetingState {
        &self.state
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.state, MeetingState::Recording(_))
    }

    // Swift: AppCoordinator.swift > AppCoordinator.handle(_:settings:)
    /// Drive the meeting lifecycle. Returns true if the state changed.
    pub fn handle(&mut self, event: MeetingEvent) -> bool {
        let old_state = self.state.clone();
        self.state = transition(&old_state, &event);

        if self.state == old_state {
            return false;
        }

        self.perform_side_effects(&event);
        true
    }

    // Swift: AppCoordinator.swift > AppCoordinator.performSideEffects(for:settings:)
    fn perform_side_effects(&mut self, event: &MeetingEvent) {
        match event {
            MeetingEvent::UserStarted(metadata) => {
                self.transcript_store.clear();

                let template_snapshot = self
                    .template_store
                    .templates()
                    .first()
                    .map(|t| TemplateStore::snapshot(t));

                let handle = self.session_repo.start_session(SessionStartConfig {
                    template_snapshot,
                });
                info!("Session coordinator: started {}", handle.session_id);
                self.current_handle = Some(handle);
            }

            MeetingEvent::UserStopped => {
                if let Some(ref handle) = self.current_handle {
                    let utterance_count = self.transcript_store.utterances().len();
                    self.session_repo.finalize_session(
                        &handle.session_id,
                        SessionFinalizeMetadata {
                            ended_at: chrono::Utc::now().timestamp_millis(),
                            utterance_count,
                            title: None,
                            language: None,
                            meeting_app: None,
                            engine: None,
                            template_snapshot: None,
                        },
                    );
                    info!(
                        "Session coordinator: finalized {} ({} utterances)",
                        handle.session_id, utterance_count
                    );
                }
                self.current_handle = None;
                // Finalization is synchronous in the Rust port, so immediately
                // transition from Ending → Idle.
                // Swift: In the Swift app this happens async via Task + FinalizationComplete event.
                self.state = transition(&self.state, &MeetingEvent::FinalizationComplete);
            }

            MeetingEvent::UserDiscarded => {
                if let Some(ref handle) = self.current_handle {
                    self.session_repo.delete_session(&handle.session_id);
                    info!(
                        "Session coordinator: discarded {}",
                        handle.session_id
                    );
                }
                self.transcript_store.clear();
                self.current_handle = None;
            }

            MeetingEvent::FinalizationComplete | MeetingEvent::FinalizationTimeout => {
                self.current_handle = None;
            }
        }
    }

    // MARK: - Utterance Ingestion

    // Swift: LiveSessionController.swift > LiveSessionController utterance ingestion path
    //        (no single method — combines TranscriptStore.append + SessionRepository.appendLiveUtterance)
    /// Called when a new utterance arrives from the transcription engine.
    /// Returns true if the utterance was accepted (not echo-suppressed).
    pub fn on_utterance(&mut self, utterance: Utterance) -> bool {
        let accepted = self.transcript_store.append(utterance.clone());
        if accepted {
            self.session_repo.append_live_utterance(&utterance);
        }
        accepted
    }

    // MARK: - Volatile Text

    pub fn set_volatile_you_text(&mut self, text: String) {
        self.transcript_store.volatile_you_text = text;
    }

    pub fn set_volatile_them_text(&mut self, text: String) {
        self.transcript_store.volatile_them_text = text;
    }

    // MARK: - Queries

    // Swift: LiveSessionController.swift > LiveSessionState struct
    pub fn live_state(&self) -> LiveSessionState {
        LiveSessionState {
            is_recording: self.is_recording(),
            session_id: self.current_handle.as_ref().map(|h| h.session_id.clone()),
            utterance_count: self.transcript_store.utterances().len(),
            volatile_you_text: self.transcript_store.volatile_you_text.clone(),
            volatile_them_text: self.transcript_store.volatile_them_text.clone(),
        }
    }

    pub fn utterances(&self) -> &[Utterance] {
        self.transcript_store.utterances()
    }

    // Swift: AppCoordinator.swift > AppCoordinator.loadHistory()
    pub fn list_sessions(&self) -> Vec<SessionIndex> {
        self.session_repo.list_sessions()
    }

    pub fn load_transcript(&self, session_id: &str) -> Vec<SessionRecord> {
        self.session_repo.load_transcript(session_id)
    }

    pub fn delete_session(&self, session_id: &str) -> bool {
        self.session_repo.delete_session(session_id)
    }

    pub fn rename_session(&self, session_id: &str, title: &str) {
        self.session_repo.rename_session(session_id, title);
    }

    pub fn transcript_store(&self) -> &TranscriptStore {
        &self.transcript_store
    }

    pub fn template_store(&self) -> &TemplateStore {
        &self.template_store
    }

    pub fn template_store_mut(&mut self) -> &mut TemplateStore {
        &mut self.template_store
    }

    pub fn session_repo(&self) -> &SessionRepository {
        &self.session_repo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_coordinator() -> (SessionCoordinator, TempDir) {
        let dir = TempDir::new().unwrap();
        let coord = SessionCoordinator::new(Some(dir.path().to_path_buf()));
        (coord, dir)
    }

    #[test]
    fn start_creates_session() {
        let (mut coord, _dir) = make_coordinator();
        assert!(!coord.is_recording());

        let changed = coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        assert!(changed);
        assert!(coord.is_recording());
        assert!(coord.current_handle.is_some());

        let state = coord.live_state();
        assert!(state.is_recording);
        assert!(state.session_id.is_some());
    }

    #[test]
    fn utterance_routes_to_store_and_repo() {
        let (mut coord, _dir) = make_coordinator();
        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        let session_id = coord.live_state().session_id.clone().unwrap();

        let u1 = Utterance::new("hello from mic".to_string(), Speaker::You);
        let u2 = Utterance::new("hello from system".to_string(), Speaker::Them);

        assert!(coord.on_utterance(u1));
        assert!(coord.on_utterance(u2));

        // Check transcript store
        assert_eq!(coord.utterances().len(), 2);

        // Stop session so writer flushes
        coord.handle(MeetingEvent::UserStopped);

        // Check persisted transcript
        let records = coord.load_transcript(&session_id);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].text, "hello from mic");
        assert_eq!(records[1].text, "hello from system");
    }

    #[test]
    fn stop_finalizes_session() {
        let (mut coord, _dir) = make_coordinator();
        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));

        coord.on_utterance(Utterance::new("test".to_string(), Speaker::You));
        coord.handle(MeetingEvent::UserStopped);

        assert!(!coord.is_recording());
        assert!(coord.current_handle.is_none());

        // Session should appear in history
        let sessions = coord.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].ended_at.is_some());
        assert_eq!(sessions[0].utterance_count, 1);
    }

    #[test]
    fn echo_suppressed_utterances_not_persisted() {
        let (mut coord, _dir) = make_coordinator();
        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        let session_id = coord.live_state().session_id.clone().unwrap();

        // "Them" says something
        let them = Utterance {
            id: uuid::Uuid::new_v4().to_string(),
            text: "we should discuss the quarterly results".to_string(),
            speaker: Speaker::Them,
            timestamp: 1000,
            refined_text: None,
            refinement_status: None,
        };
        assert!(coord.on_utterance(them));

        // "You" says the same thing 1 second later (acoustic echo)
        let echo = Utterance {
            id: uuid::Uuid::new_v4().to_string(),
            text: "we should discuss the quarterly results".to_string(),
            speaker: Speaker::You,
            timestamp: 2000,
            refined_text: None,
            refinement_status: None,
        };
        assert!(!coord.on_utterance(echo)); // suppressed

        // Only 1 utterance in store
        assert_eq!(coord.utterances().len(), 1);

        // Stop and check persisted — only 1 record
        coord.handle(MeetingEvent::UserStopped);
        let records = coord.load_transcript(&session_id);
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn discard_deletes_session() {
        let (mut coord, _dir) = make_coordinator();
        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        let session_id = coord.live_state().session_id.clone().unwrap();

        coord.on_utterance(Utterance::new("test".to_string(), Speaker::You));
        coord.handle(MeetingEvent::UserDiscarded);

        assert!(!coord.is_recording());
        // Session should be deleted
        let sessions = coord.list_sessions();
        assert!(sessions.is_empty());
        // Transcript should be cleared
        assert!(coord.utterances().is_empty());
    }

    #[test]
    fn double_start_is_noop() {
        let (mut coord, _dir) = make_coordinator();
        coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        let first_id = coord.live_state().session_id.clone();

        // Second start while recording — should be a no-op
        let changed = coord.handle(MeetingEvent::UserStarted(MeetingMetadata::manual()));
        assert!(!changed);
        assert_eq!(coord.live_state().session_id, first_id);
    }
}
