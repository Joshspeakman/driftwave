import AppKit
import ShortwaveCore
import SwiftUI

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
}

@main
struct ShortwaveApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @ViewState private var app = AppModel()

    var body: some Scene {
        Window("Shortwave", id: "main") {
            MainView().environment(app)
        }
        .defaultSize(width: 1080, height: 700)
        .commands { ShortwaveCommands(app: app) }

        Window("Mini Player", id: "mini") {
            MiniPlayer()
                .frame(width: 380)
                .environment(app)
        }
        .windowResizability(.contentSize)
        .windowLevel(.floating)
        .windowStyle(.hiddenTitleBar)
        .defaultPosition(.topTrailing)

        Settings {
            SettingsView().environment(app)
        }

        MenuBarExtra {
            MenuBarContent().environment(app)
        } label: {
            Image(systemName: app.player.isPlaying ? "dot.radiowaves.left.and.right" : "radio")
        }
    }
}

struct ShortwaveCommands: Commands {
    let app: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Add Local Station…") {
                openWindow(id: "main")
                app.showingAddStation = true
            }
            .keyboardShortcut("n")
            Divider()
            Button("Import Stations…") {
                openWindow(id: "main")
                app.chooseImportFile()
            }
            .keyboardShortcut("i", modifiers: [.command, .shift])
            Button("Export Stations…") { app.chooseExportFile() }
                .keyboardShortcut("e", modifiers: [.command, .shift])
                .disabled(app.library.isEmpty)
        }
        CommandGroup(after: .sidebar) {
            Button("Library") { app.page = .library }.keyboardShortcut("1")
            Button("Discover") { app.page = .discover }.keyboardShortcut("2")
            Button("Search Stations") { app.page = .search }.keyboardShortcut("f")
            Divider()
            Button(app.showPlayer ? "Hide Player" : "Show Player") { app.showPlayer.toggle() }
                .keyboardShortcut("p", modifiers: [.command, .option])
            Picker("Sort Library By", selection: Binding(get: { app.settings.librarySorting }, set: { app.settings.librarySorting = $0 })) {
                ForEach(StationSorting.allCases) { Text($0.title).tag($0) }
            }
            Divider()
        }
        CommandMenu("Playback") {
            Button(app.player.isPlaying ? "Stop" : "Play") { app.player.togglePlayback() }
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(app.player.station == nil)
            Button("Next Library Station") { app.player.skip(1) }
                .keyboardShortcut(.rightArrow, modifiers: .command)
            Button("Previous Library Station") { app.player.skip(-1) }
                .keyboardShortcut(.leftArrow, modifiers: .command)
            Divider()
            Button("Increase Volume") { app.player.increaseVolume() }
                .keyboardShortcut(.upArrow, modifiers: .command)
            Button("Decrease Volume") { app.player.decreaseVolume() }
                .keyboardShortcut(.downArrow, modifiers: .command)
            Button(app.player.isMuted ? "Unmute" : "Mute") { app.player.toggleMute() }
                .keyboardShortcut("m", modifiers: [.command, .option])
            Divider()
            Button("Cancel Recording") { app.player.cancelRecording() }
                .disabled(app.player.playingTrack?.state != .recording)
        }
        CommandGroup(before: .windowList) {
            Button("Mini Player") { openWindow(id: "mini") }
                .keyboardShortcut("m", modifiers: [.command, .shift])
            Divider()
        }
    }
}
