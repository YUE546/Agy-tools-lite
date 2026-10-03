# Local Linux validation — 2026-09-30

Source: local `linux-support` branch, based on `ffc2101`. No remote push or release publication was performed.

## Delivered package

- File: `artifacts/linux/Antigravity-Tools-Lite-4.7.6-linux-amd64.deb`
- Package/version/architecture: `antigravity-tools-lite` / `4.7.6` / `amd64`
- Size: 15,202,108 bytes
- SHA-256: `35856bdc7156408d52dbfe3a7867ce33096144c89455a0c2f98a8eff732ed260`
- Executable: `/usr/bin/antigravity-tools-lite`
- Runtime dependencies: GTK 3, WebKitGTK 4.1, Ayatana AppIndicator, xdg-utils. GNOME Keyring is recommended; desktop account switching requires a working Secret Service.

The package and logs are local generated artifacts ignored by Git. The source, build scripts and this validation record are committed together.

## Results

| Check | Result | Evidence |
| --- | --- | --- |
| Frontend TypeScript/Vite production build | PASS | `artifacts/linux/build.log` |
| Linux Rust check with locked dependencies | PASS | Completed before packaging; release build also compiled the final configuration |
| Rust formatting, shell syntax, YAML/JSON parsing and whitespace checks | PASS | Local checks |
| Backend tests | PASS: 52 passed, 2 explicitly ignored | `artifacts/linux/backend-tests.log` |
| Isolated credential tests | PASS: 2 passed | `artifacts/linux/credential-tests.log` |
| Native Linux release packaging | PASS | `artifacts/linux/build.log` |
| Ubuntu 22.04 apt installation | PASS | `artifacts/linux/install.log`, `package-info.txt`, `package-contents.txt` |
| Installed package UI under Ubuntu 22.04/X11 | PASS: dashboard, settings and native file dialog render | `linux-dashboard.png`, `linux-settings.png`, `linux-settings-paths.png`, `linux-file-picker.png` |
| Executable picker and window close | PASS: selected `/usr/bin/true` in disposable configuration; normal Alt+F4 exits with tray disabled | `linux-configured-path.png`, `ui-smoke-result.log` |
| Ubuntu 26.04.1 host library resolution and UI | PASS: extracted package executable renders dashboard | `host-ldd.log`, `ui-host.log`, `window-host.txt`, `linux-host-dashboard.png` |

The two isolated tests exercise real GNOME Keyring D-Bus read/write, distinct login/default collections and recovery after a simulated CLI commit failure; they also switch two artificial CLI tokens and verify `0600` permissions and an unchanged generic Gemini OAuth file. All tests and UI launches used temporary HOME directories and private D-Bus sessions. Existing user accounts and credentials were not accessed.

Build baseline: Ubuntu 22.04 amd64 container, Rust 1.98.1, Node 22.23.3, GTK 3.24.33 and WebKitGTK 2.50.4. The native build helper ran inside that provisioned container. The fresh `--docker` image-build path and GitHub Actions jobs have not been executed end to end.

UI checks used Xvfb; no WebKit sandbox-disable override was added. The host's Xvfb emitted software-rendering/DRI3 warnings while displaying the UI successfully. Host portal services required temporary-directory cleanup after they exited.

## Not run

- Google OAuth login, live account switching and Antigravity/IDE restart against real accounts.
- Native Wayland, KDE/KWallet, physical GPU rendering and tray interaction.
- ARM64 packaging and macOS/Windows builds in this session.
- Host-wide apt installation: installation was tested in the disposable Ubuntu 22.04 container; the current host ran the extracted same package.

These results prove the delivered amd64 package installs and renders in the tested environments. They do not imply authenticated account acceptance on every desktop distribution.
