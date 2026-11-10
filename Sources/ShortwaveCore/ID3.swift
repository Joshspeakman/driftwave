import Foundation

/// Builds a minimal ID3v2.3 tag (UTF-16 text frames) to put in front of a saved MP3.
public enum ID3 {
    public static func tag(title: String, artist: String?, album: String?, comment: String? = nil) -> Data {
        var frames = Data()
        frames.append(textFrame("TIT2", title))
        if let artist, !artist.isEmpty { frames.append(textFrame("TPE1", artist)) }
        if let album, !album.isEmpty { frames.append(textFrame("TALB", album)) }
        if let comment, !comment.isEmpty { frames.append(commentFrame(comment)) }

        var tag = Data("ID3".utf8)
        tag.append(contentsOf: [3, 0, 0])            // v2.3.0, no flags
        tag.append(contentsOf: synchsafe(frames.count))
        tag.append(frames)
        return tag
    }

    static func utf16WithBOM(_ s: String) -> Data {
        var d = Data([0xFF, 0xFE])
        for unit in s.utf16 { d.append(UInt8(unit & 0xFF)); d.append(UInt8(unit >> 8)) }
        return d
    }

    static func textFrame(_ id: String, _ value: String) -> Data {
        var body = Data([1])                        // encoding: UTF-16 with BOM
        body.append(utf16WithBOM(value))
        return frame(id, body)
    }

    static func commentFrame(_ value: String) -> Data {
        var body = Data([1])
        body.append(contentsOf: Array("eng".utf8))
        body.append(utf16WithBOM(""))               // empty description
        body.append(contentsOf: [0, 0])
        body.append(utf16WithBOM(value))
        return frame("COMM", body)
    }

    static func frame(_ id: String, _ body: Data) -> Data {
        var f = Data(id.utf8)
        let n = UInt32(body.count)
        f.append(contentsOf: [UInt8(n >> 24), UInt8((n >> 16) & 0xFF), UInt8((n >> 8) & 0xFF), UInt8(n & 0xFF)])
        f.append(contentsOf: [0, 0])
        f.append(body)
        return f
    }

    static func synchsafe(_ n: Int) -> [UInt8] {
        [UInt8((n >> 21) & 0x7F), UInt8((n >> 14) & 0x7F), UInt8((n >> 7) & 0x7F), UInt8(n & 0x7F)]
    }
}
