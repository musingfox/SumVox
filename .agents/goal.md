# Goal

開發 SumVox 兩個新功能：

## Feature 1: Audio File Playback Support
聲音通知支援播放音訊檔案 — 讓用戶可以配置自訂音效檔案（如 .wav, .mp3）作為通知音，而不僅僅依賴 TTS 語音

## Feature 2: Notification Queue System
通知佇列系統 — 當使用者同時使用多個 Claude Code session 時，通知會依序播放而不會同時重疊

## Project Context
- Project: SumVox
- Language: Rust
- Type: CLI Tool / Claude Code Hook
- Current Version: v1.2.0
- Existing Audio Infrastructure: src/audio/ with RodioAudioPlayer, src/playback.rs with flock
- Current Test Coverage: 268 tests passing
- Configuration: TOML at ~/.config/sumvox/config.toml
