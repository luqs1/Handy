//! 5-stage suggestion pipeline:
//! heuristic filter → trigger detection → conversation state update → surfacing gate → generation.
//! Translated from: OpenOats/Sources/OpenOats/Intelligence/SuggestionEngine.swift
//!
//! This module exposes the pure, testable functions. The async LLM orchestration
//! (stages 2, 4, 5) uses llm_client and is wired up in session_coordinator.

use crate::domain::models::{SuggestionDecision, SuggestionTrigger, SuggestionTriggerKind};
use crate::domain::utterance::Utterance;
use crate::stores::text_similarity;
use std::collections::HashSet;

// MARK: - Configuration

pub const MIN_UTTERANCE_WORD_COUNT: usize = 8;
pub const MIN_UTTERANCE_CHAR_COUNT: usize = 30;
pub const MIN_KB_RELEVANCE_SCORE: f64 = 0.35;

// Base gate thresholds (scaled by verbosity multiplier)
pub const BASE_RELEVANCE_SCORE: f64 = 0.72;
pub const BASE_HELPFULNESS_SCORE: f64 = 0.75;
pub const BASE_TIMING_SCORE: f64 = 0.70;
pub const BASE_NOVELTY_SCORE: f64 = 0.65;
pub const BASE_CONFIDENCE_SCORE: f64 = 0.75;

/// Verbosity level controls cooldown and threshold scaling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SuggestionVerbosity {
    Quiet,
    Balanced,
    Eager,
}

impl SuggestionVerbosity {
    pub fn cooldown_seconds(&self) -> f64 {
        match self {
            Self::Quiet => 90.0,
            Self::Balanced => 45.0,
            Self::Eager => 20.0,
        }
    }

    /// Multiplier applied to base thresholds. Lower = easier to surface.
    pub fn threshold_multiplier(&self) -> f64 {
        match self {
            Self::Quiet => 1.1,
            Self::Balanced => 1.0,
            Self::Eager => 0.85,
        }
    }
}

// MARK: - Stage 1: Heuristic Pre-Filter

lazy_static::lazy_static! {
    static ref FILLER_WORDS: HashSet<&'static str> = {
        [
            "yeah", "yes", "no", "ok", "okay", "right", "sure", "uh", "um",
            "hmm", "huh", "mhm", "like", "so", "well", "anyway", "basically",
            "literally", "actually", "honestly", "totally", "exactly",
        ].into_iter().collect()
    };
}

// Swift: SuggestionEngine.swift > SuggestionEngine.shouldEvaluateUtterance(_:)
/// Determines whether an utterance should be evaluated by the suggestion pipeline.
/// Returns false for short, filler-heavy, or near-duplicate utterances.
pub fn should_evaluate_utterance(
    utterance: &Utterance,
    recent_them_texts: &[&str],
    last_suggestion_elapsed_secs: Option<f64>,
    cooldown_seconds: f64,
) -> bool {
    let text = utterance.text.trim();

    // Minimum length checks
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < MIN_UTTERANCE_WORD_COUNT {
        return false;
    }
    if text.len() < MIN_UTTERANCE_CHAR_COUNT {
        return false;
    }

    // Cooldown
    if let Some(elapsed) = last_suggestion_elapsed_secs {
        if elapsed < cooldown_seconds {
            return false;
        }
    }

    // Filler detection — skip mostly filler utterances
    let lowercase_words: Vec<String> = words
        .iter()
        .map(|w| w.to_lowercase().trim_matches(|c: char| c.is_ascii_punctuation()).to_string())
        .collect();
    let filler_count = lowercase_words
        .iter()
        .filter(|w| FILLER_WORDS.contains(w.as_str()))
        .count();
    let filler_ratio = filler_count as f64 / words.len() as f64;
    if filler_ratio > 0.6 {
        return false;
    }

    // Near-duplicate check against recent them utterances
    for recent_text in recent_them_texts.iter().take(3) {
        if text_similarity::jaccard(text, recent_text) > 0.8 {
            return false;
        }
    }

    true
}

// MARK: - Stage 1b: Trigger Detection

