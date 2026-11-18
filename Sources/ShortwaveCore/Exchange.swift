import Foundation

/// Moving saved stations between the Mac and Windows apps (and between machines).
///
/// The file format is described in docs/STATION-EXCHANGE.md and is shared with the Windows app's
/// `StationExchange` class. An exchange file is also a valid `library.json`, and a bare
/// `library.json` can be imported.
public enum StationExchange {
    public static let format = "shortwave-stations"
    public static let version = 1
    /// Suggested file name for an export.
    public static let defaultFileName = "Shortwave Stations.json"

    public enum ReadError: LocalizedError, Equatable {
        case notJSON
        case noStations
        public var errorDescription: String? {
            switch self {
            case .notJSON: "This file isn’t a Shortwave station file."
            case .noStations: "This file doesn’t contain a list of stations."
            }
        }
    }

    /// What an exchange file holds.
    public struct Contents: Sendable, Equatable {
        public var stations: [Station]
        /// Cover images by the `customCover` file name the stations use.
        public var covers: [String: Data]
        /// Entries in the station list that couldn't be read.
        public var unreadable: Int
        public init(stations: [Station], covers: [String: Data] = [:], unreadable: Int = 0) {
            self.stations = stations; self.covers = covers; self.unreadable = unreadable
        }
    }

    private struct FileOut: Encodable {
        var app: String
        var covers: [String: String]
        var exported: String
        var format: String
        var stations: [Station]
        var version: Int
    }

    /// Writes an exchange file. `cover` returns the image data for a `customCover` file name.
    public static func encode(_ stations: [Station], app: String, date: Date = Date(), cover: (String) -> Data?) throws -> Data {
        var covers: [String: String] = [:]
        for s in stations {
            if let name = s.customCover, covers[name] == nil, let data = cover(name), isImage(data) {
                covers[name] = data.base64EncodedString()
            }
        }
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime]
        let file = FileOut(app: app, covers: covers, exported: iso.string(from: date), format: format, stations: stations, version: version)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try enc.encode(file)
    }

    private struct Lossy: Decodable {
        var station: Station?
        init(from decoder: Decoder) throws { station = try? Station(from: decoder) }
    }

    /// Reads an exchange file, a `library.json`, or a bare JSON array of stations.
    public static func decode(_ data: Data) throws -> Contents {
        guard let root = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) else { throw ReadError.notJSON }
        let list: Any
        var covers: [String: Data] = [:]
        if let array = root as? [Any] {
            list = array
        } else if let object = root as? [String: Any], let stations = object["stations"] as? [Any] {
            list = stations
            for (name, value) in object["covers"] as? [String: Any] ?? [:] {
                if let b64 = value as? String, let d = Data(base64Encoded: b64, options: .ignoreUnknownCharacters), isImage(d) {
                    covers[name] = d
                }
            }
        } else {
            throw ReadError.noStations
        }
        let listData = try JSONSerialization.data(withJSONObject: list)
        let items = (try? JSONDecoder().decode([Lossy].self, from: listData)) ?? []
        let stations = items.compactMap(\.station).filter { !$0.id.isEmpty && !$0.streamURL.trimmingCharacters(in: .whitespaces).isEmpty }
        return Contents(stations: stations, covers: covers, unreadable: (list as? [Any])?.count.advanced(by: -stations.count) ?? 0)
    }

    public struct MergeResult: Sendable, Equatable {
        /// The library with the new stations appended.
        public var library: [Station]
        public var added: [Station]
        /// Stations skipped because the library already has them.
        public var alreadyPresent: Int
        public var unreadable: Int
        /// Cover images to write to the covers folder, by their new file names.
        public var newCovers: [String: Data]
    }

    /// Adds the stations the library doesn't have yet. Never changes or removes existing ones.
    ///
    /// A station is already present when its `id` matches, or, for local stations, when a local
    /// station with the same stream address exists. Imported covers get new file names from
    /// `newCoverName` (without extension: `.png` or `.jpg` is added); a station whose cover image isn't in the file loses its `customCover`.
    public static func merge(_ contents: Contents, into library: [Station], newCoverName: () -> String) -> MergeResult {
        var result = MergeResult(library: library, added: [], alreadyPresent: 0, unreadable: contents.unreadable, newCovers: [:])
        func key(_ url: String) -> String { url.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
        var ids = Set(library.map(\.id))
        var localURLs = Set(library.filter(\.isLocal).map { key($0.streamURL) })
        for var s in contents.stations {
            if ids.contains(s.id) || (s.isLocal && localURLs.contains(key(s.streamURL))) {
                result.alreadyPresent += 1
                continue
            }
            if let name = s.customCover {
                if let data = contents.covers[name] {
                    let isPNG = data.starts(with: [0x89, 0x50, 0x4E, 0x47])
                    let fresh = newCoverName() + (isPNG ? ".png" : ".jpg")
                    result.newCovers[fresh] = data
                    s.customCover = fresh
                } else {
                    s.customCover = nil
                }
            }
            ids.insert(s.id)
            if s.isLocal { localURLs.insert(key(s.streamURL)) }
            result.library.append(s)
            result.added.append(s)
        }
        return result
    }

    /// PNG or JPEG, by their signatures.
    static func isImage(_ d: Data) -> Bool {
        let b = [UInt8](d.prefix(8))
        return b.starts(with: [0x89, 0x50, 0x4E, 0x47]) || b.starts(with: [0xFF, 0xD8, 0xFF])
    }
}
