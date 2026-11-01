//! Reads the addon's pixel strip off the screen. The layout must match
//! addon/Translate/Strip.lua: 64 x 8 blocks, 3 sync blocks, then
//! [seq][len hi][len lo][text...][fletcher16 hi][fletcher16 lo] packed
//! 3 bytes into 4 six-bit blocks.

use crate::translate::Direction;
use crate::worker::{Job, UiEvent, UiSender};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};
use xcap::image::RgbaImage;
use xcap::Monitor;

pub const COLS: usize = 64;
pub const ROWS: usize = 8;
const SYNC: [u8; 3] = [51, 15, 60];

/// Where the strip sits in an image, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lock {
    pub x: u32,
    pub y: u32,
    pub block: u32,
}

#[derive(Debug, PartialEq)]
pub struct Frame {
    pub seq: u8,
    pub checksum: u16,
    pub text: String,
}

#[derive(Debug, PartialEq)]
pub enum Decoded {
    Frame(Frame),
    /// Sync blocks present but the data did not check out (mid-repaint).
    Corrupt,
    /// No strip at this spot.
    Missing,
}

/// Maps a channel value to one of the four levels the addon draws (0, 85, 170, 255).
fn level(v: f32) -> u8 {
    ((v + 42.0) / 85.0).min(3.0) as u8
}

/// The pixel's channels, or None outside the image.
fn pixel(img: &RgbaImage, x: u32, y: u32) -> Option<[f32; 3]> {
    if x >= img.width() || y >= img.height() {
        return None;
    }
    let p = img.get_pixel(x, y).0;
    Some([p[0] as f32, p[1] as f32, p[2] as f32])
}

/// Reads a sync color using only on/off per channel, which survives a display
/// color profile that shifts the four levels into each other.
fn sync_symbol(img: &RgbaImage, x: u32, y: u32) -> Option<u8> {
    let [r, g, b] = pixel(img, x, y)?;
    let on = |v: f32| if v >= 128.0 { 3 } else { 0 };
    Some(on(r) * 16 + on(g) * 4 + on(b))
}

fn to_linear(v: f32) -> f32 {
    (v / 255.0).powi(2)
}

fn invert(m: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let cofactor = |r: usize, c: usize| {
        let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        m[r1][c1] * m[r2][c2] - m[r1][c2] * m[r2][c1]
    };
    let det = m[0][0] * cofactor(0, 0) + m[0][1] * cofactor(0, 1) + m[0][2] * cofactor(0, 2);
    if det.abs() < 1e-3 {
        return None;
    }
    let mut inverse = [[0.0; 3]; 3];
    for (r, row) in inverse.iter_mut().enumerate() {
        for (c, value) in row.iter_mut().enumerate() {
            *value = cofactor(c, r) / det;
        }
    }
    Some(inverse)
}

