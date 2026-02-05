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

> **⚡ Super Quick Setup?** See [QUICKSTART.md](QUICKSTART.md) for a 5-minute guide.

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

#### Step 1: Initialize Configuration

```bash
sumvox init
```

This creates `~/.config/sumvox/config.yaml` with sensible defaults:
- **LLM**: Google Gemini + Ollama (local fallback)
- **TTS**: macOS say (system default voice)
- **Language**: English (customize in config)

#### Step 2: Set API Key

Edit your config file:

```bash
open ~/.config/sumvox/config.yaml
```

Replace `${PROVIDER_API_KEY}` with your actual API key. For example, to use Google Gemini:

```yaml
llm:
  providers:
    - name: google
      model: gemini-2.5-flash
      api_key: "your-actual-api-key-here"  # Get from https://ai.google.dev
```

You can configure multiple providers. The system will try them in order until one succeeds.

#### Step 3: Test Voice Notification

```bash
sumvox say "Hello, this is a test"
```

If you hear the message, TTS is working! If not, check your system audio settings.

#### Step 4: Register Claude Code Hook

Add to `~/.claude/settings.json`:

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

**Note**: Update the path if you installed sumvox elsewhere (check with `which sumvox`)

#### Step 5: Verify Integration

Start a Claude Code session and trigger a notification. You should hear:
- **Notification events**: Instant voice alerts (e.g., "Permission required")
- **Stop events**: AI-generated summaries of the conversation

## 📖 Configuration Guide

### Quick Start Configuration

After running `sumvox init`, your config at `~/.config/sumvox/config.yaml` looks like this:

```yaml
version: "1.0.0"

llm:
  providers:
    - name: google
      model: gemini-2.5-flash
      api_key: ${GEMINI_API_KEY}
      timeout: 10
    - name: ollama
      model: llama3.2
      timeout: 60
  parameters:
    max_tokens: 10000
    temperature: 0.3

tts:
  providers:
    - name: macos
      rate: 200
    - name: google
      model: gemini-2.5-flash-preview-tts
      voice: Aoede
      api_key: ${GEMINI_API_KEY}

summarization:
  turns: 1
  system_message: "You are a voice notification assistant. Generate concise summaries suitable for voice playback."
  fallback_message: "Task completed"

hooks:
  claude_code:
    notification_filter:
      - permission_prompt
      - idle_prompt
      - elicitation_dialog
    notification_tts_provider: macos
    stop_tts_provider: auto
```

### Understanding Fallback Chains

SumVox uses **automatic fallback** for both LLM and TTS providers. If one fails, it tries the next:

#### LLM Fallback Example

```yaml
llm:
  providers:
    - name: google      # Try first
    - name: anthropic   # Try if Google fails
    - name: ollama      # Try if Anthropic fails (always works, local)
```

**How it works:**
1. Try Google Gemini with your API key
2. If fails (no key, network error, quota exceeded) → try Anthropic
3. If fails → try Ollama (local, no API key needed)
4. If all fail → use fallback message

#### TTS Fallback Example

```yaml
tts:
  providers:
    - name: macos    # Try first (free, fast, always available)
    - name: google   # Try if macOS fails (requires API key)
```

**Hook-specific TTS:**
```yaml
hooks:
  claude_code:
    # For quick notifications: use fast macOS TTS
    notification_tts_provider: macos

    # For summaries: use fallback chain (try all providers)
    stop_tts_provider: auto
```

- `macos` = Use only macOS TTS
- `google` = Use only Google TTS
- `auto` = Try all TTS providers in order (recommended for summaries)

### Configuration Examples

#### Example 1: Minimal Setup (Free, Local Only)

```yaml
llm:
  providers:
    - name: ollama
      model: llama3.2

tts:
  providers:
    - name: macos
```

**Pros:** Completely free, works offline, no API keys needed
**Cons:** Slower LLM, basic TTS quality

#### Example 2: Cloud-First with Local Fallback (Recommended)

```yaml
llm:
  providers:
    - name: google
      model: gemini-2.5-flash
      api_key: ${GEMINI_API_KEY}
    - name: ollama
      model: llama3.2

tts:
  providers:
    - name: macos
    - name: google
      model: gemini-2.5-flash-preview-tts
      voice: Aoede
      api_key: ${GEMINI_API_KEY}
```

**Pros:** Fast cloud LLM, reliable local fallback, free TTS
**Cons:** Requires internet, small API costs

#### Example 3: High Quality (Cloud Only)

```yaml
llm:
  providers:
    - name: anthropic
      model: claude-haiku-4-5-20251001
      api_key: ${ANTHROPIC_API_KEY}

tts:
  providers:
    - name: google
      model: gemini-2.5-flash-preview-tts
      voice: Aoede
      api_key: ${GEMINI_API_KEY}
```

