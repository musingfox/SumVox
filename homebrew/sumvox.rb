class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.9.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.9.0/sumvox-macos-aarch64.tar.gz"
      sha256 "c0603c6789d4d10ff7090e1e83e9c85d8c01b16163720c34a3829caf251e990c"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.9.0/sumvox-macos-x86_64.tar.gz"
      sha256 "d09c8b62ef0480632648a329b398f23dc8c988139bc39b01182f020770bd5bb1"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.9.0/sumvox-linux-aarch64.tar.gz"
      sha256 "17a4ffefb2a6af4ec934106c62fc4e98df48672d44adfaabb3222b2d4b5237cd"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.9.0/sumvox-linux-x86_64.tar.gz"
      sha256 "acaba4998baac27fb10f7a8123c907bb3ed672d088c54710e0b9f71325604d05"
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
      1. Create the config file and set your API keys:
         sumvox init
         open ~/.config/sumvox/config.toml
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

      Config: ~/.config/sumvox/config.toml
      Docs: https://github.com/musingfox/sumvox
    EOS
  end

  test do
    assert_match "sumvox", shell_output("#{bin}/sumvox --version")
    assert_match "init", shell_output("#{bin}/sumvox --help")
  end
end
