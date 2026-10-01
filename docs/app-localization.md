# Antigravity App localization (experimental)

The Settings subsection is an opt-in, default-off feature for **official Antigravity App 2.19.1 on macOS**. The verified layout covers **nine static labels**, not full-App translation: the Settings entry button, Settings heading, General, Application, Appearance, Models, Customizations, Shortcuts and Provide Feedback. It uses the App's existing local debugging endpoint; it does not launch the App or enable a new port. Linux, Windows and other App versions remain unsupported.

**Remaining release acceptance:** The standalone Settings-button and nine-label Mac round trips passed. The final Tools package's integrated setting, reload/reconnection lifecycle and disable/restoration checks below have not yet completed native Mac acceptance.

No new navigation tab is added. Tools' existing display language is independent. No installation files, account data, credentials, system startup entries, security settings or debugging flags are changed. No upstream installer is executed.

## Dictionary provenance

- Source: [yiheng8023/antigravity-chinese](https://github.com/yiheng8023/antigravity-chinese/tree/573fa3c40aa6b410070a0b023730f0d1b4727bb9)
- Pinned commit: `573fa3c40aa6b410070a0b023730f0d1b4727bb9` (package 3.3.15)
- Source file: `dist/zh-CN.bundle.json`, Git blob `7662d488450eb89476d392407c9dfb2165dbd086`
- License: MIT, retained in `src-tauri/resources/app-localization/LICENSE-MIT.txt`
- Only manually selected short exact UI terms are included. `SOURCE.json` records the count. No regular-expression rules or upstream runtime/installer code is used
- Selection was checked for plain string values, HTML, URLs, executable content and category scope. The reviewed phrases cover labels such as Settings, Help, appearance and navigation; matching a phrase alone never permits a DOM write
- This candidate is **not confirmed to be the pack in the forum screenshot**. The forum comment supplied no code or pack link. The screenshot's 5,185-entry claim is not used

## Source evidence and verification boundary

The [official download page](https://antigravity.google/download) supplied this Linux build:

`https://storage.googleapis.com/antigravity-public/antigravity-hub/2.19.1-6046815158665216/linux-x64/Antigravity.tar.gz`

SHA-256:

- Official archive: `7068fa471c3e5a1225e24e4f683497748b849d08ec702313d49c518cd45fa4af`
- `resources/app.asar`: `341234faf45bd1776fd5418a3c288dedc5487ebfcf153f53d75de17cfe15c1de`
- UI `main.js` read from the language server's embedded ZIP: `47f36abaabd34f7d54a95df40d942b609c02db9f5c04d56b124a048571b9f16d`

Static inspection traces the Settings button through the sidebar button and shared button renderer: a `BUTTON[data-testid="settings-button"]` has a direct `SPAN` with exact class `truncate text-sm`, containing the literal Settings label. The icon has a different sibling span. The source-derived fixture models only this tiny shape, with unrelated user content alongside it. It is an original fixture, not redistributed official source or a captured live DOM. This evidence does not establish all runtime branches, platforms or versions.

Notably, `settings-nav-item-Account` contains the user's name/email. Its test id does **not** make it safe to translate. It is excluded from the proposed scope.

Official 2.19.1 main-process code itself adds `remote-debugging-port=0` when no port is provided. Therefore disabling this feature cannot truthfully promise to close the official app's debugging capability. The feature must never add wildcard origins, disable the sandbox or expose a non-loopback endpoint.

### Actual runtime blocker

With authorization, the official App was attempted using a new temporary HOME and no account/login information. This cloud execution environment rejects Unix sockets (`Operation not permitted`). Xvfb could not establish its local listener; the Ozone-headless attempt stopped at Chromium's process singleton socket before showing an App page. The normal Chromium sandbox was retained; no login or injection happened. Retrying Xvfb with the supported permission-review route did not resolve the environment restriction. No further execution workaround was used.

The Linux rendering path remains unverified. A separately authorized macOS test subsequently succeeded; this does not turn the cloud attempt into a pass or establish Linux/Windows support.

### Actual macOS acceptance evidence (2026-10-01)

The official Antigravity App 2.19.1 was checked using its existing active-port file. The test independently verified the App executable and PID, complete loopback listener ownership, CDP browser PID, and the language-server executable/parent relationship before selecting a page. No credentials, account text, chat content or input values were captured.

- Read-only probe: `ok=true`, `shape_verified=true`, `changed=false`
- One Settings TextNode flip for two seconds: `ok=true`, `changed=true`, `restored=true`, `english_postcheck=true`
- The test detached, the App exited, and temporary test files were cleaned; the account was unchanged
- This first check validates **only the Settings entry button**. It does not establish automatic reconnect or Tools UI end-to-end behavior

A second authorized official-App check used the fixed-hash controls package and verified **nine actual labels**, of which eight were in Settings navigation: Settings, General, Application, Appearance, Models, Customizations, Shortcuts and Provide Feedback. The other label was the independent Settings entry button. The two-second temporary translation returned `restored=true` and `english_postcheck=true`; the App exited normally and temporary files were cleaned. Five other source-known labels were not displayed and were **not** accepted by this result.

Production is now restricted to exactly that observed eight-plus-one set. Skin, Notifications, Developer, Tab and Editor are structurally checked when present but never translated. `SOURCE.json` records the historical tested-runtime/dictionary hashes and distinguishes that live test from the subsequently restricted runtime's offline regression tests.

The reusable bounded acceptance scripts are `scripts/test-installed-app-label.mjs` and `scripts/test-installed-app-controls.mjs`. The current controls runner pins the restricted production runtime and dictionary hashes and executes those bytes unchanged. The original candidate test package remains the evidence for the recorded Mac result; the current script's existence is not a claim that its newer hash or the Tools integration has already been live-tested. Neither script launches the App or creates a port.

## Runtime contract

`runtime.js` evaluates to a factory `(window, JSON config) => controller` with `probe`, `apply`, `dispose`, `renewLease` and `getStatus`. The configuration supplies only an exact App version, `zh-CN`, and dictionary data. It cannot provide adapters or selectors.

- The production registry accepts exactly 2.19.1; the platform collector additionally permits only macOS
- Only the eight recorded Settings-navigation labels are enabled; five other known labels stay untranslated even if present
- Offline tests cover production and test-only synthetic/source-derived adapters separately
- Writes are limited to literal, unique static control paths and exact source labels
- Chat, Markdown, editors, code, inputs, paths and editable ancestors are excluded at every ancestor depth
- Original node identity and original/translated values are tracked. Restore only reverts still-owned values and preserves subsequent App/user changes
- Reapply is idempotent, reinjection disposes the old controller, and DOM drift shuts it down
- A 15-second runtime lease schedules restoration if its owner disappears. The worker attempts renewal every 3 seconds; mutations and repeated apply do not renew it. Browser timer throttling means cleanup after a lost connection is best-effort, not a precise wall-clock guarantee
- It does not click, submit, change permissions, read account storage, call the network or load remote scripts

## Tests

```sh
npm run test:app-localization
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib app_localization
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib client_localization
```

The 49 runtime tests cover exact writes, all exclusion categories, original-value rollback, legitimate concurrent changes, reapply/reinjection, DOM drift, unknown versions, lease expiry and the source-derived Settings shape. Rust tests cover default-off migration, separation from Tools' language, safe package identity/version parsing, bounded malformed-ASAR handling and the server-side unsupported-version gate.

## Existing-endpoint discovery and constrained transport

`modules/localization_macos.rs` reads only bounded installation identity metadata, the known `Antigravity/DevToolsActivePort` file, and scoped process/listener metadata. It verifies the official App executable and its language-server child independently. A complete listener observation must show loopback-only binding with the expected owner. The collector does not scan arbitrary ports, read account storage, change startup flags or request extra system permissions.

`modules/app_localization.rs` owns one in-memory session while Tools is running. Enabling saves the opt-in preference; if the App is closed, the worker waits for the user to open it normally. Every connection/reconnection repeats identity validation. It never selects the first available page as trusted. IPC intents receive an entry-time generation and serialize the complete persistence/runtime transition; a delayed older enable cannot revive localization after a newer disable. The dedicated preference change and ordinary Settings saves share a configuration lock, so stale theme/language forms cannot silently re-enable localization. Failed off persistence still cancels this session and leaves a retry action visible. Disabling attempts owned-label restoration; an unconfirmed response keeps the cleanup handle and reports `restore_pending`, rather than claiming success. A destroyed verified process/page can no longer retain those in-memory changes. While cleanup is unresolved, no new translation is applied.

`modules/localization_transport.rs`:

- accepts only an observed 127.0.0.1 listener owned by the expected browser PID; rejects any non-loopback/mismatched binding in the observation
- validates the two-line official active-port file, and creates a direct WebSocket connection with no proxy, discovery scan, redirect, wildcard Origin or TLS override
- checks `SystemInfo.getProcessInfo` before page enumeration and each operation
- considers only page targets at the exact independently verified local App origin; never treats the first target as trusted
- rechecks target identity and origin before evaluation, and guards `location.origin` inside the synchronous script to cover navigation races
- exposes only fixed probe/apply/renew/dispose scripts with bundled runtime/dictionary data. It accepts no arbitrary script or selector from the UI or endpoint
- caps messages at 256 KiB, event processing at 64 messages, targets at eight, and each request at two seconds
- discards remote descriptions/page data and returns only a small allowlisted status report
- detaches sessions after an operation; disconnect stops future lease renewal, allowing the runtime to restore its own changes
- probes through the existing controller or a private host facade, preserving an active controller

Pure validation tests cover URL aliases, redirection attempts, non-loopback and wrong-PID evidence, response IDs/errors, target scope and JavaScript interpolation. Three synthetic localhost WebSocket tests cover the complete constrained protocol, rejection before page access for a wrong PID, and a bounded timeout. These are protocol tests, not a substitute for Mac Tools end-to-end acceptance.

## Coverage limits and future candidates

Nine labels are enabled on the verified layout. The following source-known Settings entries remain untranslated pending their own live evidence: Skin, Notifications, Developer, Tab and Editor. The 71 dictionary entries are reviewed source data, **not a count of translated controls**. The Settings UI shows the actual applied label count.

Do not blindly allow all `settings-nav-item-*` nodes: workspace and project names reuse the component and identifier prefix. Only the source-verified first global group and fixed tail are eligible; Account and user groups are excluded. Unknown global entries or changed known structures fail closed.

The official source also has fixed New Conversation and Conversation History entry labels with their own test IDs. These are potential separately scoped future tests, not part of this release. Labels inside Settings forms need individual structure/ownership review and live checks before expansion. Broad DOM text scans are not an acceptable way to increase coverage.

## Remaining release acceptance

1. Keep the one-label and nine-label Mac evidence above; do not expand it into a full-interface claim
2. Validate the exact restricted production runtime in the final integrated test copy; the five skipped candidates remain outside its scope
3. Build the updated Tools test copy in cloud CI. Verify its Settings switch against the official App: enable, status/count, repeated apply, normal App reload/reopen, disable/restore, reconnect and unknown-version rejection
4. Verify failure status and restoration after a lost connection. An optional localization failure must never interfere with account switching
5. Run final frontend, Rust and platform CI checks against the exact integrated release commit, then review the enabled scope and status copy

No launchd, startup daemon, ASAR mutation, third-party installer, broad text replacement or full-App coverage is part of this feature.
