/// Review-before-paste: after transcription, show a small editable panel
/// instead of pasting immediately. The user can fix recognition errors and
/// confirm with Enter, tap Option/Alt to paste as-is, or press Escape to
/// cancel. Confirmed edits are stored on the history entry (`edited_text`)
/// so they can later be exported as (audio, corrected transcript) pairs.
///
/// macOS only: the panel is a non-activating NSPanel that can become the key
/// window, so it accepts typing while the target application stays active —
/// the same mechanism Spotlight-style launchers use. On other platforms
/// `request_review` is a pass-through.
use log::{debug, error, warn};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

#[cfg(target_os = "macos")]
use tauri::WebviewUrl;

#[cfg(target_os = "macos")]
use tauri_nspanel::{tauri_panel, CollectionBehavior, PanelBuilder, PanelLevel, StyleMask};

pub const REVIEW_PANEL_LABEL: &str = "review_panel";

const REVIEW_WIDTH: f64 = 560.0;
const REVIEW_HEIGHT: f64 = 180.0;

/// How long the panel waits for a decision before giving up (no paste).
const REVIEW_TIMEOUT_SECS: u64 = 300;

#[cfg(target_os = "macos")]
tauri_panel! {
    panel!(ReviewPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true
        }
    })
}

/// The outcome of a review round-trip.
#[derive(Debug)]
pub enum ReviewDecision {
    /// Paste this text (possibly edited by the user).
    Paste(String),
    /// User dismissed the panel; paste nothing.
    Cancelled,
}

static PENDING_REVIEW: Mutex<Option<oneshot::Sender<ReviewDecision>>> = Mutex::new(None);

#[derive(Clone, Serialize)]
struct ReviewShowPayload {
    text: String,
}

/// Creates the hidden review panel at startup (macOS).
#[cfg(target_os = "macos")]
pub fn create_review_panel(app_handle: &AppHandle) {
    match PanelBuilder::<_, ReviewPanel>::new(app_handle, REVIEW_PANEL_LABEL)
        .url(WebviewUrl::App("src/review/index.html".into()))
        .title("Review Transcription")
        .level(PanelLevel::Status)
        .size(tauri::Size::Logical(tauri::LogicalSize {
            width: REVIEW_WIDTH,
            height: REVIEW_HEIGHT,
        }))
        .has_shadow(true)
        .transparent(true)
        .corner_radius(12.0)
        // Non-activating: the panel takes key status without activating Handy,
        // so the target app keeps focus and receives the eventual Cmd+V.
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .with_window(|w| w.decorations(false).transparent(true))
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .build()
    {
        Ok(panel) => {
            panel.hide();
        }
        Err(e) => {
            error!("Failed to create review panel: {}", e);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn create_review_panel(_app_handle: &AppHandle) {}

/// Centers the panel horizontally on the monitor with the cursor, in the
/// upper third of the screen (Spotlight-style), sized for the text.
#[cfg(target_os = "macos")]
fn position_review_panel(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(REVIEW_PANEL_LABEL) {
        if let Some(monitor) = crate::overlay::get_monitor_with_cursor(app_handle) {
            let scale = monitor.scale_factor();
            let mx = monitor.position().x as f64 / scale;
            let my = monitor.position().y as f64 / scale;
            let mw = monitor.size().width as f64 / scale;
            let mh = monitor.size().height as f64 / scale;
            let x = mx + (mw - REVIEW_WIDTH) / 2.0;
            let y = my + mh * 0.28;
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        }
    }
}

/// Shows the review panel with `text` and waits for the user's decision.
///
/// Returns `Paste(text)` on confirm (Enter or Option tap), `Cancelled` on
/// Escape/timeout/preemption by a newer transcription.
#[cfg(target_os = "macos")]
pub async fn request_review(app_handle: &AppHandle, text: String) -> ReviewDecision {
    use tauri_nspanel::ManagerExt;

    let (tx, rx) = oneshot::channel();
    // A newer transcription preempts any stale pending review: dropping the
    // old sender resolves its receiver as Cancelled below.
    if let Some(_old) = PENDING_REVIEW.lock().unwrap().replace(tx) {
        warn!("Replacing a stale pending review");
    }

    let ah = app_handle.clone();
    let payload = ReviewShowPayload { text };
    let shown = app_handle.run_on_main_thread(move || {
        position_review_panel(&ah);
        // Targeted emit so other windows never see transcription content.
        if let Some(window) = ah.get_webview_window(REVIEW_PANEL_LABEL) {
            let _ = window.emit("review-show", payload);
        }
        match ah.get_webview_panel(REVIEW_PANEL_LABEL) {
            Ok(panel) => {
                panel.show_and_make_key();
            }
            Err(e) => error!("Review panel missing: {:?}", e),
        }
    });

    if let Err(e) = shown {
        error!("Failed to show review panel: {:?}", e);
        PENDING_REVIEW.lock().unwrap().take();
        return ReviewDecision::Cancelled;
    }

    let decision =
        match tokio::time::timeout(std::time::Duration::from_secs(REVIEW_TIMEOUT_SECS), rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => ReviewDecision::Cancelled, // sender dropped (preempted)
            Err(_) => {
                debug!("Review timed out after {}s", REVIEW_TIMEOUT_SECS);
                PENDING_REVIEW.lock().unwrap().take();
                ReviewDecision::Cancelled
            }
        };

    hide_review_panel(app_handle);
    decision
}

#[cfg(not(target_os = "macos"))]
pub async fn request_review(_app_handle: &AppHandle, text: String) -> ReviewDecision {
    ReviewDecision::Paste(text)
}

/// Hides the panel and returns key status to the target application.
pub fn hide_review_panel(app_handle: &AppHandle) {
    let ah = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        if let Some(window) = ah.get_webview_window(REVIEW_PANEL_LABEL) {
            let _ = window.hide();
        }
    });
}

/// Cancels any pending review (e.g. when a new recording starts).
pub fn cancel_pending_review(app_handle: &AppHandle) {
    if PENDING_REVIEW.lock().unwrap().take().is_some() {
        debug!("Cancelled pending review");
        hide_review_panel(app_handle);
    }
}

/// Frontend command: the user confirmed the (possibly edited) text.
#[tauri::command]
#[specta::specta]
pub fn review_submit(app: AppHandle, text: String) -> Result<(), String> {
    match PENDING_REVIEW.lock().unwrap().take() {
        Some(tx) => {
            let _ = tx.send(ReviewDecision::Paste(text));
            Ok(())
        }
        None => {
            hide_review_panel(&app);
            Err("No pending review".to_string())
        }
    }
}

/// Frontend command: the user dismissed the panel.
#[tauri::command]
#[specta::specta]
pub fn review_cancel(app: AppHandle) -> Result<(), String> {
    match PENDING_REVIEW.lock().unwrap().take() {
        Some(tx) => {
            let _ = tx.send(ReviewDecision::Cancelled);
            Ok(())
        }
        None => {
            hide_review_panel(&app);
            Ok(())
        }
    }
}
