# Tools Lite CLI (`agy-switch`)

`agy-switch` manages accounts saved by Antigravity Tools Lite. It is separate from Google's `agy`: it does not run an AI session. The same interactive workflow works on macOS, Windows and Linux, including masked refresh-token entry.

## Install and run

- **macOS:** the [Homebrew cask](homebrew.md) installs `agy-switch`. A manual app installation also exposes commands through `/Applications/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools`.
- **Windows:** the installer includes console `agy-switch.exe` beside the desktop executable; the release also offers a standalone console ZIP. Open PowerShell in that directory and run `.\agy-switch.exe`. The console executable preserves normal shell waiting, stdout/stderr and `$LASTEXITCODE`; use it instead of scripting the GUI-subsystem executable. [Windows guide](windows.md)
- **Linux:** the deb installs `/usr/bin/agy-switch`; a console tarball is also available. Cached reads and terminal interaction do not require a display, but the executable still needs GTK/WebKitGTK runtime libraries. [Linux guide](linux.md)

For a local build:

```sh
npm ci
npm run build
cargo build --locked --manifest-path src-tauri/Cargo.toml --bin agy-switch
./src-tauri/target/debug/agy-switch
```

Use ↑/↓ to select, Enter/→ to enter and Esc/← to return. Number shortcuts, j/k and q remain supported. Terminal input is restored on exit. Secret entry refuses to proceed if the terminal cannot disable echo. In a pipe, bare `agy-switch` prints help. Launching the original desktop executable without arguments preserves GUI startup.

## Commands

```sh
agy-switch                      # interactive dashboard on all three platforms
agy-switch stats                # local usage and cached-price estimates
agy-switch stats --json
agy-switch refresh              # refresh all saved accounts over the network
agy-switch refresh user@example.com
agy-switch accounts list
agy-switch current --json
agy-switch quota                       # current account's cached quota
agy-switch quota user@example.com --json
agy-switch switch ACCOUNT_ID
agy-switch switch user@example.com --target app --json
agy-switch switch ACCOUNT_ID --target ide
```

`accounts current`, `accounts quota` and `accounts switch` are also accepted. Selectors are exact account IDs or case-insensitive exact emails; duplicate emails require an ID. There is no fuzzy selection. Bare `agy-switch` opens the dashboard in an interactive terminal on all three platforms.

- `accounts list`, `current`, and `quota` only read local files. They do not initialize the GUI, refresh tokens, query Google, create directories/logs, or repair corrupt indexes
- `current` is Tools Lite's recorded selection, not a live check of the APP keyring or `agy` session. Changes made outside Tools Lite can make it stale
- `quota` reports cached data and `last_updated` (Unix seconds). Use `agy-switch refresh` when fresh quota is needed. A cache can be stale even when the command succeeds
- `--json` may appear before or after a command. Success goes to stdout; errors go to stderr. All JSON has `schema_version: 1`
- Output uses an explicit field allow-list: no access/refresh/ID tokens, raw OAuth responses, validation URLs, or stored error strings. Treat emails, account IDs, names and quota as personal data when sharing output
- `ABV_DATA_DIR` selects the account-data directory, matching the GUI. If unset, it is `~/.antigravity_tools`. Set it identically for GUI and CLI if you use a custom directory

## Switching and safety

There is no `--target cli` mode. The APP and Google’s `agy` may share a system credential store; writing only a session file cannot guarantee that a new `agy` process uses that account. Use the normal APP+agy synchronization path and verify the active identity in the client. This CLI does not install `agy` or create a missing native CLI data directory. If that directory already exists, normal synchronization can create its first native token file. A valid system token profile may also let `agy` sign in locally without a browser, as described in the [official authentication guide](https://antigravity.google/docs/cli/install/#local-silent-keyring-sign-in).

Switching is an explicit mutating command. It reuses the GUI's token validation/refresh, credential synchronization, process handling and account-index update. The default `--target app` follows the GUI behavior: it may close and restart Antigravity and synchronize an initialized native `agy` session. The CLI requires a discoverable APP for this target and rejects a file-only fallback; the GUI’s existing fallback behavior is unchanged. `--target ide` retains the independent IDE database path.

When the CLI relaunches an APP or IDE, child stdin/stdout/stderr are detached so the command returns promptly and JSON output stays machine-readable. GUI launches preserve their existing I/O behavior.

Save work in Antigravity before switching. Start a new `agy` invocation afterward; a running invocation may retain the previous token. A failure can follow a partial external credential change, so inspect both clients before retrying. CLI errors intentionally omit raw server details; use the GUI for detailed troubleshooting.

GUI and CLI switches in this version share an OS-level lock (`account-switch.lock` inside the data directory). A competing switch fails with exit code 5; the OS releases the lock when the owning process exits. Do not remove the lock file while a switch is running. GUI quota refresh/add/delete operations are not coordinated by this switch-only lock; avoid editing or deleting accounts while a CLI switch is in progress. A CLI switch does not directly refresh an already-open GUI's tray; reopen the account page to read the saved state.

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Data, runtime or switch error (switch may be partially applied) |
| 2 | Invalid arguments or ambiguous email |
| 3 | No matching/current account |
| 4 | No cached quota |
| 5 | Another switch is in progress |

## Verification

```sh
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib cli::
cargo build --locked --manifest-path src-tauri/Cargo.toml
node scripts/test-cli.mjs ./src-tauri/target/debug/agy-switch
node scripts/test-cli-terminal.mjs ./src-tauri/target/debug/agy-switch
node scripts/test-cli-launch.mjs
node --test scripts/test-homebrew-generator.mjs
```

The launch regression test compiles the production process-launch functions with a synthetic configuration and harmless child executable; it covers manual/auto-detected launches, JSON output and timely pipe EOF without credentials.

The separate `Release CLI` workflow builds actual optimized Windows and Linux executables and runs the same synthetic-data smoke test. Windows also runs `scripts/test-windows-cli.ps1`, which checks the GUI PE subsystem, `Start-Process -Wait` and `WaitForExit()` completion/exit codes, and redirected JSON success/errors. Inherited-I/O checks establish completion and exit codes; console text visibility and a real account switch still need interactive acceptance. Debug smoke alone does not establish release shell behavior.

The smoke test uses a temporary data directory and synthetic tokens. It does not call `switch`, log in, contact Google or modify real accounts. Real credential-store switches and Homebrew installation require platform testing before a release is advertised as verified.

## Interactive workflow and coverage

The main menu groups Accounts & Quotas, Statistics, Refresh, Add Account, and Status. Use Up/Down and Enter to navigate, or a displayed number to open a section; Escape returns. Account management supports switching, quota details, labels, enable/disable, and deletion with confirmation. Refresh and authorization use the network; switching can change credentials and restart clients. Read-only JSON commands are suitable for scripts.

Cost estimates use the same exact model matching as the native menu and the cached public price table. Unknown prices display `Unpriced`; partial estimates are labelled. Missing quota windows are unknown, never inferred as 100%. API-equivalent costs are estimates, not the subscription bill.

A future CLI expansion should expose account management, configuration, candidate ordering and update checks as stable commands, sharing the existing backend. Visual theme settings, menu layout and interactive charts belong in the GUI. The terminal menu and one-line commands should share the same operations rather than duplicate their implementations.

## CLI scope

Account management, cached quotas, refresh, switching and local usage are terminal workflows. Appearance, desktop startup, update notices and the background smart-switch scheduler remain desktop features. Linux terminal-only users can inspect/add/refresh accounts without a display; APP switching still requires an installed APP and its credential backend. No file-only agy switch or background CLI daemon is provided.
