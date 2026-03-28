//! Meeting lifecycle state machine.
//! Translated from: OpenOats/Sources/OpenOats/Domain/MeetingState.swift

use super::meeting_types::MeetingMetadata;
use serde::{Deserialize, Serialize};

// MARK: - Meeting State

/// The lifecycle state of a meeting recording session.
/// Designed as a pure value type for testability.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "state", content = "metadata")]
pub enum MeetingState {
    /// No active session. The system is waiting.
    Idle,
    /// A session is actively recording.
    Recording(MeetingMetadata),
    /// Recording has stopped; the session is being finalized.
    Ending(MeetingMetadata),
}

// MARK: - Meeting Event

/// Events that drive state transitions in the meeting lifecycle.
#[derive(Clone, Debug)]
pub enum MeetingEvent {
    /// The user pressed Start.
    UserStarted(MeetingMetadata),
    /// The user pressed Stop.
    UserStopped,
    /// The user discarded the current session.
    UserDiscarded,
    /// Finalization completed.
    FinalizationComplete,
    /// Finalization timed out.
    FinalizationTimeout,
}

// MARK: - Pure Transition Function

// Swift: MeetingState.swift > transition(from:on:)
/// Pure function: given a state and event, returns the next state.
/// No side effects. All side effects are dispatched by the coordinator after transition.
pub fn transition(state: &MeetingState, event: &MeetingEvent) -> MeetingState {
    match (state, event) {
        // idle + userStarted -> recording
        (MeetingState::Idle, MeetingEvent::UserStarted(metadata)) => {
            MeetingState::Recording(metadata.clone())
        }

        // recording + userStopped -> ending
        (MeetingState::Recording(metadata), MeetingEvent::UserStopped) => {
            MeetingState::Ending(metadata.clone())
        }

        // recording + userDiscarded -> idle
        (MeetingState::Recording(_), MeetingEvent::UserDiscarded) => MeetingState::Idle,

        // ending + finalizationComplete -> idle
        (MeetingState::Ending(_), MeetingEvent::FinalizationComplete) => MeetingState::Idle,

        // ending + finalizationTimeout -> idle
        (MeetingState::Ending(_), MeetingEvent::FinalizationTimeout) => MeetingState::Idle,

        // All other combinations are no-ops
        _ => state.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_to_recording() {
        let metadata = MeetingMetadata::manual();
        let state = transition(
            &MeetingState::Idle,
            &MeetingEvent::UserStarted(metadata.clone()),
        );
        assert!(matches!(state, MeetingState::Recording(_)));
    }

    #[test]
    fn test_recording_to_ending() {
        let metadata = MeetingMetadata::manual();
        let state = transition(
            &MeetingState::Recording(metadata),
            &MeetingEvent::UserStopped,
        );
        assert!(matches!(state, MeetingState::Ending(_)));
    }

    #[test]
    fn test_ending_to_idle() {
        let metadata = MeetingMetadata::manual();
        let state = transition(
            &MeetingState::Ending(metadata),
            &MeetingEvent::FinalizationComplete,
        );
        assert!(matches!(state, MeetingState::Idle));
    }

    #[test]
    fn test_noop_double_start() {
        let metadata = MeetingMetadata::manual();
        let recording = MeetingState::Recording(metadata.clone());
        let state = transition(&recording, &MeetingEvent::UserStarted(metadata));
        assert!(matches!(state, MeetingState::Recording(_)));
    }
}
