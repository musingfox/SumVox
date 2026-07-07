// SumVox menu bar app: mute toggle + recent voice notification history.
// Zero polling — state is read from disk only when the menu opens.
//
// Build: just menubar   (or: swiftc -O menubar/SumVoxMenu.swift -o sumvox-menubar)
// Shared contract with the sumvox binary (src/notify_log.rs):
//   mute flag   = ~/.config/sumvox/muted (existence)
//   history     = ~/.config/sumvox/history.log, one entry per line: "RFC3339\ttext"
//   now_playing = ~/.config/sumvox/now_playing, path of the audio file playing now
//
// Talking avatar: a vector blob (Catmull-Rom path + radial gradient) inside the
// glass shell. Driven continuously by setLevel(0..1) — idle breathes with slow
// drifting lobes, speaking is driven by the now_playing audio's RMS envelope;
// the typewriter path synthesizes a level when there is no real audio file.

import AppKit
import AVFoundation
import QuartzCore

let configDir = FileManager.default.homeDirectoryForCurrentUser
    .appendingPathComponent(".config/sumvox")
let muteFile = configDir.appendingPathComponent("muted")
let historyFile = configDir.appendingPathComponent("history.log")
let nowPlayingFile = configDir.appendingPathComponent("now_playing")
let configFile = configDir.appendingPathComponent("config.toml")

// Orb palette — emerald→cyan (SumVox cool tone).
let orbColorFrom = NSColor(srgbRed: 52/255, green: 211/255, blue: 153/255, alpha: 1)  // #34d399
let orbColorTo = NSColor(srgbRed: 34/255, green: 211/255, blue: 238/255, alpha: 1)    // #22d3ee

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

func makeGlassView(frame: NSRect, cornerRadius: CGFloat) -> NSView {
    let view: NSView
    if #available(macOS 26.0, *) {
        view = NSGlassEffectView(frame: frame)
    } else {
        let effect = NSVisualEffectView(frame: frame)
        effect.material = .popover
        effect.blendingMode = .behindWindow
        effect.state = .active
        view = effect
    }
    view.wantsLayer = true
    view.layer?.cornerRadius = cornerRadius
    view.layer?.masksToBounds = true
    return view
}

// Vector Orb — a smooth, deformable blob drawn as a Catmull-Rom path filled
// with a radial gradient. Original visual language (not voiceorbs): idle = a
// soft organic blob breathing + drifting low-frequency lobes; speaking = the
// envelope level swells it and drives multi-harmonic radial wobble. Drawn
// each frame into an 80×80 CoreGraphics bitmap -> CALayer.contents. Pure
// vector curves + gradient, zero dependencies, ~60fps.
final class VectorOrbView: NSView {
    private let size = 80
    private let center = CGPoint(x: 40, y: 40)
    private let baseR: CGFloat = 27
    private let segments = 48

    private var ctx: CGContext?
    private var timer: Timer?
    private var last: Double = 0

    // Gradient color stops (sRGB floats), built once from the orb palette.
    private var grad: CGGradient?

    private var breathePhase: Float = 0
    private var idlePhase: Float = 0
    private var wobbleA: Float = 0
    private var wobbleB: Float = 0
    private var lvlSmooth: Float = 0
    private var lvlTarget: Float = 0
    private var active = false   // true = alerted/speaking (full brightness + 60fps)
    private var tickCount = 0     // for idle frame-skip

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        let layer = CALayer()
        layer.frame = bounds
        layer.contentsGravity = .resize
        layer.opacity = 0.5   // idle: dim
        self.layer = layer
        self.wantsLayer = true
        buildGradient()
        let cs = CGColorSpaceCreateDeviceRGB()
        ctx = CGContext(data: nil, width: size, height: size,
                        bitsPerComponent: 8, bytesPerRow: size * 4,
                        space: cs,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        render(dt: 0)
        let t = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) not supported") }

    deinit { timer?.invalidate() }

    func setLevel(_ v: Float) { lvlTarget = max(0, min(1, v)) }

    // Toggle alerted/speaking mode: full brightness + 60fps when on, dim + 20fps
    // when off. Idempotent. The brightness step itself is the "new message" cue.
    func setActive(_ on: Bool) {
        guard on != active else { return }
        active = on
        CATransaction.begin()
        CATransaction.setAnimationDuration(on ? 0.25 : 0.4)
        layer?.opacity = on ? 1.0 : 0.5
        CATransaction.commit()
    }

