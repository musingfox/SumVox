# SumVox

**Intelligent voice notifications for AI coding tools**

SumVox turns your AI coding sessions into voice notifications. It reads Claude Code conversation
transcripts, summarizes them with an LLM, and speaks the result aloud — so you stay informed
without switching context. Runs on macOS and Linux.

[![CI](https://github.com/musingfox/sumvox/actions/workflows/ci.yml/badge.svg)](https://github.com/musingfox/sumvox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![crates.io](https://img.shields.io/crates/v/sumvox.svg)](https://crates.io/crates/sumvox)

## ✨ Features

- ⚡ **Single Rust binary** — no runtime, no dependencies to install
- 🧠 **Multi-model LLM**: Google Gemini (recommended), Anthropic Claude, OpenAI GPT, xAI Grok, Ollama
- 🔊 **Eight TTS engines**: macOS `say`, espeak-ng, piper (local/offline) plus Google TTS,
  Google Cloud TTS, xAI, OpenAI and ElevenLabs (cloud)
- 🔄 **Smart fallback**: providers are tried in order until one succeeds — for both LLM and TTS
- 🔉 **Volume control everywhere**: one 0–100 knob applies to every engine on both platforms
- 🎨 **TOML configuration** with inline comments
- 📝 **Localization**: works in any language the chosen engines support
- 🪝 **Claude Code hooks**: Notification and Stop events out of the box
- 🖥️ **Optional macOS menu bar app**: mute toggle, notification history, talking orb

## 🚀 Quick Start

> **In a hurry?** [QUICKSTART.md](QUICKSTART.md) is the 5-minute version.

### Installation

```bash
# Homebrew (macOS and Linux)
brew tap musingfox/sumvox
brew install sumvox

# Cargo
cargo install sumvox
```

Or grab a binary from [GitHub Releases](https://github.com/musingfox/sumvox/releases/latest) —
`sumvox-macos-aarch64`, `sumvox-macos-x86_64`, `sumvox-linux-x86_64`, `sumvox-linux-aarch64`:

```bash
curl -LO https://github.com/musingfox/sumvox/releases/latest/download/sumvox-macos-aarch64.tar.gz
tar xzf sumvox-macos-aarch64.tar.gz
sudo mv sumvox /usr/local/bin/
```

### Step 1: Initialize configuration

```bash
sumvox init
```

This writes `~/.config/sumvox/config.toml` with:

- **LLM**: Google Gemini → Anthropic → OpenAI → Ollama (local last)
- **TTS**: macOS `say` → Google TTS. On Linux, replace `macos` with `espeak` or `piper` —
  see [Local TTS on Linux](#local-tts-on-linux)

An existing `config.yaml` or `config.json` from an older release is migrated to TOML on first run
(the original is kept as a timestamped backup).

### Step 2: Set an API key

```bash
$EDITOR ~/.config/sumvox/config.toml
```

Replace the `${PROVIDER_API_KEY}` placeholder with a real key. The placeholder is not expanded —
a provider whose key still looks like `${...}` is treated as unconfigured and skipped:

```toml
[[llm.providers]]
name = "google"
model = "gemini-3.1-flash-lite"
api_key = "AIza..."   # Get one from https://ai.google.dev
```

Alternatively, leave the placeholder and export the key as an environment variable — see
[Environment Variables](#environment-variables).

### Step 3: Test playback

```bash
sumvox say "Hello, this is a test"
```

If you hear nothing, see [Troubleshooting](#troubleshooting).

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

Start a Claude Code session and you should hear instant alerts on **Notification** events
("Permission required") and an LLM-written summary on **Stop** events.

## 📖 Configuration

Config lives at `~/.config/sumvox/config.toml`.
[config/recommended.toml](config/recommended.toml) is a fully commented reference with every
provider block, including the ones that ship commented out.

### What `sumvox init` writes

Abridged below — the generated file also carries the default `prompt_template`, and puts an
`api_key = "${PROVIDER_API_KEY}"` placeholder on *every* provider, including the ones that need no
key (`ollama`, `macos`). Leave those alone or delete them; a placeholder key is treated as unset.

```toml
[[llm.providers]]
name = "google"
model = "gemini-3.1-flash-lite"
api_key = "${PROVIDER_API_KEY}"
timeout = 10

[[llm.providers]]
name = "anthropic"
model = "claude-haiku-4-5-20251001"
api_key = "${PROVIDER_API_KEY}"
timeout = 10

[[llm.providers]]
name = "openai"
model = "gpt-5-nano"
api_key = "${PROVIDER_API_KEY}"
timeout = 10

[[llm.providers]]
name = "ollama"
model = "llama3.2"
timeout = 60

[llm.parameters]
max_tokens = 10000
temperature = 0.3
disable_thinking = false

[[tts.providers]]
name = "macos"
rate = 200

[[tts.providers]]
name = "google"
model = "gemini-2.5-flash-preview-tts"
voice = "Aoede"
api_key = "${PROVIDER_API_KEY}"

[summarization]
content_source = "transcript"
turns = 1
system_message = "You are a voice notification assistant. Generate concise summaries suitable for voice playback."
fallback_message = "Task completed"

[hooks.claude_code]
notification_filter = ["permission_prompt", "idle_prompt", "elicitation_dialog"]
notification_tts_provider = "macos"
stop_tts_provider = "auto"
```

### Fallback chains

`[[llm.providers]]` and `[[tts.providers]]` are ordered lists. Each entry is tried in turn until
one succeeds; an entry that is missing its API key, binary or voice model is skipped rather than
failing the chain. If every LLM fails, `summarization.fallback_message` is spoken. If every TTS
fails, SumVox stays silent instead of crashing.

### Per-hook TTS selection

```toml
[hooks.claude_code]
notification_tts_provider = "macos"  # short alerts: fastest engine
stop_tts_provider = "auto"           # summaries: whole fallback chain
notification_volume = 80             # 0-100
stop_volume = 100
```

A provider selector is either `auto` (walk the whole chain) or the name of one configured entry:
`macos`, `espeak`, `piper`, `google`, `cloud_tts`, `gemini_tts`, `xai`, `openai`, `elevenlabs`,
`audio_file`.

### Common setups

**Free and offline** — no API keys, no network:

```toml
[[llm.providers]]
name = "ollama"
model = "llama3.2"
timeout = 60

[[tts.providers]]
name = "macos"   # Linux: "espeak", or "piper" with a downloaded .onnx voice
```

Slower summaries (30–60 s), but zero cost and nothing leaves the machine.

**Cloud LLM with a local fallback** (recommended) — fast summaries, free speech:

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
model = "gemini-2.5-flash-preview-tts"
voice = "Aoede"
api_key = "${PROVIDER_API_KEY}"
```

**Highest quality** — expressive cloud voices for everything:

```toml
[[llm.providers]]
name = "anthropic"
model = "claude-haiku-4-5-20251001"
api_key = "${PROVIDER_API_KEY}"

[[tts.providers]]
name = "elevenlabs"
model = "eleven_multilingual_v2"
voice = "21m00Tcm4TlvDq8ikWAM"
api_key = "${PROVIDER_API_KEY}"
```

### Customization

```toml
# Voice and language (macOS: run `say -v ?` to list voices)
[[tts.providers]]
name = "macos"
voice = "Meijia"   # Traditional Chinese; Tingting = Simplified; omit for the system default
rate = 200

[summarization]
# "transcript" reads the JSONL transcript; "last_message" uses Claude Code's
# last_assistant_message field directly, skipping the file read
content_source = "transcript"
turns = 1
system_message = "Summarize in a friendly, casual tone."
prompt_template = "Based on the following context, generate a concise summary.\n\nContext:\n{context}\n\nSummary:"
fallback_message = "Task completed"

[hooks.claude_code]
# Which notifications get spoken. ["*"] speaks all of them; [] disables them.
# Entries are matched literally against Claude Code's own `notification_type`
# field, so the valid values are whatever it sends.
notification_filter = ["permission_prompt", "idle_prompt", "elicitation_dialog"]
```

## 🎯 CLI Commands

```bash
sumvox init                  # create the config file
sumvox init --force          # overwrite an existing one

# Direct TTS, no LLM
sumvox say "Hello world"
sumvox say "Hello" --tts macos --voice Daniel
sumvox say "Hello" --tts espeak --voice cmn+f3
sumvox say "Hello" --tts piper --voice ~/voices/zh_CN-huayan-medium.onnx
sumvox say "Hello" --tts google --voice Aoede
sumvox say "Hello" --volume 80

# Speech rate: local engines only, ignored by the cloud providers.
#   macos  - words per minute, 90-300
#   espeak - words per minute, 80-450 (espeak-ng default 175)
#   piper  - inverse length-scale; 200 is piper's own default, higher is faster
sumvox say "Hello" --rate 250

# LLM summarization, then speech
sumvox sum "Long text to summarize..."
echo "Long text..." | sumvox sum -
sumvox sum "Text" --provider anthropic
sumvox sum "Text" --no-speak      # print the summary, don't speak it

# Hook mode: JSON on stdin, format auto-detected
echo '{"hook_event_name":"Notification","message":"Test"}' | sumvox

# Logs: trace, debug, info, warn, error
RUST_LOG=debug sumvox say "test"
```

## 🔧 Provider Reference

### LLM providers

| Provider | Example model | API key | Speed | Cost |
|----------|---------------|---------|-------|------|
| **Google Gemini** | `gemini-3.1-flash-lite` | ✅ | Fast | Low |
| **Anthropic** | `claude-haiku-4-5-20251001` | ✅ | Fast | Medium |
| **OpenAI** | `gpt-5-nano` | ✅ | Medium | Medium |
| **xAI Grok** | `grok-build-0.1` | ✅ | Fast | Low |
| **Ollama** | `llama3.2` | ❌ | Slow | Free |

Keys: [Gemini](https://ai.google.dev) · [Anthropic](https://console.anthropic.com) ·
[OpenAI](https://platform.openai.com) · [xAI](https://console.x.ai).
xAI uses the OpenAI-compatible endpoint at `https://api.x.ai/v1`; the provider name is `xai`
(alias `grok`). Any provider accepts an optional `base_url` for proxies and compatible APIs.

### TTS providers

Every engine supports the 0–100 `volume` knob. The single exception is a Linux host where `aplay`
is the only audio player installed — it cannot change volume, so SumVox skips it when
`volume = 0`.

| Provider | Voices | API key | Quality | Cost |
|----------|--------|---------|---------|------|
| **macOS say** | System voices | ❌ | Good | Free |
| **espeak-ng** | 100+ languages/variants | ❌ | Robotic | Free |
| **piper** | One model per voice | ❌ | Good (neural) | Free |
| **Google TTS** | 6 Gemini voices | ✅ | Excellent | ~$0.016/1K chars |
| **Google Cloud TTS** | 100+ (Standard/WaveNet/Chirp3-HD) | ✅ Service account | Professional | $4–16/1M chars |
| **xAI TTS** | 5 voices | ✅ | Excellent | $4.20/1M chars |
| **OpenAI TTS** | 10 voices | ✅ | Excellent | ~$0.015/min |
| **ElevenLabs** | Library + Voice Design | ✅ | Premium | $0.06–0.12/1K chars |

- **macOS say** — `say -v ?` lists voices. Omit `voice` for the system default. English: `Alex`,
  `Samantha`, `Daniel`. Chinese: `Meijia` (繁體), `Tingting` (简体).
- **Google TTS (Gemini)** — `Aoede`, `Charon`, `Fenrir`, `Kore`, `Puck`, `Orus`. Both `model`
  (e.g. `gemini-2.5-flash-preview-tts`) and `voice` are required; there is no default for either.
- **Google Cloud TTS** — `cloud_tts`; needs `service_account_key` pointing at a JSON key file
  ([setup](https://cloud.google.com/text-to-speech/docs/before-you-begin)). Set a Gemini-TTS
  `model` (e.g. `gemini-2.5-flash-tts`) to use bare voice names plus a `style_prompt`; the
  `gemini_tts` alias binds to its own config entry.
- **xAI TTS** — `eve` (default), `ara`, `rex`, `sal`, `leo`. Language is auto-detected or pinned
  with `language_code`.
- **OpenAI TTS** — `alloy`, `ash`, `ballad`, `coral`, `echo`, `fable`, `nova`, `onyx`, `sage`,
  `shimmer`, plus `speed` (0.25–4.0) and a `style_prompt` for tone and accent.
- **ElevenLabs** — `voice` is a Voice ID (default `21m00Tcm4TlvDq8ikWAM`, Rachel) from the
  [voice library](https://elevenlabs.io/app/voice-library). Models: `eleven_flash_v2_5`,
  `eleven_turbo_v2_5`, `eleven_multilingual_v2`, `eleven_v3`. Tuning: `speed` (0.7–1.2),
  `stability` and `style` (0.0–1.0).

### Local TTS on Linux

macOS `say` is unavailable off macOS; `espeak` and `piper` are its offline replacements. Both need
their binary on `PATH`, and both are skipped by the fallback chain when it is missing.

- **espeak-ng** — instant and tiny, but robotic. Install with `pacman -S espeak-ng` or
  `apt install espeak-ng`. `voice` takes an espeak-ng voice name; append a variant with `+`
  (`cmn` Mandarin, `cmn+f3` female, `en-us`, `yue` Cantonese). Run `espeak-ng --voices` to list
  them. Traditional and Simplified Chinese produce identical phonemes, so no conversion is needed.
- **piper** — neural, noticeably better Chinese prosody, still fast enough for notifications.
  Install with `uv tool install piper-tts`. Voice models are downloaded by hand from
  [rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices); each voice is a `.onnx` file
  that must sit next to its `.onnx.json` sidecar. Point `voice`, `model` or `path` at the `.onnx`
  (first non-blank wins, `~` is expanded) — piper is treated as unavailable until both the binary
  and the model exist. The model is reloaded on every invocation (~640 ms), so espeak suits
  per-notification alerts and piper suits summaries.

Playback needs a command-line player: SumVox probes `paplay` → `pw-play` → `ffplay` → `mpv` →
`aplay` and uses the first one installed. `paplay` ships with both `pulseaudio-utils` and
`pipewire-pulse`, so most desktops already have it.

```toml
[[tts.providers]]
name = "espeak"
voice = "cmn+f3"   # optional; omit for the espeak-ng default voice
rate = 175         # words per minute (80-450)
volume = 80

[[tts.providers]]
name = "piper"
voice = "~/.local/share/piper/zh_CN-huayan-medium.onnx"
rate = 200         # inverse length-scale; higher = faster
volume = 80
```

## ⚙️ Reference

### Configuration file structure

```toml
[[llm.providers]]     # ordered fallback chain
# name, model, api_key, base_url, timeout, disable_thinking

[llm.parameters]      # shared by every LLM provider
# max_tokens, temperature, disable_thinking

[[tts.providers]]     # ordered fallback chain
# name, model, voice, api_key, rate, volume, path, service_account_key,
# language_code, speed, stability, style, style_prompt

[summarization]
# content_source ("transcript" | "last_message"), turns, system_message,
# prompt_template ({context} placeholder), fallback_message

[hooks.claude_code]
# notification_filter, notification_tts_provider, stop_tts_provider,
# notification_volume, stop_volume, queue_timeout
```

### Environment Variables

API keys belong in the config file; the variables below are a fallback for when the config value
is missing or still holds the `${...}` placeholder.

| Variable | Description |
|----------|-------------|
| `SUMVOX_DISABLE` | Any value skips all SumVox processing |
| `GEMINI_API_KEY` | Google Gemini LLM and Google TTS |
| `GOOGLE_API_KEY` | Google TTS only (fallback after `GEMINI_API_KEY`) |
| `ANTHROPIC_API_KEY` | Anthropic LLM |
| `OPENAI_API_KEY` | OpenAI LLM and OpenAI TTS |
| `XAI_API_KEY` | xAI Grok LLM and xAI TTS |
| `ELEVENLABS_API_KEY` | ElevenLabs TTS |
| `RUST_LOG` | Log level: `trace`, `debug`, `info`, `warn`, `error` |

For a quiet Claude Code session:

```bash
SUMVOX_DISABLE=1 claude               # Bash / Zsh
env SUMVOX_DISABLE=1 claude           # Fish

alias claude-quiet='SUMVOX_DISABLE=1 claude'      # Bash / Zsh
alias claude-quiet 'env SUMVOX_DISABLE=1 claude'  # Fish
```

### Troubleshooting

**"No API key found"** — check `~/.config/sumvox/config.toml`: an `api_key` still reading
`${PROVIDER_API_KEY}` counts as unset. Either paste the real key or export the matching
environment variable.

**"Provider not available"** — verify the key, the network, and the order of the fallback chain.

**No audio** — test with `sumvox say "test"`.
On macOS check System Settings → Sound → Output.
On Linux install a player (`paplay`, `pw-play`, `ffplay`, `mpv` or `aplay`); SumVox lists the ones
it looked for when none is found. `aplay` has no volume control and is skipped when `volume = 0`.
Over SSH `XDG_RUNTIME_DIR` is often unset — SumVox defaults it to `/run/user/{uid}` for the
player, so export the real value if yours differs.

**Ollama not responding** — `ollama serve`, then `ollama pull llama3.2`.

## 🏗️ How It Works

```
Claude Code session
        │  hook event (JSON on stdin)
        ▼
┌──────────────────────────────────────────────┐
│ sumvox                                       │
│  1. parse the event                          │
│  2. read the transcript   ~/.claude/projects │
│  3. summarize             LLM chain          │
│  4. speak                 TTS chain          │
└──────────────────────────────────────────────┘
        │  audio
        ▼
   🔊 system audio
```

Both chains behave the same way: try each provider in order, first success wins. An LLM chain that
runs out of providers speaks `summarization.fallback_message`; a TTS chain that runs out stays
silent. SumVox never fails the hook.

## 🖥️ Menu Bar App (macOS)

An optional companion app lives in [`menubar/SumVoxMenu.swift`](menubar/SumVoxMenu.swift) — one
Swift file, system frameworks only:

```bash
just menubar                      # builds target/release/sumvox-menubar
target/release/sumvox-menubar &
```

It launches with no Dock icon and puts up two things: a 🔊/🔇 status item, and a small floating
**orb** that stays on screen (borderless, always-on-top, draggable to wherever you want it).
Clicking either one opens the same menu:

- **播放語音** — toggles `~/.config/sumvox/muted`. While that flag exists, SumVox skips playback on
  *every* path — the hooks, `sumvox say` and `sumvox sum` — but still records the text.
- **最近通知** — the last 50 spoken texts from `~/.config/sumvox/history.log`; click one to copy it.
- **開啟設定檔** — opens `~/.config/sumvox/config.toml`.
- **結束** — quits.

When a new notification is recorded, a speech bubble appears next to the orb — on whichever side
has room, clamped to the visible screen — types the text out, and fades away after 4–12 s (scaled
to the text length). Because the text is recorded before
the mute check, the bubble still appears while muted — you see the notification, you just don't
hear it. The orb itself deforms in time with the audio: it reads the path in
`~/.config/sumvox/now_playing`, decodes an RMS envelope off the main thread, and animates from
that; with no real audio file (macOS `say`, for example) the typewriter synthesizes a level
instead. It renders at 20 fps idle and 60 fps while speaking.

State is never polled: the three files are watched with `DispatchSource`, and the menu is rebuilt
only when it opens. Those files — `muted`, `history.log`, `now_playing` — are the entire contract
between the two binaries (`src/notify_log.rs`), so not running the app just leaves them unread.

## 🛠️ Development

```bash
cargo build --release
cargo test
cargo fmt && cargo clippy -- -D warnings
RUST_LOG=debug cargo run
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development and release guide.

## 🤝 Contributing

Contributions welcome. Areas that need help:

- Test coverage for the non-Gemini providers
- Additional TTS engines
- Windows support
- Documentation

## 📄 License

MIT — see [LICENSE](LICENSE).

## 🔗 Links

- **GitHub**: https://github.com/musingfox/sumvox
- **Issues**: https://github.com/musingfox/sumvox/issues
- **crates.io**: https://crates.io/crates/sumvox
- **Changelog**: [CHANGELOG.md](CHANGELOG.md)
