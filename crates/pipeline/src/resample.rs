use crate::audio::CapturedAudio;

pub fn to_whisper_format(captured: CapturedAudio) -> Vec<f32> {
    downmix_to_mono(&captured.samples, captured.channels)
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
}
