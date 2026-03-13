class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.3.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-macos-aarch64.tar.gz"
      sha256 "83077f202c43e257d2dd355a2475aa9d46d206935f22e1570234739023af8d08"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-macos-x86_64.tar.gz"
      sha256 "608de271ba5f39d092fd7c34d5ba2cf6bdf9e1b6f3f8cabf302f80a3a0a121e0"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-linux-aarch64.tar.gz"
      sha256 "e5ef2f51c76a5d84d725b1fff45ec60267bc81e6020f26bf02ed1d9506483ba5"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.3.0/sumvox-linux-x86_64.tar.gz"
      sha256 "20b9f04ccc3b96f281d13c6323347fe7f2c8bc4454f939ba0b9e284d77469fa2"
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
