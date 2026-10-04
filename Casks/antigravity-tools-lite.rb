# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "antigravity-tools-lite" do
  version "4.7.9"
  sha256 "e7bec8cbb1656682cb4876d1bc61948c55d97c7e9264955cb2da4a2b2a8745a9"

  url "https://github.com/anglee0323/antigravity-tools-lite/releases/download/v4.7.9/Antigravity-Tools-Lite-4.7.9-macos-arm64.zip"
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
