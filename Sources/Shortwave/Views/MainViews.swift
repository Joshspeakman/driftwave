import ShortwaveCore
import SwiftUI

struct MainView: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var app = app
        NavigationSplitView {
            List(Page.allCases, selection: Binding(get: { app.page }, set: { if let p = $0 { app.page = p } })) { page in
                Label(page.title, systemImage: page.symbol).tag(page)
            }
            .navigationSplitViewColumnWidth(min: 160, ideal: 180, max: 240)
        } detail: {
            // The player is a plain side panel, not .inspector: inside this split view the inspector
            // drew its content a toolbar-height lower than where it took clicks, so the volume
            // slider and buttons could not be used.
            HStack(spacing: 0) {
                Group {
                    switch app.page {
                    case .library: LibraryPage()
                    case .discover: DiscoverPage()
                    case .search: SearchPage()
                    }
                }
                .frame(minWidth: 420, maxWidth: .infinity, maxHeight: .infinity)
                .safeAreaInset(edge: .bottom, spacing: 0) {
                    if !app.showPlayer && app.player.station != nil {
                        VStack(spacing: 0) {
                            Divider()
                            MiniPlayer().background(.bar)
                        }
                    }
                }
                if app.showPlayer {
                    Divider()
                    PlayerPanel()
                        .frame(width: 300)
                        .frame(maxHeight: .infinity)
                        .transition(.move(edge: .trailing))
                }
            }
            .animation(.snappy(duration: 0.2), value: app.showPlayer)
            .toolbar {
                ToolbarItem {
                    Button { app.showingAddStation = true } label: {
                        Label("Add Station", systemImage: "plus")
                    }
                    .help("Add a local station")
                }
                ToolbarItem {
                    Button { app.showPlayer.toggle() } label: {
                        Label("Player", systemImage: "sidebar.trailing")
                    }
                    .help(app.showPlayer ? "Hide Player" : "Show Player")
                }
            }
        }
        .frame(minWidth: 760, minHeight: 480)
        .sheet(item: $app.detailStation) { StationDetailSheet(station: $0) }
        .sheet(item: $app.detailTrack) { TrackDetailSheet(track: $0) }
        .sheet(isPresented: $app.showingAddStation) { LocalStationSheet() }
        .sheet(item: $app.editingStation) { LocalStationSheet(editing: $0) }
        .alert(app.notice?.title ?? "", isPresented: Binding(get: { app.notice != nil }, set: { if !$0 { app.notice = nil } }),
               presenting: app.notice) { _ in
            Button("OK") { app.notice = nil }
        } message: { Text($0.message) }
        .onDisappear {
            // Closing the window stops playback unless background playback is on.
            if !app.settings.backgroundPlayback { app.player.stopPlayback(reason: "window closed") }
        }
    }
}

struct SettingsView: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var settings = app.settings
        TabView {
            Form {
                Section("Playback") {
                    Toggle(isOn: $settings.backgroundPlayback) {
                        Text("Background Playback")
                        Text("Playback continues when the window is closed.")
                    }
                    Toggle(isOn: Binding(get: { settings.notifications }, set: { app.setNotifications($0) })) {
                        Text("Notifications")
                        Text("Show a notification when a new track gets played.")
                    }
                    Stepper(value: $settings.pastTracksCount, in: 1...50) {
                        LabeledContent("Played tracks to keep", value: "\(settings.pastTracksCount)")
                    }
                }
            }
            .formStyle(.grouped)
            .tabItem { Label("General", systemImage: "gearshape") }

            Form {
                Section {
                    Picker("Recording", selection: $settings.recordingMode) {
                        ForEach(RecordingMode.allCases) { Text($0.title).tag($0) }
                    }
                    .pickerStyle(.radioGroup)
                    Text(settings.recordingMode.explanation).foregroundStyle(.secondary)
                } footer: {
                    Text("Tracks are recognised from the station’s stream metadata. MP3 and AAC streams can be recorded; HLS and Ogg streams play without recording.")
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                Section {
                    LabeledContent("Track Folder") {
                        HStack {
                            Text(settings.trackDirectory.path(percentEncoded: false)).lineLimit(1).truncationMode(.middle)
                            Button("Choose…") { app.chooseTrackDirectory() }
                            Button { NSWorkspace.shared.open(settings.trackDirectory) } label: { Image(systemName: "folder") }
                                .help("Show in Finder")
                                .disabled(!FileManager.default.fileExists(atPath: settings.trackDirectory.path))
                        }
                    }
                    Stepper(value: $settings.minimumDuration, in: 0...600, step: 5) {
                        LabeledContent("Minimum Duration", value: Duration.seconds(settings.minimumDuration).formatted(.units(allowed: [.minutes, .seconds], width: .abbreviated)))
                    }
                    Text("Tracks shorter than the minimum duration are ignored.").font(.caption).foregroundStyle(.secondary)
                    Stepper(value: $settings.maximumDuration, in: 60...7200, step: 60) {
                        LabeledContent("Maximum Duration", value: Duration.seconds(settings.maximumDuration).formatted(.units(allowed: [.hours, .minutes], width: .abbreviated)))
                    }
                    Text("Recording ends when the maximum duration is reached.").font(.caption).foregroundStyle(.secondary)
                }
                .disabled(settings.recordingMode == .nothing)
            }
            .formStyle(.grouped)
            .tabItem { Label("Recording", systemImage: "record.circle") }
        }
        .frame(width: 520, height: 520)
        .onChange(of: settings.recorderConfig) { app.player.settingsChanged() }
    }
}

struct MenuBarContent: View {
    @Environment(AppModel.self) private var app
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        let player = app.player
        if let station = player.station {
            Text(station.name)
            if let track = player.playingTrack { Text(track.title) }
            Button(player.isPlaying ? "Stop" : "Play") { player.togglePlayback() }
        } else {
            Text("No Playback")
        }
        if !app.library.isEmpty {
            Divider()
            Menu("Library") {
                ForEach(app.sortedLibrary) { s in
                    Button {
                        app.play(s)
                    } label: {
                        if player.station?.id == s.id { Label(s.name, systemImage: "speaker.wave.2.fill") } else { Text(s.name) }
                    }
                }
            }
        }
        Divider()
        Button("Show Shortwave") {
            openWindow(id: "main")
            NSApp.activate()
        }
        Button("Mini Player") {
            openWindow(id: "mini")
            NSApp.activate()
        }
        SettingsLink { Text("Settings…") }
        Divider()
        Button("Quit Shortwave") { NSApp.terminate(nil) }
    }
}
