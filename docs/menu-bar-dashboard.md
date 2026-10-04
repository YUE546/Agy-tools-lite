# Quick dashboard and startup preferences

On macOS the overview is an AppKit `NSMenu`, following CodexBar's native menu structure, system typography, fine progress bars and standard action rows. AppKit owns sizing, screen clamping, keyboard tracking and dismissal. Overview shows accounts directly, with separate Gemini and Claude/GPT percentages for both 5-hour and weekly windows and an explicit switch button on each row. Each account has a native submenu containing all reported pools/model observations. Window → Quota Overview (Command–Shift–M) opens the same menu.

Windows/Linux retain the WebView panel at 424 × 680 logical pixels, clamped to the work area, with adaptive pagination for accounts and quota details. The browser acceptance suite covers this shared panel; it is not evidence of the Mac native menu's appearance.

Click the macOS menu-bar icon or Windows tray icon to toggle it. Linux uses the native tray menu's Quick Dashboard action. Escape, a second tray click and loss of focus dismiss the panel. The main application remains available if the tray cannot initialize.

## Material

The Mac menu uses the system's own menu background, rounding, shadows, semantic foreground colors and accessibility appearance. It has no transparent WebView window or custom glass container. Earlier direct `NSGlassEffectView` root replacement triggered a foreign exception; the background-only attempt was unreadable over bright content. Both were removed. The native menu does not replace any managed window view. Windows/Linux use opaque panel surfaces. Native appearance requires installed-host acceptance, not browser screenshots.

## Quota and identity

General Preferences → Menu bar aggregate quotas selects Gemini, Claude/GPT or all families. The overview always shows both **5 hours** and **weekly**. Each is an equal-weight **mean remaining percentage**, not summed tokens or absolute capacity. Shared bucket/window observations are deduplicated. All-family means require both families to be reported for an account. Missing, conflicting, stale, expired, blocked, disabled, forbidden or protected data is excluded. Known low and zero observations stay in the mean; unknown is a dash. The available count uses all indexed accounts as denominator and the configured reserve as its cutoff; hovering the count exposes reported-data coverage. Availability describes quota headroom; native sign-in is verified during switching.

The menu's read-only, credential-free snapshot checks the running standalone Mac App identity. A verified live email supersedes the saved Tools index. Failed, unknown and ambiguous observations do not select the first account. With the App closed, the saved Tools account is identified as such. The full application and `agy-switch current` retain their documented Tools-record semantics.

Per-row switching uses the existing manual switch backend and can close/reopen the App. Disabled/blocked/unreadable accounts, repeated clicks, active coordinator commits and unreadable coordinator state disable activation. Refresh updates **all** quotas and reports partial failures. Mac operations dismiss the native menu and expose completion/failure on its next open. The Usage Dashboard action opens the existing full application. Windows/Linux show today's local machine usage; it is not attributed to an account. Failed reads show unavailable data instead of a fabricated zero.

Both interfaces read the existing low-quota coordinator and never create another scheduler. Mac exposes pending cancellation using the original pending ID, and the full Settings UI retains instructions/check actions. The shared panel's detail view preserves pending IDs, source/target, instructions, cancellation and check actions. Backend switching decisions remain authoritative.

## Settings

General Preferences includes OS launch-at-login and macOS Dock visibility controls. Hiding the Dock icon also enables background startup; an explicitly launched App still opens its main window. Login registration is queried from the OS and is only changed by an explicit setting, not migration/startup. Development builds cannot register login items.

Desktop and menu-bar preferences use dedicated atomic setters under the ordinary configuration lock. Older ordinary Settings saves preserve the latest dedicated preferences. Failed OS writes are compensated and rollback failures remain visible. Reopening a window does not replay an old Dock preference. If the tray fails, background start is suppressed and a regular Dock presence is retained.

## Verification

Frontend build, 27 existing quota/presentation checks, 11 scoped-aggregation checks, 10 settings contract checks, 6 desktop UI race checks, and 5 synthetic-IPC menu browser cases cover the shared layout and data guards. Rust tests additionally cover the actual Mac menu projection: independent windows/families, duplicate/conflicting pools, unknown and expired observations, stale/disabled accounts, known zero, scoped means and separate availability counts. Synthetic UI tests include 424×640, 380×480 and 320×400, all accounts/models, failed reads/refreshes, concurrent activation, preferences, language and material events.

Installed Mac acceptance is recorded separately in the project audit. Login boot, additional displays and Windows/Linux native GUI still require their own host-specific acceptance; browser or CLI success does not establish those results.

## References

The layout references [CodexBar](https://github.com/steipete/CodexBar). AppKit renders the native material rather than a CSS blur. No third-party UI source or assets are copied.
