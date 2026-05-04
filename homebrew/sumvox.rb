class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.7.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-macos-aarch64.tar.gz"
      sha256 "4a7d41f3679911c8cffec43fe7a947e1f00009d75e6c6d7523803ab7a4ffdc43"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-macos-x86_64.tar.gz"
      sha256 "859f95ad1cbac3718151d9d78220dafad392a0c47a8ae5fd3e26d9b9286c7dc5"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-linux-aarch64.tar.gz"
      sha256 "dfbbef15d4bca5321edb91e6ad8c8d91c3276e9a305467cc046e8085a6587619"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.7.0/sumvox-linux-x86_64.tar.gz"
      sha256 "71ed2085965082a4e696afb328b0d65444c7ce8de0e2b3ad1e47d1d345b853b4"
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
