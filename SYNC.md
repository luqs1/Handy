# OpenOats Upstream Sync Contract

This document defines how to translate the upstream macOS Swift app (`OpenOats/Sources/OpenOats/`)
into the cross-platform Tauri app (`openoats-desktop/`).

## Module Mapping

### Domain Layer (Pure types, no platform deps)

| Swift Source | Rust Target | Status |
|---|---|---|
| `Domain/MeetingTypes.swift` | `src-tauri/src/domain/meeting_types.rs` | Synced |
| `Domain/MeetingState.swift` | `src-tauri/src/domain/meeting_state.rs` | Synced |
| `Domain/Utterance.swift` | `src-tauri/src/domain/utterance.rs` | Synced |
| `Domain/ExternalCommand.swift` | `src-tauri/src/domain/external_command.rs` | Synced |
| `Models/Models.swift` | `src-tauri/src/domain/models.rs` | Synced |

### Intelligence / Engines

| Swift Source | Rust Target | Status |
|---|---|---|
| `Intelligence/NotesEngine.swift` | `src-tauri/src/engines/notes.rs` | Synced |
| `Intelligence/OpenRouterClient.swift` | `src-tauri/src/llm_client.rs` (existing) | Partial |
| `Intelligence/SuggestionEngine.swift` | `src-tauri/src/engines/suggestion.rs` | TODO |
| `Intelligence/KnowledgeBase.swift` | `src-tauri/src/engines/knowledge_base.rs` | TODO |
| `Intelligence/TranscriptCleanupEngine.swift` | `src-tauri/src/engines/transcript_cleanup.rs` | TODO |
| `Intelligence/TranscriptRefinementEngine.swift` | `src-tauri/src/engines/transcript_refinement.rs` | TODO |
| `Intelligence/VoyageClient.swift` | `src-tauri/src/engines/voyage_client.rs` | TODO |
| `Intelligence/OllamaEmbedClient.swift` | `src-tauri/src/engines/ollama_embed_client.rs` | TODO |
| `Intelligence/MarkdownMeetingWriter.swift` | `src-tauri/src/engines/markdown_writer.rs` | TODO |

### Audio Layer

| Swift Source | Rust Target | Status |
|---|---|---|
| `Audio/MicCapture.swift` | `src-tauri/src/audio_toolkit/audio/recorder.rs` | Exists (cpal) |
| `Audio/SystemAudioCapture.swift` | `src-tauri/src/audio_toolkit/system_capture.rs` | Exists (cpal) |
| `Audio/AudioRecorder.swift` | `src-tauri/src/managers/audio.rs` | Exists |

### Transcription Layer

| Swift Source | Rust Target | Status |
|---|---|---|
| `Transcription/TranscriptionEngine.swift` | `src-tauri/src/managers/transcription.rs` | Exists |
| `Transcription/TranscriptionBackend.swift` | (trait in transcription.rs) | Exists |
| `Transcription/StreamingTranscriber.swift` | `src-tauri/src/transcription_coordinator.rs` | Exists |
| `Transcription/BatchTranscriptionEngine.swift` | `src-tauri/src/engines/batch_transcription.rs` | TODO |
| `Transcription/DiarizationManager.swift` | `src-tauri/src/engines/diarization.rs` | TODO |
| `Transcription/AcousticEchoFilter.swift` | `src-tauri/src/engines/echo_filter.rs` | TODO |
| `Transcription/WhisperKitBackend.swift` | (whisper-rs in transcription.rs) | Exists |
| `Transcription/ParakeetBackend.swift` | (transcribe-rs in transcription.rs) | Exists |

### App / Controllers

| Swift Source | Rust Target | Status |
|---|---|---|
| `App/AppContainer.swift` | `src-tauri/src/lib.rs` (Tauri setup) | Exists (different pattern) |
| `App/AppCoordinator.swift` | `src-tauri/src/domain/meeting_state.rs` (transition fn) | Synced |
| `App/LiveSessionController.swift` | `src-tauri/src/meeting_session.rs` | Exists |
| `App/NotesController.swift` | `src-tauri/src/commands/meeting.rs` | Partial |
| `App/MeetingDetectionController.swift` | TODO | TODO |
| `App/MenuBarController.swift` | `src-tauri/src/tray.rs` | Exists |

### Settings

| Swift Source | Rust Target | Status |
|---|---|---|
| `Settings/SettingsStore.swift` | `src-tauri/src/settings.rs` | Exists |
| `Settings/SettingsTypes.swift` | `src-tauri/src/settings.rs` | Exists |
| `Settings/SettingsStorage.swift` | `src-tauri/src/settings.rs` | Exists |

### Storage

