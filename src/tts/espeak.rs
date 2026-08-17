// espeak-ng TTS provider (local, offline, no network).
//
// The Linux counterpart to the macOS `say` provider: fast (~9 ms per short
// sentence) and free. Like `say`, it renders to a temp WAV and hands the file
// to the shared platform player, so the 0-100 volume knob and the menu-bar
// avatar hook work exactly as they do everywhere else. espeak's own `-a`
// amplitude flag is deliberately unused — volume is a playback-time knob.

use std::path::Path;

/// espeak-ng's own default speech rate, in words per minute.
const DEFAULT_RATE: u32 = 175;

/// Bounds espeak-ng accepts for `-s`; outside them it rejects the argument.
const MIN_RATE: u32 = 80;
const MAX_RATE: u32 = 450;

/// Build the espeak-ng argument list.
///
/// Note there is deliberately **no text parameter**: the text to speak is
/// written to the child's stdin (`--stdin`), so it is structurally incapable of
/// reaching the command line. That matters because LLM summaries are arbitrary
/// text, and one beginning with `-` would otherwise be eaten as a flag.
///
/// `-v` is omitted entirely when no voice is set or the voice is blank, letting
/// espeak pick its own default. `rate` is clamped to the range espeak accepts.
pub fn espeak_args(voice: Option<&str>, rate: u32, out_path: &Path) -> Vec<String> {
    let mut args = vec![
        "--stdin".to_string(),
        "-w".to_string(),
        out_path.to_string_lossy().to_string(),
    ];

    if let Some(voice) = voice {
        if !voice.trim().is_empty() {
            args.push("-v".to_string());
            args.push(voice.to_string());
        }
    }

    args.push("-s".to_string());
    args.push(rate.clamp(MIN_RATE, MAX_RATE).to_string());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(voice: Option<&str>, rate: u32) -> Vec<String> {
        espeak_args(voice, rate, Path::new("/tmp/o.wav"))
    }

    #[test]
    fn test_espeak_args_with_voice_and_rate() {
        assert_eq!(
            args(Some("cmn+f3"), 200),
            ["--stdin", "-w", "/tmp/o.wav", "-v", "cmn+f3", "-s", "200"]
        );
    }

    #[test]
    fn test_espeak_args_omits_voice_when_absent() {
        assert_eq!(
            args(None, 175),
            ["--stdin", "-w", "/tmp/o.wav", "-s", "175"]
        );
    }

    #[test]
    fn test_espeak_args_omits_blank_voice() {
        assert_eq!(
            args(Some("   "), 175),
            ["--stdin", "-w", "/tmp/o.wav", "-s", "175"]
        );
    }

    #[test]
    fn test_espeak_args_clamps_rate_to_minimum() {
        let argv = args(Some("cmn"), 10);
        assert!(
            argv.windows(2).any(|w| w == ["-s", "80"]),
            "expected clamp to 80: {argv:?}"
        );
    }

    #[test]
    fn test_espeak_args_clamps_rate_to_maximum() {
        let argv = args(Some("cmn"), 10000);
        assert!(
            argv.windows(2).any(|w| w == ["-s", "450"]),
            "expected clamp to 450: {argv:?}"
        );
    }

    #[test]
    fn test_espeak_args_never_carries_the_text() {
        // The builder has no text parameter at all — a structural guarantee,
        // stronger than asserting the absence of a string.
        let argv = args(Some("cmn"), 175);
        assert!(argv.contains(&"--stdin".to_string()), "{argv:?}");
    }
}
