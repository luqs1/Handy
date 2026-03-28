# macOS-Only Features (No Windows Equivalent)

## Features to skip entirely during translation

### Apple Intelligence
**Swift:** `apple_intelligence.swift` — Detects Apple Silicon + macOS 15.1 for on-device LLM via Apple's foundation models.
**Windows:** No equivalent. The desktop app already stubs this out (`apple_intelligence.rs` returns false on non-Apple platforms).
**Action:** Ignore all changes to Apple Intelligence code. It's already handled.

### Sparkle (Auto-Updates)
**Swift:** Uses the Sparkle framework for checking/downloading/installing updates via appcast XML feeds.
**Windows:** Tauri has `tauri-plugin-updater` which serves the same purpose with a different mechanism (GitHub releases / custom update server).
**Action:** Don't translate Sparkle code. Changes to `AppUpdaterController.swift` should be ignored — the Tauri updater plugin handles this independently.

### LaunchAtLogin
**Swift:** Uses `LaunchAtLogin-Modern` to register as a login item.
**Windows:** Tauri has `tauri-plugin-autostart` which handles this cross-platform.
**Action:** Don't translate. Already handled.

### Menu Bar App (NSStatusItem)
**Swift:** `MenuBarController.swift` + `MenuBarPopoverView.swift` — Creates a menu bar icon with a popover.
**Windows:** Tauri's system tray (`tray.rs`) serves the same purpose. Already implemented.
**Action:** Translate changes to tray **behavior** (what actions are available, what state is shown), but don't translate AppKit-specific tray code.

### MiniBarPanel (NSPanel floating window)
**Swift:** Uses `NSPanel` with specific window levels for an always-on-top compact recording indicator.
**Windows:** The desktop app uses `tauri-nspanel` on macOS and a regular Tauri window with always-on-top on Windows (`overlay.rs`).
**Action:** Translate changes to the panel's **content and behavior**, not the window management code.

### Screen Sharing Detection
**Swift:** Uses `CGWindowListCopyWindowInfo` to detect if the user is sharing their screen (to hide sensitive UI).
**Windows:** Could use `EnumWindows` + `DwmGetWindowAttribute` but this is very low priority.
**Action:** Skip for now. Note as TODO if upstream adds significant screen-sharing-aware features.

## General Rule
If a Swift file imports `AppKit`, `Cocoa`, `CoreServices`, or `Security` — it's likely macOS-only UI or system integration. Check this list before attempting translation. The business logic inside those files may still be translatable even if the platform calls aren't.
