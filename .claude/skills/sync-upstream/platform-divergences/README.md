# Platform Divergences

These files document where the macOS Swift app and Windows Tauri app **cannot** be translated 1:1 — places where the architecture or APIs are fundamentally different.

When translating upstream Swift changes, check if the changed file touches any of these areas. If it does, read the relevant divergence doc before translating.

## Index

| File | Covers | Swift files affected |
|---|---|---|
| [system-audio-capture.md](system-audio-capture.md) | Core Audio process taps vs WASAPI loopback | `Audio/SystemAudioCapture.swift` |
| [meeting-detection.md](meeting-detection.md) | NSWorkspace app scanning vs Win32 process enum | `Meeting/MeetingDetector.swift`, `App/MeetingDetectionController.swift` |
| [diarization.md](diarization.md) | FluidAudio LS-EEND vs ONNX-based alternatives | `Transcription/DiarizationManager.swift` |
| [secure-storage.md](secure-storage.md) | Keychain vs Windows Credential Manager | `Settings/SettingsStorage.swift` |
| [calendar-integration.md](calendar-integration.md) | EventKit vs CalDAV/ICS/Graph API | `Domain/MeetingTypes.swift` (CalendarEvent type) |
| [notifications.md](notifications.md) | UserNotifications vs Toast/Tauri plugin | `Meeting/NotificationService.swift` |
| [macos-only-features.md](macos-only-features.md) | Features that don't translate at all | Apple Intelligence, Sparkle, NSPanel, etc. |
| [ml-inference.md](ml-inference.md) | Metal/CoreML vs ONNX/whisper-cpp | `Transcription/*.swift` backends |

## Quick Decision Tree

When you encounter a changed Swift file:

1. Does it import `AppKit`, `Cocoa`, `CoreAudio`, `AVFoundation`, `EventKit`, `Security`, or `FluidAudio`?
   - **Yes** → Check the relevant divergence doc above
   - **No** → It's likely pure business logic. Translate directly.

2. Is the file listed in `macos-only-features.md`?
   - **Yes** → Skip translation entirely
   - **No** → Continue

3. Does the divergence doc say "Translate the public API / business logic"?
   - **Yes** → Translate the logic, swap the platform calls
   - **No** → Create a stub with `todo!()` and note in SYNC.md
