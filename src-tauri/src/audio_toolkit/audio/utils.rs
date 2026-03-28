use anyhow::Result;
use hound::{WavReader, WavSpec, WavWriter};
use log::debug;
use std::path::Path;

/// Audio samples loaded from a WAV file, normalized to f32 at 16kHz mono.
pub struct WavAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
}

/// Load a WAV file and return f32 samples normalized to [-1.0, 1.0].
/// Automatically downmixes to mono and resamples to 16kHz if needed.
pub fn load_wav_file<P: AsRef<Path>>(file_path: P) -> Result<WavAudio> {
    let reader = WavReader::open(file_path.as_ref())?;
    let spec = reader.spec();

    debug!(
        "Loading WAV: {}Hz, {} channels, {} bits",
        spec.sample_rate, spec.channels, spec.bits_per_sample
    );

    // Read all samples as f32
    let raw_samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    // Downmix to mono if multi-channel
    let mono_samples: Vec<f32> = if spec.channels > 1 {
        raw_samples
            .chunks(spec.channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / spec.channels as f32)
            .collect()
    } else {
        raw_samples
    };

    // Simple linear resampling to 16kHz if needed
    let target_rate = 16000u32;
    let final_samples = if spec.sample_rate != target_rate {
        let ratio = target_rate as f64 / spec.sample_rate as f64;
        let new_len = (mono_samples.len() as f64 * ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);
        for i in 0..new_len {
            let src_idx = i as f64 / ratio;
            let idx = src_idx as usize;
            let frac = src_idx - idx as f64;
            let s0 = mono_samples.get(idx).copied().unwrap_or(0.0);
            let s1 = mono_samples.get(idx + 1).copied().unwrap_or(s0);
            resampled.push(s0 + (s1 - s0) * frac as f32);
        }
        resampled
    } else {
        mono_samples
    };

    let duration_secs = final_samples.len() as f64 / target_rate as f64;

    debug!(
        "Loaded WAV: {} samples ({:.2}s) at {}Hz mono",
        final_samples.len(),
        duration_secs,
        target_rate
    );

    Ok(WavAudio {
        samples: final_samples,
        sample_rate: target_rate,
        channels: 1,
        duration_secs,
    })
}

/// Save audio samples as a WAV file
pub async fn save_wav_file<P: AsRef<Path>>(file_path: P, samples: &[f32]) -> Result<()> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(file_path.as_ref(), spec)?;

    // Convert f32 samples to i16 for WAV
    for sample in samples {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }

    writer.finalize()?;
    debug!("Saved WAV file: {:?}", file_path.as_ref());
    Ok(())
}
