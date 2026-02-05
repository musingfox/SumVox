# SumVox

**Intelligent voice notifications for AI coding tools**

SumVox transforms your AI coding sessions into voice notifications. It reads Claude Code conversation transcripts, generates concise summaries using LLM, and speaks them aloud - perfect for staying informed without context switching.

[![CI](https://github.com/musingfox/sumvox/actions/workflows/ci.yml/badge.svg)](https://github.com/musingfox/sumvox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/sumvox.svg)](https://crates.io/crates/sumvox)

## ✨ Features

- ⚡ **Blazing Fast**: 7ms startup time (Rust implementation)
- 🧠 **Multi-Model LLM Support**:
  - Google Gemini (recommended, tested and optimized)
  - Anthropic Claude, OpenAI GPT, Ollama (code support, not fully tested)
- 🔊 **Multi-TTS Engines**:
  - Google TTS (high quality, cloud-based)
  - macOS say (local, always available)
- 🎨 **Simple Configuration**: YAML format with comments and easy setup
- 🔄 **Smart Fallback**: Automatic provider switching on failure
- ✅ **Production Ready**: 90+ automated tests
- 📝 **Localization**: Native Chinese/English support
- 🎛️ **CLI Management**: Credential management and configuration tools
- 🪝 **Seamless Integration**: Claude Code hook support

## 🚀 Quick Start

### Installation

#### Homebrew (macOS)

```bash
brew tap musingfox/sumvox
brew install sumvox
```

#### Cargo

```bash
cargo install sumvox
```

#### Binary Download

Download the appropriate binary from [GitHub Releases](https://github.com/musingfox/sumvox/releases/latest):

```bash
# macOS Apple Silicon
curl -LO https://github.com/musingfox/sumvox/releases/latest/download/sumvox-macos-aarch64.tar.gz
tar xzf sumvox-macos-aarch64.tar.gz
sudo mv sumvox /usr/local/bin/

# macOS Intel
curl -LO https://github.com/musingfox/sumvox/releases/latest/download/sumvox-macos-x86_64.tar.gz
tar xzf sumvox-macos-x86_64.tar.gz
sudo mv sumvox /usr/local/bin/

# Linux x86_64
curl -LO https://github.com/musingfox/sumvox/releases/latest/download/sumvox-linux-x86_64.tar.gz
tar xzf sumvox-linux-x86_64.tar.gz
sudo mv sumvox /usr/local/bin/
```

### Setup

1. **Initialize configuration**:
   ```bash
   sumvox init
   ```

2. **Set API credentials** (Gemini recommended):
   ```bash
   sumvox credentials set google
   # Enter your Gemini API key (get it from https://ai.google.dev)
   ```

3. **Register Claude Code hook** in `~/.claude/settings.json`:
   ```json
   {
     "hooks": {
       "Notification": [{
         "matcher": "",
         "hooks": [{
           "type": "command",
           "command": "/opt/homebrew/bin/sumvox"
         }]
       }],
       "Stop": [{
         "matcher": "",
         "hooks": [{
           "type": "command",
           "command": "/opt/homebrew/bin/sumvox"
         }]
       }]
     }
   }
   ```

4. **Test it**: Trigger a Claude Code notification and hear your summary!

## ⭐ Recommended Configuration

For the best balance of performance, quality, and cost, we recommend **Google Gemini**:

```yaml
# ~/.config/sumvox/config.yaml
version: "1.0.0"

llm:
  providers:
    - name: google
      model: gemini-2.5-flash
      api_key: ${GEMINI_API_KEY}
      timeout: 10
    - name: ollama  # Local fallback
      model: llama3.2
      timeout: 60
  parameters:
    max_tokens: 10000
    temperature: 0.3

tts:
  providers:
    - name: macos      # Fast, offline, free
      voice: Meijia
      rate: 220
    - name: google     # High quality fallback
      model: gemini-2.5-flash-preview-tts
      voice: Aoede
      api_key: ${GEMINI_API_KEY}

summarization:
  turns: 1  # Only read last conversation turn
  system_message: "You are a voice notification assistant. Generate concise summaries suitable for voice playback in Traditional Chinese like an Engineer in Taiwan, keep your tone breezy, focus on result and next action."
  fallback_message: "任務已完成"

hooks:
  claude_code:
    notification_filter:
      - permission_prompt
      - idle_prompt
      - elicitation_dialog
    notification_tts_provider: macos  # Use fast local TTS
    stop_tts_provider: auto           # Try all providers
```

**Why This Setup?**

- ⚡ **Fast**: macOS TTS for instant notifications
- 💰 **Cost-effective**: Free local TTS, Gemini for LLM
- 🎯 **High quality**: Gemini summaries, multi-provider fallback
- 🔊 **Flexible**: Easy YAML editing with comments
- ✅ **Tested**: Fully validated configuration

**⚠️ Note**: Other LLM providers (Anthropic Claude, OpenAI GPT, Ollama) are supported in code but not fully tested yet. Use Gemini for the best experience.

**Config Format**: YAML is now the preferred format (supports comments). JSON format (`config.json`) is still supported for backward compatibility.

See [config/recommended.yaml](config/recommended.yaml) for the complete configuration with detailed comments.

## 📚 Documentation

- [Configuration Guide](#-configuration)
- [CLI Usage](#-usage)
- [Architecture](#️-architecture)
- [Development](#️-development)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## 🎯 Usage

### As Claude Code Hook (Recommended)

SumVox automatically integrates with Claude Code through hooks. Once registered (as shown in Quick Start), it will:

1. Receive event notifications from Claude Code (via stdin)
2. Read the session transcript
3. Generate a concise summary using LLM
4. Speak the summary via TTS
5. All automatic - no manual intervention!

### Standalone CLI

```bash
# Basic usage with transcript
echo '{"session_id":"test","transcript_path":"path/to/transcript.jsonl"}' | sumvox

# Override LLM provider
sumvox --provider google --model gemini-2.5-flash

# Override TTS provider
sumvox --tts google --tts-voice Aoede

# Debug mode
RUST_LOG=debug sumvox

# Manage credentials
sumvox credentials set google
sumvox credentials list
sumvox credentials remove google

# Show config location
sumvox init --show-path
```

## 📝 Configuration

### Location

**Preferred**: `~/.config/sumvox/config.yaml` (YAML format)
**Legacy**: `~/.config/sumvox/config.json` (JSON format, still supported)

SumVox will automatically load YAML if it exists, otherwise fall back to JSON.

### Structure

```yaml
version: "1.0.0"

llm:
  providers:  # LLM provider array with fallback
    - name: google
      model: gemini-2.5-flash
      api_key: ${GEMINI_API_KEY}
      timeout: 10
  parameters:
    max_tokens: 10000
    temperature: 0.3

tts:
  providers:  # TTS provider array with fallback
    - name: macos
      voice: Meijia
      rate: 220

summarization:
  turns: 1  # Read last N conversation turns
  system_message: "..."
  prompt_template: "..."
  fallback_message: "任務已完成"

hooks:
  claude_code:
    notification_filter:
      - permission_prompt
      - idle_prompt
    notification_tts_provider: macos  # Or "auto" for fallback chain
    stop_tts_provider: auto
```

### Key Changes from Previous Versions

- **Simplified**: Removed `enabled`, `max_length`, `initial_delay_ms`, `retry_delay_ms`, `cost_control`
- **YAML Format**: Now the preferred format with inline comments support
- **Sane Defaults**: Less configuration needed, works out of the box
- **Backward Compatible**: Old JSON configs still work (deprecated fields are ignored)

### LLM Providers

#### Google Gemini (Recommended)

```json
{
  "name": "google",
  "model": "gemini-2.5-flash",
  "api_key": "${GEMINI_API_KEY}",
  "timeout": 10
}
```

Models: `gemini-2.5-flash` (recommended, tested), `gemini-2.5-pro` (requires API upgrade)

#### Others (Code Support Only)

- **Anthropic**: `claude-haiku-4-5-20251001` (alias: `claude-haiku-4-5`)
- **OpenAI**: `gpt-4o-mini`
- **Ollama**: `llama3.2` (local)

### TTS Providers

#### Google TTS (Recommended)

```json
{
  "name": "google",
  "voice": "Aoede",
  "api_key": "${GEMINI_API_KEY}",
  "volume": 75
}
```

Popular voices: `Aoede`, `en-US-Journey-D`, `en-US-Journey-F`

#### macOS say

```json
{
  "name": "macos",
  "voice": "Tingting",
  "rate": 200
}
```

**Available voices:**
- Chinese (Simplified): `Tingting`
- Chinese (Traditional): `Meijia`
- English (US): `Samantha`, `Alex`
- System default: `""` (empty string, uses macOS language setting)

List all voices: `say -v '?'`

**Note:** macOS `say` command does not support volume control. Use system volume settings instead.

### Environment Variables

```bash
# Add to ~/.zshrc or ~/.bashrc
export GEMINI_API_KEY="AIza..."
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

## 🏗️ Architecture

```
Claude Code Event → Read Transcript → LLM Summary → TTS → Audio
     (stdin)           (JSONL)        (Gemini)    (Google/say)
```

**Array-Based Fallback**: If first provider fails, automatically tries next.

## 🛠️ Development

```bash
# Build
cargo build --release

# Test
cargo test
cargo test llm::
cargo test -- --nocapture

# Code quality
cargo fmt
cargo clippy -- -D warnings

# Run with debug
RUST_LOG=debug cargo run
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed development guide.

## 📊 Performance

- **Startup**: ~7ms
- **Memory**: ~10MB
- **Binary size**: ~2.1MB
- **LLM latency**: 1-2s (Gemini)
- **TTS latency**: 0.5-1s (macOS), 1-2s (Google)

## 💰 Cost Estimation

### Gemini + Google TTS (Recommended)

- **Per notification**: ~$0.00046
- **Daily budget ($0.10)**: ~217 notifications
- Breakdown:
  - Gemini LLM: ~$0.00006/summary
  - Google TTS: ~$0.0004/audio

## 🤝 Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md).

Areas for contribution:
- Test coverage for non-Gemini providers
- Additional TTS engines
- Windows support
- Documentation improvements

## 📄 License

MIT License - see [LICENSE](LICENSE)

## 🙏 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Powered by [Google Gemini](https://ai.google.dev/)
- Designed for [Claude Code](https://claude.com/claude-code)

## 🔗 Links

- **GitHub**: https://github.com/musingfox/sumvox
- **Issues**: https://github.com/musingfox/sumvox/issues
- **crates.io**: https://crates.io/crates/sumvox
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)

---

**Made with ❤️ for AI-powered development**
