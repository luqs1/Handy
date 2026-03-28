//! In-memory transcript store for live meeting sessions.
//! Translated from: OpenOats/Sources/OpenOats/Models/TranscriptStore.swift

use crate::domain::utterance::{ConversationState, RefinementStatus, Speaker, Utterance};
use crate::stores::text_similarity;

const ACOUSTIC_ECHO_WINDOW_MS: i64 = 1750; // 1.75 seconds
const ACOUSTIC_ECHO_SIMILARITY_THRESHOLD: f64 = 0.78;
const ACOUSTIC_ECHO_MIN_WORD_COUNT: usize = 4;
const ACOUSTIC_ECHO_MIN_CHAR_COUNT: usize = 20;

pub struct TranscriptStore {
    utterances: Vec<Utterance>,
    conversation_state: ConversationState,
    pub volatile_you_text: String,
    pub volatile_them_text: String,
    remote_utterances_since_state_update: usize,
}

impl TranscriptStore {
    pub fn new() -> Self {
        Self {
            utterances: Vec::new(),
            conversation_state: ConversationState::default(),
            volatile_you_text: String::new(),
            volatile_them_text: String::new(),
            remote_utterances_since_state_update: 0,
        }
    }

    // Swift: TranscriptStore.swift > TranscriptStore.append(_:)
    /// Append an utterance, returning false if it was suppressed as acoustic echo.
    pub fn append(&mut self, utterance: Utterance) -> bool {
        if self.should_suppress_acoustic_echo(&utterance) {
            return false;
        }
        if utterance.speaker.is_remote() {
            self.remote_utterances_since_state_update += 1;
        }
        self.utterances.push(utterance);
        true
    }

    // Swift: TranscriptStore.swift > TranscriptStore.updateRefinedText(id:refinedText:status:)
    /// Update an existing utterance's refined text by ID.
    pub fn update_refined_text(&mut self, id: &str, refined_text: Option<String>, status: RefinementStatus) {
        if let Some(idx) = self.utterances.iter().position(|u| u.id == id) {
            let updated = self.utterances[idx].with_refinement(refined_text, status);
            self.utterances[idx] = updated;
        }
    }

    // Swift: TranscriptStore.swift > TranscriptStore.clear()
    pub fn clear(&mut self) {
        self.utterances.clear();
        self.volatile_you_text.clear();
        self.volatile_them_text.clear();
        self.conversation_state = ConversationState::default();
        self.remote_utterances_since_state_update = 0;
    }

    // Swift: TranscriptStore.swift > TranscriptStore.updateConversationState(_:)
    pub fn update_conversation_state(&mut self, state: ConversationState) {
        self.conversation_state = state;
        self.remote_utterances_since_state_update = 0;
    }

    // Swift: TranscriptStore.swift > TranscriptStore.needsStateUpdate
    /// Whether conversation state needs a refresh (every 2+ finalized remote utterances).
    pub fn needs_state_update(&self) -> bool {
        self.remote_utterances_since_state_update >= 2
    }

    pub fn utterances(&self) -> &[Utterance] {
        &self.utterances
    }

    pub fn conversation_state(&self) -> &ConversationState {
        &self.conversation_state
    }

    pub fn last_remote_utterance(&self) -> Option<&Utterance> {
        self.utterances.iter().rev().find(|u| u.speaker.is_remote())
    }

    /// Last N utterances for prompt context.
    pub fn recent_utterances(&self, n: usize) -> &[Utterance] {
        let start = self.utterances.len().saturating_sub(n);
        &self.utterances[start..]
    }

    /// Recent 6 utterances for gate/generation prompts.
    pub fn recent_exchange(&self) -> &[Utterance] {
        self.recent_utterances(6)
    }

    /// Recent remote-only utterances for trigger analysis.
    pub fn recent_remote_utterances(&self) -> Vec<&Utterance> {
        self.utterances
            .iter()
            .rev()
            .take(10)
            .filter(|u| u.speaker.is_remote())
            .collect()
    }

