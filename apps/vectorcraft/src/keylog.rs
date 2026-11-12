//! LOCAL DEBUG ONLY (not for upstream): logs the keyboard messages Windows sends the window and
//! the keyboard events egui then gets, to find where text typed with a keyboard-hook tool (NHM
//! Writer's legacy mode) is lost. Off unless the `VC_KEYLOG` environment variable is set; the log
//! goes to `%USERPROFILE%\vc_keylog.txt`.

#![cfg_attr(not(windows), allow(dead_code))]

use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

struct Log {
    file: std::fs::File,
    start: Instant,
}

fn log() -> Option<&'static Mutex<Log>> {
    static LOG: OnceLock<Option<Mutex<Log>>> = OnceLock::new();
    LOG.get_or_init(|| {
        std::env::var_os("VC_KEYLOG")?;
        let dir = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("vc_keylog.txt")).ok()?;
        Some(Mutex::new(Log { file, start: Instant::now() }))
    })
    .as_ref()
}

fn write(line: &str) {
    let Some(m) = log() else { return };
    if let Ok(mut l) = m.lock() {
        let ms = l.start.elapsed().as_millis();
        // Debug logging: a failed write just loses the line.
        let _ = writeln!(l.file, "{ms:>8} {line}");
    }
}

/// A character's code point and, when printable, the character.
fn describe(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let shown = if c.is_control() { ' ' } else { c };
        let _ = write!(out, "U+{:04X}'{shown}' ", u32::from(c));
    }
    out
}

/// A Windows keyboard message (`WM_KEYDOWN`..`WM_SYSCHAR`, plus `WM_IME_CHAR` and `WM_UNICHAR`).
pub fn msg(message: u32, wparam: usize, lparam: isize) {
    if log().is_none() {
        return;
    }
    let name = match message {
        0x0100 => "WM_KEYDOWN",
        0x0101 => "WM_KEYUP",
        0x0102 => "WM_CHAR",
        0x0103 => "WM_DEADCHAR",
        0x0104 => "WM_SYSKEYDOWN",
        0x0105 => "WM_SYSKEYUP",
        0x0106 => "WM_SYSCHAR",
        0x0109 => "WM_UNICHAR",
        0x0286 => "WM_IME_CHAR",
        _ => return,
    };
    let extra = if matches!(message, 0x0102 | 0x0106 | 0x0109 | 0x0286) {
        char::from_u32((wparam & 0xffff) as u32).map(|c| describe(&c.to_string())).unwrap_or_default()
    } else {
        String::new()
    };
    write(&format!("MSG  {name:<14} w=0x{wparam:04X} l=0x{lparam:08X} {extra}"));
}

/// The keyboard and text events egui gets this frame.
pub fn events(events: &[egui::Event]) {
    if log().is_none() {
        return;
    }
    for e in events {
        let line = match e {
            egui::Event::Text(t) => format!("EGUI Text {}", describe(t)),
            egui::Event::Key { key, pressed, modifiers, .. } => {
                format!("EGUI Key {key:?} pressed={pressed} ctrl={} alt={} shift={}", modifiers.ctrl, modifiers.alt, modifiers.shift)
            }
            egui::Event::Ime(ime) => format!("EGUI Ime {ime:?}"),
            egui::Event::Paste(t) => format!("EGUI Paste {}", describe(t)),
            _ => continue,
        };
        write(&line);
    }
}
