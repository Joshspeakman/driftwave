import Foundation

/// Where the app keeps its files.
public struct AppPaths: Sendable {
    public let support: URL
    public let caches: URL

    public init(appFolder: String = "Shortwave") {
        let fm = FileManager.default
        support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent(appFolder)
        caches = fm.urls(for: .cachesDirectory, in: .userDomainMask)[0].appendingPathComponent(appFolder)
    }

    public init(support: URL, caches: URL) { self.support = support; self.caches = caches }

    public var library: URL { support.appendingPathComponent("library.json") }
    public var covers: URL { support.appendingPathComponent("Covers") }
    public var favicons: URL { caches.appendingPathComponent("Favicons") }
    public var recording: URL { caches.appendingPathComponent("Recording") }

    public static var defaultTrackDirectory: URL {
        FileManager.default.urls(for: .musicDirectory, in: .userDomainMask)[0].appendingPathComponent("Shortwave")
    }
}

/// The saved stations, stored as JSON so they survive the station vanishing from the database.
public struct LibraryFile: Codable, Sendable {
    public var version = 1
    public var stations: [Station]
    public init(stations: [Station]) { self.stations = stations }

    public static func load(from url: URL) -> [Station] {
        guard let data = try? Data(contentsOf: url) else { return [] }
        return (try? JSONDecoder().decode(LibraryFile.self, from: data))?.stations ?? []
    }

    public static func save(_ stations: [Station], to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        try enc.encode(LibraryFile(stations: stations)).write(to: url, options: .atomic)
    }
}
