# Claude Voice Rust 重寫計畫

> 此計畫記錄將 Python 版 claude-voice 重寫為 Rust 的實作方案。

## 目標

將 claude-voice 從需要 uv 環境的 Python hook 腳本，轉換為：
- 單一 Rust 執行檔 (6-8 MB)
- 標準 Claude Code Plugin 格式
- 零依賴安裝

---

## 現狀 vs 目標

| 指標 | Python 版 (現狀) | Rust 版 (目標) |
|------|-----------------|----------------|
| 執行檔大小 | 176 MB (venv) | 6-8 MB |
| 啟動時間 | 200-300ms | 5-15ms |
| 依賴安裝 | 需要 uv + Python | 無 |
| 程式碼量 | 1,398 行 | ~1,500 行 (預估) |

---

## 專案結構

```
claude-voice/
├── Cargo.toml
├── src/
│   ├── main.rs              # 入口點、管道協調
│   ├── config.rs            # JSON 配置載入
│   ├── voice.rs             # macOS say 命令包裝
│   ├── summarizer.rs        # 摘要生成 + 正則解析
│   └── llm/
│       ├── mod.rs           # LLM trait 定義
│       ├── gemini.rs        # Gemini API client
│       ├── anthropic.rs     # Claude API client
│       ├── openai.rs        # OpenAI API client
│       ├── ollama.rs        # Ollama 本地模型
│       └── cost_tracker.rs  # 成本追蹤
├── .claude-plugin/
│   └── plugin.json
├── hooks/
│   └── hooks.json
└── README.md
```

---

## Rust Crates 依賴

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
once_cell = "1"
chrono = "0.4"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
dirs = "5"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

---

## 模組對應

| Python 模組 | Rust 模組 | 行數 | 難度 |
|-------------|-----------|------|------|
| voice_config.py | config.rs | 78 | ★☆☆ |
| voice_engine.py | voice.rs | 236 | ★☆☆ |
| llm_adapter.py | llm/*.rs | 349 | ★★★ |
| summarizer.py | summarizer.rs | 416 | ★★★★ |
| voice_notification.py | main.rs | 319 | ★★☆ |

---

## LLM 支援範圍

完整支援 4 個 LLM provider，優先順序：

1. **Gemini** (gemini-2.0-flash-exp) - 主要，成本最低
2. **Anthropic** (claude-3-haiku) - Fallback #1
3. **OpenAI** (gpt-4o-mini) - Fallback #2
4. **Ollama** (llama3.2) - Fallback #3，本地執行

---

## 實作階段

### Phase 1: 專案初始化 (2h)
- [ ] `cargo new claude-voice`
- [ ] 設定 Cargo.toml 依賴
- [ ] 建立模組結構

### Phase 2: 基礎層 (4h)
- [ ] config.rs - JSON 配置載入 (serde)
- [ ] voice.rs - macOS say 命令包裝 (std::process)
- [ ] 單元測試

### Phase 3: LLM API 層 (8h)
- [ ] llm/mod.rs - LLM trait 定義 (統一介面)
- [ ] llm/gemini.rs - Gemini API 直接呼叫
- [ ] llm/anthropic.rs - Claude API
- [ ] llm/openai.rs - OpenAI API
- [ ] llm/ollama.rs - Ollama 本地呼叫
- [ ] llm/cost_tracker.rs - 成本追蹤與持久化
- [ ] 整合測試

### Phase 4: 摘要引擎 (6h)
- [ ] summarizer.rs - 正則表達式引擎 (25+ patterns)
- [ ] 操作類型檢測 (11 種)
- [ ] 繁體中文處理驗證

### Phase 5: 主入口與整合 (4h)
- [ ] main.rs - stdin JSON 讀取
- [ ] 管道協調 (trigger → summarize → speak)
- [ ] 錯誤處理與日誌

### Phase 6: 優化與發布 (4h)
- [ ] Release build 優化 (LTO, strip)
- [ ] Plugin 結構 (plugin.json, hooks.json)
- [ ] 跨架構編譯 (x86_64, arm64)
- [ ] README 與安裝文件

**總計: ~28 小時**

---

## Plugin 配置

### `.claude-plugin/plugin.json`
```json
{
  "name": "claude-voice-notification",
  "version": "1.0.0",
  "description": "Voice notifications for Claude Code (Traditional Chinese)",
  "author": { "name": "Nick Huang" },
  "hooks": "./hooks/hooks.json"
}
```

### `hooks/hooks.json`
```json
{
  "hooks": {
    "Stop": [{
      "hooks": [{
        "type": "command",
        "command": "${CLAUDE_PLUGIN_ROOT}/bin/claude-voice"
      }]
    }]
  }
}
```

---

## 驗證計畫

1. **單元測試**: `cargo test`
2. **手動測試**: `echo '{"hook_type":"stop",...}' | ./target/release/claude-voice`
3. **性能測試**: 驗證啟動時間 < 20ms
4. **Plugin 安裝測試**: 模擬 enabledPlugins 啟用
5. **語音輸出驗證**: 確認中文語音正常播放

---

## 風險與應對

| 風險 | 應對方案 |
|------|---------|
| LLM API 實作複雜度 | 優先支援 Gemini，其他 API 後續加入 |
| 25+ 正則式移植 | 建立對照測試，逐一驗證 |
| 繁體中文 Unicode | 使用 Rust 原生 UTF-8 支援 |

---

## 參考資源

- 現有 Python 實作: `.claude/hooks/`
- Python 測試套件: `tests/`
- 配置範例: `.claude/hooks/voice_config.json`

---

*計畫建立日期: 2026-01-22*
