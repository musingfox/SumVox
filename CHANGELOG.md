# Changelog

All notable changes to SumVox will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-02-04

### 🎉 Initial Release

**SumVox** - Intelligent voice notifications for AI coding tools

### Added

#### Core Features
- ⚡ **Blazing Fast**: 7ms startup time (Rust implementation)
- 🧠 **LLM Support**:
  - Google Gemini (recommended, tested and optimized)
  - Other providers (Anthropic Claude, OpenAI GPT, Ollama) - code support, not yet fully tested
- 🔊 **Multi-TTS Engines**:
  - Google TTS (high quality, cloud-based)
  - macOS say (local, always available)
- 💰 **Cost Control**: Daily budget limits and usage tracking
- ✅ **Production Ready**: 90+ automated tests
- 🔄 **Array-Based Fallback**: Automatic provider switching on failure
- 📝 **Localization**: Native Chinese/English support
- 🎛️ **CLI Management**: Credential management and configuration tools
- 🪝 **Claude Code Integration**: Seamless hook support

#### Configuration
- XDG standard config location: `~/.config/sumvox/config.json`
- Environment variable support for API keys
- Recommended Gemini-based configuration template
- Array-based provider fallback chains for both LLM and TTS
- Configurable notification filters
- Volume control for both macOS and Google TTS
- Thinking control for Gemini models

#### Pipeline
1. Reads Claude Code session transcripts (JSONL format)
2. Generates concise summaries using LLM (configurable max length)
3. Converts summaries to speech with TTS
4. Automatic provider fallback on errors

#### CLI Commands
- `sumvox init` - Initialize configuration
- `sumvox credentials set <provider>` - Set API credentials
- `sumvox credentials list` - List configured providers
- `sumvox credentials remove <provider>` - Remove credentials
- CLI overrides: `--provider`, `--model`, `--tts`, `--tts-voice`

### Documentation
- Complete README with quick start guide
- MIT License
- Contributing guidelines (CONTRIBUTING.md)
- GitHub Issue/PR templates
- Recommended configuration (config/recommended.json)
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
- New config location: `~/.config/sumvox/config.json` (XDG standard)
- Binary renamed: `claude-voice` → `sumvox`
- Homebrew tap: `musingfox/sumvox`

[1.0.0]: https://github.com/musingfox/sumvox/releases/tag/v1.0.0
