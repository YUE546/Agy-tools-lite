# Homebrew distribution

## Status

This repository provides a reproducible **macOS Apple Silicon cask recipe** and a release-asset generator. A public tap has not been created by this change, and existing releases may not include the CLI. Do not advertise `brew install` against a tap or asset until that exact release archive and generated cask have been published and tested.

The cask installs both `Antigravity Tools Lite.app` and the management command `agy-lite`. It uses Homebrew's documented [`app` and `binary` artifacts](https://docs.brew.sh/Cask-Cookbook#stanza-binary). A separate formula would still carry the current desktop-linked executable, so no lightweight CLI-only formula is claimed here. Linux and Intel macOS Homebrew packages are not provided; use the existing Linux packages or build from source.

## Release inputs and generation

The macOS release job explicitly builds `aarch64-apple-darwin`, verifies the Mach-O architecture, and produces:

- `Antigravity-Tools-Lite-VERSION-macos-arm64.zip`
- `antigravity-tools-lite.rb`, generated from the exact ZIP with a real SHA-256 checksum

The ZIP and cask are uploaded together by the existing release workflow. Editing this workflow does not itself trigger a release or create a tap. The workflow refuses a tag/version mismatch. Replacing a published ZIP changes its checksum: regenerate and republish the matching cask, or preferably release a new version.

For manual generation, set these to a real archive and its intended immutable release URL; no sample release URL is assumed to exist:

```sh
node scripts/generate-homebrew.mjs \
  --archive "$RELEASE_ZIP" \
  --url "$RELEASE_ARCHIVE_URL" \
  --version "$RELEASE_VERSION" \
  --output artifacts/homebrew/Casks/antigravity-tools-lite.rb
```

The generator validates the filename/version pair, HTTPS URL and bundled executable path. It computes SHA-256 from the file; it does not use `:no_check`, upload anything, fetch the URL, or claim the release is live. The release job separately verifies that the bundled executable is ARM64. The recipe requires Apple Silicon and macOS 11 or later; running the built app on the oldest supported macOS still needs a release smoke test.

## Test a generated cask locally on a Mac

After the exact ZIP is publicly available at the cask URL:

```sh
# A local-only developer tap. This command does not create a GitHub repository.
brew tap-new local/antigravity-tools-lite
mkdir -p "$(brew --repository local/antigravity-tools-lite)/Casks"
cp artifacts/homebrew/Casks/antigravity-tools-lite.rb \
  "$(brew --repository local/antigravity-tools-lite)/Casks/antigravity-tools-lite.rb"
brew style --cask local/antigravity-tools-lite/antigravity-tools-lite
brew audit --cask local/antigravity-tools-lite/antigravity-tools-lite
brew install --cask local/antigravity-tools-lite/antigravity-tools-lite
agy-lite --version
agy-lite accounts list --json
brew uninstall --cask local/antigravity-tools-lite/antigravity-tools-lite
```

Review any Homebrew trust prompt yourself. `brew uninstall` retains saved accounts and OS credentials. No `zap` stanza deletes them. Test that upgrades preserve account files and that `agy-lite` follows a custom `--appdir`, then publish the verified cask under `Casks/` in an authorized tap. Only after publication should end-user installation instructions name that tap. See Homebrew's [tap maintenance guide](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap).

## Signing and Gatekeeper

The existing release workflow does not configure Developer ID signing/notarization. This change does not claim notarized binaries or disable quarantine/Gatekeeper. Homebrew installation does not remove that limitation. A distributable release needs appropriate signing/notarization or clearly documented user review of the unsigned app; do not add quarantine-removal commands to the cask. Confirm Apple trust behavior on a clean Mac before promoting installation instructions.

## Publish the cask in this repository

A separate `homebrew-*` repository is optional. Homebrew's [two-argument tap form](https://docs.brew.sh/Taps) supports this existing Git repository. After the final release ZIP is published and its generated cask is tested, open a small PR that copies the exact generated `antigravity-tools-lite.rb` into `Casks/antigravity-tools-lite.rb` at the repository root. Keep the real immutable release URL and SHA-256 from that archive. A release attachment alone is not a tap entry.

Only after that PR is merged and the following installation has been verified should the README advertise these commands:

```sh
brew tap anglee0323/antigravity-tools-lite https://github.com/anglee0323/antigravity-tools-lite.git
brew install --cask anglee0323/antigravity-tools-lite/antigravity-tools-lite
agy-lite --version
agy-lite --help
```

The explicit Git URL matters: the one-argument `brew tap` form would look for a different, `homebrew-`-prefixed repository. The cask's `app` artifact installs the app, and its `binary` artifact links the bundled executable as `$(brew --prefix)/bin/agy-lite`, following a custom `--appdir`. It neither installs nor replaces Google's `agy`.

For later versions, publish the new ZIP first, generate and test the matching cask, then update the same root-level cask through a PR. Users can then run:

```sh
brew update
brew upgrade --cask anglee0323/antigravity-tools-lite/antigravity-tools-lite
```

Use the [release checklist](release-checklist.md) to keep the source commit, version, assets and installation evidence aligned.
