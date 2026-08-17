// Platform audio playback.
//
// Every TTS provider renders (or downloads) audio and then plays it here — this
// module is the single playback choke point for the whole crate. On macOS the
// player is `afplay`; elsewhere we probe a list of common command-line players
// and use the first one installed, so the 0-100 volume knob and the menu-bar
// avatar hook keep working on Linux instead of being macOS-only.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::error::{Result, VoiceError};

/// Wall-clock cap on a single playback. The cap exists to break a wedged
/// player, not to police latency, so it is deliberately generous: any realistic
/// notification or read-aloud summary finishes far inside it.
const PLAYBACK_TIMEOUT: Duration = Duration::from_secs(120);

/// Players we try, in descending order of "likely installed AND honours
/// volume". `paplay` ships with both PulseAudio and `pipewire-pulse`, so it
/// covers most desktops; `aplay` is last because it bypasses the sound server
/// entirely (and so fails when PipeWire holds the device) and has no volume
/// control at all.
pub fn default_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["afplay"]
    } else {
        &["paplay", "pw-play", "ffplay", "mpv", "aplay"]
    }
}

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

/// Pick the first candidate that is installed and can honour `volume`.
///
/// Candidates are probed in order, so the list encodes our preference. `aplay`
/// is a special case: it has no volume flag, so it is skipped entirely when the
/// user asked for `volume == 0`. Playing at full blast when someone asked for
/// silence is an active harm, whereas playing at 100 when they asked for 60 is
/// merely a degradation — so the two are not treated the same.
///
/// `None` means nothing usable is installed; the caller turns that into an
/// error rather than a silent no-op.
pub fn select_player(candidates: &[&str], volume: u32) -> Option<String> {
    for candidate in candidates {
        if *candidate == "aplay" && volume == 0 {
            tracing::debug!("Skipping aplay: it cannot honour volume 0");
            continue;
        }
        if find_on_path(candidate).is_some() {
            if *candidate == "aplay" {
                tracing::warn!(
                    "Using aplay: it has no volume control, so volume {volume} is ignored"
                );
            }
            return Some((*candidate).to_string());
        }
    }
    None
}

/// Decide whether the player child needs an `XDG_RUNTIME_DIR` we supply.
///
/// Returns `Some(dir)` only when `current` is absent or blank **and**
/// `/run/user/{uid}` exists — otherwise `None`, meaning "set nothing".
///
/// This exists because SumVox runs as a Claude Code hook and Claude Code is
/// frequently driven over SSH, where `XDG_RUNTIME_DIR` is unset. Without it,
/// every PulseAudio/PipeWire client fails to find the session socket and the
/// notifier is silently mute even though the sound server is running. An
/// existing value is never overridden: the session owns that variable, and a
/// guess is only ever a last resort. Guessing wrong is harmless — the player
/// then fails loudly and the error carries its stderr.
pub fn runtime_dir_default(
    current: Option<&str>,
    uid: u32,
    run_user_dir_exists: bool,
) -> Option<String> {
    if let Some(value) = current {
        if !value.trim().is_empty() {
            return None;
        }
    }
    if !run_user_dir_exists {
        return None;
    }
    Some(format!("/run/user/{}", uid))
}

/// Read the live environment and decide the child's `XDG_RUNTIME_DIR`.
///
/// A thin wrapper over [`runtime_dir_default`] that supplies the three real
/// inputs; our own process environment is never mutated.
fn runtime_dir_for_child() -> Option<String> {
    let current = std::env::var("XDG_RUNTIME_DIR").ok();
    let uid = nix::unistd::Uid::current().as_raw();
    let exists = Path::new(&format!("/run/user/{}", uid)).is_dir();
    runtime_dir_default(current.as_deref(), uid, exists)
}

/// Play `file_path` to completion (blocking), applying `volume` (0-100).
///
/// This is the single playback choke point for the whole crate: every provider
/// arrives here, which is what keeps the volume knob and the menu-bar avatar
/// hook working uniformly across platforms.
///
/// # Errors
/// Every failure is a [`VoiceError::Voice`] prefixed `"Audio playback failed"`.
/// Crucially, this function never returns `Ok(())` without a player having
/// exited zero — silence is always reported, never swallowed.
pub fn play_file(file_path: &Path, volume: u32) -> Result<()> {
    play_file_with(default_candidates(), file_path, volume, PLAYBACK_TIMEOUT)
}

