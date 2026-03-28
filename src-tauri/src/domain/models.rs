//! Domain model types: suggestions, session records, templates.
//! Translated from: OpenOats/Sources/OpenOats/Models/Models.swift

use super::utterance::Speaker;
use serde::{Deserialize, Serialize};

// MARK: - Suggestion Trigger

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionTriggerKind {
    ExplicitQuestion,
    DecisionPoint,
    Disagreement,
    Assumption,
    Prioritization,
    CustomerProblem,
    DistributionGoToMarket,
    ProductScope,
    Unclear,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionTrigger {
    pub kind: SuggestionTriggerKind,
    pub utterance_id: String,
    pub excerpt: String,
    pub confidence: f64,
}

// MARK: - Suggestion Evidence

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionEvidence {
    pub source_file: String,
    pub header_context: String,
    pub text: String,
    pub score: f64,
}

// MARK: - Suggestion Decision (Surfacing Gate)

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionDecision {
    pub should_surface: bool,
    pub confidence: f64,
    pub relevance_score: f64,
    pub helpfulness_score: f64,
    pub timing_score: f64,
    pub novelty_score: f64,
    pub reason: String,
    pub trigger: Option<SuggestionTrigger>,
}

// MARK: - Suggestion Feedback

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionFeedback {
    Helpful,
    NotHelpful,
    Dismissed,
}

// MARK: - KB Result

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KBResult {
    pub id: String,
    pub text: String,
    pub source_file: String,
    pub header_context: String,
    pub score: f64,
}

impl KBResult {
    pub fn new(text: String, source_file: String, header_context: String, score: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            source_file,
            header_context,
            score,
        }
    }
}

// MARK: - Suggestion

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub text: String,
    pub timestamp: i64,
    pub kb_hits: Vec<KBResult>,
    pub decision: Option<SuggestionDecision>,
    pub trigger: Option<SuggestionTrigger>,
    pub summary_snapshot: Option<String>,
    pub feedback: Option<SuggestionFeedback>,
}

impl Suggestion {
    pub fn new(text: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            timestamp: chrono::Utc::now().timestamp_millis(),
            kb_hits: Vec::new(),
            decision: None,
            trigger: None,
            summary_snapshot: None,
            feedback: None,
        }
    }
}

// MARK: - Session Record

/// Codable record for JSONL session persistence.
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub speaker: Speaker,
    pub text: String,
    pub timestamp: i64,
    pub suggestions: Option<Vec<String>>,
    pub kb_hits: Option<Vec<String>>,
    pub suggestion_decision: Option<SuggestionDecision>,
    pub surfaced_suggestion_text: Option<String>,
    pub conversation_state_summary: Option<String>,
    pub refined_text: Option<String>,
}

impl SessionRecord {
    pub fn new(speaker: Speaker, text: String, timestamp: i64) -> Self {
        Self {
            speaker,
            text,
            timestamp,
            suggestions: None,
            kb_hits: None,
            suggestion_decision: None,
            surfaced_suggestion_text: None,
            conversation_state_summary: None,
            refined_text: None,
        }
    }

    pub fn with_refined_text(&self, text: Option<String>) -> Self {
        let mut record = self.clone();
        record.refined_text = text;
        record
    }
}

// MARK: - Meeting Templates & Enhanced Notes

#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingTemplate {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub system_prompt: String,
    pub is_built_in: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSnapshot {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub system_prompt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedNotes {
    pub template: TemplateSnapshot,
    pub generated_at: i64,
    pub markdown: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndex {
    pub id: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub template_snapshot: Option<TemplateSnapshot>,
    pub title: Option<String>,
    pub utterance_count: usize,
    pub has_notes: bool,
    pub language: Option<String>,
    pub meeting_app: Option<String>,
    pub engine: Option<String>,
    pub tags: Option<Vec<String>>,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSidecar {
    pub index: SessionIndex,
    pub notes: Option<EnhancedNotes>,
}