| Swift Source | Rust Target | Status |
|---|---|---|
| `Storage/SessionRepository.swift` | `src-tauri/src/managers/history.rs` | Exists |
| `Storage/TemplateStore.swift` | `src-tauri/src/domain/models.rs` (templates) | Synced |
| `Storage/LegacySessionReader.swift` | N/A (not needed) | Skip |

### Views → React Components

| Swift Source | React Target | Status |
|---|---|---|
| `Views/ContentView.swift` | `src/App.tsx` | Exists |
| `Views/SettingsView.swift` | `src/components/settings/` | Exists |
| `Views/TranscriptView.swift` | `src/components/meeting/TranscriptPanel.tsx` | Exists |
| `Views/NotesView.swift` | `src/components/meeting/NotesPanel.tsx` | Exists |
| `Views/SuggestionsView.swift` | `src/components/meeting/SuggestionsPanel.tsx` | TODO |
| `Views/ControlBar.swift` | `src/components/meeting/MeetingControlBar.tsx` | Exists |
| `Views/OnboardingView.swift` | `src/components/onboarding/Onboarding.tsx` | Exists |
| `Views/MiniBarPanel.swift` | `src/overlay/RecordingOverlay.tsx` | Exists |
| `Views/MenuBarPopoverView.swift` | (tray menu, native) | Exists |

### Stores (SwiftUI @Observable → Zustand/Tauri events)

| Swift Source | TS Target | Status |
|---|---|---|
| `Models/TranscriptStore.swift` | `src/stores/meetingStore.ts` | Exists |

---

## Translation Rules

### Language Construct Mapping

| Swift | Rust | Notes |
|---|---|---|
| `struct Foo: Codable` | `#[derive(Serialize, Deserialize)] struct Foo` | Add `Clone, Debug` by default |
| `enum Foo: Codable` | `#[derive(Serialize, Deserialize)] enum Foo` | Use `#[serde(rename_all = "camelCase")]` |
| `enum Foo { case bar(AssocData) }` | `enum Foo { Bar(AssocData) }` | Rust enums have associated data natively |
| `protocol Foo` | `trait Foo` | |
| `let x: String?` | `x: Option<String>` | |
| `Date` | `chrono::DateTime<Utc>` or `i64` (ms) | Prefer i64 for serialization |
| `UUID` | `uuid::Uuid` or `String` | Use String for simplicity in MVP |
| `URL` | `String` or `url::Url` | |
| `[String]` | `Vec<String>` | |
| `async func foo()` | `async fn foo()` | |
| `Task { }` | `tokio::spawn()` | |
| `@Observable class` | Tauri state + events | No direct equivalent |
| `@MainActor` | (not needed in Rust) | Rust has no main actor concept |
| `actor` | `Arc<Mutex<T>>` or `tokio::sync::Mutex` | |
| `Sendable` | `Send + Sync` | |
| `Identifiable` | Custom `id` field | |
| `Hashable` | `#[derive(Hash, Eq, PartialEq)]` | |

### SwiftUI → React Mapping

| SwiftUI | React | Notes |
|---|---|---|
| `@Observable class Foo` | Zustand store | |
| `@State var x` | `useState(x)` | |
| `VStack { }` | `<div className="flex flex-col">` | |
| `HStack { }` | `<div className="flex flex-row">` | |
| `List { ForEach }` | `{items.map(item => ...)}` | |
| `NavigationStack` | React Router or conditional render | |
| `Sheet(isPresented:)` | Modal component | |
| `.onAppear { }` | `useEffect(() => {}, [])` | |
| `Text("hello")` | `<span>hello</span>` | Use i18n: `t('key')` |
| `Button("label") { action }` | `<Button onClick={action}>label</Button>` | |
| `.task { }` | `useEffect` with async | |

### Platform API Mapping

| macOS API | Windows/Cross-platform | Notes |
|---|---|---|
| AVAudioEngine (mic) | cpal | Already done |
| Core Audio (system) | cpal loopback / WASAPI | Already done |
| UserDefaults | tauri-plugin-store | Already done |
| Keychain | keytar / OS credential store | TODO |
| EventKit (calendar) | Windows Calendar API / ical | TODO |
| NSWorkspace (app detection) | Windows process enumeration | TODO |
| UserNotifications | tauri-plugin-notification | TODO |
| Sparkle (updates) | tauri-plugin-updater | Already done |

---

## Sync Workflow

1. **Identify changes**: `git log --oneline upstream/main..upstream/main~N -- OpenOats/Sources/`
2. **Group by module**: Map each changed Swift file to its Rust/React target using the table above
3. **Translate**: For each changed file, translate the diff following the rules above
4. **New files**: If a Swift file has no Rust/React target yet, create one in the mapped location
5. **Test**: `cd openoats-desktop && bun run tauri build` to verify compilation

## Upstream Sync Baseline

Last synced commit: `3f84edb` (2026-03-26)
