//! Speaker, Utterance, and ConversationState types.
//! Translated from: OpenOats/Sources/OpenOats/Domain/Utterance.swift

use serde::{Deserialize, Serialize};

// MARK: - Speaker

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Speaker {
    You,
    Them,
    Remote(i32),
}

impl Speaker {
    // Swift: Utterance.swift > Speaker.displayLabel
    pub fn display_label(&self) -> String {
        match self {
            Speaker::You => "You".to_string(),
            Speaker::Them => "Them".to_string(),
            Speaker::Remote(n) => format!("Speaker {}", n),
        }
    }

    // Swift: Utterance.swift > Speaker.isRemote
    pub fn is_remote(&self) -> bool {
        !matches!(self, Speaker::You)
    }

    // Swift: Utterance.swift > Speaker.storageKey
    pub fn storage_key(&self) -> String {
        match self {
            Speaker::You => "you".to_string(),
            Speaker::Them => "them".to_string(),
            Speaker::Remote(n) => format!("remote_{}", n),
        }
    }
}

// specta::Type — Speaker serializes as a plain string, so expose it that way
impl specta::Type for Speaker {
    fn inline(
        _type_map: &mut specta::TypeMap,
        _generics: specta::Generics,
    ) -> specta::datatype::DataType {
        specta::datatype::DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}

// Swift: Utterance.swift > Speaker.init(from:) + Speaker.encode(to:)
// Custom Serialize/Deserialize to match Swift's single-value coding
impl Serialize for Speaker {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.storage_key())
    }
}

impl<'de> Deserialize<'de> for Speaker {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        match raw.as_str() {
            "you" => Ok(Speaker::You),
            "them" => Ok(Speaker::Them),
            other => {
                if let Some(n_str) = other.strip_prefix("remote_") {
                    if let Ok(n) = n_str.parse::<i32>() {
                        return Ok(Speaker::Remote(n));
                    }
                }
                Ok(Speaker::Them) // fallback, matches Swift behavior
            }
        }
    }
}

// MARK: - Refinement Status

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RefinementStatus {
    Pending,
    Completed,
    Failed,
    Skipped,
}

// MARK: - Utterance

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Utterance {
    pub id: String,
    pub text: String,
    pub speaker: Speaker,
    pub timestamp: i64, // milliseconds since epoch
    pub refined_text: Option<String>,
    pub refinement_status: Option<RefinementStatus>,
}

impl Utterance {
    pub fn new(text: String, speaker: Speaker) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            speaker,
            timestamp: chrono::Utc::now().timestamp_millis(),
            refined_text: None,
            refinement_status: None,
        }
    }

    // Swift: Utterance.swift > Utterance.displayText
    /// The best available text: refined if available, otherwise raw.
    pub fn display_text(&self) -> &str {
        self.refined_text.as_deref().unwrap_or(&self.text)
    }

    // Swift: Utterance.swift > Utterance.withRefinement(text:status:)
    pub fn with_refinement(&self, text: Option<String>, status: RefinementStatus) -> Self {
        Self {
            id: self.id.clone(),
            text: self.text.clone(),
            speaker: self.speaker.clone(),
            timestamp: self.timestamp,
            refined_text: text,
            refinement_status: Some(status),
        }
    }
}

// MARK: - Conversation State

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationState {
    pub current_topic: String,
    pub short_summary: String,
    pub open_questions: Vec<String>,
    pub active_tensions: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub them_goals: Vec<String>,
    pub suggested_angles_recently_shown: Vec<String>,
    pub last_updated_at: i64,
}

impl Default for ConversationState {
    fn default() -> Self {
        Self {
            current_topic: String::new(),
            short_summary: String::new(),
            open_questions: Vec::new(),
            active_tensions: Vec::new(),
            recent_decisions: Vec::new(),
            them_goals: Vec::new(),
            suggested_angles_recently_shown: Vec::new(),
            last_updated_at: 0,
        }
    }
}