// Swift: SuggestionEngine.swift > SuggestionEngine.detectTrigger(for:)
/// Detects what triggered a suggestion opportunity from an utterance.
pub fn detect_trigger(utterance: &Utterance) -> Option<SuggestionTrigger> {
    let text = utterance.text.to_lowercase();
    let excerpt = utterance.text.chars().take(100).collect::<String>();

    // Question detection
    if text.contains('?')
        || text.starts_with("what ")
        || text.starts_with("how ")
        || text.starts_with("why ")
        || text.starts_with("should ")
        || text.starts_with("could ")
        || text.starts_with("would ")
        || text.starts_with("do you think")
    {
        return Some(SuggestionTrigger {
            kind: SuggestionTriggerKind::ExplicitQuestion,
            utterance_id: utterance.id.clone(),
            excerpt,
            confidence: 0.8,
        });
    }

    // Decision point
    let decision_phrases = [
        "should we",
        "let's go with",
        "i think we should",
        "the decision is",
        "we need to decide",
        "which one",
        "option a or",
        "option b or",
        "pick between",
    ];
    for phrase in &decision_phrases {
        if text.contains(phrase) {
            return Some(SuggestionTrigger {
                kind: SuggestionTriggerKind::DecisionPoint,
                utterance_id: utterance.id.clone(),
                excerpt,
                confidence: 0.75,
            });
        }
    }

    // Disagreement / tension
    let tension_phrases = [
        "but ",
        "however",
        "i disagree",
        "that's not",
        "the problem is",
        "i'm not sure about",
        "on the other hand",
    ];
    for phrase in &tension_phrases {
        if text.contains(phrase) {
            return Some(SuggestionTrigger {
                kind: SuggestionTriggerKind::Disagreement,
                utterance_id: utterance.id.clone(),
                excerpt,
                confidence: 0.65,
            });
        }
    }

    // Assumption / hypothesis
    let assumption_phrases = [
        "i think",
        "i assume",
        "i believe",
        "probably",
        "maybe",
        "what if",
        "suppose",
    ];
    for phrase in &assumption_phrases {
        if text.contains(phrase) {
            return Some(SuggestionTrigger {
                kind: SuggestionTriggerKind::Assumption,
                utterance_id: utterance.id.clone(),
                excerpt,
                confidence: 0.6,
            });
        }
    }

    // Domain-specific signals
    let domain_phrases: &[(&str, SuggestionTriggerKind)] = &[
        ("customer", SuggestionTriggerKind::CustomerProblem),
        ("user", SuggestionTriggerKind::CustomerProblem),
        ("pain point", SuggestionTriggerKind::CustomerProblem),
        ("problem", SuggestionTriggerKind::CustomerProblem),
        ("retention", SuggestionTriggerKind::CustomerProblem),
        ("churn", SuggestionTriggerKind::CustomerProblem),
        ("validation", SuggestionTriggerKind::CustomerProblem),
        ("market", SuggestionTriggerKind::DistributionGoToMarket),
        ("distribution", SuggestionTriggerKind::DistributionGoToMarket),
        ("go to market", SuggestionTriggerKind::DistributionGoToMarket),
        ("pricing", SuggestionTriggerKind::DistributionGoToMarket),
        ("mvp", SuggestionTriggerKind::ProductScope),
        ("wedge", SuggestionTriggerKind::ProductScope),
        ("scope", SuggestionTriggerKind::ProductScope),
        ("feature", SuggestionTriggerKind::ProductScope),
        ("prioriti", SuggestionTriggerKind::Prioritization),
    ];
    for (phrase, kind) in domain_phrases {
        if text.contains(phrase) {
            return Some(SuggestionTrigger {
                kind: kind.clone(),
                utterance_id: utterance.id.clone(),
                excerpt,
                confidence: 0.55,
            });
        }
    }

    None
}

// MARK: - Stage 4: Threshold Check

// Swift: SuggestionEngine.swift > SuggestionEngine.passesThresholds(_:)
/// Checks whether a surfacing decision passes all score thresholds.
pub fn passes_thresholds(decision: &SuggestionDecision, verbosity: SuggestionVerbosity) -> bool {
    let m = verbosity.threshold_multiplier();
    decision.relevance_score >= BASE_RELEVANCE_SCORE * m
        && decision.helpfulness_score >= BASE_HELPFULNESS_SCORE * m
        && decision.timing_score >= BASE_TIMING_SCORE * m
        && decision.novelty_score >= BASE_NOVELTY_SCORE * m
        && decision.confidence >= BASE_CONFIDENCE_SCORE * m
}

// MARK: - Utility

