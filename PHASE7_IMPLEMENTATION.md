# Phase 7: 統一設定檔架構 - 實作完成報告

## 實作日期
2026-01-27

## 目標
將設定檔移至 `~/.claude/claude-voice.json`，讓使用者只需安裝執行檔即可使用，並實作 array-based fallback 機制。

---

## 完成項目

### ✅ 1. 更新 Config 結構 (`src/config.rs`)
- 新增 `LlmProviderConfig`, `TtsProviderConfig` 結構
- 新增 `LlmConfig`, `TtsConfig` 結構
- 實作 `load_from_home()` 從 `~/.claude/claude-voice.json` 載入
- 實作 `save_to_home()` 儲存到家目錄
- 新增 `set_llm_api_key()`, `set_tts_project_id()` 方法
- 新增 `list_llm_providers()`, `list_tts_providers()` 方法
- 環境變數 fallback: API keys 可從 config 或 env vars 讀取

### ✅ 2. 更新 Provider Factory (`src/provider_factory.rs`)
- 新增 `create_from_config()` - 依序嘗試 provider 直到成功
- 保留 `create_by_name()` 用於 CLI 覆蓋
- Fallback 邏輯: `providers[0] → providers[1] → ... → Error`
- 錯誤訊息累積，方便除錯

### ✅ 3. 更新 TTS Provider Factory (`src/tts/mod.rs`)
- 新增 `create_tts_from_config()` 函數
- 新增 `create_tts_by_name()` 用於 CLI 覆蓋
- 新增 `TtsEngine::Auto` 選項
- Fallback 邏輯: `providers[0] → providers[1] → ... → Error`

### ✅ 4. 更新 Main (`src/main.rs`)
- 改用 `VoiceConfig::load_from_home()`
- 移除 `CARGO_MANIFEST_DIR` 硬編碼路徑
- LLM: CLI 覆蓋或 config fallback chain
- TTS: `--tts auto` 使用 config fallback，指定時使用 CLI 值
- 新增 `handle_init_command()` 函數
- 新增 `handle_credentials_command()` 函數

### ✅ 5. 更新 CLI (`src/cli.rs`)
- 新增 `Commands::Init` 子命令
- `--tts` 預設值改為 `"auto"`
- 保留所有現有 CLI 參數用於覆蓋

### ✅ 6. 更新 Credentials (`src/credentials.rs`)
- 簡化為工具函數模組
- `env_var_name()` 提供環境變數名稱
- `has_api_key()` 檢查 config + env vars
- Credentials 現在內嵌在 config 中

---

## 新設定檔格式

```json
{
  "version": "2.0.0",
  "enabled": true,
  "llm": {
    "providers": [
      {
        "name": "google",
        "model": "gemini-2.5-flash",
        "api_key": "AIza...",
        "timeout": 10
      },
      {
        "name": "anthropic",
        "model": "claude-3-haiku-20240307",
        "api_key": "sk-ant-..."
      },
      {
        "name": "openai",
        "model": "gpt-4o-mini",
        "api_key": "sk-..."
      },
      {
        "name": "ollama",
        "model": "llama3.2",
        "base_url": "http://localhost:11434"
      }
    ],
    "parameters": {
      "max_tokens": 100,
      "temperature": 0.3
    },
    "cost_control": {
      "daily_limit_usd": 0.10,
      "usage_tracking": true,
      "usage_file": "~/.claude/voice-usage.json"
    }
  },
  "tts": {
    "providers": [
      {
        "name": "google",
        "voice": "Aoede",
        "project_id": "my-project-123"
      },
      {
        "name": "macos",
        "voice": "Ting-Ting",
        "rate": 200
      }
    ]
  },
  "summarization": {
    "max_length": 50,
    "prompt_template": "你是語音通知助手。根據以下 Claude Code 對話內容，生成一句繁體中文摘要（最多 {max_length} 字）。\n\n對話內容：\n{context}\n\n摘要："
  },
  "advanced": {
    "fallback_message": "任務已完成"
  }
}
```

---

## Fallback 機制

### LLM Providers
```
providers[0] → providers[1] → providers[2] → ... → 空摘要 + fallback_message
```

