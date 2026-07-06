// afplay playback utility for macOS
// Plays audio data using the afplay command-line tool

use crate::error::{Result, VoiceError};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Run `afplay -v {volume/100.0:.2} {file_path}` to completion (blocking).
///
/// # Arguments
/// * `file_path` - Path to the audio file to play
/// * `volume` - Volume level 0-100 (values above 100 are clamped to avoid
///   amplification past `-v 1.0`)
///
/// # Errors
/// Returns VoiceError::Voice on:
/// - Failed to spawn afplay process ("Failed to run afplay: {e}")
/// - afplay exited with non-zero status ("afplay exited with error")
///
/// Note: this only runs the command; it does not write or clean up temp files.
pub fn run_afplay(file_path: &Path, volume: u32) -> Result<()> {
    // afplay -v takes a float: 0.0 = silent, 1.0 = full volume. Clamp to 100 so
    // a mis-configured volume can't amplify past 1.0 and over-drive the output.
    let afplay_volume = volume.min(100) as f32 / 100.0;
    // Tell the menu bar avatar which file is playing so it can flap its mouth
    // from the real amplitude. Single choke point: every provider plays here.
    crate::notify_log::set_now_playing(file_path);
    let status = Command::new("afplay")
        .arg("-v")
        .arg(format!("{:.2}", afplay_volume))
        .arg(file_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| VoiceError::Voice(format!("Failed to run afplay: {}", e)))?;

    if !status.success() {
        return Err(VoiceError::Voice("afplay exited with error".to_string()));
    }

    Ok(())
}

/// Play audio data using macOS afplay command
///
/// # Arguments
/// * `audio_data` - Audio data bytes (must be WAV format)
/// * `volume` - Volume level 0-100
/// * `temp_file_prefix` - Prefix for temporary file (e.g., "sumvox_google")
///
/// # Returns
/// Ok(()) on success
///
/// # Errors
/// Returns VoiceError::Voice on:
/// - Failed to write temp file
/// - Failed to spawn afplay process
/// - afplay exited with non-zero status
///
/// # Implementation
/// 1. Writes audio_data to `/tmp/{temp_file_prefix}.wav`
/// 2. Spawns `afplay -v {volume/100.0:.2} {path}`
/// 3. Cleans up temp file after playback (best effort, ignores cleanup errors)
pub fn play_with_afplay(audio_data: &[u8], volume: u32, temp_file_prefix: &str) -> Result<()> {
    tracing::debug!(
        "Playing with afplay: {} bytes, volume: {}, prefix: {}",
        audio_data.len(),
        volume,
        temp_file_prefix
    );

    // Write to temp file
    let tmp_path = std::env::temp_dir().join(format!("{}.wav", temp_file_prefix));
    std::fs::File::create(&tmp_path)
        .and_then(|mut f| f.write_all(audio_data))
        .map_err(|e| VoiceError::Voice(format!("Failed to write temp WAV: {}", e)))?;

    // Capture the result before cleanup so the temp file is removed on every
    // path — including a spawn failure, which the pre-refactor `?` would have
    // skipped, leaking the file.
    let result = run_afplay(&tmp_path, volume);
    // Clean up temp file (best effort)
    let _ = std::fs::remove_file(&tmp_path);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid WAV file for testing
    fn create_test_wav() -> Vec<u8> {
        // Use our wav_header module to create a valid WAV
        crate::audio::wav_header::create_wav_file(&[0x00, 0x00], 24000, 1, 16)
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_play_with_afplay_success() {
        let wav_data = create_test_wav();
        let result = play_with_afplay(&wav_data, 50, "sumvox_test");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_play_with_afplay_zero_volume() {
        let wav_data = create_test_wav();
        let result = play_with_afplay(&wav_data, 0, "sumvox_test_zero");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_play_with_afplay_max_volume() {
        let wav_data = create_test_wav();
        let result = play_with_afplay(&wav_data, 100, "sumvox_test_max");
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_play_with_afplay_not_available() {
        let wav_data = create_test_wav();
        let result = play_with_afplay(&wav_data, 50, "sumvox_test");
        // On non-macOS, afplay won't exist, so this should error
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to run afplay"));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_run_afplay_not_available() {
        // On non-macOS, afplay won't exist, so spawning it should fail.
        let result = run_afplay(Path::new("/tmp/sumvox_nonexistent.wav"), 50);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to run afplay"));
    }
}
