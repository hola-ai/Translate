use crate::config::{self, Config};
use crate::translate::Direction;
use crate::worker::{Job, UiEvent};
use eframe::egui::{self, Color32, RichText, ViewportBuilder, ViewportCommand, ViewportId};
use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct OverlayLine {
    speaker: String,
    original: String,
    translated: String,
}

pub struct TranslateApp {
    config: Arc<Mutex<Config>>,
    draft: Config,
    events: Receiver<UiEvent>,
    interactive: Sender<Job>,
    lines: VecDeque<OverlayLine>,
    status: String,
    notice: String,
    reply: String,
    locked: bool,
    overlay_start: egui::Pos2,
    overlay_rect: Option<egui::Rect>,
    test_result: String,
    strip_status: String,
    fitted_height: f32,
    wow_focused: Option<bool>,
    overlay_hidden: bool,
    /// When the whole font texture was last sent; see `refresh_font_texture`.
    font_texture_sent: Instant,
    /// New overlay text arrived, so the font texture is re-sent after this frame.
    font_texture_stale: bool,
}

/// Smallest companion window that shows every control with Settings closed.
pub const MIN_WIDTH: f32 = 560.0;
pub const MIN_HEIGHT: f32 = 150.0;

const OFF_SCREEN: f32 = -32000.0;

/// How often the whole font texture is re-sent even without new text.
const FONT_TEXTURE_REFRESH: Duration = Duration::from_secs(5);

fn overlay_id() -> ViewportId {
    ViewportId::from_hash_of("overlay")
}

impl TranslateApp {
    pub fn new(config: Arc<Mutex<Config>>, events: Receiver<UiEvent>, interactive: Sender<Job>) -> Self {
        let draft = config.lock().unwrap().clone();
        let overlay_start = egui::pos2(draft.overlay_x, draft.overlay_y);
        Self {
            config,
            draft,
            events,
            interactive,
            lines: VecDeque::new(),
            status: "Ready".into(),
            notice: String::new(),
            reply: String::new(),
            locked: true,
            overlay_start,
            overlay_rect: None,
            fitted_height: 0.0,
            wow_focused: None,
            overlay_hidden: false,
            test_result: String::new(),
            strip_status: "Starting the pixel strip reader".into(),
            font_texture_sent: Instant::now(),
            font_texture_stale: false,
        }
    }

    fn push_line(&mut self, line: OverlayLine) {
        self.lines.push_back(line);
        while self.lines.len() > self.draft.max_lines.max(1) {
            self.lines.pop_front();
        }
        self.font_texture_stale = true;
    }

