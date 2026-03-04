# Changelog

All notable changes to SumVox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.2] - 2026-03-04

### Fixed
- **Auto Mode Volume Override**: Fixed `stop_volume` and `notification_volume` hook settings being ignored when TTS provider is set to `"auto"`. The volume override is now correctly propagated through `speak_with_provider_fallback`, preventing unexpectedly loud Gemini TTS playback.

### Changed
- **Recommended Config**: Updated Google TTS volume guidance to suggest 40-60 range (Gemini TTS output is loud by default).

## [1.2.1] - 2026-03-03

### Fixed
- **Audio File Volume Control**: Fixed `notification_volume` setting being ignored for `audio_file` TTS provider, causing playback at maximum volume (100) regardless of config. The hook volume override is now correctly applied, consistent with macOS and Google TTS providers.

## [1.2.0] - 2026-03-01

### Added
- **Audio File Playback Provider**: Play `.wav`, `.mp3`, `.flac`, `.ogg` sound effects via `audio_file` TTS provider
  - Single file or directory mode (random selection from directory)
  - Configurable volume control (0-100)
  - Non-blocking async playback for hooks
- **Cross-Process Notification Queue**: File-lock based queue (`flock`) prevents overlapping TTS output across concurrent hook invocations
- **Vorbis Codec Support**: Added OGG/Vorbis decoding via `rodio` with vorbis feature
- **E2E Test Infrastructure**: 25 end-to-end tests covering CLI commands, hook dispatch, audio playback, and concurrent queue behavior
- **Separate E2E CI Job**: E2E tests run independently with secret-based config, not blocking unit test pipeline

### Fixed
- **Silent Hook Audio Playback**: Fixed bugs where hook audio playback produced no sound
- **Async Audio Process Hang**: Fixed process hang in async audio playback by properly managing tokio runtime and thread lifecycle

## [1.1.1] - 2026-02-15

### Fixed
- **Transcript Turn Detection**: Fixed turn boundary logic that treated `tool_result` entries as new turns. In Claude Code transcripts, both human input and tool results share `type: "user"`, causing the last "turn" to often contain only `thinking`/`tool_use` blocks with no text — resulting in empty summaries on every Stop hook. Now only human-authored messages (with text content) are used as turn boundaries.

## [1.1.0] - 2026-02-10

### Added
- **TOML Configuration Format**: New TOML format support with automatic migration from YAML/JSON
  - Auto-migration creates timestamped backup of legacy config files
  - Priority: `config.toml` > `config.yaml` > `config.json`
  - Recommended config updated to TOML format (`config/recommended.toml`)
- **Separate Volume Control**: Independent volume settings for notifications and summaries
  - `notification_volume` (default: 80) - quieter for non-intrusive alerts
  - `stop_volume` (default: 100) - full volume for task completion summaries
  - Volume priority: CLI override > Hook config > Provider config > Defaults
  - **⚠️ Important**: Volume control only works with Google TTS; macOS TTS does not support volume control (uses system volume)

### Changed
- **Configuration Format**: TOML is now the preferred format (YAML/JSON still supported for backward compatibility)
- **Default Volumes**: Notification volume reduced from 100 to 80 for better user experience
- **Documentation**: Updated all config references to TOML format

### Fixed
- Documentation references to non-existent `credentials.rs` file in CONTRIBUTING.md
- Justfile `show-config` command using outdated config path
- Justfile invalid `credentials` command removed

### Removed
- GeminiCli hook format (unimplemented feature removed from codebase)

### Migration Guide
When upgrading to v1.1.0:
1. Your existing `config.yaml` or `config.json` will be automatically migrated to `config.toml`
2. A timestamped backup will be created (e.g., `config.yaml.backup-20260210-120000`)
3. To customize volumes, add to your `config.toml`:
   ```toml
   [hooks.claude_code]
   notification_volume = 80  # 0-100, default: 80
   stop_volume = 100         # 0-100, default: 100
   ```
