use std::mem;
use std::sync::Mutex;

use pipeline::audio::CapturedAudio;

pub type StopFn = Box<dyn FnOnce() -> CapturedAudio + Send>;
pub type AudioStartFn = Box<dyn Fn() -> Result<StopFn, SessionError> + Send + Sync>;
pub type PasteFn = Box<dyn Fn(&str) -> Result<(), SessionError> + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionError {
    AlreadyActive,
    NotActive,
    Audio(String),
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
    paste: PasteFn,
}

impl Session {
    pub fn new(
        start_audio: impl Fn() -> Result<StopFn, SessionError> + Send + Sync + 'static,
        paste: impl Fn(&str) -> Result<(), SessionError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            state: Mutex::new(SessionState::Idle),
            start_audio: Box::new(start_audio),
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

    pub fn stop(&self, text: &str) -> Result<(), SessionError> {
        let mut state = self.state.lock().expect("session state poisoned");
        let stop_fn = match mem::replace(&mut *state, SessionState::Idle) {
            SessionState::Idle => return Err(SessionError::NotActive),
            SessionState::Active(stop_fn) => stop_fn,
        };
        let _pcm = stop_fn();
        (self.paste)(text)?;
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

    #[test]
    fn stop_pastes_the_text_after_a_started_session() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(fake_audio(), fake_paste(pasted.clone()));

        session.start().expect("start should succeed");
        session.stop("Hello, type-less!").expect("stop should succeed");

        assert_eq!(
            *pasted.lock().expect("sink poisoned"),
            vec!["Hello, type-less!".to_string()]
        );
    }

    #[test]
    fn start_while_active_returns_already_active() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(fake_audio(), fake_paste(pasted));

        session.start().expect("first start should succeed");
        let second = session.start();

        assert_eq!(second, Err(SessionError::AlreadyActive));
    }

    #[test]
    fn stop_without_start_returns_not_active() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(fake_audio(), fake_paste(pasted.clone()));

        let result = session.stop("Hello, type-less!");

        assert_eq!(result, Err(SessionError::NotActive));
        assert!(pasted.lock().expect("sink poisoned").is_empty());
    }

    #[test]
    fn start_audio_failure_keeps_session_idle() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(
            || Err(SessionError::Audio("device busy".into())),
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
            |_text: &str| Err(SessionError::Paste("clipboard locked".into())),
        );

        session.start().expect("start should succeed");
        let stop_result = session.stop("Hello, type-less!");
        assert_eq!(stop_result, Err(SessionError::Paste("clipboard locked".into())));

        session
            .start()
            .expect("session must be Idle after paste failure so the next start succeeds");
    }

    #[test]
    fn cancel_returns_not_implemented_pending_slice_6() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(fake_audio(), fake_paste(pasted));

        let result = session.cancel();

        assert_eq!(result, Err(SessionError::NotImplemented));
    }

    #[test]
    fn start_again_after_stop_is_allowed() {
        let pasted: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let session = Session::new(fake_audio(), fake_paste(pasted.clone()));

        session.start().expect("first start");
        session.stop("first").expect("first stop");
        session.start().expect("second start should succeed");
        session.stop("second").expect("second stop");

        assert_eq!(
            *pasted.lock().expect("sink poisoned"),
            vec!["first".to_string(), "second".to_string()]
        );
    }
}
