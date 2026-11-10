import Foundation

/// Client for the community station database at radio-browser.info.
public actor RadioBrowser {
    public static let userAgent = "ShortwaveMac/1.0"
    public static let lookupHost = "all.api.radio-browser.info"
    static let fallbackServers = ["de1.api.radio-browser.info", "de2.api.radio-browser.info", "fi1.api.radio-browser.info"]

    public enum Failure: LocalizedError {
        case noServer
        case http(Int)
        public var errorDescription: String? {
            switch self {
            case .noServer: "Unable to reach the radio-browser.info station database."
            case .http(let code): "The station database answered with HTTP \(code)."
            }
        }
    }

    private let session: URLSession
    private var servers: [String] = []

    public init(session: URLSession? = nil) {
        if let session { self.session = session } else {
            let config = URLSessionConfiguration.default
            config.timeoutIntervalForRequest = 15
            config.httpAdditionalHeaders = ["User-Agent": RadioBrowser.userAgent]
            self.session = URLSession(configuration: config)
        }
    }

    // MARK: Queries

    public struct Search: Sendable, Equatable {
        public var name = ""
        public var tag = ""
        public var countryCode = ""
        public var language = ""
        public var order: Order = .votes
        public var limit = 150
        public init() {}
        public var isEmpty: Bool { name.isEmpty && tag.isEmpty && countryCode.isEmpty && language.isEmpty }

        public enum Order: String, CaseIterable, Sendable, Identifiable {
            case votes, clickcount, name, bitrate, random
            public var id: String { rawValue }
            public var title: String {
                switch self {
                case .votes: "Most Voted"
                case .clickcount: "Most Played"
                case .name: "Name"
                case .bitrate: "Bitrate"
                case .random: "Random"
                }
            }
            var reverse: Bool { self != .name }
        }
    }

    public func search(_ s: Search) async throws -> [Station] {
        var q: [URLQueryItem] = [
            .init(name: "order", value: s.order.rawValue),
            .init(name: "reverse", value: s.order.reverse ? "true" : "false"),
            .init(name: "limit", value: String(s.limit)),
            .init(name: "hidebroken", value: "true"),
        ]
        if !s.name.isEmpty { q.append(.init(name: "name", value: s.name)) }
        if !s.tag.isEmpty { q.append(.init(name: "tag", value: s.tag)) }
        if !s.countryCode.isEmpty { q.append(.init(name: "countrycode", value: s.countryCode)) }
        if !s.language.isEmpty { q.append(.init(name: "language", value: s.language)) }
        return try await stations("/json/stations/search", q)
    }

    /// Popular stations for a country, with vote-botted entries removed, shuffled down to `count`.
    public func popular(countryCode: String, count: Int = 12) async throws -> [Station] {
        let all = try await stations("/json/stations/search", [
            .init(name: "countrycode", value: countryCode),
            .init(name: "order", value: "votes"),
            .init(name: "reverse", value: "true"),
            .init(name: "limit", value: "100"),
            .init(name: "hidebroken", value: "true"),
        ])
        // Anything with more than 50k votes is almost certainly botted.
        return Array(all.filter { $0.votes < 50_000 }.shuffled().prefix(count))
    }

    public func random(countryCode: String, count: Int = 18) async throws -> [Station] {
        try await stations("/json/stations/search", [
            .init(name: "countrycode", value: countryCode),
            .init(name: "order", value: "random"),
            .init(name: "limit", value: String(count)),
            .init(name: "hidebroken", value: "true"),
        ])
    }

    public func trending(count: Int = 18) async throws -> [Station] {
        let all = try await stations("/json/stations/search", [
            .init(name: "order", value: "clicktrend"),
            .init(name: "reverse", value: "true"),
            .init(name: "limit", value: "60"),
            .init(name: "hidebroken", value: "true"),
        ])
        return Array(all.prefix(count))
    }

    public func stations(byUUIDs uuids: [String]) async throws -> [Station] {
        var result: [Station] = []
        var start = 0
        while start < uuids.count {
            let chunk = uuids[start..<min(start + 50, uuids.count)]
            result += try await stations("/json/stations/byuuid", [.init(name: "uuids", value: chunk.joined(separator: ","))])
            start += 50
        }
        return result
    }

    public struct Facet: Decodable, Hashable, Sendable, Identifiable {
        public var name: String
        public var stationcount: Int
        public var iso_3166_1: String?
        public var id: String { iso_3166_1 ?? name }
    }

    public func countries() async throws -> [Facet] {
        try await get("/json/countries", [.init(name: "hidebroken", value: "true")], as: [Facet].self)
            .filter { $0.stationcount > 0 && ($0.iso_3166_1?.isEmpty == false) }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    public func languages() async throws -> [Facet] {
        try await get("/json/languages", [.init(name: "hidebroken", value: "true"), .init(name: "order", value: "stationcount"), .init(name: "reverse", value: "true")], as: [Facet].self)
            .filter { $0.stationcount >= 5 && !$0.name.isEmpty }
    }

    public func tags() async throws -> [Facet] {
        try await get("/json/tags", [.init(name: "hidebroken", value: "true"), .init(name: "order", value: "stationcount"), .init(name: "reverse", value: "true"), .init(name: "limit", value: "300")], as: [Facet].self)
            .filter { !$0.name.isEmpty }
    }

    /// Tell the database a station was played (it uses this for its popularity statistics).
    public func countClick(stationID: String) async {
        _ = try? await data("/json/url/\(stationID)", [])
    }

    // MARK: Transport

    private func stations(_ path: String, _ query: [URLQueryItem]) async throws -> [Station] {
        try await get(path, query, as: [APIStation].self).map(\.station).filter { !$0.streamURL.isEmpty }
    }

    private func get<T: Decodable>(_ path: String, _ query: [URLQueryItem], as: T.Type) async throws -> T {
        try JSONDecoder().decode(T.self, from: try await data(path, query))
    }

    private func data(_ path: String, _ query: [URLQueryItem]) async throws -> Data {
        if servers.isEmpty { servers = await discoverServers() }
        var lastError: Error = Failure.noServer
        for (index, host) in servers.enumerated() {
            var c = URLComponents()
            c.scheme = "https"; c.host = host; c.path = path
            if !query.isEmpty { c.queryItems = query }
            do {
                let (data, response) = try await session.data(from: c.url!)
                if let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) {
                    throw Failure.http(http.statusCode)
                }
                if index > 0 { servers.insert(servers.remove(at: index), at: 0) }
                return data
            } catch {
                if error is CancellationError || (error as? URLError)?.code == .cancelled { throw error }
                lastError = error
            }
        }
        throw lastError
    }

    private func discoverServers() async -> [String] {
        struct Server: Decodable { var name: String }
        if let url = URL(string: "https://\(Self.lookupHost)/json/servers"),
           let (data, _) = try? await session.data(from: url),
           let list = try? JSONDecoder().decode([Server].self, from: data) {
            let names = Array(Set(list.map(\.name))).shuffled()
            if !names.isEmpty { return names + Self.fallbackServers.filter { !names.contains($0) } }
        }
        return Self.fallbackServers
    }
}
