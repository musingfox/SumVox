// SumVox menu bar app: mute toggle + recent voice notification history.
// Zero polling — state is read from disk only when the menu opens.
//
// Build: just menubar   (or: swiftc -O menubar/SumVoxMenu.swift -o sumvox-menubar)
// Shared contract with the sumvox binary (src/notify_log.rs):
//   mute flag   = ~/.config/sumvox/muted (existence)
//   history     = ~/.config/sumvox/history.log, one entry per line: "RFC3339\ttext"
//
// Talking avatar (Tier 1): each toast is a character + speech bubble. Drop two
// PNGs at ~/.config/sumvox/avatar/{closed,open}.png (mouth shut / open) and the
// toast shows them, flapping the mouth while the bubble types out. No art → a
// text face is used instead.

import AppKit

let configDir = FileManager.default.homeDirectoryForCurrentUser
    .appendingPathComponent(".config/sumvox")
let muteFile = configDir.appendingPathComponent("muted")
let historyFile = configDir.appendingPathComponent("history.log")
let configFile = configDir.appendingPathComponent("config.toml")
let avatarDir = configDir.appendingPathComponent("avatar")

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()

    // Toast state
    var fileSource: DispatchSourceFileSystemObject?
    var dirSource: DispatchSourceFileSystemObject?
    var toasts: [NSPanel] = []
    var lastShownText = ""
    let toastW: CGFloat = 380

    // Avatar frames, loaded once. Either nil → text-face fallback (both required).
    static let mouthClosed = NSImage(contentsOf: avatarDir.appendingPathComponent("closed.png"))
    static let mouthOpen = NSImage(contentsOf: avatarDir.appendingPathComponent("open.png"))

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
        // Size the card to the text so long messages aren't clipped. Height is
        // capped (past it, text truncates) so a giant summary can't fill the
        // screen. restack + the entry animation read each panel's own height,
        // so mixed-size toasts stack cleanly. ponytail: 220pt cap, raise if
        // summaries routinely need more room.
        let bubbleX: CGFloat = 88
        let bubbleW = toastW - bubbleX - 12
        let font = NSFont.systemFont(ofSize: 13, weight: .medium)
        // Measure wrapped text height with TextKit — fittingSize/boundingRect give
        // single-line results outside a view hierarchy; this is reliable headless.
        let ts = NSTextStorage(string: text, attributes: [.font: font])
        let tc = NSTextContainer(size: NSSize(width: bubbleW, height: .greatestFiniteMagnitude))
        tc.lineFragmentPadding = 0
        let lm = NSLayoutManager()
        lm.addTextContainer(tc)
        ts.addLayoutManager(lm)
        lm.ensureLayout(for: tc)
        let h = min(max(ceil(lm.usedRect(for: tc).height) + 24, 88), 220)

        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: toastW, height: h),
                            styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: false)
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.backgroundColor = .clear
        panel.ignoresMouseEvents = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let card = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: toastW, height: h))
        card.material = .hudWindow
        card.state = .active
        card.wantsLayer = true
        card.layer?.cornerRadius = 14
        card.layer?.masksToBounds = true

        // Character on the left. `flap(true)` shows the open mouth, `flap(false)` shut.
        let avatarFrame = NSRect(x: 12, y: (h - 64) / 2, width: 64, height: 64)
        let flap: (Bool) -> Void
        if let closed = Self.mouthClosed, let open = Self.mouthOpen {
            let iv = NSImageView(frame: avatarFrame)
            iv.imageScaling = .scaleProportionallyUpOrDown
            iv.image = closed
            card.addSubview(iv)
            flap = { iv.image = $0 ? open : closed }
        } else {
            let face = NSTextField(labelWithString: "(・ω・)")
            face.frame = avatarFrame
            face.alignment = .center
            face.font = .systemFont(ofSize: 22)
            card.addSubview(face)
            flap = { face.stringValue = $0 ? "(・o・)" : "(・ω・)" }
        }

        let label = NSTextField(wrappingLabelWithString: "")
        label.frame = NSRect(x: bubbleX, y: 12, width: bubbleW, height: h - 24)
        label.maximumNumberOfLines = 0   // wrappingLabel already word-wraps; don't
                                         // override lineBreakMode or it goes single-line.
        label.font = font
        label.textColor = .labelColor
        label.isSelectable = false
        card.addSubview(label)
        panel.contentView = card

        // Typewriter reveal drives the mouth. ponytail: flap follows the text
        // stream, not real TTS audio (Swift has no play signal yet) — wire to a
        // "speaking" file from the sumvox binary later if the two drift. Reveal
        // cadence is capped so even long text finishes inside the 4s dismiss.
        let chars = Array(text)
        var shown = 0
        let interval = min(0.045, 2.8 / Double(max(chars.count, 1)))
        // .common mode so the reveal keeps animating while the status-item menu
        // is open (menu tracking runs the run loop in .eventTracking).
        let timer = Timer(timeInterval: interval, repeats: true) { t in
            shown += 1
            label.stringValue = String(chars.prefix(shown))
            flap(shown % 2 == 0)
            if shown >= chars.count { t.invalidate(); flap(false) }
        }
        RunLoop.main.add(timer, forMode: .common)

        toasts.append(panel)
        // Reposition survivors, but skip the entering panel — the entry
        // animation below owns its motion to the final origin.
        restack(excluding: panel)

        // alphaValue = 0 must be set BEFORE orderFrontRegardless, else the
        // first frame flashes opaque.
        panel.alphaValue = 0
        let size = NSSize(width: toastW, height: h)
        if let vf = NSScreen.main?.visibleFrame {
            // New panel sits on top: bottom margin + heights of everyone below it.
            var finalY = vf.minY + 16
            for p in toasts where p != panel { finalY += p.frame.height + 8 }
            let finalOrigin = NSPoint(x: vf.maxX - toastW - 16, y: finalY)
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

        // Linger to match reading time: base + ~0.12s/char, clamped 4–12s.
        let dwell = min(max(3.0 + Double(chars.count) * 0.12, 4.0), 12.0)
        DispatchQueue.main.asyncAfter(deadline: .now() + dwell) { [weak self] in
            guard let self else { return }
            // Stop the reveal in case a slow run loop left it running past dwell,
            // so it can't mutate a hidden panel or hold the view chain alive.
            timer.invalidate()
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

    // Bottom-right of the active screen, stacked upward from the bottom margin.
    // `excluding` skips a panel whose motion is owned by the entry animation.
    func restack(excluding: NSPanel? = nil) {
        guard let vf = NSScreen.main?.visibleFrame else { return }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            var y = vf.minY + 16
            for p in toasts {
                let sz = p.frame.size
                if p != excluding {
                    let origin = NSPoint(x: vf.maxX - sz.width - 16, y: y)
                    p.animator().setFrame(NSRect(origin: origin, size: sz), display: true)
                }
                y += sz.height + 8
            }
        }
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // no Dock icon
app.run()
