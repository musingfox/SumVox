class Sumvox < Formula
  desc "Intelligent voice notifications for AI coding tools"
  homepage "https://github.com/musingfox/sumvox"
  version "1.4.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.0/sumvox-macos-aarch64.tar.gz"
      sha256 "71eb252550dab3ce849bd716a0444b0cc911360737321285be9a179566d20797"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.0/sumvox-macos-x86_64.tar.gz"
      sha256 "307817107903a646ed13409d267d9dd027343d240760965411fdff503a1f3dc5"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.0/sumvox-linux-aarch64.tar.gz"
      sha256 "fece597af9a843c9b8289977b474cb0ac4e88fd3eb29b0c6972a7cede3e022f7"
    else
      url "https://github.com/musingfox/sumvox/releases/download/v1.4.0/sumvox-linux-x86_64.tar.gz"
      sha256 "51f42dd8b414cc3e86eaf7881ab8af9eb444e2479a48aa1b4863f156b9d2ff6e"
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
