# Linux support

Linux builds use Tauri 2, GTK 3 and WebKitGTK 4.1. The first supported package is a native x86-64 `.deb`, built on Ubuntu 22.04. Other architectures and distributions need separate validation.

## Install

Download or copy the `.deb` onto the target machine, then run:

```bash
sudo apt install ./Antigravity-Tools-Lite-4.7.6-linux-amd64.deb
```

Open **Antigravity Tools Lite** from the application menu, or run `antigravity-tools-lite`. The Linux binary and icon names are distinct from the upstream full manager. A working desktop Secret Service is needed for Antigravity desktop account switching (GNOME Keyring or a compatible KWallet setup). CLI-only installations with an initialized agy session do not require Secret Service.

## Build

Docker can build without installing a Rust toolchain or development libraries on the host:

```bash
./scripts/build-linux-deb.sh --docker
```

Native Ubuntu/Debian builds need Node.js 22, stable Rust (at least 1.87), and development libraries:

```bash
sudo apt install build-essential git pkg-config cmake clang libclang-dev libssl-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
./scripts/build-linux-deb.sh --native
```

Packages are written to `src-tauri/target/release/bundle/deb/`. The Docker build uses Ubuntu 22.04 so a newer host's glibc does not become an accidental requirement.

## Accounts and locations

- The desktop credential payload keeps the `service=gemini`, `username=antigravity` attributes used by Antigravity. The login collection and default collection, when distinct, are both updated and verified. Existing credentials are restored if an update fails; incomplete recovery is reported.
- The initialized agy session at `~/.gemini/antigravity-cli/antigravity-oauth-token` is synchronized separately. CLI discovery requires an executable and an existing session directory. The application does not create a new CLI installation or change generic Gemini CLI OAuth files on Linux.
- When Antigravity desktop is not installed but an initialized agy installation is found, the normal account switch updates agy without trying to restart a missing desktop application.
- IDE configuration respects an absolute `XDG_CONFIG_HOME`, otherwise `~/.config`. Portable data and explicit `--user-data-dir` settings retain priority.
- Executables are discovered from running processes, configured paths, absolute `PATH` entries, user-local bins and common system directories. Symlinks are resolved. Custom executables can be selected under **Settings → Application locations**.
- Local account files and CLI session replacements use atomic writes with `0600` permissions on Unix. Existing account files are protected when next saved; this does not encrypt the app's local JSON account store.

## Desktop compatibility

Linux uses an opaque window. Existing graphics overrides remain available:

```bash
ANTIGRAVITY_DISABLE_TRAY=1 antigravity-tools-lite
ANTIGRAVITY_FORCE_WAYLAND=1 antigravity-tools-lite
WEBKIT_DISABLE_DMABUF_RENDERER=1 antigravity-tools-lite
```

When the tray is disabled, closing the window exits the app. A native Wayland compositor may restrict window positioning and always-on-top behavior.

## Validation

Run the backend tests with:

```bash
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib
```

The real Secret Service test is intentionally ignored by default. Install `dbus-x11` and `gnome-keyring`, then use the helper below. It creates a disposable HOME and a separate D-Bus session:

```bash
./scripts/test-linux-credentials.sh
```

The helper uses artificial tokens and two disposable keyrings. It also checks credential restoration after a simulated CLI write failure. Do not invoke the ignored tests directly against your normal desktop D-Bus session. Package installation, visible rendering and authenticated account switching are separate checks; the results for the delivered local package are recorded in [linux-validation.md](linux-validation.md).
