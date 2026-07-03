class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.8.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.8.0/sumvox-macos-aarch64.tar.gz"
      sha256 "1f851b9b56ef74ed5c8b9596187e6745dfeae963274a8883b11537dbcfa019d7"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.8.0/sumvox-macos-x86_64.tar.gz"
      sha256 "565f35740ac19608359dd2232cc8d49b8512e24a6539ad74b40f633511a6c19c"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.8.0/sumvox-linux-aarch64.tar.gz"
      sha256 "5dba53052bc47af9020d2feb1d45bd381749c3cfc925b2dd768a7ecd8499afd0"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.8.0/sumvox-linux-x86_64.tar.gz"
      sha256 "6bff3eb0a125b1de0e168fa5980291b0c87f6f2de6083cecba2bc90813a9fe68"
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
