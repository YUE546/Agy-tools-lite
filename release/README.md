# Antigravity Tools Lite —— 无 WebView 服务版（本机运行包）

本目录是一个**自包含**的服务运行包：不依赖 WebView2 / 任何浏览器内核，
账号管理 + 用量仪表盘的 WebUI + `/api` 管理桥全部由一个二进制 + 静态 `dist/` 提供，
用系统浏览器访问。基于 [antigravity-tools-lite](https://github.com/anglee0323/antigravity-tools-lite)
代码基座改造，**不含 AI 代理网关**（需要代理功能请使用上游 Antigravity-Manager）。

## 一键启动

- **Windows**：双击 `start-webui.bat`（或命令行运行）
- **Linux / macOS**：`chmod +x start-webui.sh && ./start-webui.sh`

脚本会以 `--headless --open` 启动服务，并自动用系统默认浏览器打开 Web UI。

- 访问地址：`http://localhost:8045`（健康检查：`/health`）
- 登录密码：优先使用 `WEB_PASSWORD` 环境变量；未设置时首次启动会在控制台/日志打印
  自动生成的随机密码（同时持久化到 `%USERPROFILE%\.antigravity_tools\web_config.json`）。
- 数据目录：`%USERPROFILE%\.antigravity_tools`（Linux/macOS: `~/.antigravity_tools`），
  与桌面版 Antigravity Tools Lite 共享，账号数据互通。

## 常用环境变量（可在启动前设置）

| 变量 | 说明 | 默认值 |
| --- | --- | --- |
| `WEB_PASSWORD` | Web UI 登录密码（`ABV_WEB_PASSWORD` 优先） | 首次运行自动生成并打印 |
| `PORT` | 服务端口（`ABV_PORT` 优先） | `8045` |
| `ABV_BIND_LOCAL_ONLY` | `1` 仅绑定 127.0.0.1（一键脚本默认）；`0` 允许局域网访问 | 脚本内默认 `1` |
| `ABV_DIST_PATH` | WebUI 静态资源目录 | 脚本内指向 `./dist` |
| `ANTIGRAVITY_DISABLE_TRAY` | `1` 禁用系统托盘（仅 Windows） | 不禁用 |

环境变量覆盖会持久化回 `web_config.json`——这是修改密码的正规途径。

## 目录结构

```
release/
├── antigravity-tools.exe   # 服务二进制（纯 tokio/axum，不链接 WebView2/wry/tauri）
├── dist/                   # WebUI 静态资源（React 构建产物）
├── start-webui.bat         # Windows 一键启动
├── start-webui.sh          # Linux/macOS 一键启动
└── README.md
```

## 与桌面版的区别

- 无主窗口、无快捷浮窗；Windows 下保留系统托盘（打开 Web UI / 退出）。
- WebUI 在浏览器中运行：账号管理、配额刷新、OAuth 添加、低配额自动切换、用量仪表盘均可用。
- 桌面专属能力（开机自启设置、App 界面本地化入口的窗口联动、menubar 浮窗）不可用。
- `agy-lite` CLI（`antigravity-tools.exe` 复制/改名为 `agy-lite.exe` 后可用）仍随包提供。

## 停止服务

在启动窗口按 `Ctrl+C`，或使用系统托盘菜单的"退出"。如需后台常驻，
建议注册为计划任务或系统服务（Windows 可用 NSSM）。
