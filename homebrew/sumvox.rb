class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.7.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.1/sumvox-macos-aarch64.tar.gz"
      sha256 "5c7c855866729067044f110af6457cdfd1b72f603d3c5c0a3f1b95919285f720"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.1/sumvox-macos-x86_64.tar.gz"
      sha256 "bc3b0b2005c8e559c68e12c077c3d00fd3a6737dccd2d7b5a8a384ba1633625e"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.1/sumvox-linux-aarch64.tar.gz"
      sha256 "62997f78028f70832954371e28a229f15105fd35bf72ec5ebd05f7f36f0fda9b"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.1/sumvox-linux-x86_64.tar.gz"
      sha256 "23e1dd83b41989f752fdde113d1468ce12a78904d62f8221ec0487bf499bb17f"
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
