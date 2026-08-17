// piper TTS provider (local, offline, no network).
//
// The quality option for Chinese summaries: neural synthesis at ~640-690 ms per
// short sentence (Python start + a ~63 MB ONNX load on every invocation), versus
// espeak-ng's ~9 ms. That 70x spread is why both engines ship — this one is the
// deliberate choice for a read-aloud summary, not for a per-keystroke notifier.
//
// A piper "voice" IS a downloaded .onnx model file (plus its .onnx.json
// sidecar). SumVox never downloads one: the user fetches it and points config
// at the path, which keeps the per-voice licensing choice theirs.
//
// piper's own `--volume` flag is deliberately unused — volume is applied by the
// player, exactly as for espeak and macOS `say`.

use std::path::Path;

/// SumVox's own default rate, chosen so it lands exactly on piper's default
/// length-scale of 1.0.
pub const DEFAULT_RATE: u32 = 200;

/// piper's rate knob is `--length-scale` (phoneme duration), so it runs
/// *inversely* to a words-per-minute rate: a bigger scale means slower speech.
const MIN_LENGTH_SCALE: f64 = 0.50;
const MAX_LENGTH_SCALE: f64 = 2.00;

/// The rate that maps to length-scale 1.0 (piper's default).
const NEUTRAL_RATE: f64 = 200.0;

/// Convert SumVox's words-per-minute-ish `rate` knob into piper's
/// `--length-scale`.
///
/// `rate == 0` yields infinity, which clamps to the slowest allowed scale
/// rather than panicking — a config typo must not take the notifier down.
fn length_scale(rate: u32) -> f64 {
    (NEUTRAL_RATE / rate as f64).clamp(MIN_LENGTH_SCALE, MAX_LENGTH_SCALE)
}

/// Build the piper argument list.
///
/// Note there is deliberately **no text parameter**: the text to speak is
/// written to the child's stdin, so it is structurally incapable of reaching
/// the command line (an LLM summary starting with `-` would otherwise be eaten
/// as a flag).
pub fn piper_args(model_path: &Path, rate: u32, out_path: &Path) -> Vec<String> {
    vec![
        "-m".to_string(),
        model_path.to_string_lossy().to_string(),
        "-f".to_string(),
        out_path.to_string_lossy().to_string(),
        "--length-scale".to_string(),
        format!("{:.2}", length_scale(rate)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rate: u32) -> Vec<String> {
        piper_args(Path::new("/m/zh.onnx"), rate, Path::new("/tmp/o.wav"))
    }

    fn assert_scale(argv: &[String], expected: &str) {
        assert!(
            argv.windows(2).any(|w| w == ["--length-scale", expected]),
            "expected --length-scale {expected}: {argv:?}"
        );
    }

    #[test]
    fn test_piper_args_neutral_rate_is_default_scale() {
        assert_eq!(
            args(200),
            [
                "-m",
                "/m/zh.onnx",
                "-f",
                "/tmp/o.wav",
                "--length-scale",
                "1.00"
            ]
        );
    }

    #[test]
    fn test_piper_args_faster_rate_shortens_phonemes() {
        assert_scale(&args(400), "0.50");
    }

    #[test]
    fn test_piper_args_slower_rate_lengthens_phonemes() {
        assert_scale(&args(100), "2.00");
    }

    #[test]
    fn test_piper_args_clamps_very_slow_rate() {
        assert_scale(&args(50), "2.00");
    }

    #[test]
    fn test_piper_args_clamps_very_fast_rate() {
        assert_scale(&args(1000), "0.50");
    }

    #[test]
    fn test_piper_args_zero_rate_does_not_panic() {
        // 200/0 is infinity, which must clamp rather than blow up.
        assert_scale(&args(0), "2.00");
    }
}
