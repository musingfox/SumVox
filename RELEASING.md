# Release Guide

本指南說明如何發布 `claude-voice` 的新版本。

## 發布流程

### 1. 準備發布

```bash
# 確保所有測試通過
cargo test

# 確保程式碼已提交
git status

# 更新版本號 (Cargo.toml)
# 更新 CHANGELOG (如果有)
```

### 2. 創建版本標籤

```bash
# 創建並推送 tag (自動觸發 GitHub Actions)
git tag v1.0.0
git push origin v1.0.0
```

### 3. 自動化構建

GitHub Actions 會自動:
- 為 macOS (x86_64, ARM64) 和 Linux (x86_64, ARM64) 建構二進制檔案
- 創建 GitHub Release
- 上傳壓縮檔案和 SHA256 校驗和

### 4. 更新 Homebrew Formula

#### 選項 A: 個人 Tap (推薦初期使用)

```bash
# 1. 創建 Homebrew tap 倉庫
# 倉庫名稱必須是: homebrew-<tap-name>
# 例如: homebrew-claude-voice

# 2. 複製 formula
cp homebrew/claude-voice.rb /path/to/homebrew-claude-voice/Formula/

# 3. 更新 SHA256
# 從 GitHub Release 頁面獲取 tar.gz 的 SHA256
SHA256=$(curl -sL https://github.com/nickhuang/claude-voice/archive/refs/tags/v1.0.0.tar.gz | shasum -a 256 | awk '{print $1}')

# 4. 更新 formula 中的 url 和 sha256
sed -i '' "s/PLACEHOLDER_SHA256/$SHA256/" Formula/claude-voice.rb

# 5. 提交並推送
git add Formula/claude-voice.rb
git commit -m "Release claude-voice v1.0.0"
git push
```

使用者安裝方式:
```bash
brew tap nickhuang/claude-voice
brew install claude-voice
```

#### 選項 B: 提交到 Homebrew Core (需要專案成熟後)

Homebrew Core 的要求:
- 專案有一定知名度和用戶基礎
- 30 天內有 50+ stars 或 75+ forks
- 持續維護和更新
- 遵循 Homebrew 的所有規範

提交流程:
```bash
# 1. Fork homebrew-core
# 2. 添加 formula 到 Formula/ 目錄
# 3. 測試 formula
brew install --build-from-source ./Formula/claude-voice.rb
brew test claude-voice
brew audit --strict claude-voice

# 4. 提交 PR 到 Homebrew/homebrew-core
```

### 5. 發布到 crates.io (可選)

```bash
# 1. 登錄 crates.io
cargo login

# 2. 發布
cargo publish --dry-run  # 先測試
cargo publish           # 正式發布
```

使用者安裝方式:
```bash
cargo install claude-voice
```

## 版本號規範

遵循 [Semantic Versioning](https://semver.org/):

- `MAJOR.MINOR.PATCH` (例如: 1.0.0)
- **MAJOR**: Breaking changes (不向後兼容)
- **MINOR**: 新功能 (向後兼容)
- **PATCH**: Bug 修復 (向後兼容)

## 發布檢查清單

- [ ] 所有測試通過 (`cargo test`)
- [ ] 更新 `Cargo.toml` 中的版本號
- [ ] 更新 README.md 中的版本相關資訊
- [ ] 提交所有變更
- [ ] 創建 git tag (`git tag vX.Y.Z`)
- [ ] 推送 tag (`git push origin vX.Y.Z`)
- [ ] 等待 GitHub Actions 完成構建
- [ ] 驗證 GitHub Release 頁面
- [ ] 更新 Homebrew formula (如果使用 tap)
- [ ] 測試安裝流程

## 回滾發布

如果需要撤回發布:

```bash
# 刪除 tag
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0

# 刪除 GitHub Release (手動在 GitHub 上操作)

# 如果已發布到 crates.io (無法刪除,只能 yank)
cargo yank --vers 1.0.0
```

## 常見問題

### Q: GitHub Actions 構建失敗?
檢查:
- Cargo.toml 中的依賴版本是否正確
- 跨平台編譯工具是否安裝
- 查看 Actions 日誌找出具體錯誤

### Q: Homebrew formula 測試失敗?
```bash
# 本地測試
brew install --build-from-source ./homebrew/claude-voice.rb
brew test claude-voice
brew audit --strict claude-voice
```

### Q: 如何更新 SHA256?
```bash
# 計算 tar.gz 的 SHA256
curl -sL https://github.com/USER/REPO/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256
```

## 相關資源

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Cargo Publishing Guide](https://doc.rust-lang.org/cargo/reference/publishing.html)
- [Semantic Versioning](https://semver.org/)
