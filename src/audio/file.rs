// Audio file provider - plays audio files (.wav, .mp3, .flac, .ogg)

use async_trait::async_trait;
use rand::seq::SliceRandom;
use std::fs;
use std::path::PathBuf;

use crate::error::{Result, VoiceError};
use crate::tts::TtsProvider;

/// Mode for audio file selection
#[derive(Debug, Clone)]
pub enum AudioFileMode {
    /// Single file path
    SingleFile(PathBuf),
    /// Directory path (randomly select file on each playback)
    Directory(PathBuf),
}

/// Audio file provider implementing TtsProvider trait
#[derive(Debug)]
pub struct AudioFileProvider {
    mode: AudioFileMode,
    volume: u32, // 0-100
}

impl AudioFileProvider {
    /// Create new audio file provider
    ///
    /// # Arguments
    /// * `path` - Path to audio file or directory
    /// * `volume` - Volume level 0-100
    ///
    /// # Errors
    /// Returns error if path doesn't exist or isn't readable
    pub fn new(path: PathBuf, volume: u32) -> Result<Self> {
        let mode = if path.is_dir() {
            AudioFileMode::Directory(path)
        } else if path.is_file() {
            AudioFileMode::SingleFile(path)
        } else if !path.exists() {
            return Err(VoiceError::Config(format!(
                "Audio file path does not exist: {:?}",
                path
            )));
        } else {
            return Err(VoiceError::Config(format!(
                "Audio file path is neither file nor directory: {:?}",
                path
            )));
        };

        Ok(Self { mode, volume })
    }

    /// Get a file to play (handles single file or random directory selection)
    fn get_playback_file(&self) -> Result<PathBuf> {
        match &self.mode {
            AudioFileMode::SingleFile(path) => Ok(path.clone()),
            AudioFileMode::Directory(dir) => {
                // List all audio files in directory
                let entries = fs::read_dir(dir).map_err(|e| {
                    VoiceError::Config(format!("Failed to read directory {:?}: {}", dir, e))
                })?;

                let audio_files: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| {
                        p.is_file()
                            && p.extension()
                                .and_then(|s| s.to_str())
                                .map(|ext| {
                                    matches!(
                                        ext.to_lowercase().as_str(),
                                        "wav" | "mp3" | "flac" | "ogg" | "m4a"
                                    )
                                })
                                .unwrap_or(false)
                    })
                    .collect();

                if audio_files.is_empty() {
                    return Err(VoiceError::Config(format!(
                        "No audio files found in directory: {:?}",
                        dir
                    )));
                }

                // Randomly select one file
                let mut rng = rand::thread_rng();
                let selected = audio_files.choose(&mut rng).ok_or_else(|| {
                    VoiceError::Config("Failed to select random audio file".into())
                })?;

                Ok(selected.clone())
            }
        }
    }
}

/// Play an audio file to completion (blocking) using afplay.
fn play_audio_blocking(file_path: &PathBuf, volume: u32) -> Result<()> {
    tracing::debug!("Playing audio file: {:?}, volume: {}", file_path, volume);

    // afplay -v takes a float: 0.0 = silent, 1.0 = full volume
    let afplay_volume = volume as f32 / 100.0;
    let status = std::process::Command::new("afplay")
        .arg("-v")
        .arg(format!("{:.2}", afplay_volume))
        .arg(file_path)
        .status()
        .map_err(|e| VoiceError::Voice(format!("Failed to run afplay: {}", e)))?;

    if !status.success() {
        return Err(VoiceError::Voice("afplay exited with error".to_string()));
    }

    Ok(())
}

#[async_trait]
impl TtsProvider for AudioFileProvider {
    fn name(&self) -> &str {
        "audio_file"
    }

    fn is_available(&self) -> bool {
        match &self.mode {
            AudioFileMode::SingleFile(path) => path.exists() && path.is_file(),
            AudioFileMode::Directory(path) => path.exists() && path.is_dir(),
        }
    }

