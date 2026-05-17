use std::mem;
use std::sync::Mutex;

use pipeline::audio::CapturedAudio;
use pipeline::resample::to_whisper_format;

pub type StopFn = Box<dyn FnOnce() -> CapturedAudio + Send>;
pub type AudioStartFn = Box<dyn Fn() -> Result<StopFn, SessionError> + Send + Sync>;
pub type AsrFn = Box<dyn Fn(&[f32]) -> Result<String, SessionError> + Send + Sync>;
pub type CleanupFn = Box<dyn Fn(&str) -> Result<String, SessionError> + Send + Sync>;
pub type PasteFn = Box<dyn Fn(&str) -> Result<(), SessionError> + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    AlreadyActive,
    NotActive,
    Audio(String),
    Asr(String),
    Cleanup(String),
    Paste(String),
    NotImplemented,
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
}

impl Session {
    pub fn new(
        start_audio: impl Fn() -> Result<StopFn, SessionError> + Send + Sync + 'static,
        asr: impl Fn(&[f32]) -> Result<String, SessionError> + Send + Sync + 'static,
        cleanup: impl Fn(&str) -> Result<String, SessionError> + Send + Sync + 'static,
        paste: impl Fn(&str) -> Result<(), SessionError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            state: Mutex::new(SessionState::Idle),
            start_audio: Box::new(start_audio),
            asr: Box::new(asr),
            cleanup: Box::new(cleanup),
            paste: Box::new(paste),
        }
    }

    pub fn start(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("session state poisoned");
        if matches!(*state, SessionState::Active(_)) {
            return Err(SessionError::AlreadyActive);
        }
        let stop_fn = (self.start_audio)()?;
        *state = SessionState::Active(stop_fn);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("session state poisoned");
        let stop_fn = match mem::replace(&mut *state, SessionState::Idle) {
            SessionState::Idle => return Err(SessionError::NotActive),
            SessionState::Active(stop_fn) => stop_fn,
        };
        drop(state);

        let captured = stop_fn();
        let samples = to_whisper_format(captured);
        let transcript = (self.asr)(&samples)?;
        let cleaned = (self.cleanup)(&transcript)?;
        (self.paste)(&cleaned)?;
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), SessionError> {
        Err(SessionError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
        );

        session.start().expect("start should succeed");
        session.stop().expect("stop should succeed");

        assert_eq!(
            *pasted.lock().expect("sink poisoned"),
            vec!["Hello, world.".to_string()],
        );
    }

    #[test]
    fn cleanup_failure_surfaces_error_and_transitions_session_to_idle() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            fake_cleanup(|_transcript| Err(SessionError::Cleanup("llama crashed".into()))),
            fake_paste(pasted.clone()),
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
        );

        let result = session.cancel();

        assert_eq!(result, Err(SessionError::NotImplemented));
    }

    #[test]
    fn start_again_after_stop_is_allowed() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            fake_audio(),
            ok_asr("hello world"),
            ok_cleanup("Hello, world."),
            fake_paste(pasted.clone()),
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
