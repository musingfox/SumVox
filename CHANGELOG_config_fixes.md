# 配置修正 - 基於用戶反饋 (v1.0.0)

## 實施日期: 2026-01-31

## 用戶反饋修正

### 1. ✅ 移除 Hook 的 `enabled` 欄位

**原因**: 用戶可以直接在 Claude Code 的 hook 配置中啟用/禁用,config 中的 `enabled` 是多餘的。

**變更**:
- `StopHookConfig`: 移除 `enabled` 欄位
- `NotificationHookConfig`: 移除 `enabled` 欄位

---

### 2. ✅ Notification Hook 簡化

**原因**: Notification 應該直接播報 hook 傳送的文字,不需要 LLM 處理。

**變更**:
- **移除**: `system_message`、`prompt_template` 欄位
- **保留**: `filter` 欄位用於過濾通知類型
- **行為**: 直接播報通知訊息,無需 LLM 摘要

**代碼變更** (`src/main.rs`):
```rust
// 舊邏輯: 使用硬編碼過濾 + LLM 處理
let should_speak = matches!(
    notification_type,
    "permission_prompt" | "idle_prompt" | "elicitation_dialog"
);
let processed_message = generate_summary(config, cli, system_message, &user_prompt).await?;

// 新邏輯: 使用配置過濾 + 直接播報
let should_speak = if filter.is_empty() {
    false  // Empty filter = disabled
} else if filter.contains(&"*".to_string()) {
    true   // Wildcard = all notifications
} else {
    filter.contains(&notification_type.to_string())
};
speak_summary(cli, config, message).await?;  // Direct speak
```

---

### 3. ✅ Notification Filter 文檔化

**問題**: Filter 選項不清楚。

**解決方案**: 在配置結構中添加詳細的文檔註釋。

```rust
/// Filter: which notification types to speak (speaks the notification message directly)
///
/// Available notification types:
/// - "permission_prompt": User permission required
/// - "idle_prompt": Agent waiting for user action
/// - "elicitation_dialog": MCP tool needs user input
/// - "auth_success": Authentication completed
/// - "*": All notifications
///
/// Examples:
/// - ["*"]: Speak all notifications
/// - ["permission_prompt", "idle_prompt"]: Only speak prompts
/// - []: Disable all notifications
```

**預設值**: `["permission_prompt", "idle_prompt", "elicitation_dialog"]`

---

### 4. ✅ Cost Control 超過時通知用戶

**問題**: 當 daily budget 超過時,只有 log warning,用戶不知道發生了什麼。

**解決方案**: 使用 `eprintln!` 輸出到 stderr,顯示在 Claude Code 終端。

```rust
if !under_budget {
    eprintln!("⚠️  Claude Voice: 每日預算 ${:.2} 已超過,語音功能已停用", daily_limit);
    tracing::warn!("Daily budget limit ${} exceeded", daily_limit);
    return Ok(String::new());
}
```

**結果**: 用戶在 Claude Code 中會看到清晰的警告訊息。

---

## 測試結果

### 單元測試
```bash
cargo test --lib config::
```
✅ 14 tests passed

### 完整測試套件
```bash
cargo test
```
✅ 96 passed, 6 ignored

### 配置驗證
生成的配置:
```json
{
  "stop_hook": {
    "max_length": 50,
    "system_message": "...",
    "prompt_template": "...",
    "fallback_message": "任務已完成"
    // ✅ 沒有 enabled
  },
  "notification_hook": {
    "filter": ["permission_prompt", "idle_prompt", "elicitation_dialog"]
    // ✅ 沒有 enabled, system_message, prompt_template
  },
  "cost_control": { ... }  // ✅ 在頂層
}
```

---

## 文件變更清單

| 文件 | 修改內容 |
|------|---------|
| `src/config.rs` | 移除 enabled、system_message、prompt_template;添加 filter 文檔 |
| `src/main.rs` | 重寫 notification 處理邏輯;添加 cost_control stderr 輸出 |

---

## Breaking Changes

**重要**: 需要重新生成配置文件。

```bash
rm ~/.claude/claude-voice.json
claude-voice init
```

---

## 功能對比

| 功能 | 舊版 | 新版 |
|------|------|------|
| Stop Hook | 有 `enabled` | 無 `enabled`,由 Claude Code 控制 |
| Notification Hook | LLM 處理訊息 | 直接播報原始訊息 |
| Notification Filter | 硬編碼 | 配置化,有文檔 |
| Cost Control 超過 | 靜默失敗 | stderr 警告訊息 |

---

## 下一步

配置結構已優化完成,可以進行以下工作:
1. 更新 README.md 說明新的配置結構
2. 添加 notification filter 的使用範例
3. 測試實際 Claude Code hook 整合
