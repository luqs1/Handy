//! Text normalization and similarity utilities.
//! Translated from: OpenOats/Sources/OpenOats/Models/TextSimilarity.swift

use std::collections::HashSet;

// Swift: TextSimilarity.swift > TextSimilarity.normalizedWords(in:)
/// Splits text into lowercase alphanumeric words.
pub fn normalized_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

// Swift: TextSimilarity.swift > TextSimilarity.normalizedText(_:)
/// Returns a normalized string: lowercase, only alphanumeric words, single-space separated.
pub fn normalized_text(text: &str) -> String {
    normalized_words(text).join(" ")
}

// Swift: TextSimilarity.swift > TextSimilarity.jaccard(_:_:)
/// Jaccard similarity between two strings (based on word sets).
/// Returns 1.0 for two empty strings (matching Swift behavior).
pub fn jaccard(a: &str, b: &str) -> f64 {
    let set_a: HashSet<String> = normalized_words(a).into_iter().collect();
    let set_b: HashSet<String> = normalized_words(b).into_iter().collect();

    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_identical_strings() {
        assert!((jaccard("hello world", "hello world") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_disjoint_strings() {
        assert!((jaccard("hello world", "foo bar")).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_partial_overlap() {
        // {"hello", "world"} ∩ {"hello", "foo"} = {"hello"} → 1/3
        let sim = jaccard("hello world", "hello foo");
        assert!((sim - 1.0 / 3.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_empty_strings() {
        assert!((jaccard("", "") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normalized_words_strips_punctuation() {
        let words = normalized_words("Hello, World! How's it going?");
        assert_eq!(words, vec!["hello", "world", "how", "s", "it", "going"]);
    }

    #[test]
    fn normalized_text_joins_with_spaces() {
        assert_eq!(normalized_text("Hello,  World!!"), "hello world");
    }
}
