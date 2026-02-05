# Changelog

All notable changes to SumVox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[1.0.0]: https://github.com/musingfox/sumvox/releases/tag/v1.0.0
