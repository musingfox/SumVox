# SumVox

**Intelligent voice notifications for AI coding tools**

SumVox turns your AI coding sessions into voice notifications. It reads Claude Code conversation
transcripts, summarizes them with an LLM, and speaks the result aloud — so you stay informed
without switching context. Runs on macOS and Linux.

[![CI](https://github.com/musingfox/sumvox/actions/workflows/ci.yml/badge.svg)](https://github.com/musingfox/sumvox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- Single Rust binary, no runtime to install
- LLM summaries from Gemini, Anthropic, OpenAI, xAI Grok or Ollama
- Eight TTS engines: macOS `say`, espeak-ng and piper (local), Google TTS, Google Cloud TTS, xAI,
  OpenAI and ElevenLabs (cloud)
- Fallback chains: providers are tried in order until one succeeds, for both LLM and TTS
- Claude Code hooks for Notification and Stop events
- Optional macOS menu bar app: mute toggle, notification history, talking orb

## Quick Start

### Install

```bash
# Homebrew (macOS and Linux)
brew tap musingfox/sumvox
brew install sumvox

# Cargo (from source; sumvox is not published on crates.io)
cargo install --git https://github.com/musingfox/sumvox
```

Prebuilt binaries for macOS and Linux (aarch64 and x86_64) are on
[GitHub Releases](https://github.com/musingfox/sumvox/releases/latest).

### Step 1: Initialize configuration

```bash
sumvox init
```

This writes `~/.config/sumvox/config.toml` with the built-in defaults: an LLM chain of Google
Gemini, Anthropic, OpenAI and Ollama, and a TTS chain of the platform's local engine (`macos` on
macOS, `espeak` elsewhere) followed by Google TTS.

### Step 2: Set an API key

Edit the config and replace the `${PROVIDER_API_KEY}` placeholder with a real key. A key that
still starts with `${` counts as unset and that provider is skipped.

```toml
[[llm.providers]]
name = "google"
model = "gemini-3.1-flash-lite"
api_key = "AIza..."   # https://ai.google.dev
```

Instead of writing the key into the file, you can export the provider's usual variable
(`GEMINI_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `XAI_API_KEY`, `ELEVENLABS_API_KEY`).

### Step 3: Test playback

```bash
sumvox say "Hello, this is a test"
```

### Step 4: Register the Claude Code hook

Add to `~/.claude/settings.json` (find your path with `which sumvox`):

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

Notification events are spoken as short alerts. Stop events are summarized by the LLM first.

## Reference

- `sumvox --help` and `sumvox <command> --help` list every command and flag.
- [config/recommended.toml](config/recommended.toml) documents every config key and every
  provider, including the ones that ship commented out. Local TTS on Linux (espeak-ng, piper) and
  the Linux audio players are covered there.
- Set `SUMVOX_DISABLE` to any value to make SumVox do nothing, for example
  `SUMVOX_DISABLE=1 claude`.
- Set `RUST_LOG=debug` for logs.

## Troubleshooting

**"No API key found"**: an `api_key` still reading `${PROVIDER_API_KEY}` counts as unset. Paste
the real key or export the matching environment variable.

**"Provider not available"**: check the key, the network and the order of the fallback chain.

**No audio**: run `sumvox say "test"`. On macOS check System Settings, Sound, Output. On Linux
install one of `paplay`, `pw-play`, `ffplay`, `mpv` or `aplay`. SumVox lists the players it looked
for when it finds none.

**Ollama not responding**: run `ollama serve`, then `ollama pull llama3.2`.

## Menu Bar App (macOS)

An optional companion app lives in [`menubar/SumVoxMenu.swift`](menubar/SumVoxMenu.swift), one
Swift file using system frameworks only:

```bash
just menubar                      # builds target/release/sumvox-menubar
target/release/sumvox-menubar &
```

It shows a status item and a floating orb. Either one opens a menu with:

- **播放語音**: toggles `~/.config/sumvox/muted`. While that file exists SumVox skips playback
  but still records the text.
- **最近通知**: the last 50 notification texts from `~/.config/sumvox/history.log`; click one to
  copy it.
- **開啟設定檔**: opens `~/.config/sumvox/config.toml`.
- **結束**: quits.

When a notification is recorded, a speech bubble shows its text next to the orb, even while muted.
The app watches `history.log` and `now_playing` for changes and reads `muted` when the menu opens.
These three files are the whole contract between the two programs (`src/notify_log.rs`), so not
running the app leaves them unread.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT, see [LICENSE](LICENSE).
