class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.3.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.1/sumvox-macos-aarch64.tar.gz"
      sha256 "0fd224cf9648a75b3d3c2bfe2b0885ec1ed66933a4ef89990e6fa9960c32076a"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.1/sumvox-macos-x86_64.tar.gz"
      sha256 "a9fe1ec43c11a1ce49cd8582ba89015d006c659c3dec8325c3366400f6166c85"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.1/sumvox-linux-aarch64.tar.gz"
      sha256 "d9f107dead1ef4429aad181e799d7b8e7dccd8916110798c4395e9595c63bba1"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.1/sumvox-linux-x86_64.tar.gz"
      sha256 "9eeaaa3eef8911ecff8ddffae7b11e37b47fe2b34daf570938cb5ff5a5161fde"
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
