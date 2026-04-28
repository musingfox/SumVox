class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.6.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.6.0/sumvox-macos-aarch64.tar.gz"
      sha256 "3e94823c62bf4d84b14c920f2de0649e55dd6b1af2649ec460e56b8c6d492ef2"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.6.0/sumvox-macos-x86_64.tar.gz"
      sha256 "172a869beb90d3f11646837fa65d06f322039eb006da64da7277fa08fabae41f"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.6.0/sumvox-linux-aarch64.tar.gz"
      sha256 "16ec8f63f6a68f42b8c359a912c198e44f2c1de7f43c7c7b7b8e46fc102006ad"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.6.0/sumvox-linux-x86_64.tar.gz"
      sha256 "3ef86a74b10dea1f7fd4b4842090a1829debad98878e34ff8f95c73282c20069"
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
