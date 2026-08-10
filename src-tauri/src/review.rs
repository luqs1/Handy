/// Review-before-paste: after transcription, the recording overlay switches to
/// an editable "review" state instead of pasting immediately. The user can fix
/// recognition errors and confirm with Enter, tap Option/Alt to paste as-is,
/// or press Escape to cancel. Confirmed edits are stored on the history entry
/// (`edited_text`) so they can later be exported as (audio, corrected
/// transcript) pairs.
///
/// The review UI lives inside the existing recording overlay window rather
/// than a dedicated panel: that webview is woken on every dictation, so its JS
/// context is reliably alive when the review text arrives (a long-hidden
/// window's web content process gets terminated by macOS). The overlay's
/// NSPanel is non-activating but key-able, so it accepts typing while the
/// target application stays active. On non-macOS platforms `request_review`
/// is a pass-through.
use log::{debug, error, warn};
use serde::Serialize;
use std::sync::Mutex;
use tauri::AppHandle;
use tokio::sync::oneshot;

#[cfg(target_os = "macos")]
use tauri::{Emitter, Manager};

/// How long the review overlay waits for a decision before giving up (no paste).
#[cfg(target_os = "macos")]
const REVIEW_TIMEOUT_SECS: u64 = 300;

/// The outcome of a review round-trip.
#[derive(Debug)]
pub enum ReviewDecision {
    /// Paste this text (possibly edited by the user).
    Paste(String),
    /// User dismissed the review (Escape or timeout); paste nothing.
    Cancelled,
    /// A newer dictation replaced this review. Paste nothing and leave the
    /// overlay alone — the new session owns it now.
    Preempted,
}

struct PendingReview {
    text: String,
    tx: oneshot::Sender<ReviewDecision>,
}

static PENDING_REVIEW: Mutex<Option<PendingReview>> = Mutex::new(None);

#[derive(Clone, Serialize)]
struct ReviewShowPayload {
    text: String,
}

/// Shows the review overlay with `text` and waits for the user's decision.
#[cfg(target_os = "macos")]
pub async fn request_review(app_handle: &AppHandle, text: String) -> ReviewDecision {
    let (tx, rx) = oneshot::channel();
    if PENDING_REVIEW
        .lock()
        .unwrap()
        .replace(PendingReview {
            text: text.clone(),
            tx,
        })
        .is_some()
    {
        warn!("Replacing a stale pending review");
    }

    crate::overlay::show_review_overlay(app_handle);
    // Push the text via event; the overlay also pulls it via
    // `get_pending_review_text` when it enters the review state, so a lost
    // event (webview mid-reload) can't leave an empty box.
    if let Some(window) = app_handle.get_webview_window("recording_overlay") {
        let _ = window.emit("review-show", ReviewShowPayload { text });
    }

    let decision =
        match tokio::time::timeout(std::time::Duration::from_secs(REVIEW_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => ReviewDecision::Preempted, // sender dropped by a newer dictation
            Err(_) => {
                debug!("Review timed out after {}s", REVIEW_TIMEOUT_SECS);
                PENDING_REVIEW.lock().unwrap().take();
                ReviewDecision::Cancelled
            }
        };

    // Return key status to the target app before any paste happens. Harmless
    // for Cancelled; skipped for Preempted (the new session owns the overlay).
    if !matches!(decision, ReviewDecision::Preempted) {
        crate::overlay::resign_review_key(app_handle);
    }
    decision
}

#[cfg(not(target_os = "macos"))]
pub async fn request_review(_app_handle: &AppHandle, text: String) -> ReviewDecision {
    ReviewDecision::Paste(text)
}

/// Cancels any pending review (called when a new recording starts). The old
/// `request_review` resolves as `Preempted` and leaves the overlay alone.
pub fn cancel_pending_review(_app_handle: &AppHandle) {
    if PENDING_REVIEW.lock().unwrap().take().is_some() {
        debug!("Preempted pending review");
    }
}

/// Frontend pull: the overlay asks for the text when it enters review state,
/// covering the case where the `review-show` event raced a webview reload.
#[tauri::command]
#[specta::specta]
pub fn get_pending_review_text() -> Option<String> {
    PENDING_REVIEW
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.text.clone())
}

/// Frontend command: the user confirmed the (possibly edited) text.
#[tauri::command]
#[specta::specta]
pub fn review_submit(text: String) -> Result<(), String> {
    match PENDING_REVIEW.lock().unwrap().take() {
        Some(pending) => {
            let _ = pending.tx.send(ReviewDecision::Paste(text));
            Ok(())
        }
        None => {
            error!("review_submit with no pending review");
            Err("No pending review".to_string())
        }
    }
}

/// Frontend command: the user dismissed the review.
#[tauri::command]
#[specta::specta]
pub fn review_cancel() -> Result<(), String> {
    if let Some(pending) = PENDING_REVIEW.lock().unwrap().take() {
        let _ = pending.tx.send(ReviewDecision::Cancelled);
    }
    Ok(())
}