    // Swift: TranscriptStore.swift > TranscriptStore.shouldSuppressAcousticEcho(_:)
    fn should_suppress_acoustic_echo(&self, utterance: &Utterance) -> bool {
        if utterance.speaker != Speaker::You {
            return false;
        }

        let normalized_you = text_similarity::normalized_text(&utterance.text);
        if !is_eligible_for_echo_check(&normalized_you) {
            return false;
        }

        for candidate in self.utterances.iter().rev() {
            if !candidate.speaker.is_remote() {
                continue;
            }

            let time_delta = utterance.timestamp - candidate.timestamp;
            if time_delta < 0 {
                continue;
            }
            if time_delta > ACOUSTIC_ECHO_WINDOW_MS {
                break;
            }

            let normalized_them = text_similarity::normalized_text(&candidate.text);
            if !is_eligible_for_echo_check(&normalized_them) {
                continue;
            }

            let similarity = text_similarity::jaccard(&normalized_you, &normalized_them);
            let contains_other = normalized_you.contains(&normalized_them)
                || normalized_them.contains(&normalized_you);

            if similarity >= ACOUSTIC_ECHO_SIMILARITY_THRESHOLD || contains_other {
                return true;
            }
        }

        false
    }
}

fn is_eligible_for_echo_check(normalized: &str) -> bool {
    let word_count = normalized.split_whitespace().count();
    word_count >= ACOUSTIC_ECHO_MIN_WORD_COUNT || normalized.len() >= ACOUSTIC_ECHO_MIN_CHAR_COUNT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_utterance(text: &str, speaker: Speaker, timestamp: i64) -> Utterance {
        Utterance {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
            speaker,
            timestamp,
            refined_text: None,
            refinement_status: None,
        }
    }

    #[test]
    fn append_and_retrieve() {
        let mut store = TranscriptStore::new();
        let u = make_utterance("hello", Speaker::You, 1000);
        assert!(store.append(u));
        assert_eq!(store.utterances().len(), 1);
        assert_eq!(store.utterances()[0].text, "hello");
    }

    #[test]
    fn echo_suppression_drops_duplicate() {
        let mut store = TranscriptStore::new();
        // "Them" says something
        let them = make_utterance("we should discuss the quarterly results", Speaker::Them, 1000);
        store.append(them);

        // "You" says nearly the same thing 1 second later (acoustic echo)
        let you = make_utterance("we should discuss the quarterly results", Speaker::You, 2000);
        assert!(!store.append(you)); // should be suppressed
        assert_eq!(store.utterances().len(), 1); // only "them" utterance
    }

    #[test]
    fn echo_suppression_allows_dissimilar() {
        let mut store = TranscriptStore::new();
        let them = make_utterance("we should discuss the quarterly results", Speaker::Them, 1000);
        store.append(them);

        let you = make_utterance("I agree, let me pull up the dashboard now", Speaker::You, 2000);
        assert!(store.append(you)); // different content, allowed
        assert_eq!(store.utterances().len(), 2);
    }

    #[test]
    fn echo_suppression_allows_outside_window() {
        let mut store = TranscriptStore::new();
        let them = make_utterance("we should discuss the quarterly results", Speaker::Them, 1000);
        store.append(them);

        // Same text but 3 seconds later (outside 1.75s window)
        let you = make_utterance("we should discuss the quarterly results", Speaker::You, 4000);
        assert!(store.append(you)); // outside window, allowed
        assert_eq!(store.utterances().len(), 2);
    }

    #[test]
    fn echo_suppression_skips_short_utterances() {
        let mut store = TranscriptStore::new();
        let them = make_utterance("yes", Speaker::Them, 1000);
        store.append(them);

        // Short utterance — not eligible for echo check
        let you = make_utterance("yes", Speaker::You, 2000);
        assert!(store.append(you)); // too short to echo-check
        assert_eq!(store.utterances().len(), 2);
    }

    #[test]
    fn needs_state_update_after_remote_utterances() {
        let mut store = TranscriptStore::new();
        assert!(!store.needs_state_update());

        store.append(make_utterance("first point", Speaker::Them, 1000));
        assert!(!store.needs_state_update()); // only 1

        store.append(make_utterance("second point", Speaker::Them, 2000));
        assert!(store.needs_state_update()); // 2 remote utterances

        store.update_conversation_state(ConversationState::default());
        assert!(!store.needs_state_update()); // reset
    }

    #[test]
    fn clear_resets_everything() {
        let mut store = TranscriptStore::new();
        store.append(make_utterance("hello", Speaker::You, 1000));
        store.append(make_utterance("hi", Speaker::Them, 2000));
        store.volatile_you_text = "partial...".to_string();

        store.clear();
        assert!(store.utterances().is_empty());
        assert!(store.volatile_you_text.is_empty());
        assert!(!store.needs_state_update());
    }
}
