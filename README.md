# CC Sessions

![Version](https://img.shields.io/github/v/release/ccpopy/cc-sessions?label=version&sort=semver)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)
![Tauri](https://img.shields.io/badge/Tauri-2-ff9900)

CC Sessions 是一款本地桌面应用，用于浏览、检索、备份、导入导出及修复 Codex 与 Claude Code 的会话记录。应用基于 Tauri、React、TypeScript 和 Rust 构建，默认读取本机的 `.codex` 和 `.claude` 目录。

![CC Sessions 模拟数据截图](img/readme-screenshot.png)

## 功能

- 按 Codex / Claude Code 来源查看会话列表。
- 支持按 ID、标题、首条消息及工作目录进行搜索。
- 预览 JSONL 会话内容，区分用户消息、助手消息、推理过程、工具调用与工具返回。
- 备份、恢复、导入、导出会话包。
- 修复 Codex 本地索引、重建 `threads` 表、清理 orphan 记录。
- Codex 会话支持 provider 分支管理，并可从稳定对话节点创建回溯分支。
- 设置页面支持手动检查 GitHub Release 更新，并跳转至最新 Release 下载页面。

## 开发环境

前置依赖：

- Node.js 20 及以上版本
- npm
- Rust stable 工具链
- 目标平台对应的 Tauri 2 构建依赖

安装依赖：

```bash
npm ci
```

启动开发环境：

```bash
npm run tauri:dev
```

前端构建：

```bash
npm run build
```

Tauri 构建：

```bash
npm run tauri:build
```

## CLI / WSL 无桌面环境

仓库同时提供无桌面 CLI 二进制 `cc-sessions`。CLI 构建关闭 Tauri `desktop` feature，不启动窗口，也不依赖 WebView / WebKitGTK，适合 WSL、服务器或只有 SSH 的环境。

检查 CLI 构建：

```bash
npm run cli:check
```

构建 release 版 CLI：

```bash
npm run cli:build
```

构建后的二进制位于：

```bash
src-tauri/target/release/cc-sessions
```

Windows 下文件名为 `cc-sessions.exe`。也可以直接运行：

```bash
npm run cli:run -- list --limit 20
npm run cli:run -- -- --json repair diagnose
```

直接启动交互式菜单：

```bash
npm run cli:run
```

Windows release 版构建后可直接运行：

```powershell
.\src-tauri\target\release\cc-sessions.exe
```

进入菜单后输入序号即可逐层选择功能；列表页支持 `n` 下一页、`p` 上一页、`b` 返回上一层、`m` 返回主菜单、`0` 退出。删除、覆盖恢复、清理和分支切换等写入操作需要输入 `yes` 才会执行。

交互菜单里的“预览会话内容”默认只显示用户消息和助手消息，不显示工具调用、工具返回和元数据。如需排查完整 JSONL 事件流，可在预览模式中选择“全部事件”。

常用命令：

```bash
cc-sessions
cc-sessions menu
cc-sessions list --limit 20
cc-sessions --provider claude search "关键词"
cc-sessions projects --archived
cc-sessions preview ~/.codex/sessions/.../rollout-xxx.jsonl --limit 40
cc-sessions backup create --backup-dir ./backups --id <session-id> --name first-backup
cc-sessions repair diagnose --json
cc-sessions repair index --dry-run
cc-sessions bundle export --out-dir ./bundles --id <session-id>
```

默认路径与桌面端一致：Codex 读取 `~/.codex`，Claude Code 读取 `~/.claude`。可通过 `--codex-dir`、`--claude-dir` 覆盖。需要机器可读输出时加 `--json`。

### CLI 修复项说明

CLI 和桌面端的修复功能只处理 Codex 本地索引和可见性问题，不会修改会话正文语义，也不会凭空恢复已经删除的 JSONL 会话文件。

- `修复 session_index.jsonl`：扫描 `~/.codex/sessions/` 下仍存在的 active rollout 文件，重建 Codex 的 `session_index.jsonl`。它用于修复“会话文件还在，但索引缺失导致列表看不到”的问题，不是修复 JSONL 内容。
- `重建 threads 表`：从 rollout 元数据重新写入或更新 `~/.codex/state_5.sqlite` 中的 `threads` 表。它用于修复 Codex 本地列表、搜索、标题、工作目录等数据库记录缺失或漂移的问题。
- `清理 orphan 记录`：删除 `session_index.jsonl` 或 `threads` 表里指向已不存在 rollout 文件的残留记录。它不会删除仍存在的有效会话文件。
- `克隆会话到 provider` / `批量克隆到当前 provider`：用于处理 Codex `model_provider` 切换后，历史会话 provider 与当前配置不一致导致的可见性或续聊问题。
- `从事件创建回溯分支`：从某个稳定事件位置复制出新分支，并归档原 active 分支。该操作会写入本地 Codex 会话文件和索引，执行前会要求确认。

## 发布

项目中以下文件的版本号需保持一致：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

推送形如 `v0.2.6` 的 tag 将触发 GitHub Actions 打包并创建 Release：

```bash
git tag -a v0.2.6 -m "v0.2.6"
git push origin main
git push origin v0.2.6
```

工作流会在 Windows、macOS 和 Linux 上分别构建 Tauri 安装产物。macOS 打包要求 `src-tauri/icons/icon.icns` 存在，本仓库已提交 Tauri 生成的跨平台图标文件。

Windows Release 会额外上传 `cc-session-manager-portable-v版本号-windows.exe`，这是无需安装即可直接运行的便携版可执行文件。

Release 也会在 Windows、macOS 和 Linux job 中分别上传 `cc-sessions-cli-v版本号-平台.zip`，这是不依赖桌面环境的 CLI 版本。远程仓库推送版本 tag 触发发布时，CLI 包会和桌面安装包一起出现在同一个 GitHub Release 中。

## 手动打包

生成源码包：

```bash
npm run package:source
```

生成便携包：

```bash
npm run package:portable
```

在 Windows 上，该命令会同时生成便携版压缩包和可直接运行的 `cc-session-manager-portable-v版本号-windows.exe`。

生成安装器包：

```bash
npm run package:product
```

生成 CLI 包：

```bash
npm run package:cli
```

打包输出位于 `release/` 目录，该目录不会提交到仓库。

## macOS 可执行文件处理

从 GitHub Release 下载的 macOS 应用可能被 Gatekeeper 阻止运行，需移除 quarantine 扩展属性：

```bash
# 移除 .app 包的隔离标记
xattr -d com.apple.quarantine "/Applications/CC Sessions.app"
```

若使用便携包中的独立二进制文件，需额外赋予可执行权限：

```bash
chmod +x cc-session-manager
xattr -d com.apple.quarantine cc-session-manager
```

## 特别感谢

[linux.do](https://linux.do) —— 真诚、友善、团结、专业，共建你我引以为荣之社区。
[codex-session-cloner](https://github.com/goodnightzsj/codex-session-cloner) —— 参考了修复和会话导出导入的代码

## License

MIT
