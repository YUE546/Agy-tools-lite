# Antigravity Tools Lite

[简体中文](./README.md)

A macOS desktop app for Antigravity users: manage your local Google accounts and review token usage and estimated cost on your machine. All data stays on this device.

## Download

**[⬇︎ Download the latest release](https://github.com/anglee0323/Antigravity-Tools-Lite/releases/latest)** (macOS · Apple Silicon)

1. Download `Antigravity-Tools-<version>-macos-arm64.zip`
2. Unzip it and drag `Antigravity Tools Lite.app` into Applications
3. If macOS says the developer cannot be verified: **right-click the icon in Applications → Open → Open again**, or run
   `xattr -dr com.apple.quarantine "/Applications/Antigravity Tools Lite.app"`

> The app is ad-hoc signed and not notarized by Apple, so the first launch needs that manual confirmation.

**Requirements**: macOS (Apple Silicon) with Antigravity installed.

## Screenshots

| Dashboard (light) | Dashboard (dark / English) |
| :---: | :---: |
| <img src="docs/screenshots/dashboard.png" width="430" alt="Dashboard"> | <img src="docs/screenshots/dashboard-dark.png" width="430" alt="Dark mode"> |

| Accounts | Settings |
| :---: | :---: |
| <img src="docs/screenshots/accounts.png" width="430" alt="Accounts"> | <img src="docs/screenshots/settings.png" width="430" alt="Settings"> |

> Screenshots use sample data to demonstrate the interface.

## Features

- **Account management**: Add and organize Google accounts, review per-model quotas with reset times, and switch the active account in the local Antigravity environment.
- **Token dashboard**: Scan local Antigravity conversation records and view usage for today, yesterday, the last 3, 7, or 30 days. Per-model details, and hovering a chart bar shows that hour's input / output / cached / request counts plus the estimated cost.
- **Preferences**: Choose a light, dark, or system theme; use the Simplified Chinese or English interface; configure background quota refresh and active-account sync intervals.
- **Local data**: Account configuration is stored under `~/.antigravity_tools/`. Token statistics are read from local records, and public model pricing used for cost estimates is fetched and cached locally.

## Diff from upstream

This project is a customization of [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) that keeps only what a desktop app needs:

- Removed the upstream proxy backend (reverse proxy, HTTP API, Cloudflared tunnel, IP management, token statistics modules) and its UI entry points.
- Removed the Web/Docker login gate and its copy.
- Focused the UI on account management, the local token dashboard and settings, with light/dark themes, Simplified Chinese / English, and cost estimation.

## Build

Requires Node.js 20+, stable Rust, and the macOS build tools required by Tauri 2.

```bash
npm ci
npm run tauri dev       # Development
npm run build           # Frontend only
npm run tauri build     # Build the macOS app
```

The app bundle is written to `src-tauri/target/release/bundle/macos/`.

## Project layout

```text
src/                  React + TypeScript interface
src/locales/          Simplified Chinese and English
src-tauri/src/        Rust / Tauri desktop application
src-tauri/icons/      Application icons
docs/screenshots/     README screenshots
```

## License and attribution

This is a customization based on [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) and retains the repository's [CC BY-NC-SA 4.0](./LICENSE) license. Follow the license and the original project's attribution requirements when using, modifying, or redistributing this software.
