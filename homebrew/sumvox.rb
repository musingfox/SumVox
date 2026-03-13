class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-macos-aarch64.tar.gz"
      sha256 "47be311baace6fdb51cb9c74989a1b1d9e86c6e0409608a036500c3c00432bc4"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-macos-x86_64.tar.gz"
      sha256 "3ce286ad317ec598897c88e7e52f7f928f11e8b48bff7d7228ec4481b947b7e9"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-linux-aarch64.tar.gz"
      sha256 "105650966fe45ea6a14e3cdb033eeeeb4bbc532514437bedcad954cae5a71e82"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-linux-x86_64.tar.gz"
      sha256 "85dc8288a23b9599e6429cf2020dc8ef7a0a753a23c47ff621b77a2c30ea4b71"
    end
  end

  def install
    bin.install "sumvox"
  end

  def post_install
    system bin/"sumvox", "init"
  end

  def caveats
    <<~EOS
      SumVox has been installed!

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
      Docs: https://github.com/musingfox/sumvox
    EOS
  end

  test do
    assert_match "sumvox", shell_output("#{bin}/sumvox --version")
    assert_match "init", shell_output("#{bin}/sumvox --help")
  end
end
