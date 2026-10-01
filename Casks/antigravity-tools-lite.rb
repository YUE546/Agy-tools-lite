# Generated from a release archive; do not replace SHA-256 with :no_check.
cask "antigravity-tools-lite" do
  version "4.7.7"
  sha256 "ecb686028b5a565b7bdec4a610cb1ee1eb19191b0882c8e72bd4808cca2a5b70"

  url "https://github.com/anglee0323/antigravity-tools-lite/releases/download/v4.7.7/Antigravity-Tools-Lite-4.7.7-macos-arm64.zip"
  name "Antigravity Tools Lite"
  desc "Antigravity account manager, local usage dashboard and agy-lite CLI"
  homepage "https://github.com/anglee0323/antigravity-tools-lite"

  depends_on arch: :arm64
  depends_on macos: ">= :big_sur"

  app "Antigravity Tools Lite.app"
  binary "#{appdir}/Antigravity Tools Lite.app/Contents/MacOS/antigravity-tools", target: "agy-lite"

  caveats <<~EOS
    Includes the agy-lite command. Run agy-lite --help to get started.
    This is Tools Lite's management CLI, separate from Google's agy command.
    Account data and system credentials are retained when uninstalling.
  EOS
end
