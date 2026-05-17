#![allow(dead_code)]

use std::mem;
use std::sync::{Arc, Mutex};

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
