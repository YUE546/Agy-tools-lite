# Low-quota account switching

This opt-in feature prepares a permitted backup account when the selected model's remaining quota reaches the reserve threshold. It changes credentials **only after all detected Antigravity APP, IDE, and agy processes have exited**. It never sends Stop, kills a process, restarts a client, replays a tool, or submits a continuation message.

## Setup and the two modes

Open **Settings → Low-quota account switching**. Choose an actual model from the account quota data, the reserve threshold (default 10%), the minimum eligible backup quota (default 30%), and up to ten allowed accounts. Turn the feature on and save. It is off by default, including on upgrades. The original quota-protection setting is separate.

- **Switch after tasks finish:** Let the tasks and background work finish, then quit Antigravity and agy normally. Keep Tools Lite running; it completes the pending switch after observing that the clients are closed.
- **Stop first, then switch:** Use the client's native Stop/cancellation control, verify that background work has ended, then quit the clients normally. The instructions button only provides guidance; it does not stop anything. Tools Lite then uses the same safe credential-update path.

After **Account ready; takes effect when you reopen the client**, open the client, verify the signed-in account, open the original conversation from history, and continue manually. There is no claim that a running command or model generation has migrated to another account. A ten-percent reserve is a trigger, not a guarantee that a long task can finish within that balance.

**Cancel this switch** suppresses another attempt for that source account until its quota recovers to the configured backup minimum. Saving the settings clears the cancellation. Cancellations survive application restart. A failed or uncertain credential commit pauses automatic attempts until the settings are saved again; inspect both clients first.

## Eligibility and safety

- Only explicitly selected backup accounts are considered. Selection follows the configured account order and skips the current account.
- The monitored model must be present in the account's available models. Known provider bucket IDs are used, not translated display labels. Weekly and five-hour limits are both checked; weekly-only FREE accounts are supported. Unsupported/missing bucket information blocks switching instead of guessing.
- Quota data older than three minutes, invalid/reset-expired data, refresh failures, disabled accounts, and validation-blocked accounts are not eligible. A reset deadline alone is not treated as quota recovery.
- The Rust coordinator runs independently of the visible settings page. Routine per-account refreshes are limited to once a minute; explicit checks retain a ten-second floor. Backup quotas are refreshed only when the source reaches the threshold.
- Config, cancellation, source identity, target eligibility, quota freshness and process observations are revalidated. The credential commit uses the same in-process and cross-process switch locks as the Tools Lite CLI.
- An external client can still launch between process observations and the credential write. Process inspection is not a global execution lock. Do not open clients while the status says it is switching. Unknown process/configuration state blocks the operation.
- The default APP credential store and an initialized native agy session are updated together. No generic Gemini CLI files are changed. A missing native session file inside an existing agy directory can be created; an existing file associated with a different account blocks the switch.
- An installed modern APP and a matching current system-keyring identity are required. There is no file-only CLI switch option. On Linux, version discovery reads the installed `resources/app/package.json`; it never launches the APP with `--version` as a fallback. An installation without readable version metadata is reported as unsupported.

## Storage and compatibility

`~/.antigravity_tools/auto_switch.json` stores this feature's settings separately from general UI preferences. `auto_switch_state.json` stores only cancellation/failure state, with no tokens, email addresses, prompts or conversation content. An in-progress credential commit is journaled before writing so that a crash cannot cause an unattended retry. Pending process observations are never trusted across restart.

No task-observation Hooks are installed, no permission settings are changed, and no extra network listener is started. Closing an application is currently the required handoff boundary; task completion cannot be inferred simply because output or CPU activity stops.

## Verification

- Rust tests use synthetic quota windows, artificial tokens and temporary files. They cover both modes through pending state, running/unknown-process refusal, closed-client credential write/readback, single-commit behavior, quota revalidation and stale/canceled requests. No real account or keyring is used.
- UI acceptance tests use synthetic Tauri IPC in Chromium. They cover default-off setup, validation, cancellation, truthful stop guidance, unknown-process status, next-launch completion wording and a narrow viewport. CI uploads screenshots and traces. These are interface tests, not native authentication tests.
- The same Rust feature tests run in macOS, Windows and Linux CI. Full authenticated behavior still depends on the installed Antigravity version and platform credential store; passing build/unit/UI tests is not evidence of every native account-switch scenario.
- Separate manual checks on Antigravity APP 2.19.1 established native Stop persistence and reopening the same conversation, and account B continuing an existing conversation. They did not establish a public programmatic Stop endpoint or an atomic, uninterrupted migration of a running task. Native CLI cancellation/resume remains unverified.

Reference: [Antigravity Hooks](https://www.antigravity.google/docs/hooks/), [CLI cancellation controls](https://www.antigravity.google/docs/cli/prompting/), [CLI conversation resume](https://www.antigravity.google/docs/cli/commands/resume/).
