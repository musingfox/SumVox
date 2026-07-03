// SumVox menu bar app: mute toggle + recent voice notification history.
// Zero polling — state is read from disk only when the menu opens.
//
// Build: just menubar   (or: swiftc -O menubar/SumVoxMenu.swift -o sumvox-menubar)
// Shared contract with the sumvox binary (src/notify_log.rs):
//   mute flag   = ~/.config/sumvox/muted (existence)
//   history     = ~/.config/sumvox/history.log, one entry per line: "RFC3339\ttext"

import AppKit

let configDir = FileManager.default.homeDirectoryForCurrentUser
    .appendingPathComponent(".config/sumvox")
let muteFile = configDir.appendingPathComponent("muted")
let historyFile = configDir.appendingPathComponent("history.log")
let configFile = configDir.appendingPathComponent("config.toml")

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()

    var muted: Bool { FileManager.default.fileExists(atPath: muteFile.path) }

    func applicationDidFinishLaunching(_ notification: Notification) {
        menu.delegate = self
        statusItem.menu = menu
        updateIcon()
    }

    func updateIcon() {
        statusItem.button?.title = muted ? "🔇" : "🔊"
    }

    // Rebuild the whole menu on every open — cheap (≤50 lines) and always fresh.
    func menuWillOpen(_ menu: NSMenu) {
        menu.removeAllItems()

        let toggle = NSMenuItem(title: "播放語音", action: #selector(toggleMute), keyEquivalent: "")
        toggle.target = self
        toggle.state = muted ? .off : .on
        menu.addItem(toggle)
        menu.addItem(.separator())

        let historyMenu = NSMenu()
        let lines = ((try? String(contentsOf: historyFile, encoding: .utf8)) ?? "")
            .split(separator: "\n")
        for line in lines.reversed() {
            let parts = line.split(separator: "\t", maxSplits: 1)
            let time = parts.first.map(String.init) ?? ""
            let text = parts.count > 1 ? String(parts[1]) : ""
            let title = text.count > 60 ? String(text.prefix(60)) + "…" : text
            let entry = NSMenuItem(title: title, action: #selector(copyEntry(_:)), keyEquivalent: "")
            entry.target = self
            entry.toolTip = "\(time)\n\(text)"
            entry.representedObject = text
            historyMenu.addItem(entry)
        }
        if historyMenu.items.isEmpty {
            historyMenu.addItem(NSMenuItem(title: "(尚無記錄)", action: nil, keyEquivalent: ""))
        }
        let historyItem = NSMenuItem(title: "最近通知", action: nil, keyEquivalent: "")
        historyItem.submenu = historyMenu
        menu.addItem(historyItem)
        menu.addItem(.separator())

        let settings = NSMenuItem(title: "開啟設定檔", action: #selector(openConfig), keyEquivalent: "")
        settings.target = self
        menu.addItem(settings)

        menu.addItem(NSMenuItem(title: "結束", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
    }

    @objc func toggleMute() {
        if muted {
            try? FileManager.default.removeItem(at: muteFile)
        } else {
            try? FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
            FileManager.default.createFile(atPath: muteFile.path, contents: nil)
        }
        updateIcon()
    }

    @objc func copyEntry(_ sender: NSMenuItem) {
        guard let text = sender.representedObject as? String else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    @objc func openConfig() {
        NSWorkspace.shared.open(configFile)
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // no Dock icon
app.run()
