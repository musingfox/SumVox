// Platform audio playback.
//
// Every TTS provider renders (or downloads) audio and then plays it here — this
// module is the single playback choke point for the whole crate. On macOS the
// player is `afplay`; elsewhere we probe a list of common command-line players
// and use the first one installed, so the 0-100 volume knob and the menu-bar
// avatar hook keep working on Linux instead of being macOS-only.

use std::path::{Path, PathBuf};

/// Locate `name` on `PATH` without spawning it.
///
/// Returns the first `PATH` entry that contains `name` as a regular file with an
/// execute bit set. `name` must be a bare program name: anything containing a
/// path separator is rejected, because "is this program installed" is only a
/// meaningful question for `PATH` lookups. Non-absolute `PATH` entries are
/// skipped so the returned path is always absolute.
///
/// Never fails: a missing `PATH`, an unreadable directory or a missing execute
/// bit all yield `None`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(std::path::MAIN_SEPARATOR) || name.contains('/') {
        return None;
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Whether `path` is a regular file with at least one execute bit set.
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// Build the argument list for `player`, expressing `volume` (0-100) in that
/// player's own units.
///
/// `volume` is clamped to 100 first, so a mis-configured value can never
/// amplify past the player's unity gain. The file path is always the last
/// element. An unrecognised player gets the path alone — we would rather play
/// at the system volume than pass a flag the program doesn't understand and
/// have it refuse to start.
///
/// `aplay` has no volume flag at all (it writes straight to the ALSA device),
/// so its volume argument is silently dropped; [`select_player`] is what keeps
/// that from silently ignoring a deliberate `volume = 0`.
pub fn player_args(player: &str, path: &Path, volume: u32) -> Vec<String> {
    let volume = volume.min(100);
    let path_arg = path.to_string_lossy().to_string();

    match player {
        // paplay takes a linear 0-65536 scale, where 65536 is unity gain.
        // Integer math keeps 50 -> exactly 32768.
        "paplay" => vec![format!("--volume={}", volume * 65536 / 100), path_arg],
        // pw-play takes a 0.0-1.0 multiplier.
        "pw-play" => vec![format!("--volume={:.2}", volume as f32 / 100.0), path_arg],
        // ffplay is a video player by default: no window, quit at EOF, and stay
        // quiet on stdout/stderr unless something actually breaks.
        "ffplay" => vec![
            "-nodisp".to_string(),
            "-autoexit".to_string(),
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-volume".to_string(),
            volume.to_string(),
            path_arg,
        ],
        // mpv likewise needs muzzling; --no-config keeps a user's mpv.conf from
        // redirecting or reformatting our playback.
        "mpv" => vec![
            "--no-video".to_string(),
            "--no-config".to_string(),
            "--really-quiet".to_string(),
            format!("--volume={}", volume),
            path_arg,
        ],
        // aplay: no volume flag exists; -q suppresses its progress chatter.
        "aplay" => vec!["-q".to_string(), path_arg],
        // afplay takes a 0.0-1.0 multiplier (macOS).
        "afplay" => vec![
            "-v".to_string(),
            format!("{:.2}", volume as f32 / 100.0),
            path_arg,
        ],
        _ => vec![path_arg],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_on_path_missing_binary() {
        assert!(find_on_path("sumvox_definitely_not_a_binary_9f3").is_none());
    }

    #[test]
    fn test_find_on_path_returns_absolute_path() {
        let found = find_on_path("sh").expect("sh must exist on any machine running cargo test");
        assert!(found.is_absolute(), "expected absolute path, got {found:?}");
    }

    #[test]
    fn test_find_on_path_empty_name() {
        assert!(find_on_path("").is_none());
    }

    #[test]
    fn test_find_on_path_rejects_path_separator() {
        // A name with a separator is not a PATH lookup — reject rather than
        // silently resolving it relative to the cwd.
        assert!(find_on_path("./sh").is_none());
    }

    fn args(player: &str, volume: u32) -> Vec<String> {
        player_args(player, Path::new("/tmp/a.wav"), volume)
    }

    #[test]
    fn test_player_args_paplay_scales_to_65536() {
        assert_eq!(args("paplay", 50), ["--volume=32768", "/tmp/a.wav"]);
        assert_eq!(args("paplay", 100), ["--volume=65536", "/tmp/a.wav"]);
    }

    #[test]
    fn test_player_args_clamps_volume_above_100() {
        assert_eq!(args("paplay", 150), ["--volume=65536", "/tmp/a.wav"]);
    }

    #[test]
    fn test_player_args_paplay_zero_volume() {
        assert_eq!(args("paplay", 0), ["--volume=0", "/tmp/a.wav"]);
    }

    #[test]
    fn test_player_args_pw_play_uses_unit_multiplier() {
        assert_eq!(args("pw-play", 50), ["--volume=0.50", "/tmp/a.wav"]);
    }

    #[test]
    fn test_player_args_ffplay() {
        assert_eq!(
            args("ffplay", 50),
            [
                "-nodisp",
                "-autoexit",
                "-hide_banner",
                "-loglevel",
                "error",
                "-volume",
                "50",
                "/tmp/a.wav"
            ]
        );
    }

    #[test]
    fn test_player_args_mpv() {
        assert_eq!(
            args("mpv", 50),
            [
                "--no-video",
                "--no-config",
                "--really-quiet",
                "--volume=50",
                "/tmp/a.wav"
            ]
        );
    }

    #[test]
    fn test_player_args_aplay_has_no_volume_flag() {
        let argv = args("aplay", 50);
        assert_eq!(argv, ["-q", "/tmp/a.wav"]);
        assert!(
            !argv.iter().any(|a| a.contains("volume")),
            "aplay has no volume flag: {argv:?}"
        );
    }

    #[test]
    fn test_player_args_afplay_uses_unit_multiplier() {
        assert_eq!(args("afplay", 50), ["-v", "0.50", "/tmp/a.wav"]);
    }

    #[test]
    fn test_player_args_unknown_player_gets_path_only() {
        assert_eq!(args("true", 50), ["/tmp/a.wav"]);
    }
}
