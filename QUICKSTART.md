# SumVox Quick Start Guide

## 🚀 Installation (1 minute)

```bash
# Homebrew (macOS and Linux)
brew tap musingfox/sumvox
brew install sumvox

# Cargo
cargo install sumvox

# Binary download: https://github.com/musingfox/sumvox/releases
```

## ⚙️ Setup (3 minutes)

### 1. Initialize Config

```bash
sumvox init
```

### 2. Set API Key

Edit `~/.config/sumvox/config.toml` and replace the `${PROVIDER_API_KEY}` placeholder with a real
key — a placeholder counts as unset, so the provider is skipped:

```toml
[[llm.providers]]
name = "google"
model = "gemini-3.1-flash-lite"
api_key = "AIza..."   # Get one from https://ai.google.dev
```

Or leave the placeholder and export `GEMINI_API_KEY` instead.

### 3. Test Voice

```bash
sumvox say "Hello, SumVox is working!"
```

### 4. Configure Claude Code Hook

Edit `~/.claude/settings.json` (find your path with `which sumvox`):

```json
{
  "hooks": {
    "Notification": [{
      "matcher": "",
      "hooks": [{"type": "command", "command": "/opt/homebrew/bin/sumvox"}]
    }],
    "Stop": [{
      "matcher": "",
      "hooks": [{"type": "command", "command": "/opt/homebrew/bin/sumvox"}]
    }]
  }
}
```

## 🎯 Common Configurations

### Default (Recommended)

```toml
[[llm.providers]]
name = "google"
model = "gemini-3.1-flash-lite"
api_key = "${PROVIDER_API_KEY}"

[[llm.providers]]
name = "ollama"
model = "llama3.2"
timeout = 60

[[tts.providers]]
name = "macos"

[[tts.providers]]
name = "google"
model = "gemini-2.5-flash-preview-tts"   # required
voice = "Aoede"                          # required
api_key = "${PROVIDER_API_KEY}"
```

**Pros:** Fast cloud LLM, free local TTS, reliable fallback

### Free & Offline

```toml
[[llm.providers]]
name = "ollama"
model = "llama3.2"
timeout = 60

[[tts.providers]]
name = "macos"   # Linux: "espeak", or "piper" with a downloaded .onnx voice
```

**Pros:** Zero cost, works offline
**Cons:** Slower (30-60s for summaries)

On Linux, `macos` is unavailable — install `espeak-ng` (`pacman -S espeak-ng` / `apt install
espeak-ng`) or piper (`uv tool install piper-tts` plus a voice model from
[rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices)) and name it instead. See
[README](README.md#local-tts-on-linux) for the details.

### High Quality

```toml
[[llm.providers]]
name = "anthropic"
model = "claude-haiku-4-5-20251001"
api_key = "${PROVIDER_API_KEY}"

[[tts.providers]]
name = "google"
model = "gemini-2.5-flash-preview-tts"
voice = "Aoede"
api_key = "${PROVIDER_API_KEY}"
```

## 🎨 Customization Cheat Sheet

### Change Voice Language

```toml
[[tts.providers]]
name = "macos"
voice = "Meijia"   # Chinese; "Daniel" for English; omit for the system default
```

List voices: `say -v ?`

### Change Summary Style

```toml
[summarization]
# "last_message" skips the transcript file read and uses Claude Code's
# last_assistant_message directly (LLM summarization still runs)
content_source = "transcript"   # or "last_message"
system_message = "Be concise and technical"
```

### Filter Notifications

```toml
[hooks.claude_code]
notification_filter = ["*"]   # all; or ["permission_prompt", "idle_prompt"]
```

### TTS Provider per Hook

```toml
[hooks.claude_code]
notification_tts_provider = "macos"  # fast, local, free
stop_tts_provider = "auto"           # best quality: try the whole fallback chain
notification_volume = 80             # 0-100
stop_volume = 100
```

**Note:** Volume works with every provider. The one exception is a Linux host where `aplay` is the
only player installed — it has no volume control, so SumVox skips it when `volume = 0`.

## 🔇 Temporarily Disable SumVox

```bash
SUMVOX_DISABLE=1 claude       # Bash / Zsh
env SUMVOX_DISABLE=1 claude   # Fish
```

**Tip:** Create an alias for quick access:

```bash
alias claude-quiet='SUMVOX_DISABLE=1 claude'      # Bash / Zsh
alias claude-quiet 'env SUMVOX_DISABLE=1 claude'  # Fish
```

## 🔧 Troubleshooting

### "No API key found"

```bash
cat ~/.config/sumvox/config.toml
# An api_key still reading ${PROVIDER_API_KEY} counts as unset
```

### "No audio"

```bash
sumvox say "test"
# macOS: System Settings → Sound → Output
# Linux: install paplay / pw-play / ffplay / mpv / aplay
```

### "Ollama not responding"

```bash
ollama serve
ollama pull llama3.2
```

### Debug mode

```bash
RUST_LOG=debug sumvox say "test"
```

## 📖 Full Documentation

- [README.md](README.md) - Complete guide
- [config/recommended.toml](config/recommended.toml) - Annotated config example
- [CHANGELOG.md](CHANGELOG.md) - Version history

## 💬 Support

- Issues: https://github.com/musingfox/sumvox/issues
- Discussions: https://github.com/musingfox/sumvox/discussions
