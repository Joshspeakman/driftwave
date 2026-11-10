import Foundation

/// Splits a Shoutcast/Icecast byte stream (sent with `Icy-MetaData: 1`) into audio and metadata.
///
/// Every `metaInt` audio bytes the server inserts one length byte (n) followed by n*16 bytes of
/// metadata text, e.g. `StreamTitle='Artist - Title';StreamUrl='';` padded with NULs.
public struct ICYParser: Sendable {
    public enum Event: Equatable, Sendable {
        case audio(Data)
        case metadata(String)
    }

    public let metaInt: Int
    private var audioLeft: Int
    private var metaLeft = 0
    private var expectingLength = false
    private var metaBuffer = Data()

    public init(metaInt: Int) {
        precondition(metaInt > 0)
        self.metaInt = metaInt
        self.audioLeft = metaInt
    }

    public mutating func feed(_ data: Data) -> [Event] {
        var events: [Event] = []
        var audio = Data()
        var i = data.startIndex
        while i < data.endIndex {
            if expectingLength {
                metaLeft = Int(data[i]) * 16
                i += 1
                expectingLength = false
                if metaLeft == 0 { audioLeft = metaInt }
            } else if metaLeft > 0 {
                let n = min(metaLeft, data.endIndex - i)
                metaBuffer.append(data[i..<(i + n)])
                i += n
                metaLeft -= n
                if metaLeft == 0 {
                    if !audio.isEmpty { events.append(.audio(audio)); audio = Data() }
                    events.append(.metadata(Self.decodeText(metaBuffer)))
                    metaBuffer.removeAll(keepingCapacity: true)
                    audioLeft = metaInt
                }
            } else {
                let n = min(audioLeft, data.endIndex - i)
                audio.append(data[i..<(i + n)])
                i += n
                audioLeft -= n
                if audioLeft == 0 { expectingLength = true }
            }
        }
        if !audio.isEmpty { events.append(.audio(audio)) }
        return events
    }

    static func decodeText(_ data: Data) -> String {
        var bytes = [UInt8](data)
        while let last = bytes.last, last == 0 { bytes.removeLast() }
        if let s = String(bytes: bytes, encoding: .utf8) { return s }
        return String(bytes: bytes, encoding: .isoLatin1) ?? ""
    }

    /// Pulls the `StreamTitle` value out of an ICY metadata block. Titles may contain apostrophes,
    /// so the value ends at the first `';` that is followed by another key or the end of the block.
    public static func streamTitle(in metadata: String) -> String? {
        guard let start = metadata.range(of: "StreamTitle='") else { return nil }
        let rest = metadata[start.upperBound...]
        var end = rest.endIndex
        var search = rest.startIndex
        while let r = rest.range(of: "';", range: search..<rest.endIndex) {
            let after = rest[r.upperBound...]
            if after.isEmpty || after.first?.isLetter == true || after.allSatisfy({ $0 == "\0" || $0 == " " }) {
                end = r.lowerBound
                break
            }
            search = r.upperBound
        }
        if end == rest.endIndex, rest.hasSuffix("'") { end = rest.index(before: rest.endIndex) }
        return String(rest[rest.startIndex..<end]).trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

/// Splits "Artist - Title" into its parts; when there is no separator the whole string is the title.
public func splitArtistTitle(_ s: String) -> (artist: String?, title: String) {
    if let r = s.range(of: " - ") {
        let artist = s[..<r.lowerBound].trimmingCharacters(in: .whitespaces)
        let title = s[r.upperBound...].trimmingCharacters(in: .whitespaces)
        if !artist.isEmpty && !title.isEmpty { return (artist, title) }
    }
    return (nil, s)
}

/// Raw stream formats the recorder can store without re-encoding.
public enum RecordableFormat: String, Sendable {
    case mp3, aac

    public init?(contentType: String?) {
        guard let ct = contentType?.lowercased().split(separator: ";").first?.trimmingCharacters(in: .whitespaces) else { return nil }
        switch ct {
        case "audio/mpeg", "audio/mp3", "audio/mpeg3", "audio/x-mpeg": self = .mp3
        case "audio/aac", "audio/aacp", "audio/x-aac", "audio/x-aacp", "audio/aac-adts": self = .aac
        default: return nil
        }
    }

    public var fileExtension: String { rawValue }
}
