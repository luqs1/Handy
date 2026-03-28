//! Domain types for meeting detection and metadata.
//! Translated from: OpenOats/Sources/OpenOats/Domain/MeetingTypes.swift

use serde::{Deserialize, Serialize};

// MARK: - Meeting App Detection

/// A running application that may host meetings.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingApp {
    pub bundle_id: String,
    pub name: String,
}

/// A single entry in the list of known meeting apps.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingAppEntry {
    pub bundle_id: String,
    pub display_name: String,
}

// MARK: - Detection Signal

/// Describes why the system believes a meeting started or ended.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum DetectionSignal {
    /// User pressed Start manually.
    Manual,
    /// A known meeting app was detected running.
    AppLaunched(MeetingApp),
    /// A calendar event started.
    CalendarEvent(CalendarEvent),
    /// Audio activity was detected from a meeting source.
    AudioActivity,
}

// MARK: - Detection Context

/// Aggregated context about an active or pending meeting.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectionContext {
    pub signal: DetectionSignal,
    pub detected_at: i64, // milliseconds since epoch
    pub meeting_app: Option<MeetingApp>,
    pub calendar_event: Option<CalendarEvent>,
}

// MARK: - Calendar Integration

/// Minimal representation of a calendar event relevant to meeting detection.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start_date: i64, // milliseconds since epoch
    pub end_date: i64,
    pub organizer: Option<String>,
    pub participants: Vec<Participant>,
    pub is_online_meeting: bool,
    pub meeting_url: Option<String>,
}

/// A meeting participant from a calendar event.
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub name: Option<String>,
    pub email: Option<String>,
}

// MARK: - Meeting Metadata

/// Metadata assembled during a meeting session (detection context + calendar info).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingMetadata {
    pub detection_context: Option<DetectionContext>,
    pub calendar_event: Option<CalendarEvent>,
    pub title: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

impl MeetingMetadata {
    // Swift: MeetingTypes.swift > MeetingMetadata.manual()
    pub fn manual() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            detection_context: Some(DetectionContext {
                signal: DetectionSignal::Manual,
                detected_at: now,
                meeting_app: None,
                calendar_event: None,
            }),
            calendar_event: None,
            title: None,
            started_at: now,
            ended_at: None,
        }
    }
}
