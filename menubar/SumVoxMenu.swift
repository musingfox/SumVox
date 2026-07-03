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

    // Toast state
    var fileSource: DispatchSourceFileSystemObject?
    var dirSource: DispatchSourceFileSystemObject?
    var toasts: [NSPanel] = []
    var lastShownText = ""
    let toastW: CGFloat = 360
    let toastH: CGFloat = 72

    var muted: Bool { FileManager.default.fileExists(atPath: muteFile.path) }

    func applicationDidFinishLaunching(_ notification: Notification) {
        menu.delegate = self
        statusItem.menu = menu
        updateIcon()
        startWatching()
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

    // MARK: - Toast on new notification
    //
    // history.log is truncate-written in place (same inode) by notify_log.rs, so a
    // vnode source on the file catches every write. Before the file's first write we
    // watch the parent dir for its creation, then switch to watching the file.
    // ponytail: assumes history.log is never unlinked+recreated; if it is (manual
    // delete), toasts pause until relaunch. Add re-arm on .delete if that ever matters.

    func startWatching() {
        FileManager.default.fileExists(atPath: historyFile.path) ? armFile() : armDir()
    }

    func armDir() {
        let fd = open(configDir.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: .write, queue: .main)
        src.setEventHandler { [weak self] in
            guard let self, FileManager.default.fileExists(atPath: historyFile.path) else { return }
            src.cancel()
            self.armFile()
            self.showLatest()
        }
        src.setCancelHandler { close(fd) }
        src.resume()
        dirSource = src
    }

    func armFile() {
        let fd = open(historyFile.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: .write, queue: .main)
        src.setEventHandler { [weak self] in self?.showLatest() }
        src.setCancelHandler { close(fd) }
        src.resume()
        fileSource = src
    }

    // Read the newest history line and toast it, skipping repeated write events for the same entry.
    func showLatest() {
        guard let raw = try? String(contentsOf: historyFile, encoding: .utf8),
              let line = raw.split(separator: "\n").last else { return }
        let parts = line.split(separator: "\t", maxSplits: 1)
        let text = parts.count > 1 ? String(parts[1]) : String(line)
        guard text != lastShownText else { return }
        lastShownText = text
        showToast(text)
    }

    func showToast(_ text: String) {
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: toastW, height: toastH),
                            styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: false)
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.backgroundColor = .clear
        panel.ignoresMouseEvents = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let card = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: toastW, height: toastH))
        card.material = .hudWindow
        card.state = .active
        card.wantsLayer = true
        card.layer?.cornerRadius = 14
        card.layer?.masksToBounds = true

        let label = NSTextField(labelWithString: "🔊  \(text)")
        label.frame = NSRect(x: 16, y: 8, width: toastW - 32, height: toastH - 16)
        label.maximumNumberOfLines = 2
        label.lineBreakMode = .byTruncatingTail
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.textColor = .labelColor
        card.addSubview(label)
        panel.contentView = card

        toasts.append(panel)
        // Reposition survivors, but skip the entering panel — the entry
        // animation below owns its motion to the final origin.
        restack(excluding: panel)

        // alphaValue = 0 must be set BEFORE orderFrontRegardless, else the
        // first frame flashes opaque.
        panel.alphaValue = 0
        let size = NSSize(width: toastW, height: toastH)
        if let vf = NSScreen.main?.visibleFrame {
            let i = toasts.count - 1
            let finalOrigin = NSPoint(x: vf.maxX - toastW - 16,
                                      y: vf.maxY - toastH - 16 - CGFloat(i) * (toastH + 8))
            // Start offset 40pt to the right of the final stacked position.
            panel.setFrame(NSRect(origin: NSPoint(x: finalOrigin.x + 40, y: finalOrigin.y),
                                  size: size), display: false)
            panel.orderFrontRegardless()
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.22
                context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                panel.animator().setFrame(NSRect(origin: finalOrigin, size: size), display: true)
                panel.animator().alphaValue = 1
            }
        } else {
            // No screen — still show the panel, just skip the slide/fade.
            panel.alphaValue = 1
            panel.orderFrontRegardless()
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 4) { [weak self] in
            guard let self else { return }
            // Remove FIRST so restack never repositions a dying panel; a
            // double-fire removal is a harmless no-op.
            self.toasts.removeAll { $0 == panel }
            self.restack()
            let frame = panel.frame
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.18
                context.timingFunction = CAMediaTimingFunction(name: .easeIn)
                panel.animator().setFrame(NSRect(origin: NSPoint(x: frame.origin.x + 40, y: frame.origin.y),
                                                 size: frame.size), display: true)
                panel.animator().alphaValue = 0
            }, completionHandler: {
                panel.orderOut(nil)
            })
        }
    }

    // Top-right of the active screen, stacked downward under the menu bar.
    // `excluding` skips a panel whose motion is owned by the entry animation.
    func restack(excluding: NSPanel? = nil) {
        guard let vf = NSScreen.main?.visibleFrame else { return }
        let size = NSSize(width: toastW, height: toastH)
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            for (i, p) in toasts.enumerated() {
                if p == excluding { continue }
                let origin = NSPoint(x: vf.maxX - toastW - 16,
                                     y: vf.maxY - toastH - 16 - CGFloat(i) * (toastH + 8))
                p.animator().setFrame(NSRect(origin: origin, size: size), display: true)
            }
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // no Dock icon
app.run()
