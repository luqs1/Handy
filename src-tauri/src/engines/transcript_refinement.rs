//! Live utterance refinement via LLM (removes filler words, fixes punctuation).
//! Bounded concurrency: max 3 in-flight refinements at a time.
//! Translated from: OpenOats/Sources/OpenOats/Intelligence/TranscriptRefinementEngine.swift

use crate::domain::utterance::Utterance;

const MINIMUM_WORD_COUNT: usize = 5;

pub const REFINEMENT_SYSTEM_PROMPT: &str = "\
Clean up this speech transcript: remove filler words (uh, um, like, you know), \
fix punctuation, add sentence breaks. Output only the cleaned text.";

/// Hardcoded cheap model for refinement (keeps cost low).
pub const REFINEMENT_MODEL: &str = "openai/gpt-4o-mini";

// Swift: TranscriptRefinementEngine.swift > TranscriptRefinementEngine.refine(_:)
/// Determines whether an utterance should be refined.
/// Skips short utterances (below 5 words) unless they contain a question mark.
pub fn should_refine(utterance: &Utterance) -> bool {
    let word_count = utterance.text.split_whitespace().count();
    word_count >= MINIMUM_WORD_COUNT || utterance.text.contains('?')
}

// Swift: TranscriptRefinementEngine.swift > TranscriptRefinementEngine.performRefinement(_:)
/// Validates the LLM response: non-empty after trimming.
pub fn validate_refinement(response: &str) -> Option<String> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::utterance::Speaker;

    fn make_utterance(text: &str) -> Utterance {
        Utterance::new(text.to_string(), Speaker::Them)
    }

    #[test]
    fn skip_short_utterance() {
        // "yes okay" = 2 words, no question mark
        assert!(!should_refine(&make_utterance("yes okay")));
    }

    #[test]
    fn skip_single_word() {
        assert!(!should_refine(&make_utterance("hello")));
    }

    #[test]
    fn does_not_skip_question() {
        // Short but contains "?" → should refine
        assert!(should_refine(&make_utterance("really?")));
    }

    #[test]
    fn accepts_long_utterance() {
        assert!(should_refine(&make_utterance(
            "I think we should um discuss the quarterly results"
        )));
    }

    #[test]
    fn validate_empty_response() {
        assert!(validate_refinement("").is_none());
        assert!(validate_refinement("   ").is_none());
    }

    #[test]
    fn validate_trims_response() {
        assert_eq!(
            validate_refinement("  Hello there.  "),
            Some("Hello there.".to_string())
        );
    }
}