    async fn speak(&self, _text: &str) -> Result<bool> {
        let file_path = self.get_playback_file()?;

        tracing::info!(
            "Playing audio file: {:?} (volume: {})",
            file_path,
            self.volume
        );

        play_audio_blocking(&file_path, self.volume)?;
        tracing::debug!("Audio playback completed (blocking)");

        Ok(true)
    }

    fn estimate_cost(&self, _char_count: usize) -> f64 {
        // Audio file playback is free
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_create_audio_provider_single_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"fake audio data").unwrap();
        let path = temp_file.path().to_path_buf();

        let provider = AudioFileProvider::new(path, 80).unwrap();
        assert_eq!(provider.name(), "audio_file");
        assert_eq!(provider.volume, 80);
        assert!(matches!(provider.mode, AudioFileMode::SingleFile(_)));
    }

    #[test]
    fn test_create_audio_provider_directory() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().to_path_buf();

        let provider = AudioFileProvider::new(path, 100).unwrap();
        assert_eq!(provider.name(), "audio_file");
        assert!(matches!(provider.mode, AudioFileMode::Directory(_)));
    }

    #[test]
    fn test_create_audio_provider_missing_path() {
        let path = PathBuf::from("/nonexistent/path/to/audio.wav");
        let result = AudioFileProvider::new(path, 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_get_playback_file_single_mode() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"fake audio data").unwrap();
        let path = temp_file.path().to_path_buf();

        let provider = AudioFileProvider::new(path.clone(), 100).unwrap();
        let result = provider.get_playback_file().unwrap();
        assert_eq!(result, path);
    }

    #[test]
    fn test_get_playback_file_directory_mode() {
        let temp_dir = tempdir().unwrap();

        // Create a few audio files
        let file1 = temp_dir.path().join("sound1.wav");
        let file2 = temp_dir.path().join("sound2.mp3");
        std::fs::write(&file1, b"fake wav").unwrap();
        std::fs::write(&file2, b"fake mp3").unwrap();

        let provider = AudioFileProvider::new(temp_dir.path().to_path_buf(), 100).unwrap();
        let result = provider.get_playback_file().unwrap();

        // Should be one of the audio files
        assert!(result == file1 || result == file2);
    }

    #[test]
    fn test_directory_filters_non_audio_files() {
        let temp_dir = tempdir().unwrap();

        // Create non-audio files
        let txt_file = temp_dir.path().join("readme.txt");
        let json_file = temp_dir.path().join("config.json");
        std::fs::write(&txt_file, b"text").unwrap();
        std::fs::write(&json_file, b"{}").unwrap();

        let provider = AudioFileProvider::new(temp_dir.path().to_path_buf(), 100).unwrap();
        let result = provider.get_playback_file();

        // Should fail because no audio files
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No audio files found"));
    }

    #[test]
    fn test_volume_mapping() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"fake audio").unwrap();
        let path = temp_file.path().to_path_buf();

        let provider_0 = AudioFileProvider::new(path.clone(), 0).unwrap();
        assert_eq!(provider_0.volume, 0);

        let provider_50 = AudioFileProvider::new(path.clone(), 50).unwrap();
        assert_eq!(provider_50.volume, 50);

        let provider_100 = AudioFileProvider::new(path, 100).unwrap();
        assert_eq!(provider_100.volume, 100);
    }

    #[test]
    fn test_is_available_single_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"fake audio").unwrap();
        let path = temp_file.path().to_path_buf();

        let provider = AudioFileProvider::new(path, 100).unwrap();
        assert!(provider.is_available());
    }

    #[test]
    fn test_is_available_directory() {
        let temp_dir = tempdir().unwrap();
        let provider = AudioFileProvider::new(temp_dir.path().to_path_buf(), 100).unwrap();
        assert!(provider.is_available());
    }

    #[test]
    fn test_estimate_cost_is_zero() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"fake audio").unwrap();
        let path = temp_file.path().to_path_buf();

        let provider = AudioFileProvider::new(path, 100).unwrap();
        assert_eq!(provider.estimate_cost(100), 0.0);
        assert_eq!(provider.estimate_cost(10000), 0.0);
    }
}
