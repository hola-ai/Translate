use crate::config::Config;
use crate::translate::{translate, Direction};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

pub const TEST_LINE: &str = "LFM ICC 25 need MT and heals PST";

pub enum Job {
    /// An English line read from the addon's pixel strip.
    Incoming { speaker: String, text: String },
    /// A line copied from the addon window as "TR#<id> <text>".
    Tagged { id: u64, text: String },
    /// Korean the player typed, in the companion or as `/tr <text>` in game,
    /// to send as English or Chinese.
    Reply { text: String, to: Direction },
    /// The Test model button: a fixed line, never cached.
    Test,
}

pub enum UiEvent {
    Line { speaker: String, original: String, translated: String },
    PasteReady { id: u64, translated: String },
    ReplyReady { source: String, translated: String },
    TestResult { result: Result<String, String>, seconds: f32 },
    /// Pixel strip state: searching, or where it was found.
    Strip(String),
    /// Whether WoW has focus; None when the focused window cannot be read.
    WowFocused(Option<bool>),
    Status(String),
}

/// Sends to the UI and wakes it so the event shows without mouse movement.
#[derive(Clone)]
pub struct UiSender {
    tx: Sender<UiEvent>,
    ctx: eframe::egui::Context,
}

impl UiSender {
    pub fn new(tx: Sender<UiEvent>, ctx: eframe::egui::Context) -> Self {
        Self { tx, ctx }
    }

    pub fn send(&self, event: UiEvent) {
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}

/// One thread per queue keeps the chat log from delaying click-to-translate.
pub fn spawn(config: Arc<Mutex<Config>>, jobs: Receiver<Job>, ui: UiSender, clipboard: Sender<String>) {
    thread::spawn(move || {
        let mut cache: HashMap<String, String> = HashMap::new();
        for job in jobs {
            let config = config.lock().unwrap().clone();
            if let Job::Test = job {
                let started = Instant::now();
                let result = translate(&config, TEST_LINE, Direction::ToKorean);
                ui.send(UiEvent::TestResult { result, seconds: started.elapsed().as_secs_f32() });
                continue;
            }
            let (text, direction) = match &job {
                Job::Incoming { text, .. } | Job::Tagged { text, .. } => (text.clone(), Direction::ToKorean),
                Job::Reply { text, to } => (text.clone(), *to),
                Job::Test => unreachable!("handled above"),
            };
            let key = format!("{direction:?}|{text}");
            let result = match cache.get(&key) {
                Some(hit) => Ok(hit.clone()),
                None => translate(&config, &text, direction),
            };
            let translated = match result {
                Ok(t) => {
                    if cache.len() > 2000 {
                        cache.clear();
                    }
                    cache.insert(key, t.clone());
                    t
                }
                Err(e) => {
                    ui.send(UiEvent::Status(format!("Translation failed: {e}")));
                    continue;
                }
            };
            match job {
                Job::Incoming { speaker, text } => {
                    ui.send(UiEvent::Line { speaker, original: text, translated })
                }
                Job::Tagged { id, .. } => {
                    let _ = clipboard.send(format!("TR#{id} {translated}"));
                    ui.send(UiEvent::PasteReady { id, translated });
                }
                Job::Reply { text, .. } => {
                    let _ = clipboard.send(translated.clone());
                    ui.send(UiEvent::ReplyReady { source: text, translated });
                }
                Job::Test => {}
            }
        }
    });
}
