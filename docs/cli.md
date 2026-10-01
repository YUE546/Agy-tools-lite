# Tools Lite CLI (`agy-lite`)

`agy-lite` manages the accounts saved by **Antigravity Tools Lite**. It is separate from Google's `agy` executable: it does not replace `agy`, start an AI session, or implement Google login. Add accounts in the Tools Lite GUI first.

## Run it

With a CLI-enabled macOS app installed, the bundled executable accepts the same commands:

```sh
"/Applications/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools" --help
"/Applications/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools" accounts list --json
```

The [Homebrew cask](homebrew.md) installs an `agy-lite` symlink to this executable. Until a cask-enabled release is published, the cask is a packaging recipe, not an available public tap.

For a local Rust build:

```sh
npm ci
npm run build
cargo build --locked --manifest-path src-tauri/Cargo.toml
./src-tauri/target/debug/antigravity-tools accounts list
```

On Linux, the installed `antigravity-tools` executable accepts these arguments too. CLI mode starts before Tauri/GTK initialization, so read-only commands do not need a display. This is the same executable as the desktop app and still depends on its installed platform libraries; it is not a standalone server binary. On Windows, use `antigravity-tools.exe` with the same arguments.

## Commands

```sh
agy-lite accounts list
agy-lite current --json
agy-lite quota                       # current account's cached quota
agy-lite quota user@example.com --json
agy-lite switch ACCOUNT_ID
agy-lite switch user@example.com --target cli --json
agy-lite switch ACCOUNT_ID --target ide
```

`accounts current`, `accounts quota` and `accounts switch` are also accepted. Selectors are exact account IDs or case-insensitive exact emails; duplicate emails require an ID. There is no fuzzy selection. Bare `agy-lite` prints help, while launching the original app executable without arguments preserves the GUI.

- `accounts list`, `current`, and `quota` only read local files. They do not initialize the GUI, refresh tokens, query Google, create directories/logs, or repair corrupt indexes
- `current` is Tools Lite's recorded selection, not a live check of the APP keyring or `agy` session. Changes made outside Tools Lite can make it stale
- `quota` reports cached data and `last_updated` (Unix seconds). Refresh in the GUI first when fresh quota is needed. A cache can be stale even when the command succeeds
- `--json` may appear before or after a command. Success goes to stdout; errors go to stderr. All JSON has `schema_version: 1`
- Output uses an explicit field allow-list: no access/refresh/ID tokens, raw OAuth responses, validation URLs, or stored error strings. Treat emails, account IDs, names and quota as personal data when sharing output
- `ABV_DATA_DIR` selects the account-data directory, matching the GUI. If unset, it is `~/.antigravity_tools`. Set it identically for GUI and CLI if you use a custom directory

## Switching and safety

Switching is an explicit mutating command. It reuses the GUI's token validation/refresh, credential synchronization, process handling and account-index update. The default `--target app` follows the GUI behavior: it may close and restart Antigravity and synchronize an initialized native `agy` session. On Linux without an installed APP, the same GUI fallback can select an initialized `agy` installation. `--target cli` updates only an already-initialized native `agy` session; `--target ide` retains the independent IDE database path.

Save work in Antigravity before switching. Start a new `agy` invocation afterward; a running invocation may retain the previous token. A failure can follow a partial external credential change, so inspect both clients before retrying. CLI errors intentionally omit raw server details; use the GUI for detailed troubleshooting.

GUI and CLI switches in this version share an OS-level lock (`account-switch.lock` inside the data directory). A competing switch fails with exit code 5; the OS releases the lock when the owning process exits. Do not remove the lock file while a switch is running. Older Tools Lite versions do not honor the lock, so quit them before using the CLI. GUI quota refresh/add/delete operations are not coordinated by this switch-only lock; avoid editing or deleting accounts while a CLI switch is in progress. A CLI switch does not directly refresh an already-open GUI's tray; reopen the account page to read the saved state.

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
node scripts/test-cli.mjs ./src-tauri/target/debug/antigravity-tools
node --test scripts/test-homebrew-generator.mjs
```

The smoke test uses a temporary data directory and synthetic tokens. It does not call `switch`, log in, contact Google or modify real accounts. Real credential-store switches and Homebrew installation require platform testing before a release is advertised as verified.
