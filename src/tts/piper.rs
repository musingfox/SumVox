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

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::render::render_then_play;
use super::TtsProvider;
use crate::audio::player;
use crate::error::Result;

/// SumVox's own default rate, chosen so it lands exactly on piper's default
/// length-scale of 1.0.
pub const DEFAULT_RATE: u32 = 200;

/// The program we invoke. Overridable in tests only.
const BINARY: &str = "piper";

/// Resolve which `.onnx` model to speak with, from a config entry's three
/// candidate fields.
///
/// Precedence is `voice` → `model` → `path`, first non-blank wins, with `~`
/// expanded. `voice` comes first because it is the only field the CLI overlays
/// (`--voice`), so if it were inert a piper voice could not be chosen from the
/// command line at all — and for piper, the model file *is* the voice.
///
/// Returns `None` when all three are absent or blank; the caller turns that
/// into a construction-time error rather than a surprise at playback.
pub fn resolve_model_path(
    voice: Option<&str>,
    model: Option<&str>,
    path: Option<&str>,
) -> Option<PathBuf> {
    [voice, model, path]
        .into_iter()
        .flatten()
        .find(|candidate| !candidate.trim().is_empty())
        .map(|candidate| PathBuf::from(shellexpand::tilde(candidate).to_string()))
}

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

/// Local piper TTS provider.
pub struct PiperProvider {
    /// The `.onnx` voice model. For piper, this IS the voice.
    model_path: PathBuf,
    rate: u32,
    /// Applied by the player, not the engine — see the module comment.
    volume: u32,
    binary: String,
}

impl PiperProvider {
    pub fn new(model_path: impl Into<PathBuf>, rate: u32, volume: u32) -> Self {
        Self {
            model_path: model_path.into(),
            rate,
            volume,
            binary: BINARY.to_string(),
        }
    }

    #[cfg(test)]
    pub(crate) fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Swap the spawned program, so tests can inject `true`/`false` stand-ins.
    #[cfg(test)]
    fn with_binary(mut self, binary: &str) -> Self {
        self.binary = binary.to_string();
        self
    }
}

#[async_trait]
impl TtsProvider for PiperProvider {
    fn name(&self) -> &str {
        "piper"
    }

    /// Whether piper can actually speak: the binary must be installed **and**
    /// the voice model must exist. A user-supplied model path is exactly as
    /// likely to be missing as the binary, and both must skip the provider
    /// rather than fail at speak time.
    fn is_available(&self) -> bool {
        if player::find_on_path(&self.binary).is_none() {
            tracing::debug!("{} not found on PATH; skipping piper", self.binary);
            return false;
        }
        if !self.model_path.is_file() {
            tracing::debug!(
                "piper voice model {:?} not found; skipping piper",
                self.model_path
            );
            return false;
        }
        true
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        tracing::info!(
            "Speaking with piper: model={:?}, rate={}, volume={}",
            self.model_path,
            self.rate,
            self.volume
        );

        render_then_play(
            "piper",
            &self.binary,
            |wav| piper_args(&self.model_path, self.rate, wav),
            text,
            self.volume,
        )
        .await
    }
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

        // (rate, length-scale): 300 is an in-range point, 400/100 sit on the clamp
        // edges, beyond them the scale clamps, and 0 (200/0 = inf) must clamp, not panic
        for (rate, scale) in [
            (300, "0.67"),
            (400, "0.50"),
            (100, "2.00"),
            (50, "2.00"),
            (1000, "0.50"),
            (0, "2.00"),
        ] {
            assert_scale(&args(rate), scale);
        }
    }

    // Synthesis tests inject `true`/`false` as the "engine": they exercise the
    // real spawn/stdin/exit-code paths with no engine binary and no audio.
    #[tokio::test]
    async fn test_speak_reports_engine_failure() {
        let provider = PiperProvider::new("/m/zh.onnx", 200, 80).with_binary("false");
        let err = provider
            .speak("hello")
            .await
            .expect_err("a non-zero engine exit must not report success")
            .to_string();
        assert!(err.contains("piper synthesis failed"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn test_speak_rejects_exit_zero_with_no_audio() {
        // `true` exits 0 without writing a WAV — that must never reach the
        // player, or the user gets a "success" with no sound.
        let provider = PiperProvider::new("/m/zh.onnx", 200, 80).with_binary("true");
        let err = provider
            .speak("hello")
            .await
            .expect_err("exit 0 with no WAV must be an error")
            .to_string();
        assert!(err.contains("produced no audio"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn test_speak_empty_text_is_a_no_op() {
        // The binary is `false`, which would error if it were ever spawned —
        // so Ok(false) proves the guard runs before the spawn.
        let provider = PiperProvider::new("/m/zh.onnx", 200, 80).with_binary("false");
        assert!(!provider.speak("").await.expect("empty text must not error"));
        // U+3000 is whitespace, so a "blank" CJK message must not speak
        assert!(!provider
            .speak("\u{3000}")
            .await
            .expect("ideographic space must not error"));
    }

    #[test]
    fn test_is_available_false_when_model_missing() {
        // Deterministic whether or not piper itself is installed: a voice model
        // that isn't there means piper cannot speak.
        let provider = PiperProvider::new("/nonexistent/zh.onnx", 200, 100).with_binary("sh");
        assert!(!provider.is_available());
    }

    #[test]
    fn test_is_available_false_when_binary_missing() {
        let model = tempfile::NamedTempFile::new().expect("temp model file");
        let provider =
            PiperProvider::new(model.path(), 200, 100).with_binary("sumvox_no_such_binary");
        assert!(
            !provider.is_available(),
            "a missing engine must be skipped by the chain, not error"
        );
    }
}
