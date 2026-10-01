# Antigravity Tools Lite

[简体中文](./README.zh-CN.md)

A local desktop application for [Antigravity](https://antigravity.google). It manages the Google accounts used by the Antigravity app and its `agy` CLI, shows per-model quota with reset countdowns, and summarises locally recorded token usage together with an estimated API cost. All processing happens on the local machine

![Dashboard](docs/screenshots/dashboard-en.png)

## Overview

- **Account management** — import accounts already present on the machine, switch the account Antigravity uses, annotate accounts with remarks, remove them again
- **Quota overview** — per-model quota and reset time, grouped by PRO / ULTRA / FREE, with table and card views
- **Usage dashboard** — token usage for today, yesterday, the last 3, 7 or 30 days, broken down per model, with an estimated API cost
- **One switch for both clients** — switching synchronizes the credentials used by Antigravity and an initialized `agy` CLI
- **Local data only** — no proxy, no background service, no telemetry; credentials stay in local account files and the credential stores required by the clients
- **Bilingual interface** — Simplified Chinese and English, light and dark themes, tray menu

## Download

**[Latest release](https://github.com/anglee0323/antigravity-tools-lite/releases/latest)**

| Platform | Package | Installation |
| --- | --- | --- |
| macOS (Apple Silicon) | `Antigravity-Tools-Lite-<version>-macos-arm64.zip` | Unpack, then move `Antigravity Tools Lite.app` to Applications |
| Windows (x64) | `Antigravity-Tools-Lite-<version>-windows-x64-setup.exe` | NSIS installer, per-user installation |
| Linux (x64) | `Antigravity-Tools-Lite-<version>-linux-amd64.deb` | `sudo apt install ./package.deb` |

The release workflow does not configure Developer ID signing/notarization or Windows Authenticode signing. macOS or Windows may therefore warn about or block a downloaded package. Check its release source and checksum, and make any required trust decision yourself through the operating system's normal review flow. See [Apple's guidance](https://support.apple.com/en-gb/102445) and [Microsoft's app-reputation guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation). Homebrew does not remove these platform checks

## Account management

The **+** button offers three ways to add an account

- **OAuth** — opens the browser, the account is added after you approve access with Google
- **Refresh token** — paste a single token or a JSON array of tokens to import several accounts at once
- **Import from this machine** — scans the system credential store, the Antigravity databases, installed plugins, the native agy session and the legacy CLI data directory (`~/.antigravity-agent`), then imports every account it finds

![Accounts — table view](docs/screenshots/accounts-list.png)

![Accounts — card view](docs/screenshots/accounts-cards.png)

Each row provides four actions, each with a tooltip

| Action | Effect |
| --- | --- |
| **Switch to this account** | Makes this account the one Antigravity uses. A running Antigravity is closed first, the credentials are written to the credential store Antigravity reads (or to `state.vscdb` on builds older than 2.0), and the tray is updated. After reopening Antigravity you are signed in as this account. On macOS, Windows and Linux, the initialized native agy session is synchronized separately; start a new CLI command to use the selected account |
| **Refresh quota** | Re-reads this account's per-model quota and reset times |
| **Edit remark** | Stores a short label, up to 15 characters, to distinguish accounts |
| **Delete** | Removes the account from this application |

Rows can be sorted by quota reset time or by last use, reordered by dragging, and displayed as a table or as cards. Selecting several rows enables batch refresh and batch delete; the filter row narrows the list to All, PRO, ULTRA or FREE

### Why the application is closed during a switch

A single switch synchronizes the credential locations used by Antigravity and an initialized `agy` CLI. The application is closed during the switch because a running instance keeps the previous token in memory and writes it back when it refreshes, which would silently revert the switch. Start a new CLI command after switching; an already-running CLI command may retain its previous token

On Antigravity builds older than 2.0 there is no credential entry to write; the application detects this and injects the token into the local `state.vscdb` database instead, while still synchronizing an initialized native agy session. Separate IDE-targeted switches keep their own database-only behavior

Native agy sessions use `~/.gemini/antigravity-cli/antigravity-oauth-token` on all three platforms. Only an existing `antigravity-cli` directory is used; normal APP synchronization can create its first token file if needed. The APP and agy may share a system credential store, so a file-only update does not establish which account a new agy process will use. Use normal APP synchronization and verify the active identity in the client. Generic Google Gemini CLI files (`~/.gemini/oauth_creds.json` and `~/.gemini/google_accounts.json`) are neither created, changed nor deleted

Session updates use atomic replacement and readback verification, with `0600` permissions on Unix. Modern, keyring-backed Linux APP switches restore the previous keyring credentials if session synchronization fails. A legacy APP database update is not rolled back and is reported as a partial update on session failure. On macOS/Windows, a session failure after the keyring update is reported as a partial update; check both clients before retrying

## Usage dashboard

The dashboard reads Antigravity's local conversation databases (`conversation.db`, `token_usage_archive.db`) and its archive directory, then aggregates the records. No data leaves the machine and no request-level estimation is performed beyond what the records contain

![Dashboard in dark mode](docs/screenshots/dashboard-dark-en.png)

- **Date range** — today, yesterday, the last 3, 7 or 30 days; single-day ranges include an hourly chart
- **Chart details** — pointing at a bar shows that hour's input, output and cached tokens, request count and estimated cost
- **Summary cards** — total tokens, input tokens, output tokens, cache hit rate and estimated API cost
- **Model usage and model details** — which models consume the quota, with a per-model breakdown table

Cost is estimated from Google's public Gemini pricing pages, which are fetched once a day and cached, with a built-in fallback table. Models without a known price are reported as unpriced instead of being counted as free

## Settings

![Settings](docs/screenshots/settings-en.png)

- **Appearance and language** — follow the system, or choose light or dark; Simplified Chinese or English
- **Background tasks** — how often account quotas refresh, and how often the active account is re-read from local Antigravity data
- **Local data** — location of the application data (`~/.antigravity_tools/`), with a button to open the folder

## Data handling

| | |
| --- | --- |
| Read | Antigravity's local conversation databases and archives, which contain token counts, model names and timestamps |
| Written | `~/.antigravity_tools/` for accounts, configuration and cached pricing, the operating system credential store, and the initialized native agy session when an account is switched |
| Never | Conversation content and credentials are not uploaded; there is no proxy and no server component |

## Building from source

See [Linux support](docs/linux.md) for Linux build and compatibility details.

Node.js 22 or newer, a stable Rust toolchain and the platform build tools required by Tauri 2 are needed

```bash
npm ci
npm run tauri dev       # development
npm run build           # frontend only
npm run tauri build     # macOS .app or Windows installer
```

Bundles are written to `src-tauri/target/release/bundle/`. Pushing a `v*` tag runs the release workflow, which builds macOS, Windows and Linux deb packages and attaches the results to the GitHub release

## Low-quota account switching

Settings now offers two opt-in modes: **Switch after tasks finish** and **Stop first, then switch**. Both prepare a permitted backup from real quota data and update credentials only after all detected Antigravity/agy clients have exited. The tool never stops tasks or forces clients to close. Reopen the client, verify the account, and continue the original conversation manually. See [setup, limits, and verification](docs/low-quota-switching.md).

## Relation to the upstream project

This project is a focused fork of [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager). Upstream provides a full toolkit, including a reverse proxy, an HTTP API, a Cloudflared tunnel, IP management and a Docker image. This fork keeps the account manager and the local usage dashboard, removes the proxy and web-mode parts of the codebase, and adds its own dashboard, bilingual interface, theme support and release tooling. The two projects are independent; use upstream if a proxy is required

## License

Based on [lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) and distributed under the same [CC BY-NC-SA 4.0](./LICENSE) license. The license and the attribution requirements of the original project apply to any use, modification or redistribution

## Command-line and Homebrew

Tools Lite includes a local management CLI named `agy-lite`: list accounts, read the recorded current account and cached quota, and explicitly switch accounts using the same safe path as the GUI. It is separate from Google’s `agy`. See [CLI usage](docs/cli.md).

The older v4.7.6 release does not include this management CLI.

[Homebrew packaging](docs/homebrew.md) generates an Apple Silicon macOS cask with the release ZIP’s real SHA-256 and installs both the app and `agy-lite`. The verified cask will live under this repository's root `Casks/` directory, using Homebrew's explicit-URL tap form. That entry and its installation must be published and tested before the installation commands are advertised as available; see the [release checklist](docs/release-checklist.md).
