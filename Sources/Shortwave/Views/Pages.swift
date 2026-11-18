import ShortwaveCore
import SwiftUI

struct LibraryPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(spacing: 0) {
            PageHeader(title: "Library") {
                if !app.library.isEmpty { SortMenu() }
                LibraryMoreMenu()
            }
            if app.library.isEmpty {
                EmptyState(symbol: "dot.radiowaves.left.and.right", title: "Your Library Is Empty",
                           text: "Discover over 50,000 public stations and add the ones you like.", iconSize: 48) {
                    Button("Discover New Stations") { app.page = .discover }.buttonStyle(.borderedProminent)
                    Button("Add Local Station") { app.showingAddStation = true }
                    Button("Import Stations…") { app.chooseImportFile() }
                }
            } else {
                ScrollView {
                    StationGrid(stations: app.sortedLibrary)
                        .padding(EdgeInsets(top: 4, leading: 16, bottom: 16, trailing: 16))
                }
            }
        }
    }
}

/// Import and export of saved stations (docs/STATION-EXCHANGE.md).
struct LibraryMoreMenu: View {
    @Environment(AppModel.self) private var app
    var body: some View {
        Menu {
            Button("Import Stations…") { app.chooseImportFile() }
            Button("Export Stations…") { app.chooseExportFile() }.disabled(app.library.isEmpty)
        } label: {
            Image(systemName: "ellipsis")
        }
        .menuIndicator(.hidden)
        .fixedSize()
        .help("Import or export your saved stations")
        .accessibilityLabel("More")
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
            Label(settings.librarySorting.title, systemImage: "arrow.up.arrow.down")
                .labelStyle(.titleAndIcon)
        }
        .fixedSize()
        .help("Sort the library")
        .accessibilityLabel("Sort")
    }
}

struct DiscoverPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(spacing: 0) {
            PageHeader(title: "Discover") {
                CountryMenu()
                Button {
                    Task { await app.loadDiscover(force: true) }
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Show other stations")
                .accessibilityLabel("Refresh")
                .disabled(app.discoverLoading)
            }
            if let error = app.discoverError, app.popular.isEmpty {
                EmptyState(symbol: "wifi.exclamationmark", title: "Unable to Retrieve Station Data", text: error) {
                    Button("Try Again") { Task { await app.loadDiscover(force: true) } }
                }
            } else if app.popular.isEmpty && app.discoverLoading {
                VStack(spacing: 8) {
                    ProgressView()
                    Text("Loading…").foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 28) {
                        section("Popular in \(countryName)", app.popular)
                        section("Trending Worldwide", app.trending)
                        section("Random Stations", app.randomStations)
                        missingStationNote
                    }
                    .padding(EdgeInsets(top: 4, leading: 16, bottom: 16, trailing: 16))
                }
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
                Text(title).font(Fluent.subtitleFont).padding(.leading, 8)
                StationGrid(stations: stations)
            }
        }
    }

    private var missingStationNote: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Add Missing Station").fontWeight(.semibold)
            Text("If the station you are looking for is missing, you can add it to the public database, or add it just for yourself as a local station.")
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 8) {
                Link("Add Public Station", destination: URL(string: "https://www.radio-browser.info/add")!)
                Button("Add Local Station") { app.showingAddStation = true }
            }
        }
        .padding(.horizontal, 8)
    }
}

/// The country for Discover: a button showing the current one, opening a filterable list.
struct CountryMenu: View {
    @Environment(AppModel.self) private var app
    @ViewState private var open = false
    @ViewState private var filter = ""

