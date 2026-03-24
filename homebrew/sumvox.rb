class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.4.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.1/sumvox-macos-aarch64.tar.gz"
      sha256 "73a699320c4a0aa3f6dab4c88beae3a9ed303efa42fc03877e5b988eb0f53a6b"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.1/sumvox-macos-x86_64.tar.gz"
      sha256 "3ce69ce8d11aab2cb419ec2788f704cecda93974431a36ab5d0b2f907bd77319"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.1/sumvox-linux-aarch64.tar.gz"
      sha256 "2fdf2c9136e1605725092d435f163db87e42adb0cdf7ba999b9f98eae84a257c"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.1/sumvox-linux-x86_64.tar.gz"
      sha256 "956b924926d123c4feb4ad10ee9f4461422c6b79b5ac1378cfd05725a845d77e"
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
