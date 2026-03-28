//! File-based meeting template CRUD with built-in defaults.
//! Translated from: OpenOats/Sources/OpenOats/Storage/TemplateStore.swift

use crate::domain::models::{MeetingTemplate, TemplateSnapshot};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// Deterministic built-in IDs (must match Swift for cross-platform session compat)
const GENERIC_ID: &str = "00000000-0000-0000-0000-000000000000";
const ONE_ON_ONE_ID: &str = "00000000-0000-0000-0000-000000000001";
const DISCOVERY_ID: &str = "00000000-0000-0000-0000-000000000002";
const HIRING_ID: &str = "00000000-0000-0000-0000-000000000003";
const STAND_UP_ID: &str = "00000000-0000-0000-0000-000000000004";
const WEEKLY_ID: &str = "00000000-0000-0000-0000-000000000005";

#[derive(Serialize, Deserialize)]
struct StorageFormat {
    version: i32,
    templates: Vec<MeetingTemplate>,
}

pub struct TemplateStore {
    templates: Vec<MeetingTemplate>,
    storage_path: PathBuf,
    version: i32,
}

impl TemplateStore {
    /// Create a new TemplateStore. If `root_dir` is None, uses a default in-memory state
    /// (no persistence). Pass a directory path for file-backed storage.
    // Swift: TemplateStore.swift > TemplateStore.init(rootDirectory:)
    pub fn new(root_dir: Option<PathBuf>) -> Self {
        let storage_path = match root_dir {
            Some(dir) => {
                let _ = fs::create_dir_all(&dir);
                dir.join("templates.json")
            }
            None => PathBuf::from("templates.json"), // won't persist without a real dir
        };

        let mut store = Self {
            templates: Vec::new(),
            storage_path,
            version: 1,
        };
        store.load();
        store
    }

    pub fn templates(&self) -> &[MeetingTemplate] {
        &self.templates
    }

    // Swift: TemplateStore.swift > TemplateStore.add(_:)
    pub fn add(&mut self, template: MeetingTemplate) {
        self.templates.push(template);
        self.save();
    }

    // Swift: TemplateStore.swift > TemplateStore.update(_:)
    pub fn update(&mut self, template: MeetingTemplate) {
        if let Some(idx) = self.templates.iter().position(|t| t.id == template.id) {
            self.templates[idx] = template;
            self.save();
        }
    }

    // Swift: TemplateStore.swift > TemplateStore.delete(id:)
    pub fn delete(&mut self, id: &str) {
        if let Some(idx) = self.templates.iter().position(|t| t.id == id) {
            if self.templates[idx].is_built_in {
                return; // cannot delete built-in templates
            }
            self.templates.remove(idx);
            self.save();
        }
    }

    // Swift: TemplateStore.swift > TemplateStore.resetBuiltIn(id:)
    pub fn reset_built_in(&mut self, id: &str) {
        let built_in = built_in_templates().into_iter().find(|t| t.id == id);
        if let Some(original) = built_in {
            if let Some(idx) = self.templates.iter().position(|t| t.id == id) {
                self.templates[idx] = original;
                self.save();
            }
        }
    }

    // Swift: TemplateStore.swift > TemplateStore.template(for:)
    pub fn template_for(&self, id: &str) -> Option<&MeetingTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    // Swift: TemplateStore.swift > TemplateStore.snapshot(of:)
    pub fn snapshot(template: &MeetingTemplate) -> TemplateSnapshot {
        TemplateSnapshot {
            id: template.id.clone(),
            name: template.name.clone(),
            icon: template.icon.clone(),
            system_prompt: template.system_prompt.clone(),
        }
    }

    // Swift: TemplateStore.swift > TemplateStore.load()
    fn load(&mut self) {
        if !self.storage_path.exists() {
            self.templates = built_in_templates();
            self.save();
            return;
        }

        match fs::read_to_string(&self.storage_path) {
            Ok(data) => match serde_json::from_str::<StorageFormat>(&data) {
                Ok(stored) => {
                    self.version = stored.version;
                    self.templates = stored.templates;

                    // Ensure all built-ins exist (handles upgrades adding new built-ins)
                    for built_in in built_in_templates() {
                        if !self.templates.iter().any(|t| t.id == built_in.id) {
                            self.templates.push(built_in);
                        }
                    }
                }
                Err(e) => {
                    warn!("TemplateStore: failed to parse, using defaults: {}", e);
                    self.templates = built_in_templates();
                }
            },
            Err(e) => {
                warn!("TemplateStore: failed to read, using defaults: {}", e);
                self.templates = built_in_templates();
            }
        }
        self.save();
    }

