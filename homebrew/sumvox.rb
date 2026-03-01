class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.2.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.0/sumvox-macos-aarch64.tar.gz"
      sha256 "a81a0286fcedd2c109aa7643c59b611c7a7183c6d50cd65dbc0a71b39e3d1857"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.0/sumvox-macos-x86_64.tar.gz"
      sha256 "03a10dc8da3df27bcbefe98c562609e8dd52df3668468cddf9ea56a8d0f42f13"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.0/sumvox-linux-aarch64.tar.gz"
      sha256 "36f7e677920280b61e2c68242bd24f8516a4be6f1ebb7cffd5c697b354e2260c"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.0/sumvox-linux-x86_64.tar.gz"
      sha256 "655203239e99dd079557d8a95b99e9d9430615a9eba8d6b38f7c706c47ce0a46"
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
