# SumVox Quick Start Guide

## 🚀 Installation (1 minute)

### macOS (Homebrew)
```bash
brew tap musingfox/sumvox
brew install sumvox
```

### Other Methods
```bash
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

Edit your config file:
```bash
open ~/.config/sumvox/config.toml
```

Replace `${PROVIDER_API_KEY}` with your actual API key:
```yaml
providers:
  - name: google
    model: gemini-2.5-flash
    api_key: "your-actual-api-key-here"  # Get from https://ai.google.dev
```

### 3. Test Voice
```bash
sumvox say "Hello, SumVox is working!"
```

### 4. Configure Claude Code Hook

Edit `~/.claude/settings.json`:
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

**Find sumvox path:** `which sumvox`

## 🎯 Common Configurations

### Default (Recommended)
```yaml
# ~/.config/sumvox/config.toml
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
      voice: Aoede
```

**Pros:** Fast cloud LLM, free local TTS, reliable fallback
**Cost:** ~$0.01/day

### Free & Offline
```yaml
llm:
  providers:
    - name: ollama
      model: llama3.2

tts:
  providers:
    - name: macos
```

**Pros:** Zero cost, works offline
**Cons:** Slower (30-60s for summaries)

### High Quality
```yaml
llm:
  providers:
    - name: anthropic
      model: claude-haiku-4-5-20251001

tts:
  providers:
    - name: google
      voice: Aoede
```

**Pros:** Best quality
**Cost:** ~$0.10/day

## 🎨 Customization Cheat Sheet

### Change Voice Language
```yaml
tts:
  providers:
    - name: macos
      voice: Meijia  # Chinese
      # voice: Daniel  # English
      # voice: ""  # System default
```

List voices: `say -v ?`

### Change Summary Style
```yaml
summarization:
  # Skip transcript file I/O by using Claude Code's last_assistant_message field
  # directly (LLM summarization still runs — only the JSONL read is skipped)
  content_source: "last_message"  # Or "transcript" (default)
  
  system_message: "Be concise and technical"
  # Or: "Be friendly and casual"
  # Or: "用中文總結，語氣輕鬆"
```

### Filter Notifications
```yaml
hooks:
  claude_code:
    notification_filter:
      - "*"  # All notifications
      # Or selective:
      # - permission_prompt
      # - idle_prompt
```

### TTS Provider per Hook
```yaml
hooks:
  claude_code:
    notification_tts_provider: macos  # Fast (no volume control)
    stop_tts_provider: auto           # Best quality (Google TTS supports volume)
    notification_volume: 80           # Only works with Google TTS
    stop_volume: 100                  # Only works with Google TTS
```

**Note:** Volume control only works with Google TTS. macOS TTS uses system volume.

## 🔇 Temporarily Disable SumVox

Launch Claude Code without voice notifications:

```bash
# Bash / Zsh
SUMVOX_DISABLE=1 claude

# Fish
env SUMVOX_DISABLE=1 claude
```

**Tip:** Create an alias for quick access:
```bash
alias claude-quiet='SUMVOX_DISABLE=1 claude'  # Bash/Zsh
alias claude-quiet 'env SUMVOX_DISABLE=1 claude'  # Fish
```

## 🔧 Troubleshooting

### "No API key found"
```bash
# Check your config file
cat ~/.config/sumvox/config.toml

# Make sure api_key is set (not ${PROVIDER_API_KEY})
# Edit config:
open ~/.config/sumvox/config.toml
```

### "No audio"
```bash
sumvox say "test"  # Should hear "test"
# Check: System Settings → Sound → Output
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
- [config/recommended.yaml](config/recommended.yaml) - Annotated config example
- [CHANGELOG.md](CHANGELOG.md) - Version history

## 💬 Support

- Issues: https://github.com/musingfox/sumvox/issues
- Discussions: https://github.com/musingfox/sumvox/discussions
