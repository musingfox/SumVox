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
final class AvatarRootView: NSView {
    var onClick: (() -> Void)?
    var onDrag: ((NSRect) -> Void)?

    private var mouseDownScreenPoint: NSPoint?
    private var mouseDownOrigin: NSPoint?
    private var didDrag = false

    override func mouseDown(with event: NSEvent) {
        guard let window else { return }
        mouseDownScreenPoint = NSEvent.mouseLocation
        mouseDownOrigin = window.frame.origin
        didDrag = false
    }

    override func mouseDragged(with event: NSEvent) {
        guard let window,
              let mouseDownScreenPoint,
              let mouseDownOrigin else { return }
        let current = NSEvent.mouseLocation
        let dx = current.x - mouseDownScreenPoint.x
        let dy = current.y - mouseDownScreenPoint.y
        if abs(dx) > 2 || abs(dy) > 2 { didDrag = true }
        let origin = NSPoint(x: mouseDownOrigin.x + dx, y: mouseDownOrigin.y + dy)
        window.setFrameOrigin(origin)
        onDrag?(window.frame)
    }

    override func mouseUp(with event: NSEvent) {
        if !didDrag { onClick?() }
        mouseDownScreenPoint = nil
        mouseDownOrigin = nil
        didDrag = false
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        isHidden ? nil : self
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }
}