/// [`play_file`] with the candidate list and timeout injected, so the selection,
/// failure and timeout paths are testable without an audio device.
pub fn play_file_with(
    candidates: &[&str],
    file_path: &Path,
    volume: u32,
    timeout: Duration,
) -> Result<()> {
    let player = select_player(candidates, volume).ok_or_else(|| {
        VoiceError::Voice(format!(
            "Audio playback failed: no supported audio player found on PATH (tried: {})",
            candidates.join(", ")
        ))
    })?;

    // Tell the menu bar avatar which file is playing so it can flap its mouth
    // from the real amplitude. Written before the spawn so the animation starts
    // at the same instant sound does. This is the only call site.
    crate::notify_log::set_now_playing(file_path);

    let args = player_args(&player, file_path, volume);
    tracing::debug!("Playing with {}: {:?}", player, args);

    let mut command = Command::new(&player);
    command.args(&args);
    // Scope the SSH workaround to the child only — mutating our own environment
    // could leak into unrelated children (LLM/HTTP paths).
    if let Some(dir) = runtime_dir_for_child() {
        tracing::debug!("Defaulting XDG_RUNTIME_DIR={} for the player", dir);
        command.env("XDG_RUNTIME_DIR", dir);
    }

    // stdin is nulled so a player can't probe and consume the parent's stdin —
    // which, in a Claude Code hook, carries the event JSON.
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            VoiceError::Voice(format!(
                "Audio playback failed: could not run {}: {}",
                player, e
            ))
        })?;

    // Drain stderr on a helper thread so a full pipe can't deadlock us, and
    // signal on EOF so we can block on a timeout instead of busy-polling.
    let stderr_pipe = child.stderr.take();
    let (done_tx, done_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        let _ = done_tx.send(());
        buf
    });

    match done_rx.recv_timeout(timeout) {
        Ok(()) => {
            // stderr closed ⇒ the player is exiting, so wait() returns promptly.
            let status = child.wait().map_err(|e| {
                VoiceError::Voice(format!(
                    "Audio playback failed: could not wait for {}: {}",
                    player, e
                ))
            })?;
            let stderr = reader.join().unwrap_or_default();

            if !status.success() {
                return Err(VoiceError::Voice(format!(
                    "Audio playback failed: {} exited with status {}: {}",
                    player,
                    status,
                    stderr_tail(&stderr)
                )));
            }
            Ok(())
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            Err(VoiceError::Voice(format!(
                "Audio playback failed: {} timed out after {}s",
                player,
                timeout.as_secs_f32()
            )))
        }
    }
}