**Pros:** Highest quality LLM and TTS
**Cons:** Requires internet, higher API costs, no fallback

### Customization Tips

#### Change Voice Language

```yaml
# English (macOS default)
tts:
  providers:
    - name: macos
      # Uses system default voice

# Chinese
tts:
  providers:
    - name: macos
      voice: Meijia  # Traditional Chinese
      # voice: Tingting  # Simplified Chinese

# List available voices
# Run in terminal: say -v ?
```

#### Customize Summary Style

```yaml
summarization:
  system_message: "You are a helpful assistant. Summarize in a friendly, casual tone."
  # For Chinese: "你是一個友善的助理。用輕鬆的語氣總結內容。"

  fallback_message: "Done!"
  # For Chinese: "完成了！"
```

#### Filter Notification Types

```yaml
hooks:
  claude_code:
    # Speak all notifications
    notification_filter:
      - "*"

    # Or be selective
    notification_filter:
      - permission_prompt  # "Permission required"
      - idle_prompt        # "Waiting for input"
```

See [config/recommended.yaml](config/recommended.yaml) for more examples and detailed comments.

## 💡 Real-World Usage Scenarios

### Scenario 1: Multi-tasking Developer

**Setup:** Fast local TTS for notifications, cloud LLM for summaries

```yaml
tts:
  providers:
    - name: macos  # Instant alerts

hooks:
  claude_code:
    notification_tts_provider: macos  # Quick "Permission required" alerts
    stop_tts_provider: auto           # Detailed summaries when task completes
```

**Workflow:**
1. Start a Claude Code task
2. Switch to another window to continue working
3. Hear instant notifications when Claude needs input
4. Hear AI-generated summary when task completes

### Scenario 2: Offline Development

**Setup:** All local, no cloud dependencies

```yaml
llm:
  providers:
    - name: ollama
      model: llama3.2

tts:
  providers:
    - name: macos
```

**Benefits:**
- ✅ Works without internet
- ✅ Zero API costs
- ✅ Privacy (no data sent to cloud)
- ⚠️ Slower LLM (30-60s for summaries)

### Scenario 3: High-Quality Production

**Setup:** Best quality cloud services with fallback

```yaml
llm:
  providers:
    - name: anthropic
      model: claude-haiku-4-5-20251001
    - name: google
      model: gemini-2.5-flash
    - name: ollama
      model: llama3.2

tts:
  providers:
    - name: google
      voice: Aoede
    - name: macos
```

**Fallback chain:**
1. Try Anthropic (highest quality)
2. Fall back to Gemini (faster, cheaper)
3. Fall back to Ollama (local, always works)
4. TTS: Google → macOS (quality → reliability)

### Scenario 4: Cost-Conscious Setup

**Setup:** Minimal cloud usage, maximize free tier

```yaml
llm:
  providers:
    - name: google      # Free tier: 15 requests/min
      model: gemini-2.5-flash
    - name: ollama      # Unlimited local fallback

tts:
  providers:
    - name: macos       # Free, unlimited

hooks:
  claude_code:
    notification_tts_provider: macos  # Free
    stop_tts_provider: macos          # Free (skip Google TTS)
```

**Result:** ~$0.01/day for LLM, $0 for TTS

## 📚 Documentation

