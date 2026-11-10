import ShortwaveCore
import SwiftUI

struct LibraryPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        Group {
            if app.library.isEmpty {
                ContentUnavailableView {
                    Label("Your Library Is Empty", systemImage: "dot.radiowaves.left.and.right")
                } description: {
                    Text("Discover over 50,000 public stations and add the ones you like.")
                } actions: {
                    Button("Discover New Stations") { app.page = .discover }.buttonStyle(.borderedProminent)
                    Button("Add Local Station") { app.showingAddStation = true }
                }
            } else {
                ScrollView {
                    StationGrid(stations: app.sortedLibrary).padding(16)
                }
            }
        }
        .navigationTitle("Library")
        .toolbar {
            ToolbarItem {
                SortMenu()
            }
        }
    }
}

struct SortMenu: View {
    @Environment(AppModel.self) private var app
    var body: some View {
        @Bindable var settings = app.settings
        Menu {
            Picker("Sort By", selection: $settings.librarySorting) {
                ForEach(StationSorting.allCases) { Text($0.title).tag($0) }
            }
            .pickerStyle(.inline)
            Picker("Order", selection: $settings.libraryAscending) {
                Text("Ascending").tag(true)
                Text("Descending").tag(false)
            }
            .pickerStyle(.inline)
        } label: {
            Label("Sort", systemImage: "arrow.up.arrow.down")
        }
        .help("Sort the library")
    }
}

struct DiscoverPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if let error = app.discoverError, app.popular.isEmpty {
                    ContentUnavailableView {
                        Label("Unable to Retrieve Station Data", systemImage: "wifi.exclamationmark")
                    } description: {
                        Text(error)
                    } actions: {
                        Button("Try Again") { Task { await app.loadDiscover(force: true) } }
                    }
                    .frame(maxWidth: .infinity, minHeight: 300)
                } else if app.popular.isEmpty && app.discoverLoading {
                    ProgressView("Loading…").frame(maxWidth: .infinity, minHeight: 300)
                } else {
                    section("Popular in \(countryName)", app.popular)
                    section("Trending Worldwide", app.trending)
                    section("Random Stations", app.randomStations)
                    missingStationNote
                }
            }
            .padding(16)
        }
        .navigationTitle("Discover")
        .toolbar {
            ToolbarItem {
                CountryMenu()
            }
            ToolbarItem {
                Button {
                    Task { await app.loadDiscover(force: true) }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .help("Show other stations")
                .disabled(app.discoverLoading)
            }
        }
        .task { await app.loadDiscover() }
        .task { await app.loadFacets() }
    }

    private var countryName: String {
        Locale.current.localizedString(forRegionCode: app.settings.discoverCountry) ?? app.settings.discoverCountry
    }

    @ViewBuilder private func section(_ title: String, _ stations: [Station]) -> some View {
        if !stations.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text(title).font(.title2.bold()).padding(.leading, 8)
                StationGrid(stations: stations)
            }
        }
    }

    private var missingStationNote: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Add Missing Station").font(.headline)
            Text("If the station you are looking for is missing, you can add it to the public database, or add it just for yourself as a local station.")
                .foregroundStyle(.secondary)
            HStack {
                Link("Add Public Station", destination: URL(string: "https://www.radio-browser.info/add")!)
                Button("Add Local Station") { app.showingAddStation = true }
            }
        }
        .padding(.horizontal, 8)
    }
}

struct CountryMenu: View {
    @Environment(AppModel.self) private var app
    var body: some View {
        Menu {
            ForEach(app.countries) { c in
                Button("\(c.name) (\(c.stationcount))") {
                    app.settings.discoverCountry = c.iso_3166_1 ?? c.name
                    Task { await app.loadDiscover(force: true) }
                }
            }
        } label: {
            Label("Country", systemImage: "globe")
        }
        .help("Choose the country for popular and random stations")
        .disabled(app.countries.isEmpty)
    }
}

struct SearchPage: View {
    @Environment(AppModel.self) private var app
    @FocusState private var searchFocused: Bool

    var body: some View {
        @Bindable var app = app
        VStack(spacing: 0) {
            filterBar
            Divider()
            Group {
                if app.search.isEmpty {
                    ContentUnavailableView("Search Stations", systemImage: "magnifyingglass",
                                           description: Text("Search by name, or pick a tag, country or language."))
                } else if let error = app.searchError {
                    ContentUnavailableView {
                        Label("Unable to Retrieve Station Data", systemImage: "wifi.exclamationmark")
                    } description: {
                        Text(error)
                    } actions: {
                        Button("Try Again") { app.searchChanged() }
                    }
                } else if app.searchResults.isEmpty && app.searchLoading {
                    ProgressView("Searching…").frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if app.searchResults.isEmpty {
                    ContentUnavailableView.search(text: app.search.name)
                } else {
                    ScrollView {
                        StationGrid(stations: app.searchResults).padding(16)
                    }
                    .overlay(alignment: .top) {
                        if app.searchLoading { ProgressView().controlSize(.small).padding(6) }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .navigationTitle("Search")
        .searchable(text: $app.search.name, placement: .toolbar, prompt: "Search stations")
        .searchFocused($searchFocused)
        .onAppear { searchFocused = true }
        .onChange(of: app.search) { app.searchChanged() }
        .task { await app.loadFacets() }
    }

    private var filterBar: some View {
        @Bindable var app = app
        return ScrollView(.horizontal, showsIndicators: false) {
          HStack(spacing: 10) {
            Picker("Sort", selection: $app.search.order) {
                ForEach(RadioBrowser.Search.Order.allCases) { Text($0.title).tag($0) }
            }
            .frame(width: 140)
            .help("Sort results")
            Picker("Country", selection: $app.search.countryCode) {
                Text("Any Country").tag("")
                Divider()
                ForEach(app.countries) { Text($0.name).tag($0.iso_3166_1 ?? "") }
            }
            .frame(width: 170)
            Picker("Language", selection: $app.search.language) {
                Text("Any Language").tag("")
                Divider()
                ForEach(app.languages) { Text($0.name.capitalized).tag($0.name) }
            }
            .frame(width: 150)
            Picker("Tag", selection: $app.search.tag) {
                Text("Any Tag").tag("")
                Divider()
                ForEach(app.tags) { Text($0.name).tag($0.name) }
            }
            .frame(width: 140)
            if app.search != RadioBrowser.Search() {
                Button("Clear") { app.search = RadioBrowser.Search() }
                    .buttonStyle(.borderless)
            }
            if !app.searchResults.isEmpty {
                Text("\(app.searchResults.count) stations").foregroundStyle(.secondary).font(.callout).lineLimit(1)
                    .fixedSize()
            }
          }
          .labelsHidden()
          .padding(.horizontal, 16)
          .padding(.vertical, 8)
        }
    }
}
