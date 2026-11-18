import AppKit

/// `-screenshot <png>` (test runs): after `-screenshotDelay` seconds (default 4), draws the main
/// window, with any open sheet laid over it, into a PNG and quits. It draws the app's own views,
/// so it works with the display asleep, like the Windows app's flag of the same name.
/// `-appearance light|dark` picks the look for the run (it overrides the App Theme setting).
@MainActor
enum Screenshot {
    /// `openMain` opens the main window when a test copy started without it (SwiftUI doesn't always
    /// present it for a copy launched from a script).
    static func scheduleIfAsked(openMain: @escaping () -> Void) {
        guard let path = Diagnostics.screenshot else { return }
        let delay = Diagnostics.screenshotDelay ?? 4
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            if !NSApp.windows.contains(where: { $0.isVisible && $0.identifier?.rawValue.contains("main") == true }) {
                Diagnostics.log("opening the main window for the screenshot")
                openMain()
            }
        }
        // Render at the Windows app's size (1180×760) with the window active, so accent-coloured
        // controls draw as they do in use. Test copies share the installed app's saved window
        // frame, so the old frame is put back before quitting.
        var savedFrame: NSRect?
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8) {
            NSApp.activate()
            guard let w = mainWindow() else { return }
            w.makeKeyAndOrderFront(nil)
            if !Diagnostics.openMiniPlayer {
                savedFrame = w.frame
                w.setFrame(NSRect(x: w.frame.minX, y: w.frame.maxY - 760, width: 1180, height: 760), display: true)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + max(delay, 2.5)) {
            if capture(to: URL(fileURLWithPath: path)) {
                Diagnostics.log("screenshot \(path)")
            } else {
                Diagnostics.log("screenshot failed: windows \(NSApp.windows.map { "\(type(of: $0)) \($0.frame)" })")
            }
            if let f = savedFrame { mainWindow()?.setFrame(f, display: false) }
            // Let the window save its restored frame, then exit (not terminate: an open sheet can
            // hold up a normal quit).
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { exit(0) }
        }
    }

    static func mainWindow() -> NSWindow? {
        if Diagnostics.openMiniPlayer, let mini = NSApp.windows.first(where: { $0.isVisible && $0.identifier?.rawValue.contains("mini") == true }) {
            return mini
        }
        let candidates = NSApp.windows.filter { $0.isVisible && !$0.isSheet && $0.frame.width > 300 && $0.frame.height > 200 }
        return candidates.first(where: { $0.identifier?.rawValue.contains("main") == true })
            ?? candidates.first(where: { $0.identifier?.rawValue.contains("mini") != true })
            ?? candidates.max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height })
    }

    static func capture(to url: URL) -> Bool {
        guard let window = mainWindow(), let base = render(window) else { return false }
        var output = base
        if let sheet = window.attachedSheet, let sheetRep = render(sheet) {
            output = compose(base, over: sheetRep) ?? base
        }
        guard let png = output.representation(using: .png, properties: [:]) else { return false }
        return (try? png.write(to: url)) != nil
    }

    private static func render(_ window: NSWindow) -> NSBitmapImageRep? {
        guard let view = window.contentView?.superview ?? window.contentView,
              let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        return rep
    }

    /// The sheet centred near the top of the window, over a dimmed copy of it.
    private static func compose(_ base: NSBitmapImageRep, over sheet: NSBitmapImageRep) -> NSBitmapImageRep? {
        let w = base.pixelsWide, h = base.pixelsHigh
        guard let out = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8,
                                         samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                         bytesPerRow: 0, bitsPerPixel: 0),
              let ctx = NSGraphicsContext(bitmapImageRep: out) else { return nil }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = ctx
        let full = NSRect(x: 0, y: 0, width: w, height: h)
        base.draw(in: full)
        NSColor.black.withAlphaComponent(0.25).setFill()
        full.fill(using: .sourceAtop)
        let sw = sheet.pixelsWide, sh = sheet.pixelsHigh
        let top = Int(Double(h) * 0.06)
        sheet.draw(in: NSRect(x: (w - sw) / 2, y: max(0, h - sh - top), width: sw, height: sh))
        NSGraphicsContext.restoreGraphicsState()
        return out
    }
}
