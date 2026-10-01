# Quick dashboard and startup preferences

The compact panel is 380 × 480 logical pixels, with a flat system-menu layout. It never scrolls vertically: quotas and the account selector use up to four rows per page. The account selector replaces the quota view within the same fixed window.

Click the menu-bar icon on macOS (or the tray icon on Windows) to toggle a compact dashboard. Linux desktops use the native tray menu's **Quick Dashboard** action because Tauri does not deliver tray-click events there. If a Linux tray is unavailable, the ordinary application window remains usable.

On macOS the panel uses the system Popover vibrancy material over a transparent WebView. It reads Reduce Transparency and Increase Contrast when opened and falls back to an opaque surface when requested or when the effect fails. The CSS media preference provides an additional immediate reduce-transparency fallback. Windows and Linux use the same compact layout with opaque, higher-contrast surfaces; no unsupported desktop blur is simulated there. The macOS transparent WebView uses Tauri's documented `macos-private-api` feature, so this build is not intended for Mac App Store distribution.

A browser preview uses synthetic data and CSS to approximate the material. It cannot verify native macOS desktop blur or accessibility changes.

The default Overview separates the recorded current identity, saved-account readiness, and per-pool usable-account counts. Counts use all saved accounts as the denominator. Every reported/required window must be known, fresh and above the configured reserve threshold. Stale/expired, missing-window, validation-blocked, forbidden and locally protected data is never counted as usable. Model fallback and group pools remain separate; percentages are never added or averaged.

The recorded current account comes from the local Tools index; it is not a live check of the account in an open Antigravity window.

Choosing an account only changes the inspected view. A separate **Use this account** button invokes the existing switching command. Disabled accounts remain inspectable. Machine-local token totals appear only in Overview, never as per-account usage.

The panel includes:
- The current account recorded by Tools, subscription tier and quota update age
- Each **reported** quota pool and window, with remaining percentages and reset countdowns
- Pinned model quotas when the server does not report grouped windows
- Today's locally recorded tokens and requests, clearly marked as not attributed to individual accounts
- Saved-account switching through the same backend as the full application
- Refresh, account management, full dashboard, settings and quit actions

Unknown quota is shown as a dash, not zero. Cached quota is marked stale. Independent quota pools are not averaged. Blocked/disabled accounts cannot be activated in the panel, and concurrent clicks are suppressed. Switching can restart Antigravity App, as the interface explains.

## Low-quota coordinator status

The menu-bar route has its own compact view of the existing low-quota coordinator because it intentionally does not mount the full window's Layout. Its read-only status indicator uses the existing status-line space. Selecting it replaces the main content with a bounded detail view, with the same verified reason, source/target, wait/stop instructions, cancellation ID and check action as Settings. Stop guidance opens the full Settings view; it never sends a stop or kill command. During a committed coordinator switch or an unreadable coordinator state, manual activation is disabled in the panel. The backend remains the single owner of switching decisions. Overview uses the enabled coordinator's reserve as its read-only display threshold; while that feature is disabled, it retains the existing quota-protection display threshold. This statistic does not broaden the coordinator's configured model or candidate-account scope.

The frontend does not start a second coordinator or duplicate the background quota scheduler. A `tray://account-switched` event updates the inspected account data even when the main Layout is not mounted.

## Settings

**Startup & menu bar** contains three opt-in controls:
- **Launch at login** registers this installed application with the OS. The setting reads the actual OS registration, rather than trusting a saved JSON flag. It is unavailable in development builds, to avoid registering a transient development executable
- **Start in the background at login** only suppresses the main window for `--autostart` launches while a tray is available. Launching the application yourself still opens the main window
- **Hide Dock icon** is macOS-only and preserves the preference when opening or closing windows

None of these options is enabled by migrating an older configuration. The application never enables login startup when loading config. If the tray fails to initialize, the main window is shown and macOS uses a regular Dock presence. You can reopen the application through Applications/Spotlight even when its Dock icon is hidden. An explicit Quit exits the process; closing the main window keeps it in the tray when the tray is available.

The panel closes on Escape, a second tray click, or loss of focus. Its position is clamped to the selected display's work area, including negative monitor origins and display scaling. Its geometry is excluded from main-window state restoration.

## Verification

- `npm run build`
- `node scripts/test-menubar-logic.mjs`
- `cargo check --locked --manifest-path src-tauri/Cargo.toml`
- `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib`

Automated tests cover quota truthfulness, pinned-model fallback, account eligibility, stale data, startup safety and display-bound calculations. Playwright synthetic-IPC acceptance covers the 380×480 layout, pagination, explicit activation, repeated/interrupted navigation, low-quota status and cancellation, and truthful completion/errors. Captures from that runner are Linux browser UI evidence, not native macOS/Windows screenshots. No test enables startup or modifies a real user's account.

Before release, validate on an installed macOS build: menu-bar positioning on multiple Retina displays; outside-click/second-click dismissal; switching while the main window is hidden; Dock preference preservation; relaunch from Spotlight; actual login startup; System Settings changes to login/menu-bar visibility. Linux compilation is not a substitute for this native macOS validation.

## Design references

The information hierarchy was informed by [OpenUsage](https://github.com/robinebers/openusage), [CodexBar](https://github.com/steipete/CodexBar), and [ClaudeBar](https://github.com/tddworks/ClaudeBar), using their published screenshots and documentation. The implementation, layout and copy here are original. No project logos, screenshots, or source code were copied into the application.

Platform behavior follows the official Tauri [system-tray](https://v2.tauri.app/learn/system-tray/) and [autostart](https://v2.tauri.app/plugin/autostart/) APIs.