/// The last non-empty line of a player's stderr, truncated to 200 chars.
///
/// This is what makes a PipeWire `Connection refused` visible to the user
/// instead of leaving them with unexplained silence.
fn stderr_tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let line = text
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    line.chars().take(200).collect()
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

    #[test]
    fn test_select_player_none_installed() {
        assert_eq!(select_player(&["sumvox_no_such_player"], 100), None);
    }

    #[test]
    fn test_select_player_finds_installed() {
        assert_eq!(select_player(&["sh"], 100), Some("sh".to_string()));
    }

    #[test]
    fn test_select_player_first_installed_wins() {
        assert_eq!(
            select_player(&["sumvox_no_such_player", "sh"], 100),
            Some("sh".to_string())
        );
    }

    #[test]
    fn test_select_player_rejects_aplay_at_zero_volume() {
        // Deterministic with or without aplay installed: volume 0 disqualifies
        // it before the PATH probe ever runs.
        assert_eq!(select_player(&["aplay"], 0), None);
    }

    #[test]
    fn test_select_player_falls_past_aplay_at_zero_volume() {
        assert_eq!(select_player(&["aplay", "sh"], 0), Some("sh".to_string()));
    }

    #[test]
    fn test_runtime_dir_default_supplies_when_unset() {
        assert_eq!(
            runtime_dir_default(None, 1000, true),
            Some("/run/user/1000".to_string())
        );
    }

    #[test]
    fn test_runtime_dir_default_never_overrides_existing() {
        assert_eq!(
            runtime_dir_default(Some("/run/user/1000"), 1000, true),
            None
        );
    }

    #[test]
    fn test_runtime_dir_default_treats_blank_as_unset() {
        assert_eq!(
            runtime_dir_default(Some(""), 1000, true),
            Some("/run/user/1000".to_string())
        );
        assert_eq!(
            runtime_dir_default(Some("   "), 1000, true),
            Some("/run/user/1000".to_string())
        );
    }

    #[test]
    fn test_runtime_dir_default_requires_the_dir_to_exist() {
        assert_eq!(runtime_dir_default(None, 1000, false), None);
    }

    #[test]
    fn test_runtime_dir_default_handles_root() {
        assert_eq!(
            runtime_dir_default(None, 0, true),
            Some("/run/user/0".to_string())
        );
    }

    // Playback tests inject `true`/`false` as the "player": they exercise the
    // real spawn/wait/exit-code path with no audio device and no real player.
    const FIVE_SECONDS: Duration = Duration::from_secs(5);

    #[test]
    fn test_play_file_with_succeeds_when_player_exits_zero() {
        let result = play_file_with(
            &["true"],
            Path::new("/tmp/sumvox_nonexistent.wav"),
            50,
            FIVE_SECONDS,
        );
        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
    }

    #[test]
    fn test_play_file_with_reports_nonzero_exit() {
        let err = play_file_with(
            &["false"],
            Path::new("/tmp/sumvox_nonexistent.wav"),
            50,
            FIVE_SECONDS,
        )
        .expect_err("a player exiting non-zero must never report success")
        .to_string();
        assert!(err.contains("Audio playback failed"), "unexpected: {err}");
        assert!(err.contains("exited"), "unexpected: {err}");
    }

    #[test]
    fn test_play_file_with_reports_no_player_installed() {
        let err = play_file_with(
            &["sumvox_no_such_player"],
            Path::new("/tmp/x.wav"),
            50,
            FIVE_SECONDS,
        )
        .expect_err("no installed player must be an error, not silence")
        .to_string();
        assert!(
            err.contains("Audio playback failed: no supported audio player found"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn test_play_file_with_refuses_aplay_at_zero_volume() {
        // Deterministic with or without aplay installed.
        let err = play_file_with(&["aplay"], Path::new("/tmp/x.wav"), 0, FIVE_SECONDS)
            .expect_err("aplay cannot honour volume 0, so this must fail loudly")
            .to_string();
        assert!(
            err.contains("no supported audio player found"),
            "unexpected: {err}"
        );
    }

    #[test]
    fn test_play_file_with_kills_a_wedged_player() {
        // `cat /dev/zero` never exits and never closes stderr, so it wedges the
        // drain thread deterministically — no audio device involved. The hook
        // must not block on it.
        let start = std::time::Instant::now();
        let err = play_file_with(
            &["cat"],
            Path::new("/dev/zero"),
            50,
            Duration::from_millis(100),
        )
        .expect_err("a player that never exits must time out, not hang")
        .to_string();

        assert!(err.contains("timed out"), "unexpected: {err}");
        assert!(
            start.elapsed() < FIVE_SECONDS,
            "timeout took too long: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn test_default_candidates_are_platform_specific() {
        if cfg!(target_os = "macos") {
            assert_eq!(default_candidates(), &["afplay"]);
        } else {
            assert_eq!(
                default_candidates(),
                &["paplay", "pw-play", "ffplay", "mpv", "aplay"]
            );
        }
    }

    #[test]
    fn test_select_player_allows_aplay_above_zero_volume() {
        // Consistency assertion: correct on machines with and without aplay.
        assert_eq!(
            select_player(&["aplay"], 50).is_some(),
            find_on_path("aplay").is_some()
        );
    }
}
