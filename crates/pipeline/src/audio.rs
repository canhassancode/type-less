use std::fmt;
use std::mem;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Default)]
pub struct AudioBuffer {
    samples: Mutex<Vec<f32>>,
}

impl AudioBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn push(&self, samples: &[f32]) {
        self.samples
            .lock()
            .expect("audio buffer poisoned")
            .extend_from_slice(samples);
    }

    pub fn drain(&self) -> Vec<f32> {
        mem::take(&mut *self.samples.lock().expect("audio buffer poisoned"))
    }
}

#[derive(Debug)]
pub enum AudioError {
    NoInputDevice,
    DefaultConfig(cpal::DefaultStreamConfigError),
    BuildStream(cpal::BuildStreamError),
    PlayStream(cpal::PlayStreamError),
    UnsupportedSampleFormat(cpal::SampleFormat),
    ThreadDied,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInputDevice => write!(f, "no default input device available"),
            Self::DefaultConfig(err) => write!(f, "failed to read default input config: {err}"),
            Self::BuildStream(err) => write!(f, "failed to build input stream: {err}"),
            Self::PlayStream(err) => write!(f, "failed to start input stream: {err}"),
            Self::UnsupportedSampleFormat(fmt) => write!(f, "unsupported sample format: {fmt:?}"),
            Self::ThreadDied => write!(f, "audio capture thread terminated unexpectedly"),
        }
    }
}

impl std::error::Error for AudioError {}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

pub struct AudioSession {
    buffer: Arc<AudioBuffer>,
    shutdown: SyncSender<()>,
    thread: JoinHandle<()>,
    sample_rate: u32,
    channels: u16,
}

pub fn start() -> Result<AudioSession, AudioError> {
    let buffer = AudioBuffer::new();
    let buffer_for_thread = Arc::clone(&buffer);
    let (shutdown_tx, shutdown_rx) = mpsc::sync_channel::<()>(1);
    let (ready_tx, ready_rx) =
        mpsc::sync_channel::<Result<StreamFormat, AudioError>>(1);

    let thread = thread::spawn(move || {
        let (stream, format) = match build_input_stream(buffer_for_thread) {
            Ok(parts) => parts,
            Err(err) => {
                let _ = ready_tx.send(Err(err));
                return;
            }
        };
        if let Err(err) = stream.play() {
            let _ = ready_tx.send(Err(AudioError::PlayStream(err)));
            return;
        }
        let _ = ready_tx.send(Ok(format));
        let _ = shutdown_rx.recv();
        drop(stream);
    });

    match ready_rx.recv() {
        Ok(Ok(format)) => Ok(AudioSession {
            buffer,
            shutdown: shutdown_tx,
            thread,
            sample_rate: format.sample_rate,
            channels: format.channels,
        }),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(AudioError::ThreadDied),
    }
}

struct StreamFormat {
    sample_rate: u32,
    channels: u16,
}

fn build_input_stream(
    buffer: Arc<AudioBuffer>,
) -> Result<(cpal::Stream, StreamFormat), AudioError> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(AudioError::NoInputDevice)?;
    let config = device
        .default_input_config()
        .map_err(AudioError::DefaultConfig)?;
    let sample_format = config.sample_format();
    let stream_config: cpal::StreamConfig = config.into();
    let format = StreamFormat {
        sample_rate: stream_config.sample_rate.0,
        channels: stream_config.channels,
    };
    let err_fn = |err| eprintln!("[pipeline::audio] capture error: {err}");

    match sample_format {
        cpal::SampleFormat::F32 => device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| buffer.push(data),
                err_fn,
                None,
            )
            .map(|stream| (stream, format))
            .map_err(AudioError::BuildStream),
        other => Err(AudioError::UnsupportedSampleFormat(other)),
    }
}

impl AudioSession {
    pub fn stop(self) -> CapturedAudio {
        let _ = self.shutdown.send(());
        let _ = self.thread.join();
        CapturedAudio {
            samples: self.buffer.drain(),
            sample_rate: self.sample_rate,
            channels: self.channels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_returns_pushed_samples_in_order() {
        let buffer = AudioBuffer::new();
        buffer.push(&[1.0, 2.0, 3.0]);

        assert_eq!(buffer.drain(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn pushes_accumulate_across_calls() {
        let buffer = AudioBuffer::new();
        buffer.push(&[1.0, 2.0]);
        buffer.push(&[3.0]);
        buffer.push(&[4.0, 5.0]);

        assert_eq!(buffer.drain(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn drain_empties_the_buffer() {
        let buffer = AudioBuffer::new();
        buffer.push(&[1.0, 2.0]);
        let _ = buffer.drain();

        assert!(buffer.drain().is_empty());
    }

    #[test]
    fn fresh_buffer_drains_to_empty() {
        let buffer = AudioBuffer::new();

        assert!(buffer.drain().is_empty());
    }
}
