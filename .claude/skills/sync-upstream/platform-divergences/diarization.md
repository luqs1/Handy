# Speaker Diarization

## What it does
Identifies which speaker said what in the system audio stream. Without diarization, all system audio is labeled "Them". With it, you get "Speaker 1", "Speaker 2", etc.

## macOS Approach
**Library:** FluidAudio (`LSEENDDiarizer`)

The Swift app uses FluidAudio's LS-EEND (End-to-End Neural Diarization) model:

1. Audio samples (16kHz mono f32) are fed to the diarizer in real-time via `addAudio()` + `process()`
2. The diarizer maintains a timeline of speaker segments (finalized + tentative)
3. When a transcription segment comes in, `dominantSpeaker(from:to:)` queries which speaker had the most overlap in that time range
4. If multiple speakers are detected: returns `Speaker.remote(N)` where N is 1-indexed
5. If only one speaker detected: falls back to `Speaker.them` for backward compat

Supports variants: DIHARD3, AMI, CALLHOME (different training sets for different meeting types).

Lives in: `Transcription/DiarizationManager.swift`

## Windows Approach
**Library:** Does not exist yet. Options:

### Option 1: ONNX-based diarization (recommended)
- Export a pyannote/speaker-diarization model to ONNX
- Run via `ort` crate (ONNX Runtime for Rust)
- The desktop app already depends on `transcribe-rs` which uses ONNX

### Option 2: WebRTC VAD + clustering
- Use voice activity detection (already have `vad-rs`) to segment audio
- Extract speaker embeddings per segment (e.g., ECAPA-TDNN via ONNX)
- Cluster embeddings to identify distinct speakers

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| Model | LS-EEND via FluidAudio (Metal-accelerated) | ONNX diarization model (CPU/DirectML) |
| Library | FluidAudio (macOS-only, Apple Silicon) | `ort` crate (cross-platform ONNX runtime) |
| Streaming | Real-time feed + query timeline | Likely batch-per-segment |

## Translation Guidance

When the Swift code changes `DiarizationManager.swift`:
- **Translate** the public API surface: `load()`, `feedAudio()`, `dominantSpeaker(from:to:)`, `finalize()`, `reset()` — keep the same trait/interface
- **Translate** the speaker mapping logic (diarizer index to `Speaker::Remote(n)`, single-speaker fallback to `Speaker::Them`)
- **Don't translate** FluidAudio-specific calls (`LSEENDDiarizer`, `LSEENDVariant`, `DiarizerTimeline`) — replace with ONNX equivalent
- **Translate** changes to how diarization integrates with the transcription pipeline — the coordination logic is platform-independent

## Implementation Priority
High for meeting quality, but the app works without it (everything labeled "Them"). Can be stubbed initially with the existing binary You/Them speaker model.