4. **Volume Control Notes**:
   - Volume settings only work with **Google TTS**
   - **macOS TTS** does not support volume control - use system volume settings instead
   - To use volume control, set `notification_tts_provider = "google"` or `stop_tts_provider = "google"`

## [1.0.0] - 2026-02-05

### 🎉 Initial Release

**SumVox** - Intelligent voice notifications for AI coding tools

### Added

#### Core Features
- ⚡ **Blazing Fast**: 7ms startup time (Rust implementation)
- 🧠 **Multi-LLM Support**:
  - Google Gemini (gemini-2.5-flash)
  - Anthropic Claude (claude-haiku-4-5-20251001)
  - OpenAI GPT (gpt-5-nano)
  - Ollama (llama3.2, local)
  - All providers support custom API endpoints (base_url)
- 🔊 **Multi-TTS Engines**:
  - Google TTS (high quality, cloud-based)
  - macOS say (local, always available)
- ✅ **Production Ready**: 113 automated tests
- 🔄 **Array-Based Fallback**: Automatic provider switching on failure
- 🪝 **Claude Code Integration**: Seamless hook support with separate TTS configuration

#### Configuration
- **Format**: YAML (preferred) or JSON (backward compatible)
- **Location**: `~/.config/sumvox/config.yaml`
- **Default Config**: Includes all 4 LLM providers ready to use
- **Simple Setup**: Edit config file directly, no environment variables needed
- **Custom API Endpoints**: All providers support base_url for proxies/compatible APIs
- **Hook-Specific TTS**: Separate TTS provider for Notification and Stop hooks
- **Notification Filters**: Choose which notification types to speak
- **Thinking Control**: Support for Gemini 3, Claude extended thinking, OpenAI reasoning

#### Pipeline
1. Reads Claude Code session transcripts (JSONL format)
2. Generates concise summaries using LLM
3. Converts summaries to speech with TTS
4. Automatic provider fallback on errors

#### CLI Commands
- `sumvox init` - Initialize configuration with 4-provider template
- `sumvox say <text>` - Direct text-to-speech
- `sumvox sum <text>` - Summarize and speak
- CLI overrides: `--provider`, `--model`, `--tts`, `--tts-voice`

### Documentation
- Complete README with setup guide and fallback explanation
- Quick Start Guide (QUICKSTART.md) - 5-minute setup
- MIT License
- Contributing guidelines (CONTRIBUTING.md)
- GitHub Issue/PR templates
- Recommended configuration (config/recommended.yaml) with detailed comments
- Homebrew formula
- crates.io support

### Technical Details
- **Language**: Rust 2021 edition
- **Dependencies**: tokio, reqwest, serde, clap, rodio
- **Platforms**: macOS (x86_64, aarch64), Linux (x86_64, aarch64)
- **Minimum macOS**: 10.15
- **Build Optimizations**: LTO, size optimization, panic=abort

### Why Gemini?
- 🚀 **Performance**: 1-2s response time
- 💰 **Cost-effective**: Low pricing for high-frequency use
- 🎯 **Quality**: Accurate and fluent summaries
- 🔊 **Integrated**: One API key for both LLM and TTS
- ✅ **Tested**: Complete test coverage and optimization

### Migration from claude-voice
- New name: SumVox (Summarization + Voice)
- New config location: `~/.config/sumvox/config.yaml` (YAML format)
- Binary renamed: `claude-voice` → `sumvox`
- Homebrew tap: `musingfox/sumvox`
- Configuration: Edit YAML file directly instead of using environment variables

[1.2.0]: https://github.com/musingfox/sumvox/releases/tag/v1.2.0
[1.1.1]: https://github.com/musingfox/sumvox/releases/tag/v1.1.1
[1.1.0]: https://github.com/musingfox/sumvox/releases/tag/v1.1.0
[1.0.0]: https://github.com/musingfox/sumvox/releases/tag/v1.0.0
