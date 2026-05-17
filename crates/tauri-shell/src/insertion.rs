use std::fmt;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;

const CLIPBOARD_RESTORE_DELAY_MS: u64 = 200;

#[derive(Debug)]
pub enum InsertionError {
    Clipboard(String),
    Keystroke(String),
}

impl fmt::Display for InsertionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clipboard(msg) => write!(f, "clipboard error: {msg}"),
            Self::Keystroke(msg) => write!(f, "keystroke synthesis error: {msg}"),
        }
    }
}

impl std::error::Error for InsertionError {}

pub fn paste(text: &str) -> Result<(), InsertionError> {
    let mut clipboard =
        Clipboard::new().map_err(|err| InsertionError::Clipboard(err.to_string()))?;
    let snapshot = clipboard.get_text().ok();
    write_hidden(&mut clipboard, text)?;
    keystroke::paste()?;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(CLIPBOARD_RESTORE_DELAY_MS));
        let Ok(mut clipboard) = Clipboard::new() else {
            return;
        };
        match snapshot {
            Some(prior) => {
                let _ = write_hidden(&mut clipboard, &prior);
            }
            None => {
                let _ = clipboard.clear();
            }
        }
    });
    Ok(())
}

fn write_hidden(clipboard: &mut Clipboard, text: &str) -> Result<(), InsertionError> {
    #[cfg(target_os = "macos")]
    {
        use arboard::SetExtApple;
        clipboard
            .set()
            .exclude_from_history()
            .text(text)
            .map_err(|err| InsertionError::Clipboard(err.to_string()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        clipboard
            .set_text(text)
            .map_err(|err| InsertionError::Clipboard(err.to_string()))
    }
}

#[cfg(target_os = "macos")]
mod keystroke {
    use super::InsertionError;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    const KEY_V: u16 = 9;

    pub fn paste() -> Result<(), InsertionError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| InsertionError::Keystroke("failed to create event source".into()))?;
        let down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
            .map_err(|_| InsertionError::Keystroke("failed to create keydown event".into()))?;
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        down.post(CGEventTapLocation::HID);
        let up = CGEvent::new_keyboard_event(source, KEY_V, false)
            .map_err(|_| InsertionError::Keystroke("failed to create keyup event".into()))?;
        up.set_flags(CGEventFlags::CGEventFlagCommand);
        up.post(CGEventTapLocation::HID);
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod keystroke {
    use super::InsertionError;

    pub fn paste() -> Result<(), InsertionError> {
        Err(InsertionError::Keystroke(
            "keystroke synthesis not implemented on this platform".into(),
        ))
    }
}
