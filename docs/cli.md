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

On Linux, the installed `antigravity-tools-lite` executable accepts these arguments too. CLI mode starts before Tauri/GTK initialization, so read-only commands do not need a display. This is the same executable as the desktop app and still depends on its installed platform libraries; it is not a standalone server binary.

### Windows shell invocation

The Windows release `antigravity-tools.exe` keeps the GUI subsystem so normal app startup does not open a console. CLI mode attaches to the parent's console, but a shell can return its prompt before a GUI executable exits. Use explicit waiting when completion and exit codes matter. For read-only commands in PowerShell:

```powershell
$exe = (Resolve-Path .\antigravity-tools.exe).Path
$process = Start-Process -FilePath $exe -ArgumentList 'accounts list' -NoNewWindow -Wait -PassThru
$process.ExitCode
```

Read the returned process's `ExitCode`, not `$LASTEXITCODE` from `Start-Process`. For JSON capture, pass `-RedirectStandardOutput` and `-RedirectStandardError` as well; they must name different files:

```powershell
$out = [IO.Path]::GetTempFileName()
$err = [IO.Path]::GetTempFileName()
try {
    $process = Start-Process -FilePath $exe -ArgumentList 'accounts list --json' -NoNewWindow -Wait -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    if ($process.ExitCode -eq 0) { Get-Content -Raw $out | ConvertFrom-Json }
    else { Get-Content -Raw $err; Write-Error "agy-lite exited with code $($process.ExitCode)" }
} finally { Remove-Item $out, $err }
```

These calls use PowerShell's documented [waiting, process-result and redirection options](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process). Programmatic callers should likewise wait for the child process and capture stdout and stderr separately.

For `switch`, omit `Start-Process`'s `-Wait`: that option waits for descendants too and may keep waiting until the relaunched Antigravity app closes. Instead keep `-PassThru`, call `$process.WaitForExit()`, then inspect `$process.ExitCode`. [.NET's `WaitForExit()`](https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process.waitforexit) waits for the CLI process itself. The usual account-switching precautions below still apply.

## Commands

```sh
agy-lite accounts list
agy-lite current --json
agy-lite quota                       # current account's cached quota
agy-lite quota user@example.com --json
agy-lite switch ACCOUNT_ID
agy-lite switch user@example.com --target app --json
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

There is no `--target cli` mode. The APP and Google’s `agy` may share a system credential store; writing only a session file cannot guarantee that a new `agy` process uses that account. Use the normal APP+agy synchronization path and verify the active identity in the client. This CLI does not install `agy` or create a missing native CLI data directory. If that directory already exists, normal synchronization can create its first native token file. A valid system token profile may also let `agy` sign in locally without a browser, as described in the [official authentication guide](https://antigravity.google/docs/cli/install/#local-silent-keyring-sign-in).

Switching is an explicit mutating command. It reuses the GUI's token validation/refresh, credential synchronization, process handling and account-index update. The default `--target app` follows the GUI behavior: it may close and restart Antigravity and synchronize an initialized native `agy` session. The CLI requires a discoverable APP for this target and rejects a file-only fallback; the GUI’s existing fallback behavior is unchanged. `--target ide` retains the independent IDE database path.

When the CLI relaunches an APP or IDE, child stdin/stdout/stderr are detached so the command returns promptly and JSON output stays machine-readable. GUI launches preserve their existing I/O behavior.

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
node scripts/test-cli-launch.mjs
node --test scripts/test-homebrew-generator.mjs
```

The launch regression test compiles the production process-launch functions with a synthetic configuration and harmless child executable; it covers manual/auto-detected launches, JSON output and timely pipe EOF without credentials.

The separate `Release CLI` workflow builds actual optimized Windows and Linux executables and runs the same synthetic-data smoke test. Windows also runs `scripts/test-windows-cli.ps1`, which checks the GUI PE subsystem, `Start-Process -Wait` and `WaitForExit()` completion/exit codes, and redirected JSON success/errors. Inherited-I/O checks establish completion and exit codes; console text visibility and a real account switch still need interactive acceptance. Debug smoke alone does not establish release shell behavior.

The smoke test uses a temporary data directory and synthetic tokens. It does not call `switch`, log in, contact Google or modify real accounts. Real credential-store switches and Homebrew installation require platform testing before a release is advertised as verified.
