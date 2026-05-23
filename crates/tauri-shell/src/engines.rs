use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use pipeline::asr::AsrEngine;
use pipeline::cleanup::CleanupEngine;
use serde::Serialize;
use specta::Type;

#[derive(Clone)]
pub struct EngineHandles {
    asr: Arc<OnceLock<Arc<AsrEngine>>>,
    cleanup: Arc<OnceLock<Arc<CleanupEngine>>>,
    state: Arc<Mutex<EngineState>>,
}

impl Default for EngineHandles {
    fn default() -> Self {
        Self {
            asr: Arc::new(OnceLock::new()),
            cleanup: Arc::new(OnceLock::new()),
            state: Arc::new(Mutex::new(EngineState::Loading)),
        }
    }
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

    pub fn state(&self) -> EngineState {
        self.state.lock().expect("engine state poisoned").clone()
    }

    pub fn set_state(&self, state: EngineState) {
        *self.state.lock().expect("engine state poisoned") = state;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum EngineState {
    Loading,
    Ready,
    Degraded,
}

pub type EngineSinkFn = Arc<dyn Fn(EngineState) + Send + Sync>;
pub type AsrLoadFn = Box<dyn FnOnce() -> Result<AsrEngine, String> + Send>;
pub type CleanupLoadFn = Box<dyn FnOnce() -> Result<CleanupEngine, String> + Send>;

#[derive(Clone)]
pub struct Coordinator {
    remaining: Arc<AtomicUsize>,
    degraded: Arc<AtomicUsize>,
    sink: EngineSinkFn,
}

impl Coordinator {
    pub fn new(loader_count: usize, sink: EngineSinkFn) -> Self {
        sink(EngineState::Loading);
        Self {
            remaining: Arc::new(AtomicUsize::new(loader_count)),
            degraded: Arc::new(AtomicUsize::new(0)),
            sink,
        }
    }

    pub fn loader_succeeded(&self) {
        self.finalise();
    }

    pub fn loader_failed(&self) {
        self.degraded.fetch_add(1, Ordering::SeqCst);
        self.finalise();
    }

    fn finalise(&self) {
        if self.remaining.fetch_sub(1, Ordering::SeqCst) == 1 {
            let state = if self.degraded.load(Ordering::SeqCst) > 0 {
                EngineState::Degraded
            } else {
                EngineState::Ready
            };
            (self.sink)(state);
        }
    }
}

pub fn spawn_loaders(
    handles: EngineHandles,
    asr_load: AsrLoadFn,
    cleanup_load: CleanupLoadFn,
    sink: EngineSinkFn,
) {
    let coordinator = Coordinator::new(2, sink);

    let asr_slot = handles.asr.clone();
    let asr_coord = coordinator.clone();
    thread::spawn(move || {
        match asr_load() {
            Ok(engine) => {
                let _ = asr_slot.set(Arc::new(engine));
                eprintln!("[engines] ASR ready");
                asr_coord.loader_succeeded();
            }
            Err(err) => {
                eprintln!("[engines] ASR load failed: {err}");
                asr_coord.loader_failed();
            }
        }
    });

    let cleanup_slot = handles.cleanup.clone();
    let cleanup_coord = coordinator;
    thread::spawn(move || {
        match cleanup_load() {
            Ok(engine) => {
                let _ = cleanup_slot.set(Arc::new(engine));
                eprintln!("[engines] Cleanup ready");
                cleanup_coord.loader_succeeded();
            }
            Err(err) => {
                eprintln!("[engines] Cleanup load failed: {err}");
                cleanup_coord.loader_failed();
            }
        }
    });
}

pub fn load_asr_from(path: PathBuf) -> AsrLoadFn {
    Box::new(move || AsrEngine::load(&path).map_err(|err| err.to_string()))
}

pub fn load_cleanup_from(path: PathBuf, prompt: &'static str) -> CleanupLoadFn {
    Box::new(move || CleanupEngine::load(&path, prompt).map_err(|err| err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    fn assert_send_sync<T: Send + Sync>() {}

    fn event_collector() -> (Arc<Mutex<Vec<EngineState>>>, EngineSinkFn) {
        let bus: Arc<Mutex<Vec<EngineState>>> = Arc::new(Mutex::new(Vec::new()));
        let bus_for_sink = bus.clone();
        let sink: EngineSinkFn = Arc::new(move |state| {
            bus_for_sink.lock().expect("engine bus poisoned").push(state);
        });
        (bus, sink)
    }

    fn wait_until<F: Fn() -> bool>(predicate: F, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        predicate()
    }

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
        assert!(Arc::ptr_eq(&h1.state, &h2.state));
    }

    #[test]
    fn state_defaults_to_loading() {
        let h = EngineHandles::new();
        assert_eq!(h.state(), EngineState::Loading);
    }

    #[test]
    fn set_state_updates_visible_state() {
        let h = EngineHandles::new();
        h.set_state(EngineState::Ready);
        assert_eq!(h.state(), EngineState::Ready);
        h.set_state(EngineState::Degraded);
        assert_eq!(h.state(), EngineState::Degraded);
    }

    #[test]
    fn cloned_handles_observe_each_others_state_updates() {
        let h1 = EngineHandles::new();
        let h2 = h1.clone();
        h1.set_state(EngineState::Ready);
        assert_eq!(h2.state(), EngineState::Ready);
    }

    #[test]
    fn coordinator_emits_loading_at_construction() {
        let (events, sink) = event_collector();
        let _ = Coordinator::new(2, sink);
        assert_eq!(
            events.lock().expect("bus poisoned").clone(),
            vec![EngineState::Loading],
        );
    }

    #[test]
    fn coordinator_emits_ready_only_after_all_loaders_succeed() {
        let (events, sink) = event_collector();
        let coord = Coordinator::new(2, sink);

        coord.loader_succeeded();
        assert_eq!(
            events.lock().expect("bus poisoned").clone(),
            vec![EngineState::Loading],
            "Ready must not be emitted after only one loader",
        );

        coord.loader_succeeded();
        assert_eq!(
            events.lock().expect("bus poisoned").clone(),
            vec![EngineState::Loading, EngineState::Ready],
        );
    }

    #[test]
    fn coordinator_emits_degraded_if_any_loader_fails() {
        let (events, sink) = event_collector();
        let coord = Coordinator::new(2, sink);

        coord.loader_succeeded();
        coord.loader_failed();

        assert_eq!(
            events.lock().expect("bus poisoned").clone(),
            vec![EngineState::Loading, EngineState::Degraded],
        );
    }

    #[test]
    fn coordinator_emits_degraded_when_all_loaders_fail() {
        let (events, sink) = event_collector();
        let coord = Coordinator::new(2, sink);

        coord.loader_failed();
        coord.loader_failed();

        assert_eq!(
            events.lock().expect("bus poisoned").clone(),
            vec![EngineState::Loading, EngineState::Degraded],
        );
    }

    #[test]
    fn spawn_loaders_emits_loading_synchronously_then_degraded_when_loads_fail() {
        let handles = EngineHandles::new();
        let (events, sink) = event_collector();
        let asr_load: AsrLoadFn = Box::new(|| Err("model missing".into()));
        let cleanup_load: CleanupLoadFn = Box::new(|| Err("model missing".into()));

        spawn_loaders(handles, asr_load, cleanup_load, sink);

        assert_eq!(
            events.lock().expect("bus poisoned").first(),
            Some(&EngineState::Loading),
            "Loading must be emitted synchronously inside spawn_loaders",
        );

        assert!(
            wait_until(
                || events.lock().expect("bus poisoned").contains(&EngineState::Degraded),
                Duration::from_millis(500),
            ),
            "expected Degraded after loader failures: got {:?}",
            events.lock().expect("bus poisoned").clone(),
        );
    }
}
