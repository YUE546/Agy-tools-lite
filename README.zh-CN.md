# Antigravity Tools Lite

[English](./README.md)

给 [Antigravity](https://antigravity.google) 用的本地小工具：把多个 Google 账号放在手边，一键切换编辑器**和** `agy` CLI 当前使用的账号，并看清你的 Token 到底花在哪、大概值多少钱。全部在本机完成——只读取 Antigravity 自己的本地文件，切换账号时把凭据写进系统钥匙串，别的什么都不做。

![仪表盘](docs/screenshots/dashboard.png)

## 为什么做这个

Antigravity 用一个 Google 账号时没什么问题。一旦有两个以上——个人的、公司的、额度用完后备的——操作就变得烦人：退出登录、重新登录、重启、会话丢失。而且内置的用量视图不会告诉你昨天烧了多少 Token、是哪个模型吃掉的。

这个小工具就是补上这块，做成一个专一的桌面应用：

- **多账号切换** —— 一次点击，把这个账号交给 Antigravity 应用**或者** `agy` CLI 使用。
- **按模型看配额** —— 带重置倒计时，并按 PRO / ULTRA / FREE 分组，谁还有余量一眼可见。
- **真正可用的 Token 仪表盘** —— 今天 / 昨天 / 近 3 天 / 近 7 天 / 近 30 天的本地用量，按模型拆分，并给出预估 API 费用。
- **只在本机** —— 没有代理、没有服务端、没有埋点、不上传账号。凭据只进你自己的钥匙串。
- **中英双语界面**、浅色/深色主题、托盘菜单。

## 下载安装

**[⬇︎ 最新版本](https://github.com/anglee0323/antigravity-tools-lite/releases/latest)**

| 平台 | 安装包 | 说明 |
| --- | --- | --- |
| macOS（Apple Silicon） | `Antigravity-Tools-Lite-<版本>-macos-arm64.zip` | 解压后把 `Antigravity Tools Lite.app` 拖进「应用程序」 |
| Windows（x64） | `Antigravity-Tools-Lite-<版本>-windows-x64-setup.exe` | NSIS 安装程序，按用户安装 |

两个平台都是**未签名**构建，首次打开会有系统提示，属正常现象：

```bash
# macOS：在「应用程序」里右键点图标 → 打开 → 再点「打开」，或执行
xattr -dr com.apple.quarantine "/Applications/Antigravity Tools Lite.app"
```

Windows 上 SmartScreen 可能提示「Windows 已保护你的电脑」→ 点 **更多信息 → 仍要运行**。

## 账号管理

点 **+** 添加账号，内置三种方式：

- **OAuth 授权** —— 打开浏览器，用 Google 同意授权即可。
- **Refresh Token** —— 粘贴单个 Token，或粘贴 JSON 数组批量导入。
- **从本机导入** —— 扫描系统钥匙串、Antigravity IDE 数据库、插件以及 CLI 自己的目录（`~/.antigravity-agent`），找到什么就导入什么。

![账号管理](docs/screenshots/accounts.png)

每一行有五个操作，全部带悬浮说明，不用猜：

| 操作 | 作用 |
| --- | --- |
| **切换到此账号** | 让本机 Antigravity 使用这个账号：先安全关闭正在运行的 Antigravity，再把凭据写入它保存账号的位置（系统钥匙串；2.0 以前的老版本写 `state.vscdb`），并更新托盘。重新打开 Antigravity 就是该账号。**`agy` CLI 读取的是同一个凭据项**，所以下一条 CLI 命令同样会用它。 |
| **刷新此账号配额** | 重新读取该账号各模型的配额与重置时间。 |
| **编辑备注** | 写一个短标签（最多 15 字），方便区分账号。 |
| **删除此账号** | 从本应用中移除该账号。 |

列表可按「配额重置时间」「最后使用」排序，可拖拽排序，也能在表格/卡片视图之间切换；勾选多个账号可以批量刷新或删除；顶部的筛选（全部 / PRO / ULTRA / FREE）用于快速收窄列表。

### 一个按钮，两个客户端通用

Antigravity 和它的 `agy` CLI 读取的是系统钥匙串里的**同一个凭据项**，所以切换一次两边都生效。切换时会先关闭正在运行的 Antigravity，这是有意的：如果它一直开着，会继续用它手里的旧 Token 去刷新并把凭据写回去，你的切换就会悄悄失效。CLI 不需要重启，下一条 `agy` 命令就是新账号。

对于 2.0 以前的老版本 Antigravity，没有钥匙串凭据可写，应用会自动改为把 Token 写入该版本本地的 `state.vscdb`。

## Token 仪表盘

仪表盘完全在本地读取 Antigravity 自己的对话数据库（`conversation.db`、`token_usage_archive.db`）及其归档目录并做汇总，不额外估算每条请求。

![深色模式](docs/screenshots/dashboard-dark.png)

- **统计范围** —— 今天 / 昨天 / 近 3 天 / 近 7 天 / 近 30 天；单日范围显示按小时的柱状图。
- **悬浮任意柱状条** —— 该时段的输入 / 输出 / 缓存 Token、请求次数，以及该时段的预估费用。
- **指标卡片** —— 总 Token、输入、输出、缓存命中率、预估 API 费用。
- **模型用量 / 模型明细** —— 哪个模型在消耗配额，含按模型的明细表。
- 费用按 Google 公开的 Gemini 价格页估算（每天同步一次并缓存），并内置兜底价格表；遇到没有价格的模型会明确提示，而不是悄悄按 0 计。

## 设置

![设置](docs/screenshots/settings.png)

- **外观与语言** —— 跟随系统 / 浅色 / 深色，简体中文或英文。
- **后台任务** —— 账号配额自动刷新频率，以及从本地 Antigravity 数据同步当前账号的频率。
- **本地数据** —— 应用数据位置（`~/.antigravity_tools/`），一键打开目录。

## 你的数据

| | |
| --- | --- |
| 读取 | Antigravity 的本地对话数据库与归档（Token 数、模型、时间戳） |
| 写入 | `~/.antigravity_tools/`（账号、配置、价格缓存），以及切换账号时的系统凭据存储 |
| 绝不上传 | 对话内容、提示词与凭据都不会上传；没有代理或任何服务端组件 |

## 从源码构建

需要 Node.js 20+、Rust stable，以及 Tauri 2 对应的平台构建工具。

```bash
npm ci
npm run tauri dev       # 开发模式
npm run build           # 仅构建前端
npm run tauri build     # macOS .app / Windows 安装包
```

产物位于 `src-tauri/target/release/bundle/`。推送 `v*` tag 会让发布工作流同时构建两个平台，并自动挂到 GitHub Release 上。

## 与上游项目的关系

本项目是 [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) 的精简分支。上游是一套完整工具箱（反向代理、HTTP API、Cloudflared 隧道、IP 管理、Docker 镜像）；本分支只保留账号管理与本地用量仪表盘，移除了代理与 Web 模式那一半代码，并加上了自己的仪表盘、双语界面、主题和发布流程。需要代理功能请用上游——两者是独立项目。

## 许可

基于 [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) 定制，沿用 [CC BY-NC-SA 4.0](./LICENSE) 许可证。使用、修改和再分发时请遵守许可证及原项目的署名要求。
