use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::thread;

use pipeline::asr::AsrEngine;
use pipeline::cleanup::CleanupEngine;

#[derive(Clone, Default)]
pub struct EngineHandles {
    asr: Arc<OnceLock<Arc<AsrEngine>>>,
    cleanup: Arc<OnceLock<Arc<CleanupEngine>>>,
}

impl EngineHandles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_asr(&self) -> Option<Arc<AsrEngine>> {
        self.asr.get().cloned()
    }

    pub fn try_cleanup(&self) -> Option<Arc<CleanupEngine>> {
        self.cleanup.get().cloned()
    }
}

pub fn spawn_loaders(
    handles: EngineHandles,
    asr_path: PathBuf,
    cleanup_path: PathBuf,
    cleanup_prompt: &'static str,
) {
    let asr_slot = handles.asr.clone();
    thread::spawn(move || match AsrEngine::load(&asr_path) {
        Ok(engine) => {
            let _ = asr_slot.set(Arc::new(engine));
            eprintln!("[engines] ASR ready");
        }
        Err(err) => {
            eprintln!("[engines] ASR load failed: {err}");
        }
    });

    let cleanup_slot = handles.cleanup.clone();
    thread::spawn(move || match CleanupEngine::load(&cleanup_path, cleanup_prompt) {
        Ok(engine) => {
            let _ = cleanup_slot.set(Arc::new(engine));
            eprintln!("[engines] Cleanup ready");
        }
        Err(err) => {
            eprintln!("[engines] Cleanup load failed: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn lock_engine_handles_are_send_sync() {
        assert_send_sync::<EngineHandles>();
    }

    #[test]
    fn try_asr_returns_none_before_load() {
        let h = EngineHandles::new();
        assert!(h.try_asr().is_none());
    }

    #[test]
    fn try_cleanup_returns_none_before_load() {
        let h = EngineHandles::new();
        assert!(h.try_cleanup().is_none());
    }

    #[test]
    fn clones_share_the_same_underlying_slots() {
        let h1 = EngineHandles::new();
        let h2 = h1.clone();
        assert!(Arc::ptr_eq(&h1.asr, &h2.asr));
        assert!(Arc::ptr_eq(&h1.cleanup, &h2.cleanup));
    }
}