- [Quick Start](#-quick-start)
- [Configuration Guide](#-configuration-guide)
- [CLI Commands](#-cli-commands)
- [How It Works](#️-how-it-works)
- [Advanced Configuration](#-advanced-configuration)
- [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## 🎯 CLI Commands

### Initialize Configuration

```bash
# Create default config
sumvox init

# Force overwrite existing config
sumvox init --force
```

### Direct TTS (No LLM)

```bash
# Speak text directly
sumvox say "Hello world"

# Specify TTS provider
sumvox say "Hello" --tts macos
sumvox say "Hello" --tts google --voice Aoede

# Adjust speech rate (macOS only, 90-300)
sumvox say "Hello" --rate 250
```

### LLM Summarization + TTS

```bash
# Summarize text from argument
sumvox sum "Long text to summarize..."

# Read from stdin
echo "Long text..." | sumvox sum -

# Specify LLM provider
sumvox sum "Text" --provider anthropic

# Just print summary (no speech)
sumvox sum "Text" --no-speak
```

### Hook Mode (Automatic)

When registered as a Claude Code hook, SumVox runs automatically:

```bash
# Receives JSON via stdin
echo '{"hook_event_name":"Notification","message":"Test"}' | sumvox

# Or from Claude Code (automatic)
# No manual invocation needed!
```

### Debug Mode

```bash
# Show detailed logs
RUST_LOG=debug sumvox say "test"

# Log levels: trace, debug, info, warn, error
RUST_LOG=trace sumvox
```

## 🔧 Advanced Configuration

### Provider Reference

#### LLM Providers

| Provider | Model | API Key Required | Speed | Cost | Tested |
|----------|-------|------------------|-------|------|--------|
| **Google Gemini** | `gemini-2.5-flash` | ✅ | Fast | Low | ✅ |
| **Anthropic** | `claude-haiku-4-5-20251001` | ✅ | Fast | Medium | ⚠️ |
| **OpenAI** | `gpt-4o-mini` | ✅ | Medium | Medium | ⚠️ |
| **Ollama** | `llama3.2` | ❌ | Slow | Free | ✅ |

✅ = Fully tested | ⚠️ = Code support only

**Get API Keys:**
- Gemini: https://ai.google.dev
- Anthropic: https://console.anthropic.com
- OpenAI: https://platform.openai.com

#### TTS Providers

| Provider | Voices | API Key Required | Speed | Quality | Cost |
|----------|--------|------------------|-------|---------|------|
| **macOS say** | System voices | ❌ | Instant | Good | Free |
| **Google TTS** | 40+ voices | ✅ | Fast | Excellent | Low |

**macOS Voices:**
- Run `say -v ?` to list all available voices
- No voice specified = uses system default language
- English: `Alex`, `Samantha`, `Daniel`
- Chinese: `Meijia` (繁體), `Tingting` (简体)

**Google TTS Voices:**
- `Aoede`, `Charon`, `Fenrir`, `Kore` (expressive, high quality)
- Full list: https://cloud.google.com/text-to-speech/docs/voices

### Configuration File Structure

```yaml
version: "1.0.0"

llm:
  providers: [...]      # Array, tries in order
  parameters: {...}     # Shared across all providers

tts:
  providers: [...]      # Array, tries in order

summarization:
  turns: 1              # Number of conversation turns to read
  system_message: "..." # LLM instruction for summary style
  prompt_template: "..." # Template with {context} placeholder
  fallback_message: "..." # Spoken when LLM fails

hooks:
  claude_code:
    notification_filter: [...]  # Which notification types to speak
    notification_tts_provider: "macos" | "google" | "auto"
    stop_tts_provider: "macos" | "google" | "auto"
```

### Environment Variables

Optional debug logging can be enabled:

```bash
export RUST_LOG="info"  # Options: debug, info, warn, error
```

**Note:** API keys should be configured in `~/.config/sumvox/config.yaml`, not as environment variables.

### Troubleshooting

**Problem: "No API key found"**
```bash
# Check your config file
cat ~/.config/sumvox/config.yaml

# Make sure api_key is set correctly (not ${PROVIDER_API_KEY})
# Edit the config file:
open ~/.config/sumvox/config.yaml

# Replace ${PROVIDER_API_KEY} with your actual API key
```

**Problem: "Provider not available"**
- Check internet connection (for cloud providers)
- Verify API key is correct
- Check provider fallback chain order

**Problem: "No audio output"**
- Test with: `sumvox say "test"`
- Check system volume settings
- For macOS: System Settings → Sound → Output

**Problem: "Ollama not responding"**
```bash
# Start Ollama
ollama serve

# Pull model if not installed
ollama pull llama3.2
```

## 🏗️ How It Works

### Event Flow

```
┌─────────────┐
│ Claude Code │
│   Session   │
└──────┬──────┘
       │ Hook Event (JSON via stdin)
       ▼
┌─────────────────────┐
│  SumVox Process     │
│                     │
│  1. Parse Event     │
│  2. Read Transcript │◄── ~/.claude/projects/.../transcript.jsonl
│  3. Generate Summary│◄── LLM Provider (with fallback)
│  4. Speak Text      │◄── TTS Provider (with fallback)
└─────────────────────┘
       │ Audio Output
       ▼
   🔊 System Audio
```

### Fallback Mechanism

**LLM Fallback:**
```
Try Provider 1 (Google)
  ├─ Success → Generate Summary
  └─ Fail → Try Provider 2 (Anthropic)
      ├─ Success → Generate Summary
      └─ Fail → Try Provider 3 (Ollama)
          ├─ Success → Generate Summary
          └─ Fail → Use Fallback Message
```

**TTS Fallback (when `stop_tts_provider: auto`):**
```
Try Provider 1 (macOS)
  ├─ Success → Speak Text
  └─ Fail → Try Provider 2 (Google)
      ├─ Success → Speak Text
      └─ Fail → Silent (graceful degradation)
```

**Key Features:**
- ⚡ **Fast**: Providers tried in order, first success wins
- 🛡️ **Reliable**: Automatic retry with different providers
- 🎯 **Configurable**: Control fallback order via config
- 🔕 **Graceful**: Never crashes, worst case = silent or fallback message

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