    private func tick() {
        let now = ProcessInfo.processInfo.systemUptime
        let dt = last == 0 ? 1.0 / 60.0 : min(now - last, 0.1)
        last = now
        render(dt: Float(dt))
    }

    private func buildGradient() {
        let (fr, fg, fb) = rgb(orbColorFrom)   // emerald
        let (tr, tg, tb) = rgb(orbColorTo)     // cyan
        // Center = bright emerald, mid = cyan, rim = dark cyan. Light from
        // upper-left via an offset highlight drawn separately.
        let ctr = (fr * 0.6 + 1 * 0.4, fg * 0.6 + 1 * 0.4, fb * 0.6 + 1 * 0.4)
        let mid = (tr, tg, tb)
        let rim = (tr * 0.45, tg * 0.45, tb * 0.45)
        let colors = [
            CGColor(srgbRed: ctr.0, green: ctr.1, blue: ctr.2, alpha: 1),
            CGColor(srgbRed: mid.0, green: mid.1, blue: mid.2, alpha: 1),
            CGColor(srgbRed: rim.0, green: rim.1, blue: rim.2, alpha: 1),
        ]
        grad = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(),
                          colors: colors as CFArray,
                          locations: [0, 0.55, 1])
    }

    private func rgb(_ c: NSColor) -> (CGFloat, CGFloat, CGFloat) {
        (CGFloat(c.redComponent), CGFloat(c.greenComponent), CGFloat(c.blueComponent))
    }

    // Sample the blob radius at angle θ (radians), with center as origin.
    private func radius(_ theta: Float, lvl: Float) -> Float {
        let breathe = 1 + 0.04 * sin(breathePhase)
        let swell = 1 + 0.15 * lvl
        // idle: slow drifting lobes (alive but calm); speaking: level-driven wobble.
        let idle = 0.05 * sin(2 * theta + idlePhase)
            + 0.03 * sin(3 * theta + idlePhase * 0.7)
        let driven = 0.12 * lvl * sin(3 * theta + wobbleA)
            + 0.06 * sin(5 * theta - wobbleB)
        return Float(baseR) * breathe * swell * (1 + idle + driven)
    }

    // Build a smooth closed Catmull-Rom path through the sampled rim points.
    private func blobPath(lvl: Float) -> CGPath {
        let path = CGMutablePath()
        let n = segments
        var pts = [CGPoint](repeating: .zero, count: n)
        let cx = Float(center.x), cy = Float(center.y)
        for i in 0..<n {
            let theta = Float(i) / Float(n) * 2 * Float.pi
            let r = radius(theta, lvl: lvl)
            pts[i] = CGPoint(x: CGFloat(cx + r * cos(theta)),
                             y: CGFloat(cy + r * sin(theta)))
        }
        // Catmull-Rom -> cubic bezier, closed.
        for i in 0..<n {
            let p0 = pts[(i - 1 + n) % n]
            let p1 = pts[i]
            let p2 = pts[(i + 1) % n]
            let p3 = pts[(i + 2) % n]
            let c1 = CGPoint(x: p1.x + (p2.x - p0.x) / 6,
                             y: p1.y + (p2.y - p0.y) / 6)
            let c2 = CGPoint(x: p2.x - (p3.x - p1.x) / 6,
                             y: p2.y - (p3.y - p1.y) / 6)
            if i == 0 {
                path.move(to: p1)
            }
            path.addCurve(to: p2, control1: c1, control2: c2)
        }
        path.closeSubpath()
        return path
    }

    private func render(dt: Float) {
        guard let ctx = ctx, let layer = self.layer, let grad = grad else { return }
        let dtm = dt
        // Phases advance every tick so motion stays real-time even when we skip
        // redraws — idle just renders fewer snapshots (20fps), not slower motion.
        lvlSmooth += (lvlTarget - lvlSmooth) * (1 - Float(exp(-Double(12 * dtm))))
        breathePhase += 1.1 * dtm
        idlePhase += 0.7 * dtm
        wobbleA += 2.2 * dtm
        wobbleB += 1.4 * dtm

        // idle: redraw every 3rd tick (~20fps); active: every tick (60fps).
        // The path/gradient/makeImage is the cost, so skipping it is the saving.
        tickCount += 1
        let stride = active ? 1 : 3
        if (tickCount % stride) != 0 { return }

        let lvl = Float(pow(Double(lvlSmooth), 1.4))
        let rect = CGRect(x: 0, y: 0, width: size, height: size)
        ctx.clear(rect)

        let path = blobPath(lvl: lvl)

        // Fill: clip to blob, paint radial gradient (light from upper-left).
        ctx.saveGState()
        ctx.addPath(path)
        ctx.clip()
        let hl = CGPoint(x: center.x - 9, y: center.y + 9)  // upper-left in flipped? layer is bottom-origin; +y is up
        ctx.drawRadialGradient(grad, startCenter: hl, startRadius: 0,
                               endCenter: center, endRadius: CGFloat(baseR) * 1.15,
                               options: [])
        ctx.restoreGState()

        // Hairline rim.
        ctx.addPath(path)
        ctx.setStrokeColor(red: 1, green: 1, blue: 1, alpha: 0.25)
        ctx.setLineWidth(1)
        ctx.strokePath()

        if let img = ctx.makeImage() {
            layer.contents = img
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()

    // Avatar state
    var fileSource: DispatchSourceFileSystemObject?
    var dirSource: DispatchSourceFileSystemObject?
    var nowPlayingSource: DispatchSourceFileSystemObject?
    var nowPlayingDirSource: DispatchSourceFileSystemObject?
    var envelopeTimer: Timer?
    var envelopeActive = false
    var lastShownText = ""
    let toastW: CGFloat = 380
    let avatarOnlyW: CGFloat = 96
    let avatarPanelH: CGFloat = 96
    var avatarPanel: NSPanel?
    var bubbleView: NSView?
    var bubbleLabel: NSTextField?
    var avatarRootView: NSView?
    var avatarShellView: NSView?
    var avatarAnchorX: CGFloat?
    var avatarAnchorY: CGFloat?
    var speakingTimer: Timer?
    var hideBubbleWorkItem: DispatchWorkItem?
    var speechToken = 0
    var setLevel: ((Float) -> Void)?

    // Orb layers (built once in ensureAvatarPanel). nil until the panel exists.
    var orbView: VectorOrbView?

    var muted: Bool { FileManager.default.fileExists(atPath: muteFile.path) }

    func applicationDidFinishLaunching(_ notification: Notification) {
        menu.delegate = self
        statusItem.menu = menu
        updateIcon()
        ensureAvatarPanel()
        startWatching()
        watchNowPlaying()
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

    // MARK: - Amplitude-driven orb
    //
    // now_playing is truncate-written by run_afplay just before it spawns afplay,
    // so a write event on it means "real audio is starting now". We decode that
    // file, build an RMS envelope, and drive the orb continuously by env[k]/peak
    // (0..1) over the clip's real duration. Same arm-dir-then-file pattern as
    // history.log. ponytail: parallel watcher instead of generalizing
    // armFile/armDir — an 8-line proven pattern, not worth risking the working
    // history toast to share.

    func watchNowPlaying() {
        FileManager.default.fileExists(atPath: nowPlayingFile.path) ? armNowPlaying() : armNowPlayingDir()
    }

    func armNowPlayingDir() {
        let fd = open(configDir.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: .write, queue: .main)
        src.setEventHandler { [weak self] in
            guard let self, FileManager.default.fileExists(atPath: nowPlayingFile.path) else { return }
            src.cancel()
            self.armNowPlaying()
            self.onNowPlaying()
        }
        src.setCancelHandler { close(fd) }
        src.resume()
        nowPlayingDirSource = src
    }

    func armNowPlaying() {
        let fd = open(nowPlayingFile.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: .write, queue: .main)
        src.setEventHandler { [weak self] in self?.onNowPlaying() }
        src.setCancelHandler { close(fd) }
        src.resume()
        nowPlayingSource = src
    }

    func onNowPlaying() {
        guard let raw = try? String(contentsOf: nowPlayingFile, encoding: .utf8) else { return }
        let path = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !path.isEmpty else { return }
        let url = URL(fileURLWithPath: path)
        // Decode off the main thread; a summary clip can be a few seconds of PCM.
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let env = self?.loadEnvelope(url) else { return }
            DispatchQueue.main.async { self?.animateMouth(env.samples, frameDur: env.frameDur) }
        }
    }

    // Downsample the clip to one RMS value per ~40ms bucket. Channel 0 only —
    // a mono envelope is all a two-frame mouth needs.
    func loadEnvelope(_ url: URL) -> (samples: [Float], frameDur: Double)? {
        guard let file = try? AVAudioFile(forReading: url) else { return nil }
        let format = file.processingFormat
        let total = AVAudioFrameCount(file.length)
        guard total > 0, format.sampleRate > 0,
              let buf = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: total),
              (try? file.read(into: buf)) != nil,
              let chan = buf.floatChannelData else { return nil }
        let data = chan[0]
        let n = Int(buf.frameLength)
        let frameDur = 0.04
        let bucket = max(1, Int(format.sampleRate * frameDur))
        var env: [Float] = []
        env.reserveCapacity(n / bucket + 1)
        var i = 0
        while i < n {
            let end = min(i + bucket, n)
            var sum: Float = 0
            for j in i..<end { let s = data[j]; sum += s * s }
            env.append((sum / Float(end - i)).squareRoot())
            i = end
        }
        return (env, frameDur)
    }

    func animateMouth(_ env: [Float], frameDur: Double) {
        envelopeTimer?.invalidate()
        guard let peak = env.max(), peak > 0 else { setLevel?(0); return }
        envelopeActive = true
        refreshActive()   // real audio → brighten + 60fps
        var k = 0
        let timer = Timer(timeInterval: frameDur, repeats: true) { [weak self] t in
            guard let self else { t.invalidate(); return }
            if k >= env.count {
                t.invalidate()
                self.envelopeActive = false
                self.envelopeTimer = nil
                self.setLevel?(0)
                self.refreshActive()   // audio done → maybe dim if no toast
                return
            }
            self.setLevel?(env[k] / peak)
            k += 1
        }
        envelopeTimer = timer
        RunLoop.main.add(timer, forMode: .common)
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
            guard let self else { return }
            let shellX = self.avatarShellView?.frame.minX ?? 8
            let shellY = self.avatarShellView?.frame.minY ?? 8
            self.avatarAnchorX = frame.minX + shellX + 40
            self.avatarAnchorY = frame.minY + shellY + 40
        }
        root.autoresizingMask = [.width, .height]
        root.wantsLayer = true
        root.layer?.backgroundColor = NSColor.clear.cgColor

        // Shell is fully transparent — no liquid glass — so only the orb blob
        // itself is visible. The orb composites directly on the transparent
        // panel window.
        let avatarShell = NSView(frame: NSRect(x: 8, y: 8, width: 80, height: 80))
        avatarShell.wantsLayer = true
        avatarShell.layer?.backgroundColor = NSColor.clear.cgColor
        root.addSubview(avatarShell)

        let orb = VectorOrbView(frame: NSRect(x: 0, y: 0, width: 80, height: 80))
        avatarShell.addSubview(orb)
        orbView = orb
        setLevel = { [weak orb] v in orb?.setLevel(v) }

        let bubble = makeGlassView(frame: NSRect(x: 12, y: 12, width: toastW - avatarOnlyW - 12, height: 72),
                                   cornerRadius: 18)
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
        let anchorPoint = NSPoint(x: avatarAnchorX ?? NSEvent.mouseLocation.x,
                                  y: avatarAnchorY ?? NSEvent.mouseLocation.y)
        let screen = NSScreen.screens.first { $0.visibleFrame.contains(anchorPoint) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        guard let vf = screen?.visibleFrame else { return }
        let shellX: CGFloat = 8
        let shellY: CGFloat = 8
        let defaultAnchorX = vf.maxX - 64
        let defaultAnchorY = vf.minY + 48
        let proposedOrigin = NSPoint(x: (avatarAnchorX ?? defaultAnchorX) - shellX - 40,
                                     y: (avatarAnchorY ?? defaultAnchorY) - shellY - 40)
        let clampedOrigin = NSPoint(x: min(max(proposedOrigin.x, vf.minX), vf.maxX - panelW),
                                    y: min(max(proposedOrigin.y, vf.minY), vf.maxY - panelH))
        panel.setFrame(NSRect(origin: clampedOrigin, size: NSSize(width: panelW, height: panelH)), display: true)
        avatarAnchorX = panel.frame.minX + shellX + 40
        avatarAnchorY = panel.frame.minY + shellY + 40
        avatarRootView?.frame = NSRect(x: 0, y: 0, width: panelW, height: panelH)
        avatarShellView?.frame = NSRect(x: shellX, y: shellY, width: 80, height: 80)
    }

    // Active = a toast is showing or real audio is driving the orb. When idle
    // (neither), the orb dims and drops to 20fps.
    func refreshActive() {
        let on = speakingTimer != nil || envelopeActive || !(bubbleView?.isHidden ?? true)
        orbView?.setActive(on)
    }

    func showToast(_ text: String) {
        ensureAvatarPanel()
        guard let bubble = bubbleView, let label = bubbleLabel else { return }

        speechToken += 1
        let token = speechToken
        speakingTimer?.invalidate()
        speakingTimer = nil
        envelopeTimer?.invalidate()
        envelopeTimer = nil
        envelopeActive = false
        hideBubbleWorkItem?.cancel()
        hideBubbleWorkItem = nil
        setLevel?(0)

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
        let anchorPoint = NSPoint(x: avatarAnchorX ?? NSEvent.mouseLocation.x,
                                  y: avatarAnchorY ?? NSEvent.mouseLocation.y)
        let screen = NSScreen.screens.first { $0.visibleFrame.contains(anchorPoint) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        let vf = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: bubbleW + avatarOnlyW + 24, height: avatarPanelH + gap + bubbleH)
        let avatarCenterX = avatarAnchorX ?? (vf.maxX - 64)
        let avatarCenterY = avatarAnchorY ?? (vf.minY + 48)
        let avatarRect = NSRect(x: avatarCenterX - 40, y: avatarCenterY - 40, width: 80, height: 80)
        let bubbleOnLeft = avatarCenterX > vf.midX
        let bubbleAbove = avatarCenterY <= vf.midY
        var bubbleRect = NSRect(
            x: bubbleOnLeft ? (avatarRect.minX - 12 - bubbleW) : (avatarRect.maxX + 12),
            y: bubbleAbove ? (avatarRect.maxY + gap) : (avatarRect.minY - gap - bubbleH),
            width: bubbleW,
            height: bubbleH
        )
        bubbleRect.origin.x = min(max(bubbleRect.origin.x, vf.minX + 12), vf.maxX - bubbleW - 12)
        bubbleRect.origin.y = min(max(bubbleRect.origin.y, vf.minY + 12), vf.maxY - bubbleH - 12)
        let panelRect = avatarRect.union(bubbleRect).insetBy(dx: -8, dy: -8)
        let panelW = panelRect.width
        let panelH = panelRect.height
        let bubbleX = bubbleRect.minX - panelRect.minX
        let bubbleY = bubbleRect.minY - panelRect.minY
        let avatarX = avatarRect.minX - panelRect.minX
        let avatarY = avatarRect.minY - panelRect.minY
        bubble.frame = NSRect(x: bubbleX, y: bubbleY, width: bubbleW, height: bubbleH)
        label.frame = NSRect(x: 12, y: 12, width: bubbleW - 24, height: bubbleH - 24)
        label.stringValue = ""
        bubble.alphaValue = 1
        bubble.isHidden = false
        avatarShellView?.frame = NSRect(x: avatarX, y: avatarY, width: 80, height: 80)
        avatarRootView?.frame = NSRect(x: 0, y: 0, width: panelW, height: panelH)
        avatarPanel?.setFrame(panelRect, display: true)
        avatarAnchorX = avatarCenterX
        avatarAnchorY = avatarCenterY
        avatarPanel?.orderFrontRegardless()
        refreshActive()   // message arrived → brighten + 60fps as the cue

        let chars = Array(text)
        var shown = 0
        let interval = min(0.045, 2.8 / Double(max(chars.count, 1)))
        let timer = Timer(timeInterval: interval, repeats: true) { [weak self] t in
            guard let self else { t.invalidate(); return }
            guard token == self.speechToken else { t.invalidate(); return }
            shown += 1
            label.stringValue = String(chars.prefix(shown))
            // Real audio owns the orb when playing; typewriter synthesizes a
            // smooth level as the fallback (macOS `say`, no audio file, or
            // audio not yet started).
            if !self.envelopeActive {
                let lvl = 0.35 + 0.45 * sin(Double(shown) * 1.6)
                self.setLevel?(Float(max(0, lvl)))
            }
            if shown >= chars.count {
                t.invalidate()
                if !self.envelopeActive { self.setLevel?(0) }
                self.speakingTimer = nil
                self.refreshActive()   // typing done; stay bright through dwell
            }
        }
        speakingTimer = timer
        RunLoop.main.add(timer, forMode: .common)

        let dwell = min(max(3.0 + Double(chars.count) * 0.12, 4.0), 12.0)
        let hideBubble = DispatchWorkItem { [weak self] in
            guard let self, token == self.speechToken else { return }
            self.speakingTimer?.invalidate()
            self.speakingTimer = nil
            self.setLevel?(0)
            NSAnimationContext.runAnimationGroup({ context in
                context.duration = 0.18
                bubble.animator().alphaValue = 0
            }, completionHandler: { [weak self] in
                guard let self, token == self.speechToken else { return }
                bubble.isHidden = true
                bubble.alphaValue = 1
                self.positionAvatarPanel(width: self.avatarOnlyW, height: self.avatarPanelH)
                self.refreshActive()   // dismissed → dim + 20fps idle
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
