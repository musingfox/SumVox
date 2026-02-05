class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  url "https://github.com/musingfox/sumvox/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "6e8de85f673ca5072f37cadf20518edc7d4835e866cda6f68476b63d132d7807"
  license "MIT"
  version "1.0.0"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release"
    bin.install "target/release/sumvox"

    # Install documentation
    doc.install "README.md"
    doc.install "QUICKSTART.md"
    doc.install "config/recommended.yaml"
  end

  def post_install
    # Initialize config if it doesn't exist
    system bin/"sumvox", "init"
  end

  def caveats
    <<~EOS
      SumVox has been installed! 🎉

      Next steps:
      1. Edit config file and set your API keys:
         open ~/.config/sumvox/config.yaml
         # Replace ${PROVIDER_API_KEY} with your actual API keys

      2. Test voice notification:
         sumvox say "Hello, SumVox!"

      3. Configure Claude Code hook in ~/.claude/settings.json:
         {
           "hooks": {
             "Notification": [{
               "matcher": "",
               "hooks": [{"type": "command", "command": "#{bin}/sumvox"}]
             }],
             "Stop": [{
               "matcher": "",
               "hooks": [{"type": "command", "command": "#{bin}/sumvox"}]
             }]
           }
         }

      Config: ~/.config/sumvox/config.yaml
      Quick Start: #{doc}/QUICKSTART.md
      Full Guide: #{doc}/README.md
    EOS
  end

  test do
    assert_match "sumvox", shell_output("#{bin}/sumvox --version")
    assert_match "init", shell_output("#{bin}/sumvox --help")
  end
end
