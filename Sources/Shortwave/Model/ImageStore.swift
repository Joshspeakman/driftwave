import AppKit
import CryptoKit
import Foundation
import Observation
import ShortwaveCore

/// Station covers: user-chosen images, then the station's favicon (cached on disk).
@Observable @MainActor
final class ImageStore {
    private let paths: AppPaths
    @ObservationIgnored private var memory: [String: NSImage] = [:]
    @ObservationIgnored private var failed: Set<String> = []
    @ObservationIgnored private var inFlight: [String: Task<NSImage?, Never>] = [:]
    /// Bumped whenever an image arrives, so views re-read the cache.
    private(set) var revision = 0

    private let session: URLSession = {
        let c = URLSessionConfiguration.default
        c.timeoutIntervalForRequest = 15
        c.httpAdditionalHeaders = ["User-Agent": RadioBrowser.userAgent]
        return URLSession(configuration: c)
    }()

    init(paths: AppPaths) { self.paths = paths }

    private func key(for s: Station) -> String? {
        if let c = s.customCover { return "cover:" + c }
        if let f = s.faviconURL, !f.isEmpty { return "fav:" + f }
        return nil
    }

    func cached(for s: Station) -> NSImage? {
        _ = revision
        guard let k = key(for: s) else { return nil }
        return memory[k]
    }

    func invalidate(_ s: Station) {
        guard let k = key(for: s) else { return }
        memory[k] = nil
        failed.remove(k)
        revision += 1
    }

    @discardableResult
    func load(for s: Station) async -> NSImage? {
        guard let k = key(for: s) else { return nil }
        if let m = memory[k] { return m }
        if failed.contains(k) { return nil }
        if let t = inFlight[k] { return await t.value }
        let task = Task<NSImage?, Never> { [paths, session] in
            if let cover = s.customCover {
                return NSImage(contentsOf: paths.covers.appendingPathComponent(cover))
            }
            guard let fav = s.faviconURL, let url = URL(string: fav) else { return nil }
            let digest = SHA256.hash(data: Data(fav.utf8)).map { String(format: "%02x", $0) }.joined()
            let file = paths.favicons.appendingPathComponent(digest + ".png")
            if let img = NSImage(contentsOf: file) { return img }
            guard let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https",
                  let (data, response) = try? await session.data(from: url),
                  (response as? HTTPURLResponse).map({ (200..<300).contains($0.statusCode) }) ?? true,
                  let img = NSImage(data: data), img.isValid, img.size.width >= 8 else { return nil }
            guard let png = ImageStore.pngData(img, maxSide: 256) else { return img }
            try? FileManager.default.createDirectory(at: paths.favicons, withIntermediateDirectories: true)
            try? png.write(to: file)
            return NSImage(data: png) ?? img
        }
        inFlight[k] = task
        let image = await task.value
        inFlight[k] = nil
        if let image { memory[k] = image; revision += 1 } else { failed.insert(k) }
        return image
    }

    nonisolated static func pngData(_ image: NSImage, maxSide: CGFloat) -> Data? {
        var rect = CGRect(origin: .zero, size: image.size)
        guard let cg = image.cgImage(forProposedRect: &rect, context: nil, hints: nil) else { return nil }
        let w = CGFloat(cg.width), h = CGFloat(cg.height)
        let scale = min(1, maxSide / max(w, h))
        let size = CGSize(width: max(1, (w * scale).rounded()), height: max(1, (h * scale).rounded()))
        guard let ctx = CGContext(data: nil, width: Int(size.width), height: Int(size.height), bitsPerComponent: 8,
                                  bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        ctx.interpolationQuality = .high
        ctx.draw(cg, in: CGRect(origin: .zero, size: size))
        guard let out = ctx.makeImage() else { return nil }
        return NSBitmapImageRep(cgImage: out).representation(using: .png, properties: [:])
    }
}
