# Antigravity Tools

[English](./README_EN.md)

面向 Antigravity 用户的 macOS 桌面应用，用于管理本地账号，并查看设备上的 Token 使用统计。

## 功能

- **账号管理**：添加和管理 Google 账号、查看配额，并在本机 Antigravity 环境中切换账号。
- **Token 仪表盘**：扫描本地 Antigravity 对话记录，按今天、昨天、近 3 天、近 7 天或近 30 天查看用量；支持按模型查看明细与费用估算。
- **个性化设置**：浅色、深色或跟随系统主题；简体中文和英文界面；配置后台配额刷新与当前账号同步频率。
- **本地数据**：账号配置保存在本机 `~/.antigravity_tools/`。Token 统计直接读取本地记录；费用估算所需的公开模型价格会同步并缓存。

## 构建

需要 Node.js 20+、Rust stable，以及 Tauri 2 对应的 macOS 构建工具。

```bash
npm ci
npm run tauri dev       # 开发模式
npm run build           # 仅构建前端
npm run tauri build     # 构建 macOS 应用
```

应用包位于 `src-tauri/target/release/bundle/macos/`。

## 项目结构

```text
src/                  React + TypeScript 界面
src/locales/          简体中文与英文
src-tauri/src/        Rust / Tauri 桌面端
src-tauri/icons/      应用图标
```

## 许可与来源

本项目基于 [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) 定制，并沿用仓库中的 [CC BY-NC-SA 4.0](./LICENSE) 许可证。使用、修改和再分发时请遵守许可证及原项目的署名要求。
