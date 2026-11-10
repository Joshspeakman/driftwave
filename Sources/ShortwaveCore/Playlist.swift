import Foundation

/// Reads the stream URL out of .pls / .m3u playlist files, which many station links point to.
public enum Playlist {
    public static func looksLikePlaylist(url: URL, contentType: String?) -> Bool {
        let ext = url.pathExtension.lowercased()
        if ["pls", "m3u", "xspf"].contains(ext) { return true }
        let ct = (contentType ?? "").lowercased()
        return ct.contains("scpls") || ct.contains("x-mpegurl") && ext != "m3u8" || ct.contains("audio/mpegurl") || ct.contains("xspf")
    }

    /// First stream URL in the playlist text, resolved against `base`.
    public static func firstStream(in text: String, base: URL) -> URL? {
        let lines = text.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }
        for line in lines {
            // PLS: File1=http://...
            if line.lowercased().hasPrefix("file"), let eq = line.firstIndex(of: "=") {
                if let u = resolve(String(line[line.index(after: eq)...]), base) { return u }
            }
        }
        // XSPF: <location>http://...</location>
        if let r = text.range(of: "<location>"), let e = text.range(of: "</location>", range: r.upperBound..<text.endIndex) {
            if let u = resolve(String(text[r.upperBound..<e.lowerBound]), base) { return u }
        }
        // M3U: first non-comment line
        for line in lines where !line.isEmpty && !line.hasPrefix("#") && !line.hasPrefix("[") && !line.contains("=") {
            if let u = resolve(line, base) { return u }
        }
        return nil
    }

    private static func resolve(_ s: String, _ base: URL) -> URL? {
        let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, let u = URL(string: t, relativeTo: base)?.absoluteURL else { return nil }
        guard let scheme = u.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return nil }
        return u
    }
}

/// Makes a string safe to use as a file name.
public func sanitizedFileName(_ s: String, maxLength: Int = 150) -> String {
    let forbidden = CharacterSet(charactersIn: "/\\:*?\"<>|").union(.controlCharacters).union(.newlines)
    var out = String(String.UnicodeScalarView(s.unicodeScalars.map { forbidden.contains($0) ? "_" : $0 }))
    out = out.trimmingCharacters(in: .whitespaces.union(CharacterSet(charactersIn: ".")))
    if out.count > maxLength { out = String(out.prefix(maxLength)).trimmingCharacters(in: .whitespaces) }
    return out.isEmpty ? "Track" : out
}

/// `dir/name.ext`, or `dir/name (2).ext` etc. when that file already exists.
public func uniqueFileURL(in dir: URL, name: String, ext: String) -> URL {
    let fm = FileManager.default
    var candidate = dir.appendingPathComponent(name).appendingPathExtension(ext)
    var n = 2
    while fm.fileExists(atPath: candidate.path) {
        candidate = dir.appendingPathComponent("\(name) (\(n))").appendingPathExtension(ext)
        n += 1
    }
    return candidate
}
