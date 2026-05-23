use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use pipeline::asr::AsrEngine;
use pipeline::cleanup::CleanupEngine;

const BUNDLE_ID: &str = "io.github.canhassancode.type-less";
const ASR_MODEL_FILENAME: &str = "ggml-small.en.bin";
const CLEANUP_MODEL_FILENAME: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";
const CLEANUP_PROMPT: &str = include_str!("../../../pipeline/prompts/cleanup_v1.txt");
const FIXTURE_COUNT: usize = 10;
const SAMPLE_RATE_HZ: usize = 16_000;
const FIXTURE_SECONDS: usize = 5;
const P95_BUDGET: Duration = Duration::from_millis(1000);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("timing: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let models_dir = resolve_models_dir()?;
    let asr_path = models_dir.join(ASR_MODEL_FILENAME);
    let cleanup_path = models_dir.join(CLEANUP_MODEL_FILENAME);

    println!("timing: loading ASR from {}", asr_path.display());
    let asr = AsrEngine::load(&asr_path).map_err(|err| err.to_string())?;
    println!("timing: loading Cleanup from {}", cleanup_path.display());
    let cleanup = CleanupEngine::load(&cleanup_path, CLEANUP_PROMPT).map_err(|err| err.to_string())?;

    let fixtures = synthetic_fixtures(FIXTURE_COUNT);
    let mut durations: Vec<Duration> = Vec::with_capacity(fixtures.len());
    for (i, samples) in fixtures.iter().enumerate() {
        let start = Instant::now();
        let transcript = asr.transcribe(samples).map_err(|err| err.to_string())?;
        let _cleaned = cleanup.cleanup(&transcript).map_err(|err| err.to_string())?;
        let elapsed = start.elapsed();
        println!(
            "timing: fixture {i} took {elapsed:?} (transcript chars = {})",
            transcript.len()
        );
        durations.push(elapsed);
    }

    let p95 = compute_p95(&durations);
    println!("timing: p95 = {p95:?} (budget = {P95_BUDGET:?})");

    assert!(
        p95 < P95_BUDGET,
        "p95 latency {p95:?} exceeds budget {P95_BUDGET:?}",
    );
    Ok(())
}

fn resolve_models_dir() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .ok_or_else(|| "could not resolve OS data dir".to_string())?
        .join(BUNDLE_ID)
        .join("models"))
}

fn synthetic_fixtures(count: usize) -> Vec<Vec<f32>> {
    let sample_count = SAMPLE_RATE_HZ * FIXTURE_SECONDS;
    (0..count)
        .map(|i| {
            if i % 2 == 0 {
                vec![0.0_f32; sample_count]
            } else {
                low_amplitude_noise(sample_count, i as u32)
            }
        })
        .collect()
}

fn low_amplitude_noise(sample_count: usize, seed: u32) -> Vec<f32> {
    let mut state = seed.wrapping_add(1);
    (0..sample_count)
        .map(|_| {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let normalised = ((state >> 16) & 0x7fff) as f32 / 32_768.0_f32;
            (normalised - 0.5) * 0.02
        })
        .collect()
}

fn compute_p95(durations: &[Duration]) -> Duration {
    if durations.is_empty() {
        panic!("compute_p95: empty input");
    }
    let mut sorted = durations.to_vec();
    sorted.sort();
    let idx = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    #[should_panic(expected = "empty input")]
    fn compute_p95_panics_on_empty_input() {
        let _ = compute_p95(&[]);
    }

    #[test]
    fn compute_p95_single_element_returns_itself() {
        assert_eq!(compute_p95(&[ms(100)]), ms(100));
    }

    #[test]
    fn compute_p95_of_ten_sorted_durations_picks_index_nine() {
        let durations: Vec<Duration> = (1..=10).map(ms).collect();
        assert_eq!(compute_p95(&durations), ms(10));
    }

    #[test]
    fn compute_p95_sorts_before_indexing() {
        let durations = vec![ms(5), ms(1), ms(9), ms(7), ms(3), ms(2), ms(8), ms(10), ms(4), ms(6)];
        assert_eq!(compute_p95(&durations), ms(10));
    }

    #[test]
    fn compute_p95_of_twenty_picks_index_eighteen() {
        let durations: Vec<Duration> = (1..=20).map(ms).collect();
        assert_eq!(compute_p95(&durations), ms(19));
    }

    #[test]
    fn synthetic_fixtures_have_correct_length_and_count() {
        let fixtures = synthetic_fixtures(10);
        assert_eq!(fixtures.len(), 10);
        for f in &fixtures {
            assert_eq!(f.len(), SAMPLE_RATE_HZ * FIXTURE_SECONDS);
        }
    }

    #[test]
    fn synthetic_fixtures_alternate_silence_and_noise() {
        let fixtures = synthetic_fixtures(4);
        assert!(
            fixtures[0].iter().all(|&s| s == 0.0),
            "even-indexed fixtures must be silence",
        );
        assert!(
            fixtures[1].iter().any(|&s| s != 0.0),
            "odd-indexed fixtures must contain non-zero samples",
        );
    }

    #[test]
    fn synthetic_fixtures_noise_stays_in_low_amplitude_band() {
        let fixtures = synthetic_fixtures(2);
        let noise = &fixtures[1];
        let max_abs = noise.iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));
        assert!(
            max_abs < 0.05,
            "noise amplitude {max_abs} must stay below 0.05 to avoid hallucination amplification",
        );
    }
}
