# ML Model Inference (Transcription, Diarization, Embeddings)

## What it does
Runs machine learning models locally for speech-to-text, speaker diarization, and text embeddings.

## macOS Approach
**Frameworks:** Metal (GPU), Core ML, Accelerate

The Swift app uses:
- **WhisperKit** — Whisper models optimized for Apple Silicon via Core ML / Metal
- **FluidAudio** — LS-EEND diarization via Metal-accelerated inference
- **Parakeet / Qwen3** — Additional ASR backends, also Metal-optimized

All models run on Apple Silicon's Neural Engine or GPU. No CPU fallback needed — every Mac since 2020 has Apple Silicon.

Lives in: `Transcription/WhisperKitBackend.swift`, `ParakeetBackend.swift`, `Qwen3Backend.swift`, `DiarizationManager.swift`

## Windows Approach
**Frameworks:** ONNX Runtime (CPU / DirectML / CUDA)

The desktop app uses:
- **transcribe-rs** — Wraps Whisper (via whisper-cpp) and Parakeet (via ONNX Runtime)
- **whisper-rs** — Direct whisper.cpp bindings (Windows-specific dep in Cargo.toml)
- **vad-rs** — Silero VAD model via ONNX Runtime

Acceleration options:
- **DirectML** — GPU acceleration on any DirectX 12 GPU (AMD, Intel, NVIDIA)
- **CUDA** — NVIDIA-specific, higher performance
- **CPU** — Fallback, works everywhere but slower

Lives in: `managers/transcription.rs`, `audio_toolkit/vad/`

## Divergence Summary

| Aspect | macOS | Windows |
|---|---|---|
| GPU framework | Metal / Core ML | DirectML / CUDA / Vulkan |
| Whisper runtime | WhisperKit (Core ML) | whisper-cpp or ONNX |
| Parakeet runtime | Custom Metal backend | ONNX Runtime |
| VAD | Built into FluidAudio | Silero VAD via ONNX (`vad-rs`) |
| Model format | Core ML (.mlmodelc) | ONNX (.onnx) or GGML (.bin) |

## Translation Guidance

When the Swift code changes transcription backends:
- **Translate** changes to the transcription **interface** (what methods are called, what parameters they take, what results they return)
- **Translate** changes to model selection logic, model downloading, progress tracking
- **Don't translate** Metal/CoreML-specific inference code — the ONNX/whisper-cpp equivalents in the desktop app handle this differently
- **Watch for** new model backends — if upstream adds a new ASR engine, check if it has an ONNX export available

When the Swift code changes model management:
- **Translate** model download URLs, file size checks, progress reporting
- **Note** that model formats differ — a Core ML model URL won't work on Windows. You need the ONNX or GGML equivalent of the same model.

## Implementation Priority
Already implemented for core transcription. Diarization model inference is the main gap (see diarization.md).