**範例:**
```
Google (需要 API key)
  ↓ 失敗
Anthropic (需要 API key)
  ↓ 失敗
OpenAI (需要 API key)
  ↓ 失敗
Ollama (本地服務，無需 API key)
  ↓ 成功
使用 Ollama
```

### TTS Providers
```
providers[0] → providers[1] → ... → 靜默（不報錯）
```

**範例:**
```
Google TTS (需要 project_id)
  ↓ 失敗
macOS say (始終可用於 macOS)
  ↓ 成功
使用 macOS say
```

---

## 新增指令

### 初始化設定檔
```bash
claude-voice init
```
建立預設設定檔於 `~/.claude/claude-voice.json`

### 設定 API Key
```bash
# LLM providers
claude-voice credentials set google
claude-voice credentials set anthropic
claude-voice credentials set openai

# TTS provider
claude-voice credentials set google_tts
```

### 列出已設定的 Providers
```bash
claude-voice credentials list
```
輸出:
```
LLM Providers:
  - google (no key)
  - ollama (configured)

TTS Providers:
  - google (not configured)
  - macos (configured)
```

### 測試 API Key
```bash
claude-voice credentials test google
```

### 移除 Credentials
```bash
claude-voice credentials remove google
```

---

## CLI 覆蓋

所有 CLI 參數仍可覆蓋 config 設定:

```bash
# 覆蓋 LLM provider
claude-voice --provider openai --model gpt-4o-mini < input.json

# 覆蓋 TTS provider
claude-voice --tts google --tts-voice Aoede < input.json

# 使用 config fallback chain (預設)
claude-voice --tts auto < input.json
```

---

## 環境變數支援

API keys 可從環境變數讀取 (config 優先):

```bash
export GEMINI_API_KEY="AIza..."
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GOOGLE_CLOUD_PROJECT="my-project-123"

claude-voice < input.json  # 自動使用環境變數
```

---

## 測試結果

### 單元測試
```bash
cargo test --lib
```
結果: **90 passed; 0 failed; 6 ignored**

### 整合測試
```bash
# 初始化
cargo run -- init
# ✅ 成功建立 ~/.claude/claude-voice.json

# 列出 providers
cargo run -- credentials list
# ✅ 正確顯示 LLM 和 TTS providers

# 執行 (需要實際 API key)
echo '{"session_id":"test",...}' | cargo run
# ✅ 依序嘗試 providers 直到成功
```

---

## 向後相容

1. **舊設定檔路徑** - 若 `~/.claude/claude-voice.json` 不存在，使用預設值
2. **CLI 參數** - 所有原有 CLI 參數仍可使用並覆蓋 config
3. **環境變數** - 支援從環境變數讀取 API keys

---

## Zero-dependency Deployment

現在使用者只需:

1. 安裝執行檔:
   ```bash
   cargo install --path .
   ```

2. 初始化設定:
   ```bash
   claude-voice init
   claude-voice credentials set google
   ```

3. 使用:
   ```bash
   echo '{"session_id":"test",...}' | claude-voice
   ```

**不再需要:**
- ❌ 專案目錄
- ❌ `voice_config.json`
- ❌ 手動管理 credentials 檔案
- ❌ 設定 `CARGO_MANIFEST_DIR`

---

## 未來改進建議

1. **Config 遷移工具** - 自動從舊格式遷移到新格式
2. **Config 驗證指令** - `claude-voice config validate`
3. **Provider 測試指令** - `claude-voice test llm` / `claude-voice test tts`
4. **互動式設定** - `claude-voice setup` 引導式設定流程
5. **多 Profile 支援** - `~/.claude/claude-voice-{profile}.json`

---

## 總結

Phase 7 成功實現了統一設定檔架構，主要改進:

- ✅ 單一設定檔 `~/.claude/claude-voice.json`
- ✅ Array-based provider fallback chain
- ✅ Zero-dependency deployment
- ✅ Init 指令簡化首次使用
- ✅ 環境變數 fallback
- ✅ CLI 參數覆蓋機制保留
- ✅ 90 個單元測試全數通過
- ✅ 向後相容

專案現已準備好進行 single-binary 部署。
