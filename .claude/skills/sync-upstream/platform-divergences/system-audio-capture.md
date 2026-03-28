# System Audio Capture

## What it does
Captures audio playing through the system (i.e. the other person's voice in a meeting call) separately from the microphone.

## macOS Approach
**API:** Core Audio process taps (`CATapDescription` + `AudioHardwareCreateProcessTap`)

The Swift app creates a **process tap** — a low-level Core Audio mechanism that intercepts audio destined for the output device. It then creates an **aggregate device** combining the output device with the tap, and reads audio from it via an IO proc callback.

Key characteristics:
- Can tap audio from **specific processes** (via `tapDescription.processes`)
- Requires the "System Audio Recording" privacy permission
- Produces `AVAudioPCMBuffer` at the tap's native sample rate
- Is mono mixdown (`isMono = true, isMixdown = true`)
- Lives in `Audio/SystemAudioCapture.swift`

## Windows Approach
**API:** WASAPI loopback capture (via cpal)

The Tauri app uses `cpal` which on Windows backends to WASAPI. It opens the **default output device as an input** (loopback mode), which captures everything playing through that device.

Key characteristics:
- Captures **all system audio**, not per-process — no way to isolate a single app's audio
- No special permissions needed
- Sample rate depends on the output device (typically 44100 or 48000 Hz)
- Must be downmixed to mono and resampled to 16kHz for transcription
- Lives in `audio_toolkit/system_capture.rs`

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| Granularity | Per-process tap | All system audio (loopback) |
| Permission | System Audio Recording in Privacy settings | None needed |
| API | Core Audio HAL (`AudioHardwareCreateProcessTap`) | WASAPI loopback via cpal |
| Isolation | Can exclude own app's audio | Captures everything including own playback |

## Translation Guidance

When the Swift code changes `SystemAudioCapture.swift`:
- **Ignore** changes to `CATapDescription`, aggregate device setup, process object IDs — these have no Windows equivalent
- **Translate** changes to audio buffer handling, sample rate, channel count, mono mixdown logic — these apply to both
- **Translate** changes to the stream lifecycle (start/stop/finishStream) — map to the command-based lifecycle in `system_capture.rs` (Start/Stop/Shutdown commands)
- **Watch for** new per-process filtering logic — this is a macOS-only capability. If upstream adds process-specific tapping, note it as a known limitation on Windows (we get all system audio, not per-app)

## Known Limitation
Windows loopback captures **all** system audio. If the user is playing music while in a meeting, the music gets mixed into the "them" audio stream. macOS can avoid this by tapping only the meeting app's process. There's no clean fix for this on Windows — it's a platform limitation. A possible mitigation is voice activity detection to filter out non-speech audio, which the desktop app already has via `vad-rs`.
