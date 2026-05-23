use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, thiserror::Error)]
pub enum AsrError {
    #[error("failed to load ASR model: {0}")]
    Load(String),
    #[error("transcription failed: {0}")]
    Transcribe(String),
    #[error("worker thread unavailable")]
    WorkerUnavailable,
}

enum AsrJob {
    Transcribe {
        samples: Vec<f32>,
        reply: SyncSender<Result<String, AsrError>>,
    },
    Shutdown,
}

#[derive(Debug)]
pub struct AsrEngine {
    sender: SyncSender<AsrJob>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AsrEngine {
    pub fn load(model_path: &Path) -> Result<Self, AsrError> {
        if !model_path.exists() {
            return Err(AsrError::Load(format!(
                "model file not found at {}",
                model_path.display()
            )));
        }

        let model_path_str = model_path.to_string_lossy().into_owned();
        let (load_tx, load_rx) = mpsc::sync_channel::<Result<(), AsrError>>(1);
        let (job_tx, job_rx) = mpsc::sync_channel::<AsrJob>(0);

        let worker = std::thread::spawn(move || {
            let ctx = match WhisperContext::new_with_params(
                &model_path_str,
                WhisperContextParameters::default(),
            ) {
                Ok(c) => c,
                Err(e) => {
                    let _ = load_tx.send(Err(AsrError::Load(e.to_string())));
                    return;
                }
            };
            let _ = load_tx.send(Ok(()));
            drop(load_tx);

            while let Ok(job) = job_rx.recv() {
                match job {
                    AsrJob::Transcribe { samples, reply } => {
                        let _ = reply.send(transcribe_with_context(&ctx, &samples));
                    }
                    AsrJob::Shutdown => break,
                }
            }
        });

        match load_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: job_tx,
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(AsrError::Load("worker thread died during load".into())),
        }
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, AsrError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.sender
            .send(AsrJob::Transcribe {
                samples: samples.to_vec(),
                reply: reply_tx,
            })
            .map_err(|_| AsrError::WorkerUnavailable)?;
        reply_rx.recv().map_err(|_| AsrError::WorkerUnavailable)?
    }
}

impl Drop for AsrEngine {
    fn drop(&mut self) {
        let _ = self.sender.send(AsrJob::Shutdown);
        if let Ok(mut guard) = self.worker.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

fn transcribe_with_context(ctx: &WhisperContext, samples: &[f32]) -> Result<String, AsrError> {
    let mut state = ctx
        .create_state()
        .map_err(|e| AsrError::Transcribe(format!("create_state: {e}")))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    state
        .full(params, samples)
        .map_err(|e| AsrError::Transcribe(format!("full: {e}")))?;

    let mut transcript = String::new();
    let num_segments = state.full_n_segments();
    for i in 0..num_segments {
        let segment = state
            .get_segment(i)
            .ok_or_else(|| AsrError::Transcribe(format!("missing segment {i}")))?;
        let text = segment
            .to_str_lossy()
            .map_err(|e| AsrError::Transcribe(format!("segment text: {e}")))?;
        transcript.push_str(&text);
    }
    Ok(transcript.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn lock_engine_handle_is_send_sync() {
        assert_send_sync::<AsrEngine>();
    }

    #[test]
    fn lock_load_returns_err_for_missing_path() {
        let err = AsrEngine::load(Path::new("/nonexistent/path/to/whisper.bin"))
            .expect_err("loading from missing path must error");

        assert!(
            matches!(err, AsrError::Load(_)),
            "expected AsrError::Load, got {err:?}",
        );
    }

    const BUNDLE_ID: &str = "io.github.canhassancode.type-less";
    const WHISPER_FILENAME: &str = "ggml-small.en.bin";

    fn whisper_model_path() -> Option<std::path::PathBuf> {
        let path = dirs::data_dir()?
            .join(BUNDLE_ID)
            .join("models")
            .join(WHISPER_FILENAME);
        if path.exists() { Some(path) } else { None }
    }

    fn load_wav_as_mono_f32(path: &Path) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).expect("open WAV fixture");
        let spec = reader.spec();
        assert_eq!(spec.sample_rate, 16_000, "fixture must be 16 kHz");
        assert_eq!(spec.channels, 1, "fixture must be mono");
        assert_eq!(spec.bits_per_sample, 16, "fixture must be 16-bit");
        reader
            .samples::<i16>()
            .map(|s| s.expect("valid sample") as f32 / i16::MAX as f32)
            .collect()
    }

    #[test]
    #[ignore = "requires real whisper model on disk; run with --ignored"]
    fn lock_loads_and_transcribes_known_wav() {
        let Some(model_path) = whisper_model_path() else {
            eprintln!("skipping: whisper model not on disk; run `pnpm bootstrap:models`");
            return;
        };
        let engine = AsrEngine::load(&model_path).expect("load real whisper model");

        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("hello_world.wav");
        let samples = load_wav_as_mono_f32(&fixture);

        let transcript = engine.transcribe(&samples).expect("transcribe fixture");

        let lower = transcript.to_lowercase();
        assert!(
            lower.contains("hello") && lower.contains("world"),
            "expected 'hello' and 'world' in transcript, got: {transcript:?}",
        );
    }
}
