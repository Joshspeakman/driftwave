import Foundation

/// A radio station, either from the radio-browser.info database or added locally by the user.
public struct Station: Codable, Identifiable, Hashable, Sendable {
    public var id: String
    public var name: String
    public var streamURL: String
    public var homepage: String?
    public var faviconURL: String?
    public var tags: [String]
    public var country: String
    public var countryCode: String
    public var state: String
    public var language: String
    public var codec: String
    public var bitrate: Int
    public var votes: Int
    public var isHLS: Bool
    public var latitude: Double?
    public var longitude: Double?
    /// Exists only in the user's library, not in the public database.
    public var isLocal: Bool
    /// A public station that could no longer be found in the database.
    public var isOrphaned: Bool
    /// File name of a user-chosen cover image (in the app's covers folder).
    public var customCover: String?

    public init(
        id: String, name: String, streamURL: String, homepage: String? = nil, faviconURL: String? = nil,
        tags: [String] = [], country: String = "", countryCode: String = "", state: String = "",
        language: String = "", codec: String = "", bitrate: Int = 0, votes: Int = 0, isHLS: Bool = false,
        latitude: Double? = nil, longitude: Double? = nil, isLocal: Bool = false, isOrphaned: Bool = false,
        customCover: String? = nil
    ) {
        self.id = id; self.name = name; self.streamURL = streamURL; self.homepage = homepage
        self.faviconURL = faviconURL; self.tags = tags; self.country = country; self.countryCode = countryCode
        self.state = state; self.language = language; self.codec = codec; self.bitrate = bitrate
        self.votes = votes; self.isHLS = isHLS; self.latitude = latitude; self.longitude = longitude
        self.isLocal = isLocal; self.isOrphaned = isOrphaned; self.customCover = customCover
    }

    public static func local(name: String, streamURL: String, customCover: String? = nil) -> Station {
        Station(id: "local-" + UUID().uuidString.lowercased(), name: name, streamURL: streamURL,
                isLocal: true, customCover: customCover)
    }

    public var hasLocation: Bool { latitude != nil && longitude != nil }

    /// Short human description, e.g. "MP3 · 128 kbps · Germany".
    public var subtitle: String {
        var parts: [String] = []
        if !codec.isEmpty { parts.append(codec) }
        if bitrate > 0 { parts.append("\(bitrate) kbps") }
        if !country.isEmpty { parts.append(country) }
        return parts.joined(separator: " · ")
    }

    /// Copy public metadata from a fresh database entry, keeping user-owned fields.
    public func refreshed(from fresh: Station) -> Station {
        var s = fresh
        s.customCover = customCover
        s.isOrphaned = false
        s.isLocal = false
        return s
    }
}

/// Raw station JSON as returned by radio-browser.info.
struct APIStation: Decodable {
    var stationuuid: String
    var name: String
    var url: String?
    var url_resolved: String?
    var homepage: String?
    var favicon: String?
    var tags: String?
    var country: String?
    var countrycode: String?
    var state: String?
    var language: String?
    var votes: Int?
    var codec: String?
    var bitrate: Int?
    var hls: Int?
    var lastcheckok: Int?
    var geo_lat: Double?
    var geo_long: Double?

    var station: Station {
        let resolved = (url_resolved?.isEmpty == false) ? url_resolved! : (url ?? "")
        func clean(_ s: String?) -> String { (s ?? "").trimmingCharacters(in: .whitespacesAndNewlines) }
        let tagList = clean(tags).split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        return Station(
            id: stationuuid,
            name: clean(name).isEmpty ? "Unnamed Station" : clean(name),
            streamURL: resolved,
            homepage: clean(homepage).isEmpty ? nil : clean(homepage),
            faviconURL: clean(favicon).isEmpty ? nil : clean(favicon),
            tags: tagList,
            country: clean(country),
            countryCode: clean(countrycode),
            state: clean(state),
            language: clean(language).split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces).capitalized }.joined(separator: ", "),
            codec: clean(codec).uppercased() == "UNKNOWN" ? "" : clean(codec),
            bitrate: bitrate ?? 0,
            votes: votes ?? 0,
            isHLS: (hls ?? 0) != 0,
            latitude: geo_lat,
            longitude: geo_long
        )
    }
}

public enum StationSorting: String, CaseIterable, Codable, Sendable, Identifiable {
    case name, language, country, state, codec, votes, bitrate
    public var id: String { rawValue }
    public var title: String {
        switch self {
        case .name: "Name"
        case .language: "Language"
        case .country: "Country"
        case .state: "State"
        case .codec: "Codec"
        case .votes: "Votes"
        case .bitrate: "Bitrate"
        }
    }
}

public extension Array where Element == Station {
    func sorted(by sorting: StationSorting, ascending: Bool) -> [Station] {
        func cmp(_ a: String, _ b: String) -> ComparisonResult { a.localizedStandardCompare(b) }
        let result = self.sorted { a, b in
            let primary: ComparisonResult
            switch sorting {
            case .name: primary = cmp(a.name, b.name)
            case .language: primary = cmp(a.language, b.language)
            case .country: primary = cmp(a.country, b.country)
            case .state: primary = cmp(a.state, b.state)
            case .codec: primary = cmp(a.codec, b.codec)
            case .votes: primary = a.votes == b.votes ? .orderedSame : (a.votes < b.votes ? .orderedAscending : .orderedDescending)
            case .bitrate: primary = a.bitrate == b.bitrate ? .orderedSame : (a.bitrate < b.bitrate ? .orderedAscending : .orderedDescending)
            }
            if primary == .orderedSame { return cmp(a.name, b.name) == .orderedAscending }
            return ascending ? primary == .orderedAscending : primary == .orderedDescending
        }
        return result
    }
}
