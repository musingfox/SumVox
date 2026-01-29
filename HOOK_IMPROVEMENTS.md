# Hook 機制改進總結

## 問題發現

### 原始問題
- 同時註冊 `Notification` 和 `Stop` hooks，導致語音播放兩次
- Notification hook 在 transcript 尚未完整寫入時觸發，可能讀取不到最新回覆
- 缺少對不同 notification_type 的差異化處理

### Hook 觸發時機差異

| Hook 類型 | 觸發時機 | Transcript 狀態 | 適用場景 |
|----------|---------|----------------|---------|
| **Notification** | 通知產生時（Assistant 回覆過程中） | ❌ 可能不完整 | 即時通知、需要用戶操作 |
| **Stop** | Assistant 完全回覆後 | ✅ 保證完整 | 任務完成摘要 |

---

## 解決方案

### 1. 擴展 HookInput 結構體

```rust
struct HookInput {
    session_id: String,
    transcript_path: String,
    permission_mode: String,
    hook_event_name: String,
    stop_hook_active: Option<bool>,
    // ✅ 新增欄位
    message: Option<String>,           // 通知訊息內容
    notification_type: Option<String>, // 通知類型
}
```

### 2. 差異化處理兩種 Hook

#### Notification Hook 處理
- **直接播報** `message` 欄位（無需 LLM，速度快）
- 過濾通知類型，只播報重要通知
- 適合需要即時回應的場景

#### Stop Hook 處理
- 讀取**完整** transcript
- 使用 LLM 生成摘要
- 適合任務完成後的總結

### 3. Notification Type 過濾策略

根據官方文件，Claude Code 有 **4 種 notification_type**：

| notification_type | 說明 | 是否播報 | 原因 |
|------------------|------|---------|------|
| `permission_prompt` | 權限請求（如「需要使用 Bash」） | ✅ 播報 | 用戶需要批准操作 |
| `idle_prompt` | 閒置提示（60秒以上無回應） | ✅ 播報 | 提醒用戶回應 |
| `elicitation_dialog` | MCP 工具需要收集參數 | ✅ 播報 | 需要用戶輸入 |
| `auth_success` | 認證成功 | ⏭️ 跳過 | 不需即時通知 |

---

## 實現代碼

### 主函數分發邏輯

```rust
match hook_input.hook_event_name.as_str() {
    "Notification" => handle_notification_hook(&hook_input, &config, &cli).await?,
    "Stop" => handle_stop_hook(&hook_input, &config, &cli).await?,
    _ => tracing::warn!("Unknown hook event: {}", hook_input.hook_event_name),
}
```

### Notification Hook 處理函數

```rust
async fn handle_notification_hook(
    hook_input: &HookInput,
    config: &VoiceConfig,
    cli: &Cli,
) -> Result<()> {
    // 獲取通知訊息
    let message = hook_input.message.as_ref()?;
    let notification_type = hook_input.notification_type.as_deref().unwrap_or("unknown");

    // 過濾：只播報重要通知
    let should_speak = matches!(
        notification_type,
        "permission_prompt" | "idle_prompt" | "elicitation_dialog"
    );

    if should_speak {
        speak_summary(cli, config, message).await?;
    }

    Ok(())
}
```

### Stop Hook 處理函數

```rust
async fn handle_stop_hook(
    hook_input: &HookInput,
    config: &VoiceConfig,
    cli: &Cli
) -> Result<()> {
    // 讀取完整 transcript
    let texts = TranscriptReader::read_last_n_texts(&hook_input.transcript_path, 10).await?;
    let context = texts.join("\n\n");

    // 使用 LLM 生成摘要
    let prompt = config.summarization.prompt_template
        .replace("{max_length}", &cli.max_length.to_string())
        .replace("{context}", &context);

    let summary = generate_summary(config, cli, &prompt).await?;

    // 播報摘要
    speak_summary(cli, config, &summary).await?;

    Ok(())
}
```

---

## 測試結果

### 自動化測試腳本

創建了 `test_hooks.sh` 測試所有場景：

```bash
./test_hooks.sh
```

### 測試結果

✅ **Test 1: permission_prompt** - 正常播報
✅ **Test 2: idle_prompt** - 正常播報
✅ **Test 3: elicitation_dialog** - 正常播報
✅ **Test 4: auth_success** - 正確跳過
✅ **Test 5: Stop hook** - 正常讀取 transcript 並生成摘要

---

## 優勢總結

| 場景 | 改進前 | 改進後 |
|------|--------|--------|
| **權限請求** | ❌ 可能錯過或讀取不完整 | ✅ 即時播報通知訊息 |
| **任務完成** | ❌ Transcript 可能不完整 | ✅ 保證讀取完整 transcript |
| **重複播放** | ❌ 同一內容播放兩次 | ✅ 不同內容，各司其職 |
| **響應速度** | 🐌 所有場景都用 LLM（慢） | ⚡ Notification 直接播報（快） |
| **播報準確性** | ❌ 可能播報不相關通知 | ✅ 過濾低優先級通知 |

---

## 配置建議

保留兩個 hooks 的配置（`~/.claude/settings.json`）：

```json
{
  "hooks": {
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/.claude/hooks/run_voice_hook.sh"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/.claude/hooks/run_voice_hook.sh"
          }
        ]
      }
    ]
  }
}
```

程式會自動根據 `hook_event_name` 選擇正確的處理邏輯。

---

## 參考資料

- [Claude Code Hooks 官方文件](https://code.claude.com/docs/en/hooks.md)
- [Hook Lifecycle](https://code.claude.com/docs/en/hooks.md#hook-lifecycle)
- [Notification Hook Input](https://code.claude.com/docs/en/hooks.md#notification-input)
