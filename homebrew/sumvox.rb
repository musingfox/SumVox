class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.5.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.5.0/sumvox-macos-aarch64.tar.gz"
      sha256 "a90cdaaa6fcdf24bec406d223c8ee6289c184dea476d75ebc199268698b0021d"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.5.0/sumvox-macos-x86_64.tar.gz"
      sha256 "70704df3b7dab96521066c1e71b336e39c519aa9236e1bf011b29be0bca00dbf"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.5.0/sumvox-linux-aarch64.tar.gz"
      sha256 "4e579133a1fd41371bb658db22729adceb76b818ab70be79282b10ea07c27f32"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.5.0/sumvox-linux-x86_64.tar.gz"
      sha256 "59621cd913ae37ad8ca818f4acb6919cbbdd93d8fb2d973a50d900c03b91af59"
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
