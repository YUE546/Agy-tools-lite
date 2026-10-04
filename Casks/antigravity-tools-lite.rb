# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "antigravity-tools-lite" do
  version "4.8.0"
  sha256 "f77dadc3fb004d8f284a6d4134fb004c68ddf5c9dbde9694c808c53364544f53"

  url "https://github.com/anglee0323/antigravity-tools-lite/releases/download/v4.8.0/Antigravity-Tools-Lite-4.8.0-macos-arm64.zip"
  name "Antigravity Tools Lite"
  desc "Antigravity account manager, local usage dashboard and agy-switch CLI"
  homepage "https://github.com/anglee0323/antigravity-tools-lite"

  depends_on arch: :arm64
  depends_on :macos

  app "Antigravity Tools Lite.app"
  binary "#{appdir}/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools", target: "agy-switch"

  caveats <<~EOS
    Includes the agy-switch command. Run agy-switch --help to get started.
    This is Tools Lite's management CLI, separate from Google's agy command.
    Account data and system credentials are retained when uninstalling.
  EOS
end