final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()

    // Avatar state
    var fileSource: DispatchSourceFileSystemObject?
    var dirSource: DispatchSourceFileSystemObject?
    var lastShownText = ""
    let toastW: CGFloat = 380
    let avatarOnlyW: CGFloat = 96
    let avatarPanelH: CGFloat = 96
    var avatarPanel: NSPanel?
    var bubbleView: NSVisualEffectView?
    var bubbleLabel: NSTextField?
    var avatarRootView: NSView?
    var avatarShellView: NSVisualEffectView?
    var avatarAnchorRightX: CGFloat?
    var avatarAnchorBottomY: CGFloat?
    var speakingTimer: Timer?
    var hideBubbleWorkItem: DispatchWorkItem?
    var speechToken = 0
    var flap: ((Bool) -> Void)?

    // Avatar frames, loaded once. Either nil → text-face fallback (both required).
    static let mouthClosed = NSImage(contentsOf: avatarDir.appendingPathComponent("closed.png"))
    static let mouthOpen = NSImage(contentsOf: avatarDir.appendingPathComponent("open.png"))

    var muted: Bool { FileManager.default.fileExists(atPath: muteFile.path) }

    func applicationDidFinishLaunching(_ notification: Notification) {
        menu.delegate = self
        statusItem.menu = menu
        updateIcon()
        ensureAvatarPanel()
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

    func popAvatarMenu() {
        menuWillOpen(menu)
        let anchorX = avatarRootView?.bounds.width ?? avatarOnlyW
        menu.popUp(positioning: nil, at: NSPoint(x: anchorX, y: avatarPanelH), in: avatarRootView)
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

    func ensureAvatarPanel() {
        guard avatarPanel == nil else {
            positionAvatarPanel()
            return
        }

        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: avatarOnlyW, height: avatarPanelH),
                            styleMask: [.borderless, .nonactivatingPanel],
                            backing: .buffered, defer: false)
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let root = AvatarRootView(frame: NSRect(x: 0, y: 0, width: avatarOnlyW, height: avatarPanelH))
        root.onClick = { [weak self] in self?.popAvatarMenu() }
        root.onDrag = { [weak self] frame in
            self?.avatarAnchorRightX = frame.maxX
            self?.avatarAnchorBottomY = frame.minY
        }
        root.autoresizingMask = [.width, .height]
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor.clear.cgColor

        let avatarShell = NSVisualEffectView(frame: NSRect(x: 8, y: 8, width: 80, height: 80))
        avatarShell.material = .hudWindow
        avatarShell.state = .active
        avatarShell.wantsLayer = true
        avatarShell.layer?.cornerRadius = 20
        avatarShell.layer?.masksToBounds = true
        avatarShell.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.22).cgColor
        root.addSubview(avatarShell)
        if let closed = Self.mouthClosed, let open = Self.mouthOpen {
            let iv = NSImageView(frame: NSRect(x: 8, y: 8, width: 64, height: 64))
            iv.imageScaling = .scaleProportionallyUpOrDown
            iv.image = closed
            avatarShell.addSubview(iv)
            flap = { iv.image = $0 ? open : closed }
        } else {
            let face = NSTextField(labelWithString: "(・ω・)")
            face.frame = NSRect(x: 8, y: 8, width: 64, height: 64)
            face.alignment = .center
            face.font = .systemFont(ofSize: 22)
            avatarShell.addSubview(face)
            flap = { face.stringValue = $0 ? "(・o・)" : "(・ω・)" }
        }

        let bubble = NSVisualEffectView(frame: NSRect(x: 12, y: 12, width: toastW - avatarOnlyW - 12, height: 72))
        bubble.material = .hudWindow
        bubble.state = .active
        bubble.wantsLayer = true
        bubble.layer?.cornerRadius = 14
        bubble.layer?.masksToBounds = true
        bubble.isHidden = true

        let label = NSTextField(wrappingLabelWithString: "")
        label.frame = NSRect(x: 12, y: 12, width: bubble.frame.width - 24, height: bubble.frame.height - 24)
        label.maximumNumberOfLines = 0
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.textColor = .labelColor
        label.isSelectable = false
        bubble.addSubview(label)

        root.addSubview(bubble)
        panel.contentView = root

        avatarPanel = panel
        avatarRootView = root
        avatarShellView = avatarShell
        bubbleView = bubble
        bubbleLabel = label
        positionAvatarPanel(width: avatarOnlyW, height: avatarPanelH)
        panel.orderFrontRegardless()
    }

    func positionAvatarPanel(width: CGFloat? = nil, height: CGFloat? = nil) {
        guard let panel = avatarPanel else { return }
        let panelW = width ?? panel.frame.width
        let panelH = height ?? panel.frame.height
        let anchorPoint = NSPoint(x: avatarAnchorRightX ?? NSEvent.mouseLocation.x,
                                  y: avatarAnchorBottomY ?? NSEvent.mouseLocation.y)
        let screen = NSScreen.screens.first { $0.visibleFrame.contains(anchorPoint) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        guard let vf = screen?.visibleFrame else { return }
        let defaultOrigin = NSPoint(x: vf.maxX - panelW - 16, y: vf.minY + 16)
        let proposedOrigin = NSPoint(x: (avatarAnchorRightX ?? defaultOrigin.x + panelW) - panelW,
                                     y: avatarAnchorBottomY ?? defaultOrigin.y)
        let clampedOrigin = NSPoint(x: min(max(proposedOrigin.x, vf.minX), vf.maxX - panelW),
                                    y: min(max(proposedOrigin.y, vf.minY), vf.maxY - panelH))
        panel.setFrame(NSRect(origin: clampedOrigin, size: NSSize(width: panelW, height: panelH)), display: true)
        avatarAnchorRightX = panel.frame.maxX
        avatarAnchorBottomY = panel.frame.minY
        avatarRootView?.frame = NSRect(x: 0, y: 0, width: panelW, height: panelH)
        avatarShellView?.frame = NSRect(x: panelW - 88, y: 8, width: 80, height: 80)
    }

    func showToast(_ text: String) {
        ensureAvatarPanel()
        guard let bubble = bubbleView, let label = bubbleLabel else { return }

        speechToken += 1
        let token = speechToken
        speakingTimer?.invalidate()
        speakingTimer = nil
        hideBubbleWorkItem?.cancel()
        hideBubbleWorkItem = nil
        flap?(false)

        let bubbleW = toastW - avatarOnlyW - 12
        let font = NSFont.systemFont(ofSize: 13, weight: .medium)
        let ts = NSTextStorage(string: text, attributes: [.font: font])
        let tc = NSTextContainer(size: NSSize(width: bubbleW - 24, height: .greatestFiniteMagnitude))
        tc.lineFragmentPadding = 0
        let lm = NSLayoutManager()
        lm.addTextContainer(tc)
        ts.addLayoutManager(lm)
        lm.ensureLayout(for: tc)
        let bubbleH = min(max(ceil(lm.usedRect(for: tc).height) + 24, 56), 188)

        let gap: CGFloat = 8
        let panelW = bubbleW + 24
        let panelH = avatarPanelH + gap + bubbleH
        let bubbleX = panelW - bubbleW - 12
        bubble.frame = NSRect(x: bubbleX, y: avatarPanelH + gap, width: bubbleW, height: bubbleH)
        label.frame = NSRect(x: 12, y: 12, width: bubbleW - 24, height: bubbleH - 24)
        label.stringValue = ""
        bubble.alphaValue = 1
        bubble.isHidden = false
        avatarPanel?.orderFrontRegardless()
        positionAvatarPanel(width: panelW, height: panelH)

        let chars = Array(text)
        var shown = 0
        let interval = min(0.045, 2.8 / Double(max(chars.count, 1)))
        let timer = Timer(timeInterval: interval, repeats: true) { [weak self] t in
            guard let self else { t.invalidate(); return }
            guard token == self.speechToken else { t.invalidate(); return }
            shown += 1
            label.stringValue = String(chars.prefix(shown))
            self.flap?(shown % 2 == 0)
            if shown >= chars.count {
                t.invalidate()
                self.flap?(false)
                self.speakingTimer = nil
            }
        }
        speakingTimer = timer
        RunLoop.main.add(timer, forMode: .common)

        let dwell = min(max(3.0 + Double(chars.count) * 0.12, 4.0), 12.0)
        let hideBubble = DispatchWorkItem { [weak self] in
            guard let self, token == self.speechToken else { return }
            self.speakingTimer?.invalidate()
            self.speakingTimer = nil
            self.flap?(false)
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.18
                bubble.animator().alphaValue = 0
            }, completionHandler: { [weak self] in
                guard let self, token == self.speechToken else { return }
                bubble.isHidden = true
                bubble.alphaValue = 1
                self.positionAvatarPanel(width: self.avatarOnlyW, height: self.avatarPanelH)
            })
        }
        hideBubbleWorkItem = hideBubble
        DispatchQueue.main.asyncAfter(deadline: .now() + dwell, execute: hideBubble)
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory) // no Dock icon
app.run()
