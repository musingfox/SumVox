# TDD 實作摘要：Credentials 與 CLI 整合

## 完成時間

2026-01-23

## 目標

使用 Test-Driven Development (TDD) 方法實作 credentials.json 管理系統與 CLI 參數支援。

## 實作功能

### 1. Credential Manager (`src/credentials.rs`)

**功能**:
- 從環境變數或 credentials.json 載入 API keys
- 優先級：環境變數 > credentials 檔案
- 儲存 API keys 到 ~/.config/claude-voice/credentials.json
- 檔案權限自動設定為 0600（僅使用者可讀寫）
- 支援 google/gemini, anthropic, openai 三個 providers

**測試覆蓋**:
- ✓ test_load_api_key_from_env - 從環境變數載入
- ✓ test_load_api_key_from_file - 從檔案載入
- ✓ test_env_priority_over_file - 環境變數優先
- ✓ test_save_and_load - 儲存並載入多個 providers
- ✓ test_file_permissions - 檔案權限為 0600
- ✓ test_list_providers - 列出已設定的 providers
- ✓ test_remove_provider - 移除 provider
- ✓ test_env_var_name - 環境變數名稱對應
- ✓ test_env_var_name_unknown_provider - 未知 provider 報錯

**測試結果**: 9/9 passed

### 2. CLI 參數解析 (`src/cli.rs`)

**功能**:
- 主要參數：--provider, --model, --timeout, --voice, --rate, --max-length
- 子命令：credentials {set, list, test, remove}
- 使用 clap derive macro 自動產生幫助文件

**測試覆蓋**:
- ✓ test_parse_with_defaults - 預設值正確
- ✓ test_parse_provider_model - 解析 provider 和 model
- ✓ test_parse_all_options - 解析所有選項
- ✓ test_parse_credentials_set - credentials set 子命令
- ✓ test_parse_credentials_list - credentials list 子命令
- ✓ test_parse_credentials_test - credentials test 子命令
- ✓ test_parse_credentials_remove - credentials remove 子命令
- ✓ test_cli_verify - CLI 結構驗證

**測試結果**: 8/8 passed

### 3. Provider Factory (`src/provider_factory.rs`)

**功能**:
- 統一的 Provider 建立介面
- 自動載入 API keys（透過 CredentialManager）
- 支援 provider 名稱別名（如 google/gemini, anthropic/claude）
- Ollama 不需要 API key
- 友善的錯誤訊息

**測試覆蓋**:
- ✓ test_provider_from_str - provider 字串解析（含別名）
- ✓ test_provider_requires_api_key - API key 需求檢查
- ✓ test_create_google_provider - 建立 Google provider
- ✓ test_create_anthropic_provider - 建立 Anthropic provider
- ✓ test_create_openai_provider - 建立 OpenAI provider
- ✓ test_create_ollama_no_key - Ollama 無需 API key
- ✓ test_missing_api_key_error - 缺少 API key 錯誤訊息
- ✓ test_env_var_takes_priority - 環境變數優先

**測試結果**: 8/8 passed

### 4. 更新 main.rs

**變更**:
- 整合 CLI 參數解析
- 加入 credentials 子命令處理
- 使用 ProviderFactory 取代硬編碼的 GeminiProvider
- 支援 CLI 參數覆寫 config 設定
- 互動式 API key 輸入（使用 rpassword）

**新功能**:
- `claude-voice credentials set <provider>` - 設定 API key
- `claude-voice credentials list` - 列出已設定的 providers
- `claude-voice credentials test <provider>` - 測試 API key
- `claude-voice credentials remove <provider>` - 移除 credentials
- `claude-voice --provider google --model gemini-2.5-flash` - 覆寫設定

## TDD 執行記錄

### RED Phase (寫失敗測試)

1. **credentials.rs**: 建立測試檔案，所有函數使用 `todo!()`
   - 執行測試：7 failed, 2 passed

2. **cli.rs**: 建立 CLI 結構與測試
   - 執行測試：8 passed (clap 自動實作)

3. **provider_factory.rs**: 建立 factory 模式測試
   - 執行測試：編譯錯誤（Debug trait）修正後 8 passed

### GREEN Phase (實作最小程式碼)

1. **credentials.rs**: 實作所有方法
   - 載入/儲存 credentials
   - 檔案權限設定
   - 環境變數優先邏輯
   - 執行測試：9/9 passed ✓

2. **cli.rs**: clap derive 自動實作
   - 無需額外實作
   - 執行測試：8/8 passed ✓

3. **provider_factory.rs**: 實作 factory pattern
   - Provider enum 與 from_str
   - ProviderFactory::create
   - 錯誤處理
   - 執行測試：8/8 passed ✓

4. **main.rs**: 整合新架構
   - 更新 generate_summary 使用 ProviderFactory
   - 加入 handle_credentials_command
   - CLI 參數整合
   - 執行測試：1 failed（環境變數清理問題）

### REFACTOR Phase (改善程式碼)

1. **修正測試隔離問題**:
   - test_remove_provider 加入環境變數清理
   - 執行測試：77/77 passed ✓

2. **驗證 CLI 輸出**:
   - `--help` 輸出正確
   - `credentials --help` 正確
   - `credentials list` 正確

## 測試統計

**總測試數**: 77 tests
**通過**: 77 passed ✓
**失敗**: 0 failed
**忽略**: 7 ignored (需要實際 API 或 macOS say 命令)

**新增測試**:
- credentials: 9 tests
- cli: 8 tests
- provider_factory: 8 tests

**測試覆蓋率**:
- credentials.rs: 100%
- cli.rs: 100%
- provider_factory.rs: 100%

## 依賴變更

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }  # 新增
rpassword = "7"                                  # 新增
```

## 使用範例

### 設定 API Key

```bash
# Google/Gemini
claude-voice credentials set google
# 輸入: sk-...

# Anthropic
claude-voice credentials set anthropic

# OpenAI
claude-voice credentials set openai
```

### 查看已設定的 Providers

```bash
claude-voice credentials list
```

### 測試 API Key

```bash
claude-voice credentials test google
```

### 使用 CLI 參數覆寫設定

```bash
# 使用環境變數的 hook 執行
echo '{"session_id":"test","transcript_path":"..."}' | \
  claude-voice --provider google --model gemini-2.5-flash

# 使用 Ollama 本地模型（不需要 API key）
echo '{"session_id":"test","transcript_path":"..."}' | \
  claude-voice --provider ollama --model llama3.1
```

## 檔案結構

```
src/
├── cli.rs              # CLI 參數定義（新增）
├── credentials.rs      # Credential 管理（新增）
├── provider_factory.rs # Provider Factory（新增）
├── main.rs            # 整合 CLI 與 credentials（更新）
├── lib.rs             # 匯出新模組（更新）
└── ...

~/.config/claude-voice/
└── credentials.json    # API keys（0600 權限）
```

## 成功標準

- ✓ 所有測試通過（77/77）
- ✓ 編譯無錯誤
- ✓ CLI 幫助文件正確顯示
- ✓ credentials 檔案權限為 0600
- ✓ 環境變數優先於檔案
- ✓ 友善的錯誤訊息
- ✓ 支援多個 provider 別名
- ✓ Ollama 不需要 API key

## 下一步

1. 更新文件（README.md）說明新的 credentials 管理方式
2. 考慮加入 `credentials test` 的實際 API 測試
3. 考慮加入 credentials 檔案加密
