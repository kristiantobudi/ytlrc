# Homebrew formula for ytlrc.
#
# Usage once this repo (or a tap) hosts it:
#   brew install --formula ./packaging/homebrew/ytlrc.rb
# or, after publishing to a tap `kristiantobudi/homebrew-ytlrc`:
#   brew tap kristiantobudi/ytlrc
#   brew install ytlrc
#
# Update `url`/`sha256` for each release. The two asset entries must match the
# files published by .github/workflows/release.yml (ytlrc-macos-arm64 /
# ytlrc-macos-x86_64).
class Ytlrc < Formula
  desc "Synced YouTube lyrics in your terminal"
  homepage "https://github.com/kristiantobudi/ytlrc"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kristiantobudi/ytlrc/releases/download/v0.2.0/ytlrc-macos-arm64"
      sha256 "REPLACE_WITH_SHA256_OF_ytlrc-macos-arm64"
    end
    on_intel do
      url "https://github.com/kristiantobudi/ytlrc/releases/download/v0.2.0/ytlrc-macos-x86_64"
      sha256 "REPLACE_WITH_SHA256_OF_ytlrc-macos-x86_64"
    end
  end

  depends_on "curl" => :recommended

  def install
    binary_name = Hardware::CPU.arm? ? "ytlrc-macos-arm64" : "ytlrc-macos-x86_64"
    bin.install binary_name => "ytlrc"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ytlrc --version")
  end
end
