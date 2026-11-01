#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod clipboard;
mod config;
mod focus;
mod strip;
mod text;
mod translate;
mod worker;

use eframe::egui;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// The title bar and taskbar icon, built into the binary.
fn window_icon() -> egui::IconData {
    let image = xcap::image::load_from_memory(include_bytes!("../icon.png"))
        .expect("icon.png is a valid PNG")
        .to_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData { rgba: image.into_raw(), width, height }
}

fn decode_file(path: &str) {
    let img = match xcap::image::open(path) {
        Ok(img) => img.to_rgba8(),
        Err(e) => return println!("Could not open {path}: {e}"),
    };
    match strip::find(&img) {
        None => println!("No pixel strip in {path}"),
        Some(lock) => println!("Strip at {},{} ({} px blocks): {:?}", lock.x, lock.y, lock.block, strip::decode(&img, lock)),
    }
}

/// egui ships without Korean or Chinese glyphs; borrow the system fonts that
/// exist, Korean first so Hangul takes its shapes from a Korean font.
fn add_cjk_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
    ];
    let found: Vec<Vec<u8>> = CANDIDATES.iter().filter_map(|p| std::fs::read(p).ok()).collect();
    if found.is_empty() {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    for (i, bytes) in found.into_iter().enumerate() {
        let name = format!("cjk{i}");
        fonts.font_data.insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }
    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result {
    // `translate-companion --decode shot.png` reads a strip from a screenshot.
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 3 && args[1] == "--decode" {
        decode_file(&args[2]);
        return Ok(());
    }
    let config = Arc::new(Mutex::new(config::load()));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Translate companion")
            .with_inner_size([app::MIN_WIDTH, app::MIN_HEIGHT])
            .with_min_inner_size([app::MIN_WIDTH, app::MIN_HEIGHT])
            .with_icon(window_icon())
            // The renderer only gets an alpha channel when the root window asks
            // for one; without it the overlay is drawn solid black.
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "Translate companion",
        options,
        Box::new(move |cc| {
            add_cjk_font(&cc.egui_ctx);
            let (ui_tx, ui_rx) = mpsc::channel();
            let ui = worker::UiSender::new(ui_tx, cc.egui_ctx.clone());
            let (clip_tx, clip_rx) = mpsc::channel();
            let (strip_tx, strip_rx) = mpsc::channel();
            let (interactive_tx, interactive_rx) = mpsc::channel();

            worker::spawn(config.clone(), strip_rx, ui.clone(), clip_tx.clone());
            worker::spawn(config.clone(), interactive_rx, ui.clone(), clip_tx);
            strip::spawn(strip_tx, ui.clone());
            focus::spawn(ui.clone());
            clipboard::spawn(clip_rx, interactive_tx.clone(), ui);

            Ok(Box::new(app::TranslateApp::new(config, ui_rx, interactive_tx)))
        }),
    )
}
