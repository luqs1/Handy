//! Tauri commands for the new session coordinator.
//! These coexist with the existing meeting commands in commands/meeting.rs.
//! The frontend can migrate to these incrementally.

use crate::domain::models::{MeetingTemplate, SessionIndex, SessionRecord};
use crate::domain::utterance::Utterance;
use crate::llm_client;
use crate::session_coordinator::{LiveSessionState, SessionCoordinator};
use crate::settings::get_settings;
use crate::stores::template_store;
use log::info;
use specta::Type;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex as TokioMutex;

// Type alias for the managed coordinator state
pub type CoordinatorState = Arc<TokioMutex<SessionCoordinator>>;

// MARK: - Session Lifecycle

#[tauri::command]
#[specta::specta]
pub async fn session_start(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
) -> Result<String, String> {
    let mut coord = coordinator.lock().await;
    let metadata = crate::domain::meeting_types::MeetingMetadata::manual();
    coord.handle(crate::domain::meeting_state::MeetingEvent::UserStarted(metadata));

    let state = coord.live_state();
    let session_id = state.session_id.clone().unwrap_or_default();
    let _ = app.emit("session-started", &session_id);
    info!("Session started via command: {}", session_id);
    Ok(session_id)
}

#[tauri::command]
#[specta::specta]
pub async fn session_stop(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
) -> Result<(), String> {
    let mut coord = coordinator.lock().await;
    coord.handle(crate::domain::meeting_state::MeetingEvent::UserStopped);
    let _ = app.emit("session-stopped", ());
    info!("Session stopped via command");
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn session_discard(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
) -> Result<(), String> {
    let mut coord = coordinator.lock().await;
    coord.handle(crate::domain::meeting_state::MeetingEvent::UserDiscarded);
    let _ = app.emit("session-discarded", ());
    info!("Session discarded via command");
    Ok(())
}

// MARK: - Session State

#[tauri::command]
#[specta::specta]
pub async fn session_live_state(
    coordinator: State<'_, CoordinatorState>,
) -> Result<LiveSessionState, String> {
    let coord = coordinator.lock().await;
    Ok(coord.live_state())
}

#[tauri::command]
#[specta::specta]
pub async fn session_is_recording(
    coordinator: State<'_, CoordinatorState>,
) -> Result<bool, String> {
    let coord = coordinator.lock().await;
    Ok(coord.is_recording())
}

// MARK: - Session History

#[tauri::command]
#[specta::specta]
pub async fn session_list(
    coordinator: State<'_, CoordinatorState>,
) -> Result<Vec<SessionIndex>, String> {
    let coord = coordinator.lock().await;
    Ok(coord.list_sessions())
}

#[tauri::command]
#[specta::specta]
pub async fn session_transcript(
    coordinator: State<'_, CoordinatorState>,
    session_id: String,
) -> Result<Vec<SessionRecord>, String> {
    let coord = coordinator.lock().await;
    Ok(coord.load_transcript(&session_id))
}

#[tauri::command]
#[specta::specta]
pub async fn session_delete(
    coordinator: State<'_, CoordinatorState>,
    session_id: String,
) -> Result<bool, String> {
    let coord = coordinator.lock().await;
    Ok(coord.delete_session(&session_id))
}

#[tauri::command]
#[specta::specta]
pub async fn session_rename(
    coordinator: State<'_, CoordinatorState>,
    session_id: String,
    title: String,
) -> Result<(), String> {
    let coord = coordinator.lock().await;
    coord.rename_session(&session_id, &title);
    Ok(())
}

// MARK: - Templates

#[tauri::command]
#[specta::specta]
pub async fn session_templates(
    coordinator: State<'_, CoordinatorState>,
) -> Result<Vec<MeetingTemplate>, String> {
    let coord = coordinator.lock().await;
    Ok(coord.template_store().templates().to_vec())
}

// MARK: - Notes Generation

/// Generate meeting notes for a completed session using the configured LLM provider.
// Swift: NotesController.swift > NotesController.generateNotes(for:template:)
#[tauri::command]
#[specta::specta]
pub async fn session_generate_notes(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
    session_id: String,
    template_id: Option<String>,
) -> Result<String, String> {
    // Load transcript
    let transcript = {
        let coord = coordinator.lock().await;
        coord.load_transcript(&session_id)
    };

    if transcript.is_empty() {
        return Err("No transcript available for this session".to_string());
    }

    // Resolve template
    let template = {
        let coord = coordinator.lock().await;
        let tid = template_id.as_deref().unwrap_or("00000000-0000-0000-0000-000000000000");
        coord
            .template_store()
            .template_for(tid)
            .cloned()
            .unwrap_or_else(|| template_store::built_in_templates().into_iter().next().unwrap())
    };

    // Resolve LLM provider from settings
    let settings = get_settings(&app);
    let provider_id = &settings.post_process_provider_id;
    let provider = settings
        .active_post_process_provider()
        .cloned()
        .ok_or_else(|| format!("Provider '{}' not found", provider_id))?;

    let api_key = settings
        .post_process_api_keys
        .get(provider_id)
        .cloned()
        .unwrap_or_default();

    let model = settings
        .post_process_models
        .get(provider_id)
        .cloned()
        .unwrap_or_else(|| "openai/gpt-4o-mini".to_string());

    // Format transcript for LLM
    let formatted = transcript
        .iter()
        .map(|r| {
            let label = r.speaker.display_label();
            let text = r.refined_text.as_deref().unwrap_or(&r.text);
            format!("[{}] {}", label, text)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let user_content = format!(
        "Here is the meeting transcript:\n\n{}\n\nGenerate the meeting notes in markdown:",
        formatted
    );

    let app_clone = app.clone();
    let result = llm_client::stream_chat_completion(
        &provider,
        api_key,
        &model,
        Some(template.system_prompt.clone()),
        user_content,
        move |chunk| {
            let _ = app_clone.emit("session-notes-chunk", &chunk);
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    // Save notes to the session repository
    {
        let coord = coordinator.lock().await;
        let snapshot = crate::stores::template_store::TemplateStore::snapshot(&template);
        let notes = crate::domain::models::EnhancedNotes {
            template: snapshot,
            generated_at: chrono::Utc::now().timestamp_millis(),
            markdown: result.clone(),
        };
        coord.session_repo().save_notes(&session_id, &notes);
    }

    let _ = app.emit("session-notes-complete", &result);
    info!("Notes generated for session {}", session_id);
    Ok(result)
}
