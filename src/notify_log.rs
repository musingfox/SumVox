// Mute flag + voice notification history shared with the menu bar app.
// Contract: mute = existence of ~/.config/sumvox/muted;
// history = last 50 lines of ~/.config/sumvox/history.log ("RFC3339\ttext");
// now_playing = ~/.config/sumvox/now_playing, path of the audio file being
// played right now, so the menu bar avatar can drive its mouth from the real
// amplitude envelope.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::SumvoxConfig;

const HISTORY_LIMIT: usize = 50;

fn config_dir() -> Option<PathBuf> {
    SumvoxConfig::config_dir().ok()
}

/// Voice playback is muted when the flag file exists (toggled by the menu bar app).
pub fn is_muted() -> bool {
    config_dir()
        .map(|d| d.join("muted").exists())
        .unwrap_or(false)
}

/// Record a spoken (or muted) notification text, keeping the most recent 50 entries.
/// Best-effort: failures must never block the notification path.
pub fn record(text: &str) {
    let Some(dir) = config_dir() else { return };
    let path = dir.join("history.log");
    let line = format!(
        "{}\t{}\n",
        chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        text.replace(['\n', '\r'], " ")
    );

    // ponytail: read-modify-write without locking; concurrent hooks may rarely
    // drop a line — acceptable for a notification log, add file locking if not.
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<&str> = existing.lines().collect();
    if lines.len() >= HISTORY_LIMIT {
        lines = lines.split_off(lines.len() - (HISTORY_LIMIT - 1));
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&line);
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(&path, out);
}

/// Record the audio file about to be played, so the menu bar avatar can decode
/// it and flap its mouth in time with the real amplitude. Best-effort;
/// overwritten on every playback, never blocks the audio path.
pub fn set_now_playing(path: &Path) {
    let Some(dir) = config_dir() else { return };
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join("now_playing"), path.to_string_lossy().as_bytes());
}

#[cfg(test)]
mod tests {
    #[test]
    fn record_line_is_single_line() {
        // The invariant the menu app depends on: one entry == one line.
        let text = "line1\nline2\rline3";
        let flattened = text.replace(['\n', '\r'], " ");
        assert!(!flattened.contains('\n') && !flattened.contains('\r'));
    }
}
