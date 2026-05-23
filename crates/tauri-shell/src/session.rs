use std::mem;
use std::sync::Mutex;

use pipeline::audio::CapturedAudio;
use pipeline::resample::to_whisper_format;
use serde::Serialize;
use specta::Type;

pub type StopFn = Box<dyn FnOnce() -> CapturedAudio + Send>;
pub type AudioStartFn = Box<dyn Fn() -> Result<StopFn, SessionError> + Send + Sync>;
pub type AsrFn = Box<dyn Fn(&[f32]) -> Result<String, SessionError> + Send + Sync>;
pub type CleanupFn = Box<dyn Fn(&str) -> Result<String, SessionError> + Send + Sync>;
pub type PasteFn = Box<dyn Fn(&str) -> Result<(), SessionError> + Send + Sync>;
pub type EventSinkFn = Box<dyn Fn(DictationEvent) + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    AlreadyActive,
    NotActive,
    Audio(String),
    Asr(String),
    Cleanup(String),
    Paste(String),
    EngineNotReady(String),
    NotImplemented,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum DictationStage {
    Recording,
    Loading,
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum ErrorStage {
    Asr,
    Cleanup,
    Paste,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationEvent {
    StateChanged(DictationStage),
    Completed { word_count: u32 },
    Error { stage: ErrorStage, message: String },
}

enum SessionState {
    Idle,
    Active(StopFn),
}

pub struct Session {
    state: Mutex<SessionState>,
    start_audio: AudioStartFn,
    asr: AsrFn,
    cleanup: CleanupFn,
    paste: PasteFn,
    sink: EventSinkFn,
}

impl Session {
    pub fn new(
        start_audio: impl Fn() -> Result<StopFn, SessionError> + Send + Sync + 'static,
        asr: impl Fn(&[f32]) -> Result<String, SessionError> + Send + Sync + 'static,
        cleanup: impl Fn(&str) -> Result<String, SessionError> + Send + Sync + 'static,
        paste: impl Fn(&str) -> Result<(), SessionError> + Send + Sync + 'static,
        sink: impl Fn(DictationEvent) + Send + Sync + 'static,
    ) -> Self {
        Self {
            state: Mutex::new(SessionState::Idle),
            start_audio: Box::new(start_audio),
            asr: Box::new(asr),
            cleanup: Box::new(cleanup),
            paste: Box::new(paste),
            sink: Box::new(sink),
        }
    }

    pub fn start(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("session state poisoned");
        if matches!(*state, SessionState::Active(_)) {
            return Err(SessionError::AlreadyActive);
        }
        let stop_fn = (self.start_audio)()?;
        *state = SessionState::Active(stop_fn);
        drop(state);
        (self.sink)(DictationEvent::StateChanged(DictationStage::Recording));
        Ok(())
    }

    pub fn stop(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("session state poisoned");
        let stop_fn = match mem::replace(&mut *state, SessionState::Idle) {
            SessionState::Idle => return Err(SessionError::NotActive),
            SessionState::Active(stop_fn) => stop_fn,
        };
        drop(state);

        (self.sink)(DictationEvent::StateChanged(DictationStage::Loading));

        let captured = stop_fn();
        let samples = to_whisper_format(captured);

        let outcome = self.run_inference(&samples);
        match outcome {
            Ok(cleaned) => {
                let word_count = cleaned.split_whitespace().count() as u32;
                (self.sink)(DictationEvent::Completed { word_count });
                (self.sink)(DictationEvent::StateChanged(DictationStage::Idle));
                Ok(())
            }
            Err((stage, err)) => {
                (self.sink)(DictationEvent::Error {
                    stage,
                    message: format!("{err:?}"),
                });
                (self.sink)(DictationEvent::StateChanged(DictationStage::Idle));
                Err(err)
            }
        }
    }

    fn run_inference(&self, samples: &[f32]) -> Result<String, (ErrorStage, SessionError)> {
        let transcript = (self.asr)(samples).map_err(|e| (ErrorStage::Asr, e))?;
        let cleaned = (self.cleanup)(&transcript).map_err(|e| (ErrorStage::Cleanup, e))?;
        (self.paste)(&cleaned).map_err(|e| (ErrorStage::Paste, e))?;
        Ok(cleaned)
    }

    pub fn cancel(&self) -> Result<(), SessionError> {
        Err(SessionError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn no_events() -> impl Fn(DictationEvent) + Send + Sync + 'static {
        |_event| {}
    }

    fn event_collector() -> (Arc<Mutex<Vec<DictationEvent>>>, impl Fn(DictationEvent) + Send + Sync + 'static)
    {
        let bus: Arc<Mutex<Vec<DictationEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let bus_for_sink = bus.clone();
        let sink = move |event: DictationEvent| {
            bus_for_sink.lock().expect("event bus poisoned").push(event);
        };
        (bus, sink)
    }

    fn fake_paste(
        sink: Arc<Mutex<Vec<String>>>,
    ) -> impl Fn(&str) -> Result<(), SessionError> + Send + Sync + 'static {
        move |text: &str| {
            sink.lock().expect("paste sink poisoned").push(text.to_string());
            Ok(())
        }
    }

    fn fake_audio() -> impl Fn() -> Result<StopFn, SessionError> + Send + Sync + 'static {
        || {
            Ok(Box::new(|| CapturedAudio {
                samples: vec![1.0, 2.0],
                sample_rate: 16_000,
                channels: 1,
            }))
        }
    }

    fn fake_asr(
        f: impl Fn(&[f32]) -> Result<String, SessionError> + Send + Sync + 'static,
    ) -> impl Fn(&[f32]) -> Result<String, SessionError> + Send + Sync + 'static {
        f
    }

    fn fake_cleanup(
        f: impl Fn(&str) -> Result<String, SessionError> + Send + Sync + 'static,
    ) -> impl Fn(&str) -> Result<String, SessionError> + Send + Sync + 'static {
        f
    }

    fn ok_asr(transcript: &'static str)
    -> impl Fn(&[f32]) -> Result<String, SessionError> + Send + Sync + 'static {
        move |_samples| Ok(transcript.to_string())
    }

    fn ok_cleanup(cleaned: &'static str)
    -> impl Fn(&str) -> Result<String, SessionError> + Send + Sync + 'static {
        move |_transcript| Ok(cleaned.to_string())
    }

    #[test]
    fn stop_runs_resample_asr_cleanup_paste_pipeline_with_no_text_arg() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            fake_asr(|samples| {
                assert_eq!(samples, &[1.0, 2.0], "resampled 16k mono samples reach ASR");
                Ok("hello world".into())
            }),
            fake_cleanup(|transcript| {
                assert_eq!(transcript, "hello world", "ASR output reaches cleanup");
                Ok("Hello, world.".into())
            }),
            fake_paste(pasted.clone()),
            no_events(),
        );

        session.start().expect("start should succeed");
        session.stop().expect("stop should succeed");

        assert_eq!(
            *pasted.lock().expect("sink poisoned"),
            vec!["Hello, world.".to_string()],
        );
    }

    #[test]
    fn engine_not_ready_surfaces_through_session_stop() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            fake_asr(|_samples| Err(SessionError::EngineNotReady("asr".into()))),
            ok_cleanup("should not be called"),
            fake_paste(pasted.clone()),
            no_events(),
        );

        session.start().expect("start should succeed");
        let stop_result = session.stop();
        assert_eq!(stop_result, Err(SessionError::EngineNotReady("asr".into())));

        assert!(
            pasted.lock().expect("sink poisoned").is_empty(),
            "paste must not run when engines are not ready",
        );
        session
            .start()
            .expect("session must be Idle after EngineNotReady so the next start succeeds");
    }

    #[test]
    fn cleanup_failure_surfaces_error_and_transitions_session_to_idle() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            fake_cleanup(|_transcript| Err(SessionError::Cleanup("llama crashed".into()))),
            fake_paste(pasted.clone()),
            no_events(),
        );

        session.start().expect("start should succeed");
        let stop_result = session.stop();
        assert_eq!(stop_result, Err(SessionError::Cleanup("llama crashed".into())));

        assert!(
            pasted.lock().expect("sink poisoned").is_empty(),
            "paste must not run when cleanup fails",
        );
        session
            .start()
            .expect("session must be Idle after cleanup failure so the next start succeeds");
    }

    #[test]
    fn asr_failure_surfaces_error_and_transitions_session_to_idle() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            fake_asr(|_samples| Err(SessionError::Asr("whisper crashed".into()))),
            ok_cleanup("should not be called"),
            fake_paste(pasted.clone()),
            no_events(),
        );

        session.start().expect("start should succeed");
        let stop_result = session.stop();
        assert_eq!(stop_result, Err(SessionError::Asr("whisper crashed".into())));

        assert!(
            pasted.lock().expect("sink poisoned").is_empty(),
            "paste must not run when ASR fails",
        );
        session
            .start()
            .expect("session must be Idle after ASR failure so the next start succeeds");
    }

    #[test]
    fn start_while_active_returns_already_active() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted),
            no_events(),
        );

        session.start().expect("first start should succeed");
        let second = session.start();

        assert_eq!(second, Err(SessionError::AlreadyActive));
    }

    #[test]
    fn stop_without_start_returns_not_active() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted.clone()),
            no_events(),
        );

        let result = session.stop();

        assert_eq!(result, Err(SessionError::NotActive));
        assert!(pasted.lock().expect("sink poisoned").is_empty());
    }

    #[test]
    fn start_audio_failure_keeps_session_idle() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            || Err(SessionError::Audio("device busy".into())),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted),
            no_events(),
        );

        let first = session.start();
        assert_eq!(first, Err(SessionError::Audio("device busy".into())));

        let second = session.start();
        assert_eq!(
            second,
            Err(SessionError::Audio("device busy".into())),
            "session must still be Idle, not stuck in Active"
        );
    }

    #[test]
    fn paste_failure_still_transitions_session_to_idle() {
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            ok_cleanup("Hello, world."),
            |_text: &str| Err(SessionError::Paste("clipboard locked".into())),
            no_events(),
        );

        session.start().expect("start should succeed");
        let stop_result = session.stop();
        assert_eq!(stop_result, Err(SessionError::Paste("clipboard locked".into())));

        session
            .start()
            .expect("session must be Idle after paste failure so the next start succeeds");
    }

    #[test]
    fn cancel_returns_not_implemented_pending_slice_6() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted),
            no_events(),
        );

        let result = session.cancel();

        assert_eq!(result, Err(SessionError::NotImplemented));
    }

    #[test]
    fn happy_path_emits_recording_loading_completed_idle_in_order() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            ok_cleanup("Hello, world."),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("start should succeed");
        session.stop().expect("stop should succeed");

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Completed { word_count: 2 },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn asr_failure_emits_error_then_idle_with_no_completed() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            fake_asr(|_samples| Err(SessionError::Asr("whisper crashed".into()))),
            ok_cleanup("unused"),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("start should succeed");
        let _ = session.stop();

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Error {
                    stage: ErrorStage::Asr,
                    message: "Asr(\"whisper crashed\")".into(),
                },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn cleanup_failure_emits_error_then_idle_with_no_completed() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            fake_cleanup(|_t| Err(SessionError::Cleanup("llama crashed".into()))),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("start should succeed");
        let _ = session.stop();

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Error {
                    stage: ErrorStage::Cleanup,
                    message: "Cleanup(\"llama crashed\")".into(),
                },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn paste_failure_emits_error_then_idle_with_no_completed() {
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            ok_cleanup("Hello, world."),
            |_text: &str| Err(SessionError::Paste("clipboard locked".into())),
            sink,
        );

        session.start().expect("start should succeed");
        let _ = session.stop();

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Error {
                    stage: ErrorStage::Paste,
                    message: "Paste(\"clipboard locked\")".into(),
                },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn already_active_on_start_emits_no_events() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("first start should succeed");
        let before_second = events.lock().expect("event bus poisoned").len();
        let second = session.start();
        let after_second = events.lock().expect("event bus poisoned").len();

        assert_eq!(second, Err(SessionError::AlreadyActive));
        assert_eq!(
            after_second, before_second,
            "AlreadyActive must not emit any events",
        );
    }

    #[test]
    fn not_active_on_stop_emits_no_events() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("ignored"),
            ok_cleanup("ignored"),
            fake_paste(pasted),
            sink,
        );

        let result = session.stop();

        assert_eq!(result, Err(SessionError::NotActive));
        assert!(
            events.lock().expect("event bus poisoned").is_empty(),
            "NotActive must not emit any events",
        );
    }

    #[test]
    fn engine_not_ready_asr_emits_asr_error_event() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            fake_asr(|_s| Err(SessionError::EngineNotReady("asr".into()))),
            ok_cleanup("unused"),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("start should succeed");
        let _ = session.stop();

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Error {
                    stage: ErrorStage::Asr,
                    message: "EngineNotReady(\"asr\")".into(),
                },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn engine_not_ready_cleanup_emits_cleanup_error_event() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let (events, sink) = event_collector();
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            fake_cleanup(|_t| Err(SessionError::EngineNotReady("cleanup".into()))),
            fake_paste(pasted),
            sink,
        );

        session.start().expect("start should succeed");
        let _ = session.stop();

        let recorded = events.lock().expect("event bus poisoned").clone();
        assert_eq!(
            recorded,
            vec![
                DictationEvent::StateChanged(DictationStage::Recording),
                DictationEvent::StateChanged(DictationStage::Loading),
                DictationEvent::Error {
                    stage: ErrorStage::Cleanup,
                    message: "EngineNotReady(\"cleanup\")".into(),
                },
                DictationEvent::StateChanged(DictationStage::Idle),
            ],
        );
    }

    #[test]
    fn start_again_after_stop_is_allowed() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            ok_cleanup("Hello, world."),
            fake_paste(pasted.clone()),
            no_events(),
        );

        session.start().expect("first start");
        session.stop().expect("first stop");
        session.start().expect("second start should succeed");
        session.stop().expect("second stop");

        assert_eq!(
            *pasted.lock().expect("sink poisoned"),
            vec!["Hello, world.".to_string(), "Hello, world.".to_string()],
        );
    }
}
