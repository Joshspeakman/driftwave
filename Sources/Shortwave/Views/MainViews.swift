import ShortwaveCore
import SwiftUI

/// The main window, laid out like the Windows app: a title row and navigation pane on the window
/// background, and the pages on a lighter layer with a rounded top-left corner.
struct MainView: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var app = app
        GeometryReader { geo in
            let compact = geo.size.width < 1000
            VStack(spacing: 0) {
                TitleRow()
                HStack(spacing: 0) {
                    NavigationPane(compact: compact)
                    contentLayer
                }
            }
        }
        .background(Fluent.mica)
        .ignoresSafeArea(.container, edges: .top)
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

    /// The pages and the player panel, on the content layer.
    private var contentLayer: some View {
        HStack(spacing: 0) {
            VStack(spacing: 0) {
                Group {
                    switch app.page {
                    case .library: LibraryPage()
                    case .discover: DiscoverPage()
                    case .search: SearchPage()
                    case .settings: SettingsPage()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                if !app.showPlayer && app.player.station != nil {
                    VStack(spacing: 0) {
                        Rectangle().fill(Fluent.divider).frame(height: 1)
                        MiniPlayer().background(Fluent.layerAlt)
                    }
                }
            }
            .frame(minWidth: 400, maxWidth: .infinity, maxHeight: .infinity)
            if app.showPlayer {
                Rectangle().fill(Fluent.divider).frame(width: 1)
                PlayerPanel()
                    .frame(width: 300)
                    .frame(maxHeight: .infinity)
                    .transition(.move(edge: .trailing))
            }
        }
        .animation(.snappy(duration: 0.2), value: app.showPlayer)
        .background(Fluent.layer)
        .clipShape(UnevenRoundedRectangle(topLeadingRadius: 8, style: .continuous))
        .overlay(UnevenRoundedRectangle(topLeadingRadius: 8, style: .continuous)
            .strokeBorder(Fluent.layerStroke, lineWidth: 1)
            .padding(.trailing, -1).padding(.bottom, -1))
    }
}

/// The row beside the window buttons: the app icon and name, like the Windows title bar.
private struct TitleRow: View {
    var body: some View {
        HStack(spacing: 10) {
            Image(nsImage: NSApp.applicationIconImage).resizable().interpolation(.high).frame(width: 16, height: 16)
            Text(Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String ?? "Shortwave")
                .font(.caption)
            Spacer()
        }
        .padding(.leading, 84)   // clear of the close, minimise and zoom buttons
        .frame(height: 32)
        .accessibilityHidden(true)
    }
}

/// The navigation pane: pages at the top; Add Local Station, the player toggle, Mini Player and
/// Settings at the bottom. Below 1000 pt of window width it shrinks to icons, as on Windows.
private struct NavigationPane: View {
    @Environment(AppModel.self) private var app
    @Environment(\.openWindow) private var openWindow
    let compact: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Page.main) { page in
                NavRow(title: page.title, symbol: page.symbol, selected: app.page == page, compact: compact,
                       help: page.title + page.shortcut) { app.page = page }
            }
            Spacer(minLength: 12)
            NavRow(title: "Add Local Station", symbol: "plus", compact: compact, help: "Add a local station (⌘N)") {
                app.showingAddStation = true
            }
            NavRow(title: app.showPlayer ? "Hide Player" : "Show Player", symbol: "sidebar.trailing", compact: compact,
                   help: "Show or hide the player (⌥⌘P)") { app.showPlayer.toggle() }
            NavRow(title: "Mini Player", symbol: "pip", compact: compact, help: "Open the mini player (⇧⌘M)") {
                openWindow(id: "mini")
            }
            NavRow(title: "Settings", symbol: Page.settings.symbol, selected: app.page == .settings, compact: compact,
                   help: "Settings (⌘,)") { app.page = .settings }
        }
        .padding(.horizontal, 4)
        .padding(.top, 4)
        .padding(.bottom, 8)
        .frame(width: compact ? 48 : 200)
        .animation(.snappy(duration: 0.2), value: compact)
    }
}

