use crate::text::{has_hangul, parse_tagged};
use crate::worker::{Job, UiEvent, UiSender};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

/// Owns the system clipboard. Watches for "TR#<id> <English>" copied from the
/// addon window and writes replies that the player pastes back into the game.
pub fn spawn(set_requests: Receiver<String>, jobs: Sender<Job>, ui: UiSender) {
    thread::spawn(move || {
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                ui.send(UiEvent::Status(format!("Clipboard unavailable: {e}")));
                return;
            }
        };
        let mut last = clipboard.get_text().unwrap_or_default();

        loop {
            match set_requests.recv_timeout(Duration::from_millis(300)) {
                Ok(text) => {
                    if let Err(e) = clipboard.set_text(text.clone()) {
                        ui.send(UiEvent::Status(format!("Could not write the clipboard: {e}")));
                    }
                    last = text;
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }

            let Ok(current) = clipboard.get_text() else { continue };
            if current == last {
                continue;
            }
            last = current.clone();
            if let Some((id, text)) = parse_tagged(&current) {
                if !has_hangul(text) {
                    let _ = jobs.send(Job::Tagged { id, text: text.to_string() });
                }
            }
        }
    });
}