    var body: some View {
        Button { open.toggle() } label: {
            HStack(spacing: 6) {
                Image(systemName: "globe")
                Text(Locale.current.localizedString(forRegionCode: app.settings.discoverCountry) ?? app.settings.discoverCountry)
                Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold)).foregroundStyle(.secondary)
            }
        }
        .fixedSize()
        .help("Choose the country for popular and random stations")
        .accessibilityLabel("Country")
        .disabled(app.countries.isEmpty)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            VStack(spacing: 8) {
                TextField("Filter countries", text: $filter).textFieldStyle(.roundedBorder)
                List(filtered) { c in
                    Button {
                        open = false
                        filter = ""
                        app.settings.discoverCountry = c.iso_3166_1 ?? c.name
                        Task { await app.loadDiscover(force: true) }
                    } label: {
                        HStack {
                            Text(c.name)
                            Spacer()
                            Text("\(c.stationcount)").foregroundStyle(.secondary).monospacedDigit()
                            if c.iso_3166_1 == app.settings.discoverCountry {
                                Image(systemName: "checkmark").foregroundStyle(.tint)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                .listStyle(.plain)
            }
            .padding(12)
            .frame(width: 280, height: 420)
        }
    }

    private var filtered: [RadioBrowser.Facet] {
        let f = filter.trimmingCharacters(in: .whitespaces)
        return f.isEmpty ? app.countries : app.countries.filter { $0.name.localizedCaseInsensitiveContains(f) }
    }
}

struct SearchPage: View {
    @Environment(AppModel.self) private var app
    @FocusState private var searchFocused: Bool

    var body: some View {
        @Bindable var app = app
        VStack(spacing: 0) {
            PageHeader(title: "Search") {
                HStack(spacing: 6) {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("Search stations", text: $app.search.name)
                        .textFieldStyle(.plain)
                        .focused($searchFocused)
                    if !app.search.name.isEmpty {
                        Button { app.search.name = "" } label: { Image(systemName: "xmark.circle.fill") }
                            .buttonStyle(.plain).foregroundStyle(.secondary)
                            .help("Clear the search")
                    }
                }
                .padding(.horizontal, 10)
                .frame(width: 360, height: 32)
                .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(Fluent.card))
                .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous)
                    .strokeBorder(searchFocused ? AnyShapeStyle(.tint) : AnyShapeStyle(Fluent.cardStroke), lineWidth: 1))
                .accessibilityLabel("Search stations")
            }
            filterBar
            ZStack(alignment: .top) {
                if app.search.isEmpty {
                    EmptyState(symbol: "magnifyingglass", title: "Search Stations",
                               text: "Search by name, or pick a tag, country or language.")
                } else if let error = app.searchError {
                    EmptyState(symbol: "wifi.exclamationmark", title: "Unable to Retrieve Station Data", text: error) {
                        Button("Try Again") { app.searchChanged() }
                    }
                } else if app.searchResults.isEmpty && app.searchLoading {
                    VStack(spacing: 8) {
                        ProgressView()
                        Text("Searching…").foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if app.searchResults.isEmpty {
                    EmptyState(symbol: "magnifyingglass",
                               title: app.search.name.isEmpty ? "No Results" : "No Results for “\(app.search.name)”",
                               text: "Check the spelling or try a new search.")
                } else {
                    ScrollView {
                        StationGrid(stations: app.searchResults)
                            .padding(EdgeInsets(top: 4, leading: 16, bottom: 16, trailing: 16))
                    }
                    if app.searchLoading {
                        ProgressView().progressViewStyle(.linear).padding(.horizontal, 16)
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .onAppear { searchFocused = true }
        .onChange(of: app.searchFocusRequest) { searchFocused = true }
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
            .frame(width: 150)
            .help("Sort results")
            Picker("Country", selection: $app.search.countryCode) {
                Text("Any Country").tag("")
                Divider()
                ForEach(app.countries) { Text($0.name).tag($0.iso_3166_1 ?? "") }
            }
            .frame(width: 190)
            Picker("Language", selection: $app.search.language) {
                Text("Any Language").tag("")
                Divider()
                ForEach(app.languages) { Text($0.name.capitalized).tag($0.name) }
            }
            .frame(width: 170)
            Picker("Tag", selection: $app.search.tag) {
                Text("Any Tag").tag("")
                Divider()
                ForEach(app.tags) { Text($0.name).tag($0.name) }
            }
            .frame(width: 160)
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
          .padding(EdgeInsets(top: 0, leading: 24, bottom: 8, trailing: 16))
        }
    }
}
