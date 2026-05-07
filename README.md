# CC Sessions

![Version](https://img.shields.io/badge/version-0.2.5-blue)
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
- 修复 Codex 本地索引、重建 `threads` 表、清理孤儿记录。
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

## 发布

项目中以下文件的版本号需保持一致：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

推送形如 `v0.2.5` 的 tag 将触发 GitHub Actions 打包并创建 Release：

```bash
git tag -a v0.2.5 -m "v0.2.5"
git push origin main
git push origin v0.2.5
```

工作流会在 Windows、macOS 和 Linux 上分别构建 Tauri 安装产物。macOS 打包要求 `src-tauri/icons/icon.icns` 存在，本仓库已提交 Tauri 生成的跨平台图标文件。

Windows Release 会额外上传 `cc-session-manager-portable-v版本号-windows.exe`，这是无需安装即可直接运行的便携版可执行文件。

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

## License

MIT
