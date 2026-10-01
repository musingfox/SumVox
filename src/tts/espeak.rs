// espeak-ng TTS provider (local, offline, no network).
//
// The Linux counterpart to the macOS `say` provider: fast (~9 ms per short
// sentence) and free. Like `say`, it renders to a temp WAV and hands the file
// to the shared platform player, so the 0-100 volume knob and the menu-bar
// avatar hook work exactly as they do everywhere else. espeak's own `-a`
// amplitude flag is deliberately unused — volume is a playback-time knob.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::TtsProvider;
use crate::audio::player;
use crate::error::{Result, VoiceError};

/// espeak-ng's own default speech rate, in words per minute.
pub const DEFAULT_RATE: u32 = 175;

/// The program we invoke. Overridable in tests only, so the spawn/exit-code
/// paths can be exercised with `true`/`false` stand-ins.
const BINARY: &str = "espeak-ng";

/// Wall-clock cap on synthesis. espeak is ~9 ms for a short sentence, so this
/// is ~3000x headroom: the cap exists to break a wedge, not to police latency.
const SYNTH_TIMEOUT: Duration = Duration::from_secs(30);

// Per-call counter so the temp path is unique even for concurrent calls that
// share a PID — same collision-safety scheme as audio/normalize.rs.
static CALL_SEQ: AtomicU64 = AtomicU64::new(0);

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

/// Local espeak-ng TTS provider.
pub struct EspeakProvider {
    /// An espeak voice name, optionally with a `+variant` suffix (e.g.
    /// `cmn+f3`). `None` lets espeak choose.
    voice: Option<String>,
    rate: u32,
    /// Applied by the player, not the engine — see the module comment.
    volume: u32,
    /// The binary to spawn; always `espeak-ng` outside tests.
    binary: String,
}

impl EspeakProvider {
    pub fn new(voice: Option<String>, rate: u32, volume: u32) -> Self {
        Self {
            voice,
            rate,
            volume,
            binary: BINARY.to_string(),
        }
    }

    /// Swap the spawned program, so tests can inject `true`/`false` stand-ins
    /// instead of depending on a real engine being installed.
    #[cfg(test)]
    fn with_binary(mut self, binary: &str) -> Self {
        self.binary = binary.to_string();
        self
    }
}

#[async_trait]
impl TtsProvider for EspeakProvider {
    fn name(&self) -> &str {
        "espeak"
    }

    /// Whether espeak-ng is installed. A missing engine reports `false` here
    /// rather than erroring at speak time, which is the only signal the
    /// fallback chain honours in both `auto` and explicit `--tts` mode.
    fn is_available(&self) -> bool {
        let found = player::find_on_path(&self.binary).is_some();
        if !found {
            tracing::debug!("{} not found on PATH; skipping espeak", self.binary);
        }
        found
    }

    async fn speak(&self, text: &str) -> Result<bool> {
        // Guard before any temp-path or spawn work: nothing to say is a
        // deliberate no-op, not a failure. Matches every other provider.
        if text.trim().is_empty() {
            tracing::warn!("Empty message, skipping voice notification");
            return Ok(false);
        }

        tracing::info!(
            "Speaking with espeak-ng: voice={:?}, rate={}, volume={}",
            self.voice,
            self.rate,
            self.volume
        );

        // Qualify by PID + per-call counter so concurrent invocations never
        // clobber each other's file.
        let wav_path = std::env::temp_dir().join(format!(
            "sumvox_espeak_{}_{}.wav",
            std::process::id(),
            CALL_SEQ.fetch_add(1, Ordering::Relaxed)
        ));

        let args = espeak_args(self.voice.as_deref(), self.rate, &wav_path);
        tracing::debug!("espeak-ng argv: {:?}", args);

        let spawned = Command::new(&self.binary)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            // If we bail out early (timeout), don't leave the child running.
            .kill_on_drop(true)
            .spawn();

        let mut child = spawned.map_err(|e| {
            VoiceError::Voice(format!("espeak-ng synthesis failed: could not run: {}", e))
        })?;

        // The text goes to stdin, never onto the command line. A write failure
        // is not fatal on its own — the exit status decides.
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(text.as_bytes()).await {
                tracing::debug!("espeak-ng: failed writing text to stdin: {}", e);
            }
            // Dropping/closing stdin signals EOF so espeak stops reading.
            let _ = stdin.shutdown().await;
        }

