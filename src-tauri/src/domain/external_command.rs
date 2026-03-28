//! External commands issued to the app from outside (deep link, tray, global hotkey).
//! Translated from: OpenOats/Sources/OpenOats/Domain/ExternalCommand.swift

use serde::{Deserialize, Serialize};

// MARK: - External Command

/// A command issued to the app from outside (deep link, tray, global hotkey).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ExternalCommand {
    StartSession,
    StopSession,
    OpenNotes { session_id: Option<String> },
}

/// A pending external command with a stable identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCommandRequest {
    pub id: String,
    pub command: ExternalCommand,
}

impl ExternalCommandRequest {
    pub fn new(command: ExternalCommand) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            command,
        }
    }
}
