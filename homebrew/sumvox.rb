class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.2.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.1/sumvox-macos-aarch64.tar.gz"
      sha256 "8879b4cae2f9ad0aa170deb0f9672322bb65b1831817971fd8293684b6c6620c"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.1/sumvox-macos-x86_64.tar.gz"
      sha256 "3a2fe3a577cc25e54888aa2314dcff0dfe4526838686f1e1b5fe8d3885e6d4f1"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.1/sumvox-linux-aarch64.tar.gz"
      sha256 "d7ce2b3ec39d500c5f7f014fb833288be59b46e216e047edc5d7885db6ec0f1b"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.2.1/sumvox-linux-x86_64.tar.gz"
      sha256 "54d2286e17113d9c5e422d6f70d4aeb359dbc267352af90579756178c4bec3d2"
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
