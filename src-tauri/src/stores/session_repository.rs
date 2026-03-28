//! File-based session persistence with JSONL transcript streaming.
//! Translated from: OpenOats/Sources/OpenOats/Storage/SessionRepository.swift
//!
//! Canonical layout per session:
//! ```text
//! sessions/<id>/session.json
//! sessions/<id>/transcript.live.jsonl
//! sessions/<id>/transcript.final.jsonl
//! sessions/<id>/notes.md
//! sessions/<id>/notes.meta.json
//! ```

use crate::domain::models::{
    EnhancedNotes, SessionIndex, SessionRecord, TemplateSnapshot,
};
use crate::domain::utterance::Utterance;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

// MARK: - Supporting Types

#[derive(Clone, Debug)]
pub struct SessionStartConfig {
    pub template_snapshot: Option<TemplateSnapshot>,
}

impl Default for SessionStartConfig {
    fn default() -> Self {
        Self {
            template_snapshot: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SessionHandle {
    pub session_id: String,
}

#[derive(Clone, Debug)]
pub struct SessionFinalizeMetadata {
    pub ended_at: i64,
    pub utterance_count: usize,
    pub title: Option<String>,
    pub language: Option<String>,
    pub meeting_app: Option<String>,
    pub engine: Option<String>,
    pub template_snapshot: Option<TemplateSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NotesMeta {
    template_snapshot: TemplateSnapshot,
    generated_at: i64,
}

// MARK: - Canonical session.json

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMetadata {
    id: String,
    started_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ended_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_snapshot: Option<TemplateSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    utterance_count: usize,
    has_notes: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meeting_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
}

// MARK: - SessionRepository

pub struct SessionRepository {
    sessions_dir: PathBuf,
    current_session_id: Option<String>,
    live_writer: Option<BufWriter<fs::File>>,
    live_utterance_count: usize,
}

impl SessionRepository {
    // Swift: SessionRepository.swift > SessionRepository.init(rootDirectory:)
    pub fn new(root_dir: Option<PathBuf>) -> Self {
        let base = root_dir.unwrap_or_else(|| PathBuf::from("sessions_data"));
        let sessions_dir = base.join("sessions");
        let _ = fs::create_dir_all(&sessions_dir);

        Self {
            sessions_dir,
            current_session_id: None,
            live_writer: None,
            live_utterance_count: 0,
        }
    }

    fn session_dir(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(session_id)
    }

    // MARK: - Session Lifecycle

    // Swift: SessionRepository.swift > SessionRepository.startSession(config:)
    pub fn start_session(&mut self, config: SessionStartConfig) -> SessionHandle {
        let now = chrono::Utc::now();
        let session_id = format!("session_{}", now.format("%Y-%m-%d_%H-%M-%S"));
        self.current_session_id = Some(session_id.clone());
        self.live_utterance_count = 0;

        let dir = self.session_dir(&session_id);
        let _ = fs::create_dir_all(&dir);

        // Create transcript.live.jsonl and keep writer open
        let live_path = dir.join("transcript.live.jsonl");
        match fs::File::create(&live_path) {
            Ok(file) => self.live_writer = Some(BufWriter::new(file)),
            Err(e) => warn!("Failed to open live transcript: {}", e),
        }

        // Write initial session.json
        let metadata = SessionMetadata {
            id: session_id.clone(),
            started_at: now.timestamp_millis(),
            ended_at: None,
            template_snapshot: config.template_snapshot,
            title: None,
            utterance_count: 0,
            has_notes: false,
            language: None,
            meeting_app: None,
            engine: None,
            tags: None,
            source: None,
        };
        self.write_session_metadata(&metadata, &session_id);

        info!("Session started: {}", session_id);
        SessionHandle { session_id }
    }

    // MARK: - Live Utterance Writing

    // Swift: SessionRepository.swift > SessionRepository.appendLiveUtterance(sessionID:utterance:metadata:)
    pub fn append_live_utterance(&mut self, utterance: &Utterance) {
        let record = SessionRecord::new(
            utterance.speaker.clone(),
            utterance.text.clone(),
            utterance.timestamp,
        );
        self.append_record(&record);
    }

    // Swift: SessionRepository.swift > SessionRepository.appendRecord(_:)
    pub fn append_record(&mut self, record: &SessionRecord) {
        if let Some(ref mut writer) = self.live_writer {
            if let Ok(json) = serde_json::to_string(record) {
                let _ = writeln!(writer, "{}", json);
                let _ = writer.flush();
                self.live_utterance_count += 1;
            }
        }
    }

    // MARK: - Finalization

    // Swift: SessionRepository.swift > SessionRepository.finalizeSession(sessionID:metadata:)
    pub fn finalize_session(&mut self, session_id: &str, metadata: SessionFinalizeMetadata) {
        // Close writer
        self.live_writer = None;
        self.current_session_id = None;

        // Write session.json with final metadata
        if let Some(mut meta) = self.load_session_metadata(session_id) {
            meta.ended_at = Some(metadata.ended_at);
            meta.utterance_count = metadata.utterance_count;
            meta.title = metadata.title;
            meta.language = metadata.language;
            meta.meeting_app = metadata.meeting_app;
            meta.engine = metadata.engine;
            meta.template_snapshot = metadata.template_snapshot;
            self.write_session_metadata(&meta, session_id);
        }

        info!("Session finalized: {}", session_id);
    }

    // Swift: SessionRepository.swift > SessionRepository.endSession()
    pub fn end_session(&mut self) {
        self.live_writer = None;
        self.current_session_id = None;
        self.live_utterance_count = 0;
    }

    // MARK: - Final Transcript

    // Swift: SessionRepository.swift > SessionRepository.saveFinalTranscript(sessionID:records:)
    pub fn save_final_transcript(&self, session_id: &str, records: &[SessionRecord]) {
        let dir = self.session_dir(session_id);
        let _ = fs::create_dir_all(&dir);

        let final_path = dir.join("transcript.final.jsonl");
        if let Ok(mut file) = fs::File::create(&final_path) {
            for record in records {
                if let Ok(json) = serde_json::to_string(record) {
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
    }

    // MARK: - Notes

    // Swift: SessionRepository.swift > SessionRepository.saveNotes(sessionID:notes:)
    pub fn save_notes(&self, session_id: &str, notes: &EnhancedNotes) {
        let dir = self.session_dir(session_id);
        let _ = fs::create_dir_all(&dir);

        // Write notes.md
        let md_path = dir.join("notes.md");
        let _ = fs::write(&md_path, &notes.markdown);

        // Write notes.meta.json
        let meta = NotesMeta {
            template_snapshot: notes.template.clone(),
            generated_at: notes.generated_at,
        };
        if let Ok(json) = serde_json::to_string_pretty(&meta) {
            let meta_path = dir.join("notes.meta.json");
            let _ = fs::write(&meta_path, json);
        }

        // Update session.json hasNotes flag
        if let Some(mut session_meta) = self.load_session_metadata(session_id) {
            session_meta.has_notes = true;
            self.write_session_metadata(&session_meta, session_id);
        }
    }

    // Swift: SessionRepository.swift > SessionRepository.loadNotes(sessionID:)
    pub fn load_notes(&self, session_id: &str) -> Option<EnhancedNotes> {
        let dir = self.session_dir(session_id);
        let md_path = dir.join("notes.md");
        let meta_path = dir.join("notes.meta.json");

        let markdown = fs::read_to_string(&md_path).ok()?;
        let meta_data = fs::read_to_string(&meta_path).ok()?;
        let meta: NotesMeta = serde_json::from_str(&meta_data).ok()?;

        Some(EnhancedNotes {
            template: meta.template_snapshot,
            generated_at: meta.generated_at,
            markdown,
        })
    }

    // MARK: - Listing & Loading

    // Swift: SessionRepository.swift > SessionRepository.listSessions()
    pub fn list_sessions(&self) -> Vec<SessionIndex> {
        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut results: Vec<SessionIndex> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') {
                continue;
            }

            let meta_path = path.join("session.json");
            if let Ok(data) = fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<SessionMetadata>(&data) {
                    results.push(SessionIndex {
                        id: meta.id,
                        started_at: meta.started_at,
                        ended_at: meta.ended_at,
                        template_snapshot: meta.template_snapshot,
                        title: meta.title,
                        utterance_count: meta.utterance_count,
                        has_notes: meta.has_notes,
                        language: meta.language,
                        meeting_app: meta.meeting_app,
                        engine: meta.engine,
                        tags: meta.tags,
                        source: meta.source,
                    });
                }
            }
        }

        results.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        results
    }

    // Swift: SessionRepository.swift > SessionRepository.loadTranscript(sessionID:)
    pub fn load_transcript(&self, session_id: &str) -> Vec<SessionRecord> {
        let dir = self.session_dir(session_id);

        // Prefer final transcript
        let final_path = dir.join("transcript.final.jsonl");
        if final_path.exists() {
            let records = self.parse_jsonl(&final_path);
            if !records.is_empty() {
                return records;
            }
        }

        // Fall back to live transcript
        let live_path = dir.join("transcript.live.jsonl");
        if live_path.exists() {
            return self.parse_jsonl(&live_path);
        }

        Vec::new()
    }

    // MARK: - Delete

    // Swift: SessionRepository.swift > SessionRepository.deleteSession(sessionID:)
    pub fn delete_session(&self, session_id: &str) -> bool {
        let dir = self.session_dir(session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir).is_ok()
        } else {
            false
        }
    }

    // MARK: - Rename

    // Swift: SessionRepository.swift > SessionRepository.renameSession(sessionID:title:)
    pub fn rename_session(&self, session_id: &str, title: &str) {
        if let Some(mut meta) = self.load_session_metadata(session_id) {
            meta.title = if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            };
            self.write_session_metadata(&meta, session_id);
        }
    }

    // MARK: - Helpers

    fn write_session_metadata(&self, metadata: &SessionMetadata, session_id: &str) {
        let dir = self.session_dir(session_id);
        let path = dir.join("session.json");
        if let Ok(json) = serde_json::to_string_pretty(metadata) {
            let _ = fs::write(&path, json);
        }
    }

    fn load_session_metadata(&self, session_id: &str) -> Option<SessionMetadata> {
        let dir = self.session_dir(session_id);
        let path = dir.join("session.json");
        let data = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn parse_jsonl(&self, path: &PathBuf) -> Vec<SessionRecord> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<SessionRecord>(line).ok())
            .collect()
    }

    pub fn current_session_id(&self) -> Option<&str> {
        self.current_session_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::utterance::Speaker;
    use tempfile::TempDir;

    fn make_repo() -> (SessionRepository, TempDir) {
        let dir = TempDir::new().unwrap();
        let repo = SessionRepository::new(Some(dir.path().to_path_buf()));
        (repo, dir)
    }

    #[test]
    fn start_session_creates_directory() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());

        let session_dir = repo.session_dir(&handle.session_id);
        assert!(session_dir.exists());
        assert!(session_dir.join("session.json").exists());
        assert!(session_dir.join("transcript.live.jsonl").exists());
    }