    // Swift: TemplateStore.swift > TemplateStore.save()
    fn save(&self) {
        let stored = StorageFormat {
            version: self.version,
            templates: self.templates.clone(),
        };
        if let Ok(data) = serde_json::to_string_pretty(&stored) {
            let _ = fs::write(&self.storage_path, data);
        }
    }
}

// Swift: TemplateStore.swift > TemplateStore.builtInTemplates (static property)
pub fn built_in_templates() -> Vec<MeetingTemplate> {
    vec![
        MeetingTemplate {
            id: GENERIC_ID.to_string(),
            name: "Generic".to_string(),
            icon: "doc.text".to_string(),
            system_prompt: "You are a meeting notes assistant. Given a transcript of a meeting, produce structured notes in markdown.\n\nInclude these sections:\n## Summary\nA 2-3 sentence overview of what was discussed.\n\n## Key Points\nBullet points of the most important topics and insights.\n\n## Action Items\nBullet points of concrete next steps, with owners if mentioned.\n\n## Decisions Made\nAny decisions that were reached during the meeting.\n\n## Open Questions\nUnresolved questions or topics that need follow-up.".to_string(),
            is_built_in: true,
        },
        MeetingTemplate {
            id: ONE_ON_ONE_ID.to_string(),
            name: "1:1".to_string(),
            icon: "person.2".to_string(),
            system_prompt: "You are a meeting notes assistant for a 1:1 meeting. Given a transcript, produce structured notes in markdown.\n\nInclude these sections:\n## Discussion Points\nKey topics that were covered.\n\n## Action Items\nConcrete next steps with owners.\n\n## Follow-ups\nItems that need follow-up in future 1:1s.\n\n## Key Decisions\nDecisions that were made during the meeting.".to_string(),
            is_built_in: true,
        },
        MeetingTemplate {
            id: DISCOVERY_ID.to_string(),
            name: "Customer Discovery".to_string(),
            icon: "magnifyingglass".to_string(),
            system_prompt: "You are a meeting notes assistant for a customer discovery call. Given a transcript, produce structured notes in markdown.\n\nInclude these sections:\n## Customer Profile\nWho the customer is, their role, and context.\n\n## Problems Identified\nPain points and challenges the customer described.\n\n## Current Solutions\nHow they currently solve these problems.\n\n## Key Insights\nSurprising or important learnings from the conversation.\n\n## Next Steps\nFollow-up actions and commitments made.".to_string(),
            is_built_in: true,
        },
        MeetingTemplate {
            id: HIRING_ID.to_string(),
            name: "Hiring".to_string(),
            icon: "person.badge.plus".to_string(),
            system_prompt: "You are a meeting notes assistant for a hiring interview. Given a transcript, produce structured notes in markdown.\n\nInclude these sections:\n## Candidate Summary\nBrief overview of the candidate and role discussed.\n\n## Strengths\nAreas where the candidate demonstrated strong capability.\n\n## Concerns\nPotential red flags or areas needing further evaluation.\n\n## Culture Fit\nObservations about alignment with team/company values.\n\n## Recommendation\nOverall assessment and suggested next steps.".to_string(),
            is_built_in: true,
        },
        MeetingTemplate {
            id: STAND_UP_ID.to_string(),
            name: "Stand-Up".to_string(),
            icon: "arrow.up.circle".to_string(),
            system_prompt: "You are a meeting notes assistant for a stand-up meeting. Given a transcript, produce structured notes in markdown.\n\nInclude these sections:\n## Yesterday\nWhat was completed since the last stand-up.\n\n## Today\nWhat each person plans to work on.\n\n## Blockers\nAny obstacles or dependencies that need resolution.".to_string(),
            is_built_in: true,
        },
        MeetingTemplate {
            id: WEEKLY_ID.to_string(),
            name: "Weekly Meeting".to_string(),
            icon: "calendar".to_string(),
            system_prompt: "You are a meeting notes assistant for a weekly team meeting. Given a transcript, produce structured notes in markdown.\n\nInclude these sections:\n## Updates\nStatus updates from team members.\n\n## Decisions Made\nAny decisions that were reached.\n\n## Open Items\nTopics that need further discussion or action.\n\n## Action Items\nConcrete next steps with owners and deadlines if mentioned.".to_string(),
            is_built_in: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> (TemplateStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = TemplateStore::new(Some(dir.path().to_path_buf()));
        (store, dir)
    }

    #[test]
    fn new_creates_defaults() {
        let (store, _dir) = make_store();
        assert_eq!(store.templates().len(), 6);
        assert!(store.templates().iter().all(|t| t.is_built_in));
    }

    #[test]
    fn add_custom_template() {
        let (mut store, _dir) = make_store();
        let custom = MeetingTemplate {
            id: "custom-1".to_string(),
            name: "My Template".to_string(),
            icon: "star".to_string(),
            system_prompt: "Custom prompt".to_string(),
            is_built_in: false,
        };
        store.add(custom);
        assert_eq!(store.templates().len(), 7);
        assert!(store.template_for("custom-1").is_some());
    }

    #[test]
    fn update_template() {
        let (mut store, _dir) = make_store();
        let mut generic = store.template_for(GENERIC_ID).unwrap().clone();
        generic.system_prompt = "Updated prompt".to_string();
        store.update(generic);
        assert_eq!(
            store.template_for(GENERIC_ID).unwrap().system_prompt,
            "Updated prompt"
        );
    }

    #[test]
    fn delete_custom_template() {
        let (mut store, _dir) = make_store();
        let custom = MeetingTemplate {
            id: "custom-1".to_string(),
            name: "Deletable".to_string(),
            icon: "trash".to_string(),
            system_prompt: "test".to_string(),
            is_built_in: false,
        };
        store.add(custom);
        assert_eq!(store.templates().len(), 7);

        store.delete("custom-1");
        assert_eq!(store.templates().len(), 6);
        assert!(store.template_for("custom-1").is_none());
    }

    #[test]
    fn delete_built_in_rejected() {
        let (mut store, _dir) = make_store();
        store.delete(GENERIC_ID);
        // Built-in should still be there
        assert!(store.template_for(GENERIC_ID).is_some());
        assert_eq!(store.templates().len(), 6);
    }

    #[test]
    fn reset_built_in() {
        let (mut store, _dir) = make_store();
        let original_prompt = store.template_for(GENERIC_ID).unwrap().system_prompt.clone();

        // Modify it
        let mut modified = store.template_for(GENERIC_ID).unwrap().clone();
        modified.system_prompt = "Hacked prompt".to_string();
        store.update(modified);
        assert_eq!(
            store.template_for(GENERIC_ID).unwrap().system_prompt,
            "Hacked prompt"
        );

        // Reset
        store.reset_built_in(GENERIC_ID);
        assert_eq!(
            store.template_for(GENERIC_ID).unwrap().system_prompt,
            original_prompt
        );
    }

    #[test]
    fn persists_across_reloads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        // Create store and add a custom template
        {
            let mut store = TemplateStore::new(Some(path.clone()));
            store.add(MeetingTemplate {
                id: "persist-test".to_string(),
                name: "Persistent".to_string(),
                icon: "pin".to_string(),
                system_prompt: "test".to_string(),
                is_built_in: false,
            });
            assert_eq!(store.templates().len(), 7);
        }

        // Load a fresh store from the same directory
        let store2 = TemplateStore::new(Some(path));
        assert_eq!(store2.templates().len(), 7);
        assert!(store2.template_for("persist-test").is_some());
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        // Write garbage to templates.json
        fs::write(path.join("templates.json"), "not valid json {{{").unwrap();

        // Load should fall back to defaults
        let store = TemplateStore::new(Some(path));
        assert_eq!(store.templates().len(), 6);
        assert!(store.templates().iter().all(|t| t.is_built_in));
    }
}