/// Screen capture returns pixels in the display's color space, so a wide-gamut
/// monitor mixes the channels: on one Dell, magenta (255, 0, 255) reads back as
/// (233, 54, 248). The sync blocks are known colors, so they reveal how each
/// channel was mixed, and undoing that mix recovers the colors the addon drew.
fn calibration(sync: [[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let [magenta, cyan, yellow] = sync.map(|p| p.map(to_linear));
    let mut primaries = [[0.0; 3]; 3];
    for (channel, row) in primaries.iter_mut().enumerate() {
        let (m, c, y) = (magenta[channel], cyan[channel], yellow[channel]);
        *row = [(m + y - c) / 2.0, (c + y - m) / 2.0, (m + c - y) / 2.0];
    }
    invert(primaries)
}

fn symbol(inverse: &[[f32; 3]; 3], pixel: [f32; 3]) -> u8 {
    let linear = pixel.map(to_linear);
    let channel = |row: [f32; 3]| {
        let mixed = row[0] * linear[0] + row[1] * linear[1] + row[2] * linear[2];
        level(mixed.max(0.0).sqrt() * 255.0)
    };
    channel(inverse[0]) * 16 + channel(inverse[1]) * 4 + channel(inverse[2])
}

fn fletcher16(bytes: &[u8]) -> u16 {
    let (mut a, mut b) = (0u16, 0u16);
    for &byte in bytes {
        a = (a + byte as u16) % 255;
        b = (b + a) % 255;
    }
    b * 256 + a
}

pub fn decode(img: &RgbaImage, lock: Lock) -> Decoded {
    let center = |i: usize| {
        (
            lock.x + (i % COLS) as u32 * lock.block + lock.block / 2,
            lock.y + (i / COLS) as u32 * lock.block + lock.block / 2,
        )
    };
    let mut sync = [[0.0; 3]; 3];
    for (i, expected) in SYNC.iter().enumerate() {
        let (cx, cy) = center(i);
        if sync_symbol(img, cx, cy) != Some(*expected) {
            return Decoded::Missing;
        }
        sync[i] = pixel(img, cx, cy).expect("sync_symbol found the pixel");
    }
    let Some(inverse) = calibration(sync) else {
        return Decoded::Missing;
    };
    let mut symbols = Vec::with_capacity(COLS * ROWS);
    for i in 0..COLS * ROWS {
        let (cx, cy) = center(i);
        match pixel(img, cx, cy) {
            Some(p) => symbols.push(symbol(&inverse, p)),
            None => return Decoded::Missing,
        }
    }
    let mut bytes = Vec::with_capacity((symbols.len() - 3) / 4 * 3);
    for chunk in symbols[3..].chunks_exact(4) {
        let v = (chunk[0] as u32) << 18 | (chunk[1] as u32) << 12 | (chunk[2] as u32) << 6 | chunk[3] as u32;
        bytes.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    let len = (bytes[1] as usize) << 8 | bytes[2] as usize;
    if 3 + len + 2 > bytes.len() {
        return Decoded::Corrupt;
    }
    let checksum = (bytes[3 + len] as u16) << 8 | bytes[4 + len] as u16;
    if fletcher16(&bytes[..3 + len]) != checksum {
        return Decoded::Corrupt;
    }
    match String::from_utf8(bytes[3..3 + len].to_vec()) {
        Ok(text) => Decoded::Frame(Frame { seq: bytes[0], checksum, text }),
        Err(_) => Decoded::Corrupt,
    }
}

/// Scans a whole image for the sync blocks and returns where the strip is.
pub fn find(img: &RgbaImage) -> Option<Lock> {
    let is = |x: u32, y: u32, s: u8| sync_symbol(img, x, y) == Some(s);
    for y in 0..img.height() {
        let mut x = 0;
        while x < img.width() {
            if !is(x, y, SYNC[0]) {
                x += 1;
                continue;
            }
            let start = x;
            while is(x, y, SYNC[0]) {
                x += 1;
            }
            let block = x - start;
            let cyan = (x..).take_while(|&cx| is(cx, y, SYNC[1])).count() as u32;
            let yellow = (x + cyan..).take_while(|&cx| is(cx, y, SYNC[2])).count() as u32;
            if cyan == 0 || yellow == 0 {
                continue;
            }
            // Averaging the three runs tolerates a blended edge pixel.
            let block = ((block + cyan + yellow) as f32 / 3.0).round() as u32;
            let mut top = y;
            while top > 0 && is(start, top - 1, SYNC[0]) {
                top -= 1;
            }
            let lock = Lock { x: start, y: top, block: block.max(1) };
            if decode(img, lock) != Decoded::Missing {
                return Some(lock);
            }
        }
    }
    None
}

/// The payload is "<id>\t<speaker>\t<text>".
pub fn parse_payload(text: &str) -> Option<(String, String)> {
    let mut parts = text.splitn(3, '\t');
    let _id = parts.next()?;
    let speaker = parts.next()?.to_string();
    let message = parts.next()?.to_string();
    Some((speaker, message))
}

/// The addon marks text the player typed as `/tr <text>` with "@reply:<lang>"
/// in the speaker field, so it is translated out of Korean instead of into it.
pub fn reply_target(speaker: &str) -> Option<Direction> {
    match speaker.strip_prefix("@reply:")? {
        "zh" => Some(Direction::ToChinese),
        _ => Some(Direction::ToEnglish),
    }
}

/// Polls the screen: a full scan about once a second until the strip turns
/// up, then only the strip's own rectangle several times a second.
pub fn spawn(jobs: Sender<Job>, ui: UiSender) {
    thread::spawn(move || {
        let mut locked: Option<(usize, Lock)> = None;
        let mut last_seen: Option<(u8, u16)> = None;
        let mut last_good = Instant::now();
        let mut said_searching = false;

        loop {
            let monitors = match Monitor::all() {
                Ok(m) => m,
                Err(e) => {
                    ui.send(UiEvent::Status(format!("Screen capture unavailable: {e}")));
                    thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };

            let Some((index, lock)) = locked else {
                if !said_searching {
                    ui.send(UiEvent::Strip("Looking for the pixel strip. In game, type /tr test.".into()));
                    said_searching = true;
                }
                for (index, monitor) in monitors.iter().enumerate() {
                    let Ok(img) = monitor.capture_image() else { continue };
                    if let Some(lock) = find(&img) {
                        ui.send(UiEvent::Strip(format!(
                            "Reading the pixel strip on screen {} at {},{}",
                            index + 1,
                            lock.x,
                            lock.y
                        )));
                        locked = Some((index, lock));
                        last_good = Instant::now();
                        said_searching = false;
                        break;
                    }
                }
                if locked.is_none() {
                    thread::sleep(Duration::from_millis(1000));
                }
                continue;
            };

            let Some(monitor) = monitors.get(index) else {
                locked = None;
                continue;
            };
            let (w, h) = (COLS as u32 * lock.block, ROWS as u32 * lock.block);
            let decoded = match monitor.capture_region(lock.x, lock.y, w, h) {
                Ok(img) => decode(&img, Lock { x: 0, y: 0, block: lock.block }),
                Err(_) => Decoded::Missing,
            };
            match decoded {
                Decoded::Frame(frame) => {
                    last_good = Instant::now();
                    if last_seen != Some((frame.seq, frame.checksum)) {
                        // The first frame after locking may be an old line; it is
                        // still new to the overlay, so it is shown too.
                        last_seen = Some((frame.seq, frame.checksum));
                        if let Some((speaker, text)) = parse_payload(&frame.text) {
                            let job = match reply_target(&speaker) {
                                Some(to) => Job::Reply { text, to },
                                None => Job::Incoming { speaker, text },
                            };
                            let _ = jobs.send(job);
                        }
                    }
                }
                Decoded::Corrupt => {}
                Decoded::Missing => {
                    // Hidden UI, a loading screen, or the window moved.
                    if last_good.elapsed() > Duration::from_secs(3) {
                        locked = None;
                    }
                }
            }
            thread::sleep(Duration::from_millis(80));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcap::image::Rgba;

    /// Rust copy of Strip.lua's encoder, for tests only.
    fn encode(seq: u8, text: &str) -> Vec<u8> {
        let mut bytes = vec![seq, (text.len() >> 8) as u8, text.len() as u8];
        bytes.extend_from_slice(text.as_bytes());
        let sum = fletcher16(&bytes);
        bytes.extend_from_slice(&[(sum >> 8) as u8, sum as u8]);
        let mut symbols = SYNC.to_vec();
        for chunk in bytes.chunks(3) {
            let v = (chunk[0] as u32) << 16
                | (*chunk.get(1).unwrap_or(&0) as u32) << 8
                | *chunk.get(2).unwrap_or(&0) as u32;
            symbols.extend_from_slice(&[(v >> 18) as u8 & 63, (v >> 12) as u8 & 63, (v >> 6) as u8 & 63, v as u8 & 63]);
        }
        symbols.resize(COLS * ROWS, 0);
        symbols
    }

    fn paint(symbols: &[u8], at: (u32, u32), block: u32) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(800, 200, Rgba([30, 30, 40, 255]));
        for (i, s) in symbols.iter().enumerate() {
            let (r, g, b) = ((s >> 4) & 3, (s >> 2) & 3, s & 3);
            for dy in 0..block {
                for dx in 0..block {
                    let x = at.0 + (i % COLS) as u32 * block + dx;
                    let y = at.1 + (i / COLS) as u32 * block + dy;
                    // A small color shift, as a display pipeline might add.
                    img.put_pixel(x, y, Rgba([(r * 85).saturating_add(6), (g * 85).saturating_sub(5), b * 85, 255]));
                }
            }
        }
        img
    }

    #[test]
    fn finds_and_decodes() {
        let text = "7\t[Party] Lili\t副本缺T 速来";
        let img = paint(&encode(42, text), (37, 11), 4);
        let lock = find(&img).expect("strip found");
        assert_eq!(lock, Lock { x: 37, y: 11, block: 4 });
        match decode(&img, lock) {
            Decoded::Frame(f) => {
                assert_eq!(f.seq, 42);
                assert_eq!(f.text, text);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(parse_payload(text), Some(("[Party] Lili".into(), "副本缺T 速来".into())));
    }

    #[test]
    fn reply_marker_picks_the_direction() {
        assert_eq!(reply_target("@reply:en"), Some(Direction::ToEnglish));
        assert_eq!(reply_target("@reply:zh"), Some(Direction::ToChinese));
        assert_eq!(reply_target("[Say] Bob"), None);
        assert_eq!(parse_payload("0\t@reply:zh\t멈춰"), Some(("@reply:zh".into(), "멈춰".into())));
    }

    #[test]
    fn rejects_damaged_frame() {
        let mut symbols = encode(1, "1\tA\t你好");
        symbols[10] ^= 1;
        let img = paint(&symbols, (0, 0), 3);
        assert_eq!(decode(&img, Lock { x: 0, y: 0, block: 3 }), Decoded::Corrupt);
    }

    /// Applies the mixing measured on a Dell AW3821DW to a frame.
    fn through_wide_gamut_display(img: &mut RgbaImage) {
        const MIX: [[f32; 3]; 3] = [[0.8375, 0.1625, 0.0], [0.046, 0.954, 0.0], [0.0125, 0.05, 0.933]];
        for p in img.pixels_mut() {
            let linear = [p.0[0], p.0[1], p.0[2]].map(|v| (v as f32 / 255.0).powf(2.2));
            for (channel, row) in MIX.iter().enumerate() {
                let mixed: f32 = row.iter().zip(linear).map(|(a, b)| a * b).sum();
                p.0[channel] = (mixed.powf(0.5) * 255.0).round() as u8;
            }
        }
    }

    #[test]
    fn decodes_through_a_display_color_profile() {
        let text = "1\t[Say] Bob\tLFM ICC 25 need MT and heals PST 파티";
        let mut img = paint(&encode(7, text), (10, 48), 4);
        through_wide_gamut_display(&mut img);
        assert!(img.get_pixel(10, 48).0[1] > 42, "the mix must push magenta's green past the first level");
        let lock = find(&img).expect("strip found");
        match decode(&img, lock) {
            Decoded::Frame(frame) => assert_eq!(frame.text, text),
            other => panic!("{other:?}"),
        }
    }
}
