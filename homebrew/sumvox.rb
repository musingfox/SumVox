class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.7.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-macos-aarch64.tar.gz"
      sha256 "32ccf16c26e58c13c63797c8c0d0d038ba2bbe8896067dfb43ebff78678db4f7"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-macos-x86_64.tar.gz"
      sha256 "905ae2ace08a5577413d6242ef1a149e2e196ed4f61a0969b8f72a0b470e33ee"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-linux-aarch64.tar.gz"
      sha256 "effcbf013d8040590591c95df8ed0cfe57c9bf12b1a929cbe967c7016b43d069"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-linux-x86_64.tar.gz"
      sha256 "b790bdfd64c38dc5b030f7ff6ae00bb66587ce1f81220b1bc59e9c51155a489d"
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
