# codex-kanban 发布与部署清单

本文档用于发布 `@duo121/codex-kanban`，并联动 Homebrew cask。

## 1. 你需要先配置的账号与密钥

1. npm 作用域与包名
- 确保你拥有 `@duo121` scope 的发布权限。
- 需要发布的包：
  - `@duo121/codex-kanban`
  - `@duo121/codex-kanban-linux-x64`
  - `@duo121/codex-kanban-linux-arm64`
  - `@duo121/codex-kanban-darwin-x64`
  - `@duo121/codex-kanban-darwin-arm64`
  - `@duo121/codex-kanban-win32-x64`
  - `@duo121/codex-kanban-win32-arm64`

2. npm Trusted Publisher（推荐）
- 在 npm 上把上述包绑定到 GitHub 仓库 `duo121/codex-kanban` 的 release workflow（`.github/workflows/rust-release.yml`）。

3. GitHub Secrets
- `HOMEBREW_TAP_TOKEN`：用于向 tap 仓库推送 cask 更新。
- `PROJECT_VERCEL_DEPLOY_HOOK_URL`：可选，用于稳定版发布后触发文档站部署。
- `WINGET_PUBLISH_PAT`：可选，仅在你要发 WinGet 时需要。

## 2. Homebrew tap 仓库

建议使用独立 tap 仓库：`duo121/homebrew-codex-kanban`。

最小目录结构：

```text
homebrew-codex-kanban/
└── Casks/
    └── codex-kanban.rb
```

本仓库已提供自动生成 cask 的脚本：

```bash
python3 ./scripts/release/update_homebrew_cask.py \
  --version 0.1.0 \
  --repo duo121/codex-kanban \
  --tap-dir /path/to/homebrew-codex-kanban
```

## 3. 发布流程（推荐）

1. 在 `codex-kanban` 打 tag，例如：`rust-v0.1.0`。
2. 触发 `rust-release.yml`，产出 GitHub Release 与 npm tarballs。
3. `publish-npm` job 会自动发布 npm 包（基于 OIDC）。
4. 触发 `homebrew-cask.yml`，自动更新 tap 仓库中的 `Casks/codex-kanban.rb`。

## 4. 用户安装命令

npm：

```bash
npm install -g @duo121/codex-kanban
```

Homebrew（tap 方式）：

```bash
brew tap duo121/homebrew-codex-kanban
brew install --cask codex-kanban
```

启动命令：

```bash
codexkb
# or
codex-kanban
```

## 5. 本地验收顺序（你指定的流程）

```bash
# 1) brew 安装
brew tap duo121/homebrew-codex-kanban
brew install --cask codex-kanban
codexkb --version

# 2) brew 卸载
brew uninstall --cask codex-kanban

# 3) npm 全局安装
npm uninstall -g @duo121/codex-kanban || true
npm install -g @duo121/codex-kanban
codexkb --version
```

如果 `npm install -g @duo121/codex-kanban` 后运行报缺少平台可选依赖，通常表示该平台包（例如 `@duo121/codex-kanban-darwin-arm64`）尚未发布或 dist-tag 未正确设置。
