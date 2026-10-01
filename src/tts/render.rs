// Shared synth-then-play routine for engines that render text to a WAV file by
// running a local binary (espeak-ng, piper)

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::audio::player;
use crate::error::{Result, VoiceError};

const SYNTH_TIMEOUT: Duration = Duration::from_secs(30);

/// Per-call counter, so concurrent invocations never share a temp file.
static CALL_SEQ: AtomicU64 = AtomicU64::new(0);

/// Run `binary` with the argv `args` builds for the output path, feed it `text` on
/// stdin, then play the rendered WAV and remove it. Empty text is a no-op (`false`).
pub async fn render_then_play(
    label: &str,
    binary: &str,
    args: impl FnOnce(&Path) -> Vec<String>,
    text: &str,
    volume: u32,
) -> Result<bool> {
    if text.trim().is_empty() {
        tracing::warn!("Empty message, skipping voice notification");
        return Ok(false);
    }

    let wav_path = std::env::temp_dir().join(format!(
        "sumvox_{}_{}_{}.wav",
        label,
        std::process::id(),
        CALL_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let fail = |detail: String| VoiceError::Voice(format!("{label} synthesis failed: {detail}"));

    let args = args(&wav_path);
    tracing::debug!("{} argv: {:?}", label, args);

    let mut child = Command::new(binary)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        // If we bail out early (timeout), don't leave the child running.
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| fail(format!("could not run: {e}")))?;

    // The text goes to stdin, never onto the command line. A write failure is
    // not fatal on its own: the exit status decides.
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(text.as_bytes()).await {
            tracing::debug!("{}: failed writing text to stdin: {}", label, e);
        }
        // Closing stdin signals EOF so the engine stops reading.
        let _ = stdin.shutdown().await;
    }

    let output = match tokio::time::timeout(SYNTH_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            let _ = std::fs::remove_file(&wav_path);
            return Err(fail(e.to_string()));
        }
        Err(_) => {
            // kill_on_drop reaps the child as `child` is dropped here.
            let _ = std::fs::remove_file(&wav_path);
            return Err(fail(format!(
                "timed out after {}s",
                SYNTH_TIMEOUT.as_secs()
            )));
        }
    };

    if !output.status.success() {
        // The engine may have left a partial file behind before failing.
        let _ = std::fs::remove_file(&wav_path);
        return Err(fail(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    // Exit zero with no audio must not reach the player: that would look like
    // success while producing silence.
    if std::fs::metadata(&wav_path).map_or(0, |m| m.len()) == 0 {
        let _ = std::fs::remove_file(&wav_path);
        return Err(fail("produced no audio".to_string()));
    }

    // Clean up on every path, including the playback-error path.
    let result = player::play_file(&wav_path, volume);
    let _ = std::fs::remove_file(&wav_path);
    result?;

    tracing::debug!("{} playback completed", label);
    Ok(true)
}
