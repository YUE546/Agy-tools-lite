# Antigravity Tools

[简体中文](./README.md)

A macOS desktop app for Antigravity users to manage local accounts and review token usage on their device.

## Features

- **Account management**: Add and organize Google accounts, review quotas, and switch the active account in the local Antigravity environment.
- **Token dashboard**: Scan local Antigravity conversation records and view usage for today, yesterday, the last 3, 7, or 30 days. Inspect per-model details and estimated costs.
- **Preferences**: Choose a light, dark, or system theme; use the Simplified Chinese or English interface; configure background quota refresh and active-account sync intervals.
- **Local data**: Account configuration is stored on the device under `~/.antigravity_tools/`. Token statistics are read from local records. Public model pricing used for cost estimates is fetched and cached locally.

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
```

## License and attribution

This is a customization based on [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) and retains the repository's [CC BY-NC-SA 4.0](./LICENSE) license. Follow the license and the original project's attribution requirements when using, modifying, or redistributing this software.