// Swift: SuggestionEngine.swift > SuggestionEngine.extractJSON(from:)
/// Strips markdown code fences from LLM JSON responses.
pub fn extract_json(text: &str) -> String {
    let mut s = text.trim().to_string();

    if s.starts_with("```json") {
        s = s[7..].to_string();
    } else if s.starts_with("```") {
        s = s[3..].to_string();
    }
    if s.ends_with("```") {
        s = s[..s.len() - 3].to_string();
    }

    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::utterance::Speaker;

    fn make_them_utterance(text: &str) -> Utterance {
        Utterance::new(text.to_string(), Speaker::Them)
    }

    // -- should_evaluate_utterance tests --

    #[test]
    fn heuristic_rejects_short_utterance() {
        let u = make_them_utterance("yes okay");
        assert!(!should_evaluate_utterance(&u, &[], None, 45.0));
    }

    #[test]
    fn heuristic_rejects_filler_heavy() {
        // 5 out of 8 words are filler → 62.5% > 60%
        let u = make_them_utterance("yeah um like okay so basically right actually something");
        assert!(!should_evaluate_utterance(&u, &[], None, 45.0));
    }

    #[test]
    fn heuristic_rejects_near_duplicate() {
        let u = make_them_utterance("we should discuss the quarterly results for the team");
        let recent = ["we should discuss the quarterly results for the team"];
        assert!(!should_evaluate_utterance(&u, &recent, None, 45.0));
    }

    #[test]
    fn heuristic_accepts_valid_utterance() {
        let u = make_them_utterance(
            "I think we should consider switching to a different vendor for our cloud infrastructure",
        );
        assert!(should_evaluate_utterance(&u, &[], None, 45.0));
    }

    #[test]
    fn heuristic_respects_cooldown() {
        let u = make_them_utterance(
            "I think we should consider switching to a different vendor for our cloud infrastructure",
        );
        // 10 seconds elapsed, 45 second cooldown → reject
        assert!(!should_evaluate_utterance(&u, &[], Some(10.0), 45.0));
        // 50 seconds elapsed → accept
        assert!(should_evaluate_utterance(&u, &[], Some(50.0), 45.0));
    }

    // -- detect_trigger tests --

    #[test]
    fn detect_trigger_question() {
        let u = make_them_utterance("What do you think about the pricing model?");
        let trigger = detect_trigger(&u).unwrap();
        assert!(matches!(trigger.kind, SuggestionTriggerKind::ExplicitQuestion));
        assert!((trigger.confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn detect_trigger_decision_point() {
        let u = make_them_utterance("I think we should we go with option A for the launch");
        let trigger = detect_trigger(&u).unwrap();
        assert!(matches!(trigger.kind, SuggestionTriggerKind::DecisionPoint));
    }

    #[test]
    fn detect_trigger_disagreement() {
        let u = make_them_utterance("However I think that approach has some issues");
        let trigger = detect_trigger(&u).unwrap();
        assert!(matches!(trigger.kind, SuggestionTriggerKind::Disagreement));
    }

    #[test]
    fn detect_trigger_assumption() {
        let u = make_them_utterance("I believe the market is moving in a different direction");
        let trigger = detect_trigger(&u).unwrap();
        assert!(matches!(trigger.kind, SuggestionTriggerKind::Assumption));
    }

    #[test]
    fn detect_trigger_domain_signals() {
        let u = make_them_utterance("Our customer retention rate has been dropping");
        let trigger = detect_trigger(&u).unwrap();
        assert!(matches!(trigger.kind, SuggestionTriggerKind::CustomerProblem));
    }

    #[test]
    fn detect_trigger_no_match_returns_none() {
        let u = make_them_utterance("Good morning everyone, let's get started with the meeting");
        assert!(detect_trigger(&u).is_none());
    }

    // -- passes_thresholds tests --

    #[test]
    fn passes_thresholds_all_high() {
        let decision = SuggestionDecision {
            should_surface: true,
            confidence: 0.9,
            relevance_score: 0.9,
            helpfulness_score: 0.9,
            timing_score: 0.9,
            novelty_score: 0.9,
            reason: "test".to_string(),
            trigger: None,
        };
        assert!(passes_thresholds(&decision, SuggestionVerbosity::Balanced));
    }

    #[test]
    fn passes_thresholds_one_low() {
        let decision = SuggestionDecision {
            should_surface: true,
            confidence: 0.9,
            relevance_score: 0.9,
            helpfulness_score: 0.9,
            timing_score: 0.3, // too low
            novelty_score: 0.9,
            reason: "test".to_string(),
            trigger: None,
        };
        assert!(!passes_thresholds(&decision, SuggestionVerbosity::Balanced));
    }

    #[test]
    fn passes_thresholds_eager_relaxes() {
        let decision = SuggestionDecision {
            should_surface: true,
            confidence: 0.65,
            relevance_score: 0.65,
            helpfulness_score: 0.65,
            timing_score: 0.65,
            novelty_score: 0.60,
            reason: "test".to_string(),
            trigger: None,
        };
        // Balanced would reject (e.g. 0.65 < 0.72)
        assert!(!passes_thresholds(&decision, SuggestionVerbosity::Balanced));
        // Eager relaxes thresholds (0.65 >= 0.72 * 0.85 = 0.612)
        assert!(passes_thresholds(&decision, SuggestionVerbosity::Eager));
    }

    // -- extract_json tests --

    #[test]
    fn extract_json_strips_code_fences() {
        assert_eq!(
            extract_json("```json\n{\"key\": \"value\"}\n```"),
            "{\"key\": \"value\"}"
        );
    }

    #[test]
    fn extract_json_no_fences() {
        assert_eq!(extract_json("{\"key\": \"value\"}"), "{\"key\": \"value\"}");
    }

    #[test]
    fn extract_json_bare_fences() {
        assert_eq!(
            extract_json("```\n{\"key\": \"value\"}\n```"),
            "{\"key\": \"value\"}"
        );
    }
}