    /// Sends the whole font atlas to the GPU again, after new overlay text and
    /// every few seconds anyway.
    ///
    /// egui normally uploads only the glyphs added since the last frame. On
    /// macOS that stopped working once after some hours: every glyph
    /// rasterized from then on drew as fragments of other glyphs in both
    /// windows until the app was restarted. Atlas growth, light/dark theme
    /// switches, window resizes and overlay hiding did not reproduce it, so
    /// rather than guess at the trigger the companion re-sends the complete
    /// atlas, which repairs the texture whatever went wrong. The copy is a few
    /// megabytes, so it is cheap at this rate.
    fn refresh_font_texture(&mut self, ctx: &egui::Context) {
        let due = self.font_texture_stale || self.font_texture_sent.elapsed() >= FONT_TEXTURE_REFRESH;
        if !due {
            return;
        }
        let image = ctx.fonts(|fonts| fonts.image());
        let delta = egui::epaint::ImageDelta::full(image, egui::epaint::TextureAtlas::texture_options());
        ctx.tex_manager().write().set(egui::TextureId::default(), delta);
        if self.font_texture_stale {
            // The overlay was painted before this upload; show it again with the repaired texture.
            ctx.request_repaint();
        }
        self.font_texture_sent = Instant::now();
        self.font_texture_stale = false;
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                UiEvent::Line { speaker, original, translated } => {
                    self.push_line(OverlayLine { speaker, original, translated });
                }
                UiEvent::PasteReady { id, translated } => {
                    self.notice = format!("#{id} ready: paste it into the addon window");
                    self.push_line(OverlayLine { speaker: format!("#{id}"), original: String::new(), translated });
                }
                UiEvent::ReplyReady { source, translated } => {
                    self.notice = format!("Copied \"{translated}\" for \"{source}\". Paste it into WoW chat.");
                    // The player is in the game, so the overlay says when the clipboard is ready.
                    self.push_line(OverlayLine { speaker: "복사됨".into(), original: source, translated });
                }
                UiEvent::TestResult { result, seconds } => {
                    self.test_result = match result {
                        Ok(korean) if !crate::text::has_hangul(&korean) => {
                            format!("The model answered but did not translate ({seconds:.1} s): {korean}")
                        }
                        Ok(korean) => format!("Model works ({seconds:.1} s): {} -> {korean}", crate::worker::TEST_LINE),
                        Err(e) => format!("Model test failed: {e}"),
                    };
                }
                UiEvent::Strip(text) => self.strip_status = text,
                UiEvent::WowFocused(focused) => self.wow_focused = focused,
                UiEvent::Status(text) => self.status = text,
            }
        }
    }

    fn save(&mut self) {
        if let Some(rect) = self.overlay_rect {
            self.draft.overlay_x = rect.min.x;
            self.draft.overlay_y = rect.min.y;
        }
        *self.config.lock().unwrap() = self.draft.clone();
        self.notice = match config::save(&self.draft) {
            Ok(()) => format!("Saved {}", config::path().display()),
            Err(e) => format!("Could not save settings: {e}"),
        };
    }

    fn control_panel(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        let panel = egui::CentralPanel::default().show(root, |ui| {
            // Scrolls only when the screen is too short for the whole window.
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| self.control_contents(ui))
                .content_size
                .y
        });
        self.fit_window(&ctx, panel.inner);
    }

    /// Grows or shrinks the window to its contents, e.g. when Settings opens.
    fn fit_window(&mut self, ctx: &egui::Context, content_height: f32) {
        // Panel margins plus a few pixels so the scrollbar does not appear at an exact fit.
        let margin = 2.0 * ctx.global_style().spacing.window_margin.topf().max(8.0) + 6.0;
        let max = ctx
            .input(|i| i.viewport().monitor_size)
            .map_or(f32::INFINITY, |m| m.y * 0.9);
        let wanted = (content_height + margin).clamp(MIN_HEIGHT, max.max(MIN_HEIGHT));
        if (wanted - self.fitted_height).abs() < 1.0 {
            return;
        }
        self.fitted_height = wanted;
        let width = ctx.input(|i| i.viewport().inner_rect).map_or(MIN_WIDTH, |r| r.width());
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(width, wanted)));
    }

    fn control_contents(&mut self, ui: &mut egui::Ui) {
        {
            ui.heading("Translate companion");
            ui.label(&self.status);
            if !self.notice.is_empty() {
                ui.colored_label(Color32::from_rgb(120, 200, 255), &self.notice);
            }
            ui.label(&self.strip_status);
            ui.horizontal(|ui| {
                if ui.button("Test model").clicked() {
                    // Test what is typed in Settings, even before Save.
                    *self.config.lock().unwrap() = self.draft.clone();
                    let _ = self.interactive.send(Job::Test);
                    self.test_result = format!("Testing {} at {}", self.draft.model, self.draft.api_base);
                }
                if !self.test_result.is_empty() {
                    ui.label(&self.test_result);
                }
            });
            ui.separator();

            ui.label("Reply in Korean (Enter copies the English translation for pasting into WoW chat):");
            let edit = ui.add(egui::TextEdit::singleline(&mut self.reply).desired_width(f32::INFINITY));
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.reply.trim().is_empty() {
                let _ = self.interactive.send(Job::Reply { text: self.reply.trim().to_string(), to: Direction::ToEnglish });
                self.notice = "Translating your reply".into();
                self.reply.clear();
                edit.request_focus();
            }
            ui.separator();

            ui.horizontal(|ui| {
                let label = if self.locked { "Unlock overlay to move it" } else { "Lock overlay" };
                if ui.button(label).clicked() {
                    self.locked = !self.locked;
                    if self.locked {
                        self.save();
                    }
                }
                if ui.button("Clear overlay").clicked() {
                    self.lines.clear();
                }
                ui.checkbox(&mut self.draft.show_original, "Show English too");
                ui.checkbox(&mut self.draft.hide_when_unfocused, "Hide when WoW is in the background");
            });

            egui::CollapsingHeader::new("Settings").default_open(false).show(ui, |ui| {
                egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                    ui.label("API base URL");
                    ui.text_edit_singleline(&mut self.draft.api_base);
                    ui.end_row();
                    ui.label("API key");
                    ui.add(egui::TextEdit::singleline(&mut self.draft.api_key).password(true));
                    ui.end_row();
                    ui.label("Model");
                    ui.text_edit_singleline(&mut self.draft.model);
                    ui.end_row();
                    ui.label("Overlay width");
                    ui.add(egui::Slider::new(&mut self.draft.overlay_width, 200.0..=1600.0));
                    ui.end_row();
                    ui.label("Overlay height");
                    ui.add(egui::Slider::new(&mut self.draft.overlay_height, 80.0..=1000.0));
                    ui.end_row();
                    ui.label("Background opacity");
                    ui.add(egui::Slider::new(&mut self.draft.overlay_opacity, 0.0..=1.0));
                    ui.end_row();
                    ui.label("Text size");
                    ui.add(egui::Slider::new(&mut self.draft.font_size, 10.0..=32.0));
                    ui.end_row();
                    ui.label("Lines kept");
                    ui.add(egui::Slider::new(&mut self.draft.max_lines, 1..=50));
                    ui.end_row();
                });
                if ui.button("Save settings").clicked() {
                    self.save();
                }
            });
        }
    }

    fn overlay(&mut self, ctx: &egui::Context) {
        let builder = ViewportBuilder::default()
            .with_title("Translate overlay")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_mouse_passthrough(self.locked)
            .with_position(self.overlay_start)
            .with_inner_size([self.draft.overlay_width, self.draft.overlay_height]);

        let locked = self.locked;
        // Hiding parks the window off screen instead of closing or hiding it:
        // showing a window again can take focus from WoW, and Windows still
        // draws a border around an empty transparent window.
        let hidden = locked && self.draft.hide_when_unfocused && self.wow_focused == Some(false);
        if hidden != self.overlay_hidden {
            let pos = if hidden {
                egui::pos2(OFF_SCREEN, OFF_SCREEN)
            } else {
                self.overlay_rect.map_or(self.overlay_start, |r| r.min)
            };
            ctx.send_viewport_cmd_to(overlay_id(), ViewportCommand::OuterPosition(pos));
            self.overlay_hidden = hidden;
        }
        let draft = self.draft.clone();
        let lines = &self.lines;
        let mut rect = self.overlay_rect;

        ctx.show_viewport_immediate(overlay_id(), builder, |root, _class| {
            let ctx = root.ctx().clone();
            if hidden {
                return;
            }
            // Skip the parked position, which can linger for a frame after showing.
            rect = ctx.input(|i| i.viewport().outer_rect).filter(|r| r.min.x > OFF_SCREEN / 2.0).or(rect);
            let alpha = (draft.overlay_opacity * 255.0) as u8;
            let fill = if locked {
                Color32::from_black_alpha(alpha)
            } else {
                Color32::from_rgba_unmultiplied(20, 60, 110, alpha.max(140))
            };
            let frame = egui::Frame::NONE.fill(fill).inner_margin(8.0);
            egui::CentralPanel::default().frame(frame).show(root, |ui| {
                if !locked {
                    let drag = ui.interact(ui.max_rect(), ui.id().with("drag"), egui::Sense::drag());
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    ui.label(RichText::new("Drag to move. Lock it from the companion window.").color(Color32::WHITE));
                }
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for line in lines {
                        let mut text = RichText::new(&line.translated).size(draft.font_size).color(Color32::WHITE);
                        if line.speaker.is_empty() {
                            text = text.italics();
                        }
                        ui.horizontal_wrapped(|ui| {
                            if !line.speaker.is_empty() {
                                ui.label(
                                    RichText::new(format!("{}:", line.speaker))
                                        .size(draft.font_size)
                                        .color(Color32::from_rgb(255, 210, 120)),
                                );
                            }
                            ui.label(text);
                        });
                        if draft.show_original && !line.original.is_empty() {
                            ui.label(
                                RichText::new(&line.original)
                                    .size(draft.font_size * 0.85)
                                    .color(Color32::from_gray(170)),
                            );
                        }
                    }
                });
            });
        });
        self.overlay_rect = rect;
    }
}

impl eframe::App for TranslateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.drain_events();
        self.control_panel(ui);
        self.overlay(&ctx);
        // After both windows laid out their text, so this frame's new glyphs are included.
        self.refresh_font_texture(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}
