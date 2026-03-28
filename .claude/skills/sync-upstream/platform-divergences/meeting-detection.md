# Meeting Detection

## What it does
Automatically detects when the user joins a meeting (so recording can start without manual intervention).

## macOS Approach
**APIs:** Core Audio HAL property listeners + `NSWorkspace.shared.runningApplications`

The Swift app uses a two-signal system:

1. **Audio signal** (`CoreAudioSignalSource`): Monitors `kAudioDevicePropertyDeviceIsRunningSomewhere` on all physical input devices. When any mic goes active, it emits `true`. This is a passive listener — it doesn't capture audio, just watches activation status.

2. **App scanning** (`MeetingDetector.scanForMeetingApp`): When the mic signal fires, it checks `NSWorkspace.shared.runningApplications` against a list of known meeting app bundle IDs (Zoom, Teams, Slack, Discord, etc.).

3. **Debounce**: Mic must stay active for 5 seconds before confirming detection (prevents false positives from brief mic access).

The result: "mic is active AND a known meeting app is running" → meeting detected.

Lives in: `Meeting/MeetingDetector.swift`

## Windows Approach
**APIs:** Win32 process enumeration + audio session monitoring

This feature **does not exist yet** in the desktop app. Here's how to implement it:

### Process detection
Use the `sysinfo` crate to enumerate running processes:
```rust
use sysinfo::System;

fn scan_for_meeting_app(known_apps: &[&str]) -> Option<MeetingApp> {
    let mut sys = System::new();
    sys.refresh_processes();
    for (pid, process) in sys.processes() {
        let name = process.name().to_lowercase();
        if known_apps.iter().any(|app| name.contains(app)) {
            return Some(MeetingApp { name: process.name().to_string(), .. });
        }
    }
    None
}
```

### Audio activity detection
Use WASAPI audio session monitoring to detect when a mic is in use:
- `IAudioSessionManager2` → enumerate active audio sessions
- `IAudioSessionControl` → monitor session state changes
- Or simpler: poll `IAudioMeterInformation` on input devices

### Known meeting apps (Windows equivalents)
| macOS Bundle ID | Windows Process Name |
|---|---|
| `us.zoom.xos` | `Zoom.exe` |
| `com.microsoft.teams2` | `ms-teams.exe` |
| `com.cisco.webexmeetingsapp` | `CiscoCollabHost.exe` |
| `com.slack.Slack` | `slack.exe` |
| `com.hnc.Discord` | `Discord.exe` |
| `net.whatsapp.WhatsApp` | `WhatsApp.exe` |

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| App identification | Bundle IDs | Process/executable names |
| App enumeration | `NSWorkspace.shared.runningApplications` | `sysinfo` crate or Win32 `EnumProcesses` |
| Mic activity | Core Audio HAL property listener (event-driven) | WASAPI session monitoring or polling |
| Detection model | Same: mic active + meeting app running | Same logic, different APIs |

## Translation Guidance

When the Swift code changes `MeetingDetector.swift`:
- **Translate** changes to the detection logic (debounce timing, detection flow, event emission) — the algorithm is the same, only the APIs differ
- **Translate** changes to the known app list — map bundle IDs to Windows process names
- **Don't translate** `CoreAudioSignalSource` directly — replace with WASAPI-based mic monitoring
- **Don't translate** `NSWorkspace` usage — replace with process enumeration
- **Translate** the `MeetingDetectionEvent` enum and the actor's public API — these are platform-independent

## Implementation Priority
Medium. The desktop app currently requires manual start/stop. This is a quality-of-life feature, not a blocker for core functionality.
