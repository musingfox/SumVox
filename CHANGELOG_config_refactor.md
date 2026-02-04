# Configuration Refactor (v1.0.0)

## 實施日期: 2026-01-31

## 變更摘要

成功實施配置結構重構,修復 `claude-voice init` 生成的配置問題,直接採用新的配置架構。

## 完成的修改

### 1. API Key Placeholder ✅
- **問題**: `api_key` 欄位在配置文件中不顯示(使用 `skip_serializing_if = "Option::is_none"`)
- **解決方案**: 添加自定義序列化函數 `serialize_api_key()`
- **結果**: 配置文件中顯示 `"api_key": "${PROVIDER_API_KEY}"`
- **位置**: `src/config.rs:20-29`

### 2. Ollama Timeout 調整 ✅
- **問題**: 預設 10 秒 timeout 不足以處理本地 LLM
- **解決方案**: 添加 `default_ollama_timeout()` 返回 60 秒
- **結果**: Ollama provider 的 timeout 設為 60 秒
- **位置**: `src/config.rs:15-18`

### 3. 統一成本控制 ✅
- **問題**: `cost_control` 在 `llm` 層級,且 TTS 沒有成本追蹤
- **解決方案**: 將 `cost_control` 移到 `VoiceConfig` 頂層
- **結果**: 統一管理所有 API 使用成本(LLM + TTS)
- **變更**:
  - `VoiceConfig` 添加 `cost_control` 欄位
  - `LlmConfig` 移除 `cost_control` 欄位
  - `src/main.rs` 更新引用路徑: `config.llm.cost_control` → `config.cost_control`

### 4. 重組 Hook 配置 ✅
- **問題**: `summarization` 和 `advanced` 混合不同場景的配置
- **解決方案**: 引入語義化的 hook 配置結構
- **新結構**:
  - `StopHookConfig`: 任務完成時的摘要配置
    - `enabled`: 是否啟用
    - `max_length`: 最大摘要長度
    - `system_message`: LLM 系統消息
    - `prompt_template`: 提示模板
    - `fallback_message`: 失敗時的備用消息
  
  - `NotificationHookConfig`: 通知處理配置
    - `enabled`: 是否啟用
    - `filter`: 通知類型過濾器(例如 `["*"]` 所有通知)
    - `system_message`: LLM 系統消息
    - `prompt_template`: 提示模板

- **移除**: `SummarizationConfig` 和 `AdvancedConfig`
- **更新引用**:
  - `config.summarization.prompt_template` → `config.stop_hook.prompt_template`
  - `config.summarization.notification_prompt` → `config.notification_hook.prompt_template`
  - `config.advanced.fallback_message` → `config.stop_hook.fallback_message`

### 5. 版本號調整 ✅
- 從 "2.0.0" 改為 "1.0.0"(因為尚未 release)

## 測試結果

### 單元測試
```bash
cargo test --lib config::
```
- ✅ 14 tests passed
- 新增測試:
  - `test_api_key_placeholder_serialization`
  - `test_ollama_timeout_60_seconds`
  - `test_cost_control_at_top_level`
  - `test_new_hook_config_structure`

### 完整測試套件
```bash
cargo test
```
- ✅ 96 passed, 6 ignored

### 集成測試
生成的配置文件驗證:
```json
{
  "version": "1.0.0",
  "llm": {
    "providers": [
      {
        "name": "google",
        "api_key": "${PROVIDER_API_KEY}",  // ✅ 顯示 placeholder
        "timeout": 10
      },
      {
        "name": "ollama",
        "api_key": "${PROVIDER_API_KEY}",
        "timeout": 60  // ✅ 60 秒 timeout
      }
    ]
  },
  "stop_hook": { ... },  // ✅ 新結構
  "notification_hook": { ... },  // ✅ 新結構
  "cost_control": { ... }  // ✅ 頂層
}
```

## 文件變更清單

| 文件 | 修改內容 |
|------|---------|
| `src/config.rs` | 主要重構:新增序列化函數、調整 timeout、移動 cost_control、新增 hook 結構 |
| `src/main.rs` | 更新配置引用路徑(3 處) |

## Breaking Changes

**重要**: 此版本不需向後兼容性,因為尚未正式 release。

用戶需要:
1. 刪除現有配置: `rm ~/.claude/claude-voice.json`
2. 重新生成: `claude-voice init`
3. 重新設置 API keys: `claude-voice credentials set <provider>`

## 完成標準

- [x] 所有修改的代碼通過 `cargo clippy`
- [x] 所有新增的單元測試通過
- [x] 集成測試驗證新配置生成正確
- [x] 手動測試實際功能正常
- [x] 文檔已更新

## 下一步

建議在 release notes 或 README.md 中說明此配置結構變更,並提供遷移指南。
