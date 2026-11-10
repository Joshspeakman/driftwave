import AppKit
import Foundation
import Observation
import ShortwaveCore
import UniformTypeIdentifiers
import UserNotifications

enum Page: String, Hashable, CaseIterable, Identifiable {
    case library, discover, search
    var id: String { rawValue }
    var title: String {
        switch self {
        case .library: "Library"
        case .discover: "Discover"
        case .search: "Search"
        }
    }
    var symbol: String {
        switch self {
        case .library: "books.vertical"
        case .discover: "sparkles"
        case .search: "magnifyingglass"
        }
    }
}

@Observable @MainActor
final class AppModel {
    let settings = Preferences()
    let paths = AppPaths()
    let api = RadioBrowser()
    let images: ImageStore
    let player: Player

    var page: Page = .library
    private(set) var library: [Station] = []
    var showPlayer = true

    // Sheets
    var detailStation: Station?
    var detailTrack: Track?
    var showingAddStation = false
    var editingStation: Station?

    // Discover
    var popular: [Station] = []
    var trending: [Station] = []
    var randomStations: [Station] = []
    var discoverLoading = false
    var discoverError: String?

    // Search
    var search = RadioBrowser.Search()
    var searchResults: [Station] = []
    var searchLoading = false
    var searchError: String?
    var countries: [RadioBrowser.Facet] = []
    var languages: [RadioBrowser.Facet] = []
    var tags: [RadioBrowser.Facet] = []
    @ObservationIgnored private var searchTask: Task<Void, Never>?

    init() {
        images = ImageStore(paths: paths)
        player = Player(settings: settings, paths: paths, api: api)
        library = LibraryFile.load(from: paths.library)
        player.artworkProvider = { [images] station in images.cached(for: station) }
        player.onShowTrack = { [weak self] t in if self?.detailTrack == nil { self?.detailTrack = t } }
        player.stationNeighbours = { [weak self] station, delta in self?.neighbour(of: station, delta: delta) }
        if let id = settings.lastStationID, let s = library.first(where: { $0.id == id }) {
            player.restore(s)
            Task { await images.load(for: s); player.updateNowPlaying() }
        }
        Task { await refreshLibrary() }
        if Diagnostics.startMuted { player.isMuted = true }
        if let p = Diagnostics.startPage.flatMap(Page.init(rawValue:)) { page = p }
        if let q = Diagnostics.searchText { search.name = q; searchChanged() }
        if let url = Diagnostics.playURL {
            player.select(Station.local(name: "Test Stream", streamURL: url))
        }
    }

    // MARK: Library

    var sortedLibrary: [Station] { library.sorted(by: settings.librarySorting, ascending: settings.libraryAscending) }

    func isInLibrary(_ s: Station) -> Bool { library.contains { $0.id == s.id } }

    func libraryVersion(of s: Station) -> Station { library.first { $0.id == s.id } ?? s }

    func addToLibrary(_ s: Station) {
        guard !isInLibrary(s) else { return }
        library.append(s)
        saveLibrary()
    }

    func removeFromLibrary(_ s: Station) {
        if let cover = library.first(where: { $0.id == s.id })?.customCover {
            try? FileManager.default.removeItem(at: paths.covers.appendingPathComponent(cover))
        }
        library.removeAll { $0.id == s.id }
        saveLibrary()
    }

    func update(_ s: Station) {
        if let i = library.firstIndex(where: { $0.id == s.id }) { library[i] = s; saveLibrary() }
        if detailStation?.id == s.id { detailStation = s }
        images.invalidate(s)
    }

    func toggleLibrary(_ s: Station) {
        if isInLibrary(s) { removeFromLibrary(s) } else { addToLibrary(s) }
    }

    /// Copies an image into the covers folder and uses it as the station's cover.
    func setCover(_ s: Station, from url: URL?) {
        var station = libraryVersion(of: s)
        if let old = station.customCover { try? FileManager.default.removeItem(at: paths.covers.appendingPathComponent(old)) }
        station.customCover = nil
        if let url, let image = NSImage(contentsOf: url), let png = ImageStore.pngData(image, maxSide: 512) {
            try? FileManager.default.createDirectory(at: paths.covers, withIntermediateDirectories: true)
            let name = UUID().uuidString + ".png"
            if (try? png.write(to: paths.covers.appendingPathComponent(name))) != nil { station.customCover = name }
        }
        update(station)
    }

    private func saveLibrary() {
        try? LibraryFile.save(library, to: paths.library)
    }

    /// Refresh public stations from the database; mark those it no longer knows as orphaned.
    func refreshLibrary() async {
        let ids = library.filter { !$0.isLocal }.map(\.id)
        guard !ids.isEmpty, let fresh = try? await api.stations(byUUIDs: ids) else { return }
        let byID = Dictionary(fresh.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        var changed = false
        library = library.map { s in
            guard !s.isLocal else { return s }
            if let f = byID[s.id] {
                let r = s.refreshed(from: f)
                if r != s { changed = true }
                return r
            }
            if !s.isOrphaned { changed = true }
            var o = s; o.isOrphaned = true; return o
        }
        if changed { saveLibrary() }
    }

    private func neighbour(of station: Station, delta: Int) -> Station? {
        let list = sortedLibrary
        guard !list.isEmpty else { return nil }
        guard let i = list.firstIndex(where: { $0.id == station.id }) else { return list.first }
        return list[(i + delta + list.count) % list.count]
    }

    // MARK: Playback

    func play(_ s: Station) {
        player.select(libraryVersion(of: s))
        Task { await images.load(for: s); player.updateNowPlaying() }
    }

    // MARK: Discover

    func loadDiscover(force: Bool = false) async {
        guard force || popular.isEmpty, !discoverLoading else { return }
        discoverLoading = true
        discoverError = nil
        defer { discoverLoading = false }
        let cc = settings.discoverCountry
        do {
            async let p = api.popular(countryCode: cc)
            async let t = api.trending()
            async let r = api.random(countryCode: cc)
            (popular, trending, randomStations) = try await (p, t, r)
        } catch {
            discoverError = error.localizedDescription
        }
    }

    // MARK: Search

    func loadFacets() async {
        guard countries.isEmpty else { return }
        async let c = try? api.countries()
        async let l = try? api.languages()
        async let t = try? api.tags()
        countries = await c ?? []
        languages = await l ?? []
        tags = await t ?? []
    }

    func searchChanged() {
        searchTask?.cancel()
        let query = search
        guard !query.isEmpty else {
            searchResults = []
            searchLoading = false
            searchError = nil
            return
        }
        searchLoading = true
        searchTask = Task {
            try? await Task.sleep(for: .milliseconds(350))
            guard !Task.isCancelled else { return }
            do {
                let results = try await api.search(query)
                guard !Task.isCancelled else { return }
                searchResults = results
                searchError = nil
                if Diagnostics.openFirstResult, detailStation == nil { detailStation = results.first }
            } catch {
                guard !Task.isCancelled else { return }
                searchError = error.localizedDescription
            }
            searchLoading = false
        }
    }

    // MARK: Settings side effects

    func setNotifications(_ on: Bool) {
        settings.notifications = on
        guard on else { return }
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { granted, _ in
            if !granted { DispatchQueue.main.async { MainActor.assumeIsolated { self.settings.notifications = false } } }
        }
    }

    func chooseTrackDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose"
        panel.directoryURL = settings.trackDirectory
        if panel.runModal() == .OK, let url = panel.url { settings.trackDirectory = url }
    }

    func chooseImage() -> URL? {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        panel.canChooseDirectories = false
        return panel.runModal() == .OK ? panel.url : nil
    }
}
