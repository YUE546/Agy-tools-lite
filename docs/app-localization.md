# Antigravity App localization (WIP, disabled)

This draft implements a Settings subsection, migration-safe opt-in configuration, bounded read-only App version detection, and an independently tested reversible runtime. **It does not yet provide working App localization.** There is no production adapter or CDP transport in this build. Both UI controls and backend commands fail closed; editing the saved flag cannot enable injection. Do not merge or advertise this feature until the real-App validation gate below passes.

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

Live App rendering, CDP target identity, actual DOM compatibility, navigation/restart behavior, and macOS/Windows remain **unverified**. Static and synthetic tests do not replace those checks.

## Runtime contract (not activated)

`runtime.js` evaluates to a factory `(window, JSON config) => controller` with `probe`, `apply`, `dispose`, `renewLease` and `getStatus`. The configuration supplies only an exact App version, `zh-CN`, and dictionary data. It cannot provide adapters or selectors.

- Production adapter registry is empty; no version can apply
- Offline tests use transformed copies of the runtime with synthetic and source-derived adapters, never patching the distributed registry
- Writes are limited to literal, unique static control paths and exact source labels
- Chat, Markdown, editors, code, inputs, paths and editable ancestors are excluded at every ancestor depth
- Original node identity and original/translated values are tracked. Restore only reverts still-owned values and preserves subsequent App/user changes
- Reapply is idempotent, reinjection disposes the old controller, and DOM drift shuts it down
- A 15-second lease restores changes if its owner disappears. A future backend must explicitly renew every 3 seconds; mutations and repeated apply do not renew it
- It does not click, submit, change permissions, read account storage, call the network or load remote scripts

## Tests

```sh
npm run test:app-localization
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib app_localization
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib client_localization
```

The 27 runtime tests cover exact writes, all exclusion categories, original-value rollback, legitimate concurrent changes, reapply/reinjection, DOM drift, unknown versions, lease expiry and the source-derived Settings shape. Rust tests cover default-off migration, separation from Tools' language, safe package identity/version parsing, bounded malformed-ASAR handling and the server-side unsupported-version gate.

## Required before enabling/merging

1. Obtain actual App DOM evidence in an authorized environment that supports normal Chromium sandbox and Unix sockets; retain the existing temporary-profile/no-account boundary until separate access is authorized
2. Verify the minimal Settings-label adapter in real rendering, including duplicate/user-content lookalikes, navigation, label updates, shutdown and recovery
3. Implement a reviewed, local-only transport that verifies the App process, installation, exact release, target origin and loopback listener; never blindly attach to all targets
4. Verify lease renewal, cleanup on disconnect and disabling; an account switch must not fail because optional localization failed
5. Run platform-specific integration tests. Add only validated versions/platforms to the production registry; unknown builds remain blocked
6. Review the enabled feature and update the Settings notice only after real validation passes

No launchd, startup daemon, ASAR mutation, third-party installer, broad DOM scanning or all-App coverage is part of this draft.
