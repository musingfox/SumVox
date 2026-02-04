# Release Quick Start

## 一次性設置

### 1. 安裝 Just (可選,用於自動化任務)

```bash
cargo install just
```

### 2. 創建 Homebrew Tap 倉庫

```bash
# 在 GitHub 上創建新倉庫: homebrew-claude-voice
# 克隆到本地
git clone https://github.com/YOUR_USERNAME/homebrew-claude-voice.git
cd homebrew-claude-voice

# 創建 Formula 目錄
mkdir -p Formula

# 複製 formula (稍後更新)
cp /path/to/claude-voice/homebrew/claude-voice.rb Formula/
```

## 發布新版本

### 快速流程 (使用 Just)

```bash
# 1. 準備發布 (更新版本號並創建 tag)
just release 1.0.1

# 2. 推送到 GitHub (觸發自動構建)
git push origin main
git push origin v1.0.1

# 3. 等待 GitHub Actions 完成構建
# 查看: https://github.com/YOUR_USERNAME/claude-voice/actions

# 4. 更新 Homebrew formula 的 SHA256
just update-formula 1.0.1

# 5. 提交 Homebrew formula
cd /path/to/homebrew-claude-voice
cp /path/to/claude-voice/homebrew/claude-voice.rb Formula/
git add Formula/claude-voice.rb
git commit -m "Release claude-voice v1.0.1"
git push
```

### 手動流程

```bash
# 1. 更新版本號
# 編輯 Cargo.toml 中的 version = "1.0.1"

# 2. 運行測試
cargo test

# 3. 提交版本變更
git add Cargo.toml
git commit -m "chore: bump version to 1.0.1"

# 4. 創建 tag
git tag -a v1.0.1 -m "Release v1.0.1"

# 5. 推送
git push origin main
git push origin v1.0.1

# 6. GitHub Actions 會自動:
#    - 為多個平台構建二進制檔案
#    - 創建 GitHub Release
#    - 上傳 tar.gz 和 SHA256 檔案

# 7. 更新 Homebrew formula
curl -sL https://github.com/YOUR_USERNAME/claude-voice/archive/refs/tags/v1.0.1.tar.gz | shasum -a 256
# 複製 SHA256,更新 homebrew/claude-voice.rb

# 8. 提交到 Homebrew tap
cd /path/to/homebrew-claude-voice
cp /path/to/claude-voice/homebrew/claude-voice.rb Formula/
git add Formula/claude-voice.rb
git commit -m "Release claude-voice v1.0.1"
git push
```

## 測試安裝

```bash
# 測試 Homebrew formula
just test-formula

# 或手動測試
brew install --build-from-source ./homebrew/claude-voice.rb
brew test claude-voice

# 測試安裝
brew uninstall claude-voice
brew tap YOUR_USERNAME/claude-voice
brew install claude-voice
```

## 常用 Just 命令

```bash
# 查看所有可用命令
just

# 運行測試
just test

# 構建 release 版本
just build-release

# 檢查程式碼品質 (格式化 + linting + 測試)
just check

# 創建本地平台的 tarball
just package 1.0.1

# 清理建構產物
just clean
```

## 版本號規則

- **1.0.0** → **1.0.1**: Bug 修復
- **1.0.0** → **1.1.0**: 新功能 (向後兼容)
- **1.0.0** → **2.0.0**: Breaking changes

## 故障排除

### GitHub Actions 失敗?

```bash
# 檢查工作流程狀態
gh run list --workflow=release.yml

# 查看具體錯誤
gh run view <RUN_ID>
```

### Homebrew 安裝失敗?

```bash
# 查看詳細日誌
brew install --build-from-source --verbose ./homebrew/claude-voice.rb

# 檢查 formula 語法
brew audit --strict claude-voice
```

### SHA256 不匹配?

```bash
# 重新計算並更新
just update-formula 1.0.1
```

## 資源連結

- [完整發布指南](../RELEASING.md)
- [GitHub Actions 文件](https://docs.github.com/en/actions)
- [Homebrew Formula 教學](https://docs.brew.sh/Formula-Cookbook)
- [Just 命令運行器](https://github.com/casey/just)
