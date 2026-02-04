class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  url "https://github.com/musingfox/sumvox/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"  # Will be updated during release
  license "MIT"
  version "1.0.0"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release"
    bin.install "target/release/sumvox"

    # Install hook script
    (prefix/"hooks").mkpath
    (prefix/"hooks").install ".claude/hooks/run_sumvox_hook.sh"

    # Install documentation
    doc.install "README.md"
    doc.install "config/recommended.json"
  end

  def post_install
    # Initialize config if it doesn't exist
    system bin/"sumvox", "init"
  end

  def caveats
    <<~EOS
      SumVox has been installed! 🎉

      Next steps:
      1. Set API credentials:
         sumvox credentials set google

      2. Register Claude Code hook in ~/.claude/settings.json:
         {
           "hooks": {
             "Notification": [{
               "matcher": "",
               "hooks": [{
                 "type": "command",
                 "command": "#{bin}/sumvox"
               }]
             }]
           }
         }

      Config location: ~/.config/sumvox/config.json
      Recommended config: #{doc}/recommended.json

      For more details: #{doc}/README.md
    EOS
  end

  test do
    assert_match "sumvox", shell_output("#{bin}/sumvox --version")
    assert_match "init", shell_output("#{bin}/sumvox --help")
  end
end