/// Settings as a page of cards, like the Windows app's.
struct SettingsPage: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        @Bindable var settings = app.settings
        VStack(spacing: 0) {
            PageHeader(title: "Settings")
            ScrollView {
                VStack(spacing: 4) {
                    GroupTitle(text: "Appearance", top: 8)
                    FluentCard {
                        HStack(spacing: 12) {
                            RowText(title: "App Theme", detail: "Follow your Mac’s light or dark setting, or always use one.")
                            Spacer()
                            Picker("App Theme", selection: $settings.theme) {
                                ForEach(AppTheme.allCases) { Text($0.title).tag($0) }
                            }
                            .labelsHidden()
                            .fixedSize()
                        }
                    }

                    GroupTitle(text: "Playback")
                    FluentCard {
                        ToggleRow(title: "Background Playback", detail: "Playback continues when the window is closed.",
                                  isOn: $settings.backgroundPlayback)
                    }
                    FluentCard {
                        ToggleRow(title: "Notifications", detail: "Show a notification when a new track gets played.",
                                  isOn: Binding(get: { settings.notifications }, set: { app.setNotifications($0) }))
                    }
                    FluentCard {
                        HStack(spacing: 12) {
                            Text("Played tracks to keep")
                            Spacer()
                            NumberStepper(value: $settings.pastTracksCount, range: 1...50, step: 1)
                        }
                    }

                    GroupTitle(text: "Recording")
                    FluentCard {
                        VStack(alignment: .leading, spacing: 6) {
                            Picker("Recording", selection: $settings.recordingMode) {
                                ForEach(RecordingMode.allCases) { Text($0.title).tag($0) }
                            }
                            .pickerStyle(.radioGroup)
                            .labelsHidden()
                            Text(settings.recordingMode.explanation).foregroundStyle(.secondary)
                            Text("Tracks are recognised from the station’s stream metadata. MP3 and AAC streams can be recorded; HLS and Ogg streams play without recording.")
                                .font(.caption).foregroundStyle(.secondary).padding(.top, 4)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                    Group {
                        FluentCard {
                            HStack(spacing: 12) {
                                Text("Track Folder")
                                Spacer(minLength: 8)
                                Text(settings.trackDirectory.path(percentEncoded: false)).foregroundStyle(.secondary)
                                    .lineLimit(1).truncationMode(.middle)
                                Button("Choose…") { app.chooseTrackDirectory() }
                                Button { NSWorkspace.shared.open(settings.trackDirectory) } label: { Image(systemName: "folder") }
                                    .help("Show in Finder")
                                    .disabled(!FileManager.default.fileExists(atPath: settings.trackDirectory.path))
                            }
                        }
                        FluentCard {
                            HStack(spacing: 12) {
                                RowText(title: "Minimum Duration", detail: "Tracks shorter than the minimum duration are ignored.")
                                Spacer()
                                Text(Duration.seconds(settings.minimumDuration).formatted(.units(allowed: [.minutes, .seconds], width: .abbreviated)))
                                    .foregroundStyle(.secondary)
                                NumberStepper(value: $settings.minimumDuration, range: 0...600, step: 5)
                            }
                        }
                        FluentCard {
                            HStack(spacing: 12) {
                                RowText(title: "Maximum Duration", detail: "Recording ends when the maximum duration is reached.")
                                Spacer()
                                Text(Duration.seconds(settings.maximumDuration).formatted(.units(allowed: [.hours, .minutes], width: .abbreviated)))
                                    .foregroundStyle(.secondary)
                                NumberStepper(value: $settings.maximumDuration, range: 60...7200, step: 60)
                            }
                        }
                    }
                    .disabled(settings.recordingMode == .nothing)

                    GroupTitle(text: "About")
                    FluentCard {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(AppModel.appDescription).fontWeight(.semibold)
                            Text("Internet radio for macOS, after GNOME Shortwave by Felix Häcker and contributors. It contains none of Shortwave’s code or artwork.")
                                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                            Link("Station data: radio-browser.info", destination: URL(string: "https://www.radio-browser.info")!)
                                .font(.callout)
                        }
                    }
                }
                .frame(maxWidth: 760)
                .padding(EdgeInsets(top: 4, leading: 24, bottom: 24, trailing: 24))
                .frame(maxWidth: .infinity)
            }
        }
        .onChange(of: settings.recorderConfig) { app.player.settingsChanged() }
        // Don't open with the first number field focused (and its text selected).
        .onAppear { DispatchQueue.main.async { NSApp.keyWindow?.makeFirstResponder(nil) } }
    }
}

/// A setting's name with a line of explanation under it.
private struct RowText: View {
    let title: String
    let detail: String
    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            Text(detail).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct ToggleRow: View {
    let title: String
    let detail: String
    @Binding var isOn: Bool
    var body: some View {
        HStack(spacing: 12) {
            RowText(title: title, detail: detail)
            Spacer()
            Toggle(title, isOn: $isOn).toggleStyle(.switch).labelsHidden()
        }
    }
}

/// A number field with up/down arrows, like WinUI's NumberBox.
private struct NumberStepper: View {
    @Binding var value: Int
    let range: ClosedRange<Int>
    let step: Int
    var body: some View {
        HStack(spacing: 4) {
            TextField("", value: Binding(get: { value }, set: { value = min(range.upperBound, max(range.lowerBound, $0)) }),
                      format: .number)
                .textFieldStyle(.roundedBorder)
                .multilineTextAlignment(.trailing)
                .frame(width: 64)
            Stepper("", value: $value, in: range, step: step).labelsHidden()
        }
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
        Button("Settings…") {
            app.page = .settings
            openWindow(id: "main")
            NSApp.activate()
        }
        Divider()
        Button("Quit Shortwave") { NSApp.terminate(nil) }
    }
}
