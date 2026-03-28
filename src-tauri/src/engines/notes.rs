//! Meeting notes generation engine.
//! Translated from: OpenOats/Sources/OpenOats/Intelligence/NotesEngine.swift
//!
//! Streams note generation from an LLM, building markdown in real time.
//! Uses the existing llm_client infrastructure for API calls.

use crate::domain::models::{MeetingTemplate, SessionRecord};
use crate::llm_client;
use crate::settings::PostProcessProvider;
use log::{info, warn};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

const MAX_TRANSCRIPT_CHARS: usize = 60_000;

/// State for the notes generation engine.
pub struct NotesEngine {
    app_handle: AppHandle,
    is_generating: Arc<Mutex<bool>>,
    generated_markdown: Arc<Mutex<String>>,
    error: Arc<Mutex<Option<String>>>,
    cancel_token: Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
}

impl NotesEngine {
    pub fn new(app_handle: &AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
            is_generating: Arc::new(Mutex::new(false)),
            generated_markdown: Arc::new(Mutex::new(String::new())),
            error: Arc::new(Mutex::new(None)),
            cancel_token: Arc::new(Mutex::new(None)),
        }
    }

    // Swift: NotesEngine.swift > NotesEngine.generate(transcript:template:settings:)
    /// Stream note generation from the LLM, emitting chunks via Tauri events.
    pub async fn generate(
        &self,
        transcript: Vec<SessionRecord>,
        template: MeetingTemplate,
        provider: PostProcessProvider,
        api_key: String,
        model: String,
    ) -> Result<String, String> {
        // Cancel any in-flight generation
        self.cancel().await;

        *self.is_generating.lock().await = true;
        *self.generated_markdown.lock().await = String::new();
        *self.error.lock().await = None;

        let _ = self.app_handle.emit("notes-generating", true);

        let cancel = tokio_util::sync::CancellationToken::new();
        *self.cancel_token.lock().await = Some(cancel.clone());

        let transcript_text = format_transcript(&transcript);
        let system_prompt = template.system_prompt.clone();
        let user_content = format!(
            "Here is the meeting transcript:\n\n{}\n\nGenerate the meeting notes in markdown:",
            transcript_text
        );

        let markdown = self.generated_markdown.clone();
        let app = self.app_handle.clone();

        let result = llm_client::stream_chat_completion(
            &provider,
            api_key,
            &model,
            Some(system_prompt),
            user_content,
            move |chunk| {
                let mut md = markdown.blocking_lock();
                md.push_str(&chunk);
                let _ = app.emit("notes-chunk", &chunk);
            },
        )
        .await;

        *self.is_generating.lock().await = false;
        let _ = self.app_handle.emit("notes-generating", false);

        match result {
            Ok(full_text) => {
                *self.generated_markdown.lock().await = full_text.clone();
                info!("Notes generation complete ({} chars)", full_text.len());
                let _ = self.app_handle.emit("notes-complete", &full_text);
                Ok(full_text)
            }
            Err(e) => {
                warn!("Notes generation failed: {}", e);
                *self.error.lock().await = Some(e.clone());
                let _ = self.app_handle.emit("notes-error", &e);
                Err(e)
            }
        }
    }

    pub async fn cancel(&self) {
        if let Some(token) = self.cancel_token.lock().await.take() {
            token.cancel();
        }
        *self.is_generating.lock().await = false;
    }

    pub async fn is_generating(&self) -> bool {
        *self.is_generating.lock().await
    }

    pub async fn generated_markdown(&self) -> String {
        self.generated_markdown.lock().await.clone()
    }
}

// Swift: NotesEngine.swift > NotesEngine.formatTranscript(_:)
fn format_transcript(records: &[SessionRecord]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut total_chars: usize = 0;

    for record in records {
        let label = record.speaker.display_label();
        let best_text = record.refined_text.as_deref().unwrap_or(&record.text);

        // Format timestamp as HH:MM:SS from epoch millis
        let ts = chrono::DateTime::from_timestamp_millis(record.timestamp)
            .map(|dt| dt.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "??:??:??".to_string());

        let line = format!("[{}] {}: {}", ts, label, best_text);
        total_chars += line.len();
        lines.push(line);
    }

    // Truncate middle if too long (matches Swift behavior)
    if total_chars > MAX_TRANSCRIPT_CHARS {
        let keep_lines = lines.len() / 3;
        let head: Vec<_> = lines.iter().take(keep_lines).cloned().collect();
        let tail: Vec<_> = lines.iter().rev().take(keep_lines).rev().cloned().collect();
        let omitted = lines.len() - (keep_lines * 2);

        let mut result = head;
        result.push(format!("[... {} utterances omitted ...]", omitted));
        result.extend(tail);
        result.join("\n")
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::utterance::Speaker;

    #[test]
    fn test_format_transcript_basic() {
        let records = vec![
            SessionRecord::new(Speaker::You, "Hello".to_string(), 1711000000000),
            SessionRecord::new(Speaker::Them, "Hi there".to_string(), 1711000003000),
        ];

        let result = format_transcript(&records);
        assert!(result.contains("You: Hello"));
        assert!(result.contains("Them: Hi there"));
    }

    #[test]
    fn test_format_transcript_prefers_refined() {
        let mut record = SessionRecord::new(Speaker::You, "uh hello".to_string(), 1711000000000);
        record.refined_text = Some("Hello".to_string());

        let result = format_transcript(&[record]);
        assert!(result.contains("You: Hello"));
        assert!(!result.contains("uh hello"));
    }
}
