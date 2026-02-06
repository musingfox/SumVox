class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.0.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.0.0/sumvox-macos-aarch64.tar.gz"
      sha256 "a9cf75f05188e26bfcd7335aaa89d1cd948756de033834768777292da516541a"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.0.0/sumvox-macos-x86_64.tar.gz"
      sha256 "5e3e1457d56d72576f7dac3a3591084ca82c05ff45d71cdbd79c070537c3b87b"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.0.0/sumvox-linux-aarch64.tar.gz"
      sha256 "fc7da5a85b3a4e0cc3597751919e8d0cd6ccfb48ee8b8df9dea2afc8ea8915fa"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.0.0/sumvox-linux-x86_64.tar.gz"
      sha256 "0c25292e720e1e8d17b0b7706db91a2aa697a5ad5ea74d347492dcbaf06fe0a1"
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
