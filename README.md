# Antigravity Tools Lite

[简体中文](./README.zh-CN.md)

A small, local-first desktop companion for [Antigravity](https://antigravity.google): keep several Google accounts handy, switch the one your editor **and** your `agy` CLI use with a click, and see exactly where your tokens went — with an estimated API cost. Nothing is uploaded anywhere: it reads Antigravity's own local files and writes credentials into your OS keychain, nothing else.

![Dashboard](docs/screenshots/dashboard.png)

## Why it exists

Antigravity works fine with one Google account. Once you have more — a personal one, a work one, a spare for when the quota runs out — the routine gets annoying: sign out, sign in, restart, lose your session. And the built-in usage view won't tell you how many tokens you burned yesterday, or which model ate them.

This app is that missing piece, built as a focused desktop tool:

- **Multi-account switching** — one click puts an account into the Antigravity app *or* the `agy` CLI.
- **Per-model quota** with reset countdowns and PRO / ULTRA / FREE grouping, so you can see who still has headroom.
- **A real token dashboard** — local usage for today / yesterday / last 3 / 7 / 30 days, per model, with an estimated API cost.
- **Local-only by design** — no proxy, no server, no telemetry, no account sync. Your credentials stay in your keychain.
- **Bilingual UI** (Simplified Chinese / English), light and dark themes, tray menu.

## Download

**[⬇︎ Latest release](https://github.com/anglee0323/antigravity-tools-lite/releases/latest)**

| Platform | Asset | Notes |
| --- | --- | --- |
| macOS (Apple Silicon) | `Antigravity-Tools-Lite-<version>-macos-arm64.zip` | Unzip, drag `Antigravity Tools Lite.app` into Applications |
| Windows (x64) | `Antigravity-Tools-Lite-<version>-windows-x64-setup.exe` | NSIS installer, per-user install |

Unsigned builds — the usual first-run warnings apply and are expected:

```bash
# macOS: right-click the app in Applications → Open → Open, or
xattr -dr com.apple.quarantine "/Applications/Antigravity Tools Lite.app"
```

On Windows, SmartScreen may show "Windows protected your PC" → **More info → Run anyway**.

## Managing accounts

Add an account with the **+** button. Three ways are built in:

- **OAuth** — opens your browser, you approve with Google, done.
- **Refresh token** — paste one token, or a JSON array of them, to import in bulk.
- **Import from this Mac** — scans the system keychain, the Antigravity IDE databases, plugins and the CLI's own directory (`~/.antigravity-agent`) and imports everything it finds.

![Accounts](docs/screenshots/accounts.png)

Each row has five actions, each with a tooltip so you never have to guess:

| Action | What it does |
| --- | --- |
| **Switch to this account** | Makes this account the one your local Antigravity uses. It safely closes a running Antigravity, writes the credentials where Antigravity keeps them (OS keychain — or `state.vscdb` on pre-2.0 builds), and updates the tray; open Antigravity again and you are signed in as this account. The **`agy` CLI reads the same credential entry**, so your next CLI command uses it too. |
| **Refresh quota** | Re-reads this account's per-model quota and reset times. |
| **Edit remark** | A short label (max 15 characters) so you can tell accounts apart at a glance. |
| **Delete** | Removes the account from this app. |

Rows are sortable by reset time or last use, draggable to reorder, and can be switched between table and card view. Select several and you can refresh or delete them in bulk; the filter pills (All / PRO / ULTRA / FREE) narrow the list.

### One switch, both clients

Antigravity and its `agy` CLI read the same credential entry from your OS keychain, so switching an account once is enough for both. The app is closed during the switch on purpose: if it stayed open it would keep refreshing (and re-writing) the old token, and your switch would silently revert. The CLI needs no restart — the next `agy` command uses the new account.

On very old Antigravity builds (pre-2.0) there is no keychain entry to write; the app detects that and injects the token into the build's local `state.vscdb` instead.

## Token dashboard

The dashboard reads Antigravity's own conversation databases (`conversation.db`, `token_usage_archive.db`) plus its archive directory, entirely locally, and summarises them. Nothing is approximated per request beyond what the records contain.

![Dashboard dark mode](docs/screenshots/dashboard-dark.png)

- **Date range** — today, yesterday, last 3, 7 or 30 days, with an hourly chart for the single-day ranges.
- **Hover any bar** for that hour's input / output / cached tokens, request count and the estimated cost of that hour.
- **KPI cards** — total tokens, input, output, cache hit rate and estimated API cost.
- **Model usage / model details** — which model is consuming your quota, sorted, with a per-model breakdown table.
- Cost is estimated from Google's public Gemini pricing pages (fetched once a day and cached) with a built-in fallback table. When a model has no known price, the card tells you instead of silently guessing.

## Settings

![Settings](docs/screenshots/settings.png)

- **Appearance & language** — system / light / dark, Simplified Chinese or English.
- **Background tasks** — how often account quotas refresh, and how often the active account is re-read from local Antigravity data.
- **Local data** — where the app keeps its files (`~/.antigravity_tools/`), with a button to open the folder.

## Your data

| | |
| --- | --- |
| Read | Antigravity's local conversation databases and archives (token counts, models, timestamps) |
| Written | `~/.antigravity_tools/` (accounts, config, cached pricing) and your OS credential store when you switch accounts |
| Never | Conversations, prompts or credentials are never uploaded; there is no proxy or server component |

## Build from source

Requires Node.js 20+, stable Rust and the platform build tools for Tauri 2.

```bash
npm ci
npm run tauri dev       # development
npm run build           # frontend only
npm run tauri build     # macOS .app / Windows installer
```

Bundles land in `src-tauri/target/release/bundle/`. Pushing a `v*` tag makes the release workflow build both platforms and attach them to the GitHub release.

## Relation to the upstream project

This is a focused fork of [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager). Upstream is a full toolkit (reverse proxy, HTTP API, Cloudflared tunnel, IP management, Docker image). This fork keeps the account manager and the local usage dashboard, removes the proxy and web-mode half of the codebase, and adds its own dashboard, bilingual UI, theme support and release tooling. If you need the proxy, use upstream — the two are independent projects.

## License

Based on [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) and distributed under the same [CC BY-NC-SA 4.0](./LICENSE) license. Follow the license and the original attribution requirements when using, modifying or redistributing this software.