        let output = match tokio::time::timeout(SYNTH_TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                let _ = std::fs::remove_file(&wav_path);
                return Err(VoiceError::Voice(format!(
                    "espeak-ng synthesis failed: {}",
                    e
                )));
            }
            Err(_) => {
                // kill_on_drop reaps the child as `child` is dropped here.
                let _ = std::fs::remove_file(&wav_path);
                return Err(VoiceError::Voice(format!(
                    "espeak-ng synthesis failed: timed out after {}s",
                    SYNTH_TIMEOUT.as_secs()
                )));
            }
        };

        if !output.status.success() {
            // espeak may have left a partial file behind before failing.
            let _ = std::fs::remove_file(&wav_path);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::Voice(format!(
                "espeak-ng synthesis failed: {}",
                stderr.trim()
            )));
        }

        // Exit zero with no audio must not reach the player: that would look
        // like success while producing silence.
        let rendered = std::fs::metadata(&wav_path).map(|m| m.len()).unwrap_or(0);
        if rendered == 0 {
            let _ = std::fs::remove_file(&wav_path);
            return Err(VoiceError::Voice(
                "espeak-ng synthesis failed: produced no audio".to_string(),
            ));
        }

        // Clean up on every path, including the playback-error path.
        let result = player::play_file(&wav_path, self.volume);
        let _ = std::fs::remove_file(&wav_path);
        result?;

        tracing::debug!("espeak-ng playback completed");
        Ok(true)
    }

    fn estimate_cost(&self, _char_count: usize) -> f64 {
        // Local engine: free.
        0.0
    }
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

    // Synthesis tests inject `true`/`false` as the "engine": they exercise the
    // real spawn/stdin/exit-code paths with no engine binary and no audio.
    #[tokio::test]
    async fn test_speak_reports_engine_failure() {
        let provider = EspeakProvider::new(None, 175, 80).with_binary("false");
        let err = provider
            .speak("hello")
            .await
            .expect_err("a non-zero engine exit must not report success")
            .to_string();
        assert!(
            err.contains("espeak-ng synthesis failed"),
            "unexpected: {err}"
        );
    }

    #[tokio::test]
    async fn test_speak_rejects_exit_zero_with_no_audio() {
        // `true` exits 0 without writing a WAV — that must never reach the
        // player, or the user gets a "success" with no sound.
        let provider = EspeakProvider::new(None, 175, 80).with_binary("true");
        let err = provider
            .speak("hello")
            .await
            .expect_err("exit 0 with no WAV must be an error")
            .to_string();
        assert!(err.contains("produced no audio"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn test_speak_reports_missing_binary() {
        let provider = EspeakProvider::new(None, 175, 80).with_binary("sumvox_no_such_binary");
        let err = provider
            .speak("hello")
            .await
            .expect_err("a missing engine must surface as an error here")
            .to_string();
        assert!(
            err.contains("espeak-ng synthesis failed"),
            "unexpected: {err}"
        );
    }

    #[tokio::test]
    async fn test_speak_empty_text_is_a_no_op() {
        // The binary is `false`, which would error if it were ever spawned —
        // so Ok(false) proves the guard runs before the spawn.
        let provider = EspeakProvider::new(None, 175, 80).with_binary("false");
        assert!(!provider.speak("").await.expect("empty text must not error"));
    }

    #[tokio::test]
    async fn test_speak_whitespace_only_is_a_no_op() {
        let provider = EspeakProvider::new(None, 175, 80).with_binary("false");
        assert!(!provider
            .speak("   \n ")
            .await
            .expect("whitespace-only text must not error"));
    }

    #[test]
    fn test_is_available_false_when_binary_missing() {
        let provider = EspeakProvider::new(None, 175, 100).with_binary("sumvox_no_such_binary");
        assert!(
            !provider.is_available(),
            "a missing engine must be skipped by the chain, not error"
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