    #[test]
    fn start_session_writes_session_json() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());

        let meta = repo.load_session_metadata(&handle.session_id).unwrap();
        assert_eq!(meta.id, handle.session_id);
        assert_eq!(meta.utterance_count, 0);
        assert!(!meta.has_notes);
        assert!(meta.ended_at.is_none());
    }

    #[test]
    fn append_live_utterance_writes_jsonl() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());

        let utterance = Utterance::new("hello world".to_string(), Speaker::You);
        repo.append_live_utterance(&utterance);

        // Close writer to flush
        repo.end_session();

        let records = repo.load_transcript(&handle.session_id);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].text, "hello world");
    }

    #[test]
    fn append_multiple_utterances() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());

        for i in 0..5 {
            let u = Utterance::new(format!("utterance {}", i), Speaker::You);
            repo.append_live_utterance(&u);
        }
        repo.end_session();

        let records = repo.load_transcript(&handle.session_id);
        assert_eq!(records.len(), 5);
        assert_eq!(records[2].text, "utterance 2");
    }

    #[test]
    fn finalize_session_writes_metadata() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());
        let sid = handle.session_id.clone();

        repo.finalize_session(
            &sid,
            SessionFinalizeMetadata {
                ended_at: chrono::Utc::now().timestamp_millis(),
                utterance_count: 10,
                title: Some("Standup".to_string()),
                language: Some("en-US".to_string()),
                meeting_app: Some("Zoom".to_string()),
                engine: Some("parakeetV2".to_string()),
                template_snapshot: None,
            },
        );

        let meta = repo.load_session_metadata(&sid).unwrap();
        assert!(meta.ended_at.is_some());
        assert_eq!(meta.utterance_count, 10);
        assert_eq!(meta.title.as_deref(), Some("Standup"));
        assert_eq!(meta.language.as_deref(), Some("en-US"));
    }

    #[test]
    fn list_sessions_returns_all() {
        let (mut repo, _dir) = make_repo();

        // Create 3 sessions (with small delays so IDs differ)
        let ids: Vec<String> = (0..3)
            .map(|i| {
                // Manually create session dirs with unique IDs
                let sid = format!("session_test_{}", i);
                let dir = repo.sessions_dir.join(&sid);
                let _ = fs::create_dir_all(&dir);
                let meta = SessionMetadata {
                    id: sid.clone(),
                    started_at: 1000 + i as i64,
                    ended_at: None,
                    template_snapshot: None,
                    title: None,
                    utterance_count: 0,
                    has_notes: false,
                    language: None,
                    meeting_app: None,
                    engine: None,
                    tags: None,
                    source: None,
                };
                let json = serde_json::to_string_pretty(&meta).unwrap();
                let _ = fs::write(dir.join("session.json"), json);
                sid
            })
            .collect();

        let sessions = repo.list_sessions();
        assert_eq!(sessions.len(), 3);
        // Should be sorted newest first
        assert_eq!(sessions[0].id, ids[2]);
    }

    #[test]
    fn save_and_load_notes() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());
        let sid = handle.session_id.clone();
        repo.end_session();

        let notes = EnhancedNotes {
            template: TemplateSnapshot {
                id: "t1".to_string(),
                name: "Generic".to_string(),
                icon: "doc".to_string(),
                system_prompt: "test".to_string(),
            },
            generated_at: 1234567890,
            markdown: "# Meeting Notes\n\nGreat meeting.".to_string(),
        };

        repo.save_notes(&sid, &notes);

        let loaded = repo.load_notes(&sid).unwrap();
        assert_eq!(loaded.markdown, "# Meeting Notes\n\nGreat meeting.");
        assert_eq!(loaded.template.name, "Generic");
        assert_eq!(loaded.generated_at, 1234567890);

        // Verify session.json updated
        let meta = repo.load_session_metadata(&sid).unwrap();
        assert!(meta.has_notes);
    }

    #[test]
    fn save_final_transcript() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());
        let sid = handle.session_id.clone();

        // Write live utterances
        repo.append_live_utterance(&Utterance::new("live text".to_string(), Speaker::You));
        repo.end_session();

        // Save a final transcript (post-processed)
        let final_records = vec![
            SessionRecord::new(Speaker::You, "cleaned text".to_string(), 1000),
            SessionRecord::new(Speaker::Them, "response".to_string(), 2000),
        ];
        repo.save_final_transcript(&sid, &final_records);

        // load_transcript should prefer final over live
        let loaded = repo.load_transcript(&sid);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].text, "cleaned text");
    }

    #[test]
    fn delete_session_removes_directory() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());
        let sid = handle.session_id.clone();
        repo.end_session();

        assert!(repo.session_dir(&sid).exists());
        assert!(repo.delete_session(&sid));
        assert!(!repo.session_dir(&sid).exists());
    }

    #[test]
    fn load_transcript_nonexistent_returns_empty() {
        let (repo, _dir) = make_repo();
        let records = repo.load_transcript("does-not-exist");
        assert!(records.is_empty());
    }

    #[test]
    fn rename_session() {
        let (mut repo, _dir) = make_repo();
        let handle = repo.start_session(SessionStartConfig::default());
        let sid = handle.session_id.clone();
        repo.end_session();

        repo.rename_session(&sid, "Weekly Standup");
        let meta = repo.load_session_metadata(&sid).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Weekly Standup"));

        // Empty title clears it
        repo.rename_session(&sid, "");
        let meta = repo.load_session_metadata(&sid).unwrap();
        assert!(meta.title.is_none());
    }
}
