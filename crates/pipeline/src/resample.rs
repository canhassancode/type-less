use rubato::{FftFixedIn, Resampler};

use crate::audio::CapturedAudio;

const TARGET_SAMPLE_RATE: u32 = 16_000;
const RESAMPLE_CHUNK_SIZE: usize = 1024;
const RESAMPLE_SUB_CHUNKS: usize = 2;

pub fn to_whisper_format(captured: CapturedAudio) -> Vec<f32> {
    let mono = downmix_to_mono(&captured.samples, captured.channels);
    if captured.sample_rate == TARGET_SAMPLE_RATE {
        return mono;
    }
    resample_to_target(&mono, captured.sample_rate)
}

fn downmix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let channels = channels as usize;
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

fn resample_to_target(mono: &[f32], src_rate: u32) -> Vec<f32> {
    let mut resampler = FftFixedIn::<f32>::new(
        src_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        RESAMPLE_CHUNK_SIZE,
        RESAMPLE_SUB_CHUNKS,
        1,
    )
    .expect("rubato FftFixedIn construction with valid rates and chunk size");

    let mut output: Vec<f32> = Vec::new();
    let mut cursor = 0;

    while cursor + RESAMPLE_CHUNK_SIZE <= mono.len() {
        let chunk = &mono[cursor..cursor + RESAMPLE_CHUNK_SIZE];
        let processed = resampler
            .process(&[chunk], None)
            .expect("rubato process with matched chunk size");
        output.extend_from_slice(&processed[0]);
        cursor += RESAMPLE_CHUNK_SIZE;
    }

    if cursor < mono.len() {
        let tail = &mono[cursor..];
        let processed = resampler
            .process_partial(Some(&[tail]), None)
            .expect("rubato process_partial with trailing chunk");
        output.extend_from_slice(&processed[0]);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::CapturedAudio;

    #[test]
    fn passthrough_16k_mono_returns_samples_unchanged() {
        let captured = CapturedAudio {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            sample_rate: 16_000,
            channels: 1,
        };

        let out = to_whisper_format(captured);

        assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn stereo_16k_downmixes_via_l_plus_r_average() {
        let captured = CapturedAudio {
            samples: vec![1.0, 3.0, 5.0, 7.0, -2.0, 4.0],
            sample_rate: 16_000,
            channels: 2,
        };

        let out = to_whisper_format(captured);

        assert_eq!(out, vec![2.0, 6.0, 1.0]);
    }

    fn assert_target_length(actual: usize, expected: usize, tolerance: usize) {
        let diff = actual.abs_diff(expected);
        assert!(
            diff <= tolerance,
            "expected ~{expected} samples (±{tolerance}), got {actual}",
        );
    }

    fn assert_dc_preserved(samples: &[f32], expected: f32) {
        let mid = samples.len() / 2;
        let window = &samples[mid..(mid + 10).min(samples.len())];
        for &s in window {
            assert!(
                (s - expected).abs() < 0.01,
                "expected DC ≈ {expected}, got {s}",
            );
        }
    }

    #[test]
    fn downsamples_48k_mono_to_16k() {
        let captured = CapturedAudio {
            samples: vec![0.5; 4_800],
            sample_rate: 48_000,
            channels: 1,
        };

        let out = to_whisper_format(captured);

        assert_target_length(out.len(), 1_600, 100);
        assert_dc_preserved(&out, 0.5);
    }

    #[test]
    fn downsamples_44_1k_mono_to_16k() {
        let captured = CapturedAudio {
            samples: vec![0.25; 4_410],
            sample_rate: 44_100,
            channels: 1,
        };

        let out = to_whisper_format(captured);

        assert_target_length(out.len(), 1_600, 100);
        assert_dc_preserved(&out, 0.25);
    }

    fn interleave_stereo(left: f32, right: f32, frames: usize) -> Vec<f32> {
        (0..frames).flat_map(|_| [left, right]).collect()
    }

    #[test]
    fn downsamples_48k_stereo_to_16k_mono() {
        let captured = CapturedAudio {
            samples: interleave_stereo(0.3, 0.7, 4_800),
            sample_rate: 48_000,
            channels: 2,
        };

        let out = to_whisper_format(captured);

        assert_target_length(out.len(), 1_600, 100);
        assert_dc_preserved(&out, 0.5);
    }

    #[test]
    fn downsamples_44_1k_stereo_to_16k_mono() {
        let captured = CapturedAudio {
            samples: interleave_stereo(-0.1, 0.5, 4_410),
            sample_rate: 44_100,
            channels: 2,
        };

        let out = to_whisper_format(captured);

        assert_target_length(out.len(), 1_600, 100);
        assert_dc_preserved(&out, 0.2);
    }
}
