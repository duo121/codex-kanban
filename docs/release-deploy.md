# codex-kanban 发布与部署清单

本文档用于发布 `@duo121/codex-kanban`，并联动 Homebrew cask。

## 1. 你需要先配置的账号与密钥

1. npm 作用域与包名
- 确保你拥有 `@duo121` scope 的发布权限。
- 这个 fork 实际发布到 npm 的包名只有一个：`@duo121/codex-kanban`
- Linux / macOS / Windows 平台变体不是单独的 npm 包；它们是同名包的不同平台版本，例如：
  - `0.116.0-kanban.0-linux-x64`
  - `0.116.0-kanban.0-darwin-arm64`
  - `0.116.0-kanban.0-win32-x64`
- 根包里的 `optionalDependencies` 会使用 `npm:` alias 指向这些平台版本，所以你不需要额外创建 `@duo121/codex-kanban-linux-x64` 这类真实包名。

2. npm Trusted Publisher（推荐）
- 在 npm 上把 `@duo121/codex-kanban` 绑定到 GitHub 仓库 `duo121/codex-kanban` 的 release workflow（`.github/workflows/rust-release.yml`）。

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

版本号策略（建议）：

- 跟随上游官方稳定版本，并添加 fork 后缀：`<官方版本>-kanban.<序号>`
- 示例：
  - 官方 `0.116.0` -> fork 首发 `0.116.0-kanban.0`
  - 同一官方版本上的增量发布：`0.116.0-kanban.1`
  - 同步到官方 `0.117.0` 后重置为 `0.117.0-kanban.0`

1. 在 `codex-kanban` 打 tag，例如：`rust-v0.116.0-kanban.0`。
2. 触发 `rust-release.yml`，产出 GitHub Release 与 npm tarballs。
3. `publish-npm` job 会自动发布 npm 包（基于 OIDC）。
4. 触发 `homebrew-cask.yml`，自动更新 tap 仓库中的 `Casks/codex-kanban.rb`。

说明：

- 你本地只有 macOS 也没关系；Linux 和 Windows 的 npm 平台包由 GitHub Actions 在对应 runner 上构建并发布。
- 当前 workflow 只发布 `@duo121/codex-kanban` 这一组 CLI npm tarballs，不再尝试发布 `@openai/codex-responses-api-proxy` 或 `@openai/codex-sdk`。
- 如果仓库没有配置 `ENABLE_WINDOWS_RELEASE=true`、Windows runners 和签名 secrets，release 会自动跳过 win32 npm 包，只发布 Linux / macOS 平台包。
- `publish-npm` 会先发平台 tarball，再发根包，避免 `latest` 先指向一个还拿不到平台依赖的版本。
- 如果是这些 npm 包第一次出现在 npm 上，需要先完成一次首发，然后再到 npm 包设置里绑定 Trusted Publisher。

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
