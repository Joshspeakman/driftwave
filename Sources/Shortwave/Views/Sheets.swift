import MapKit
import ShortwaveCore
import SwiftUI

struct StationDetailSheet: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    let station: Station

    var body: some View {
        let inLibrary = app.isInLibrary(station)
        VStack(spacing: 0) {
            HStack(alignment: .top, spacing: 16) {
                StationCover(station: station, size: 112)
                VStack(alignment: .leading, spacing: 6) {
                    Text(station.name).font(.title2.bold()).textSelection(.enabled)
                    if !station.tags.isEmpty {
                        Text(station.tags.prefix(8).joined(separator: " · ")).foregroundStyle(.secondary).lineLimit(2)
                    }
                    HStack {
                        Button { app.play(station) } label: {
                            Label(isActive ? "Playing" : "Play", systemImage: "play.fill")
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(isActive)
                        Button { app.toggleLibrary(station) } label: {
                            Label(inLibrary ? "Remove from Library" : "Add to Library",
                                  systemImage: inLibrary ? "minus.circle" : "plus.circle")
                        }
                        if inLibrary {
                            Menu {
                                if station.isLocal {
                                    Button("Edit Station…") { app.editingStation = station; dismiss() }
                                }
                                Button("Change Station Cover…") {
                                    if let url = app.chooseImage() { app.setCover(station, from: url) }
                                }
                                if station.customCover != nil {
                                    Button("Use Default Cover") { app.setCover(station, from: nil) }
                                }
                            } label: {
                                Image(systemName: "ellipsis.circle")
                            }
                            .menuStyle(.borderlessButton)
                            .fixedSize()
                        }
                    }
                    .padding(.top, 4)
                }
                Spacer(minLength: 0)
            }
            .padding(20)

            Form {
                if station.isLocal {
                    note("Local Station", "This station exists only in your library, and is not part of the public database.")
                } else if station.isOrphaned {
                    note("Orphaned Station", "The information could not be updated, probably this station was removed from the public database.")
                }
                Section("Information") {
                    row("Language", station.language)
                    row("Tags", station.tags.joined(separator: ", "))
                    if !station.isLocal { row("Votes", station.votes > 0 ? station.votes.formatted() : "") }
                    if let home = station.homepage, let url = URL(string: home) {
                        LabeledContent("Homepage") { Link(url.host() ?? home, destination: url) }
                    }
                }
                if !station.country.isEmpty || !station.state.isEmpty || station.hasLocation {
                    Section("Location") {
                        row("Country", station.country)
                        row("State", station.state)
                        if let lat = station.latitude, let lon = station.longitude {
                            let coord = CLLocationCoordinate2D(latitude: lat, longitude: lon)
                            Map(initialPosition: .region(MKCoordinateRegion(center: coord, latitudinalMeters: 60_000, longitudinalMeters: 60_000))) {
                                Marker(station.name, coordinate: coord)
                            }
                            .frame(height: 180)
                            .clipShape(RoundedRectangle(cornerRadius: 8))
                        }
                    }
                }
                Section("Audio") {
                    row("Codec", station.codec + (station.isHLS ? " (HLS)" : ""))
                    row("Bitrate", station.bitrate > 0 ? "\(station.bitrate) kbps" : "")
                    LabeledContent("Stream") {
                        HStack {
                            Text(station.streamURL).lineLimit(1).truncationMode(.middle).textSelection(.enabled)
                            Button {
                                NSPasteboard.general.clearContents()
                                NSPasteboard.general.setString(station.streamURL, forType: .string)
                            } label: { Image(systemName: "doc.on.doc") }
                            .buttonStyle(.borderless)
                            .help("Copy Stream URL")
                        }
                    }
                }
            }
            .formStyle(.grouped)
        }
        .frame(width: 520, height: 600)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
        }
    }

    private var isActive: Bool { app.player.station?.id == station.id && app.player.isPlaying }

    @ViewBuilder private func row(_ label: String, _ value: String) -> some View {
        if !value.isEmpty { LabeledContent(label) { Text(value).textSelection(.enabled).multilineTextAlignment(.trailing) } }
    }

    private func note(_ title: String, _ text: String) -> some View {
        Section {
            Label {
                VStack(alignment: .leading) {
                    Text(title).font(.headline)
                    Text(text).foregroundStyle(.secondary)
                }
            } icon: { Image(systemName: "info.circle") }
        }
    }
}

/// Add a local station, or edit one.
struct LocalStationSheet: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    var editing: Station?

    @ViewState private var name = ""
    @ViewState private var url = ""
    @ViewState private var coverURL: URL?
    @ViewState private var loaded = false

    var body: some View {
        VStack(spacing: 0) {
            Form {
                Section {
                    TextField("Name", text: $name)
                    TextField("Stream URL", text: $url, prompt: Text("https://example.com/stream.mp3"))
                    LabeledContent("Cover") {
                        HStack {
                            if let coverURL, let img = NSImage(contentsOf: coverURL) {
                                Image(nsImage: img).resizable().scaledToFit().frame(width: 32, height: 32)
                                    .clipShape(RoundedRectangle(cornerRadius: 6))
                            }
                            Button(coverURL == nil ? "Choose…" : "Change…") { coverURL = app.chooseImage() ?? coverURL }
                        }
                    }
                } footer: {
                    Text("Playlist links (.pls, .m3u) are followed automatically.").foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if editing == nil {
                    Section {
                        LabeledContent("Public Station") {
                            Link("Add to radio-browser.info", destination: URL(string: "https://www.radio-browser.info/add")!)
                        }
                    } footer: {
                        Text("Public stations are added to the community database and become available for everyone.")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .formStyle(.grouped)
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button(editing == nil ? "Add Station" : "Save") { commit() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!isValid)
            }
            .padding(16)
        }
        .frame(width: 460)
        .navigationTitle(editing == nil ? "Add Local Station" : "Edit Station")
        .onAppear {
            guard !loaded else { return }
            loaded = true
            if let editing { name = editing.name; url = editing.streamURL }
        }
    }

    private var isValid: Bool {
        guard !name.trimmingCharacters(in: .whitespaces).isEmpty,
              let u = URL(string: url.trimmingCharacters(in: .whitespaces)),
              let scheme = u.scheme?.lowercased(), (scheme == "http" || scheme == "https"), u.host() != nil else { return false }
        return true
    }

    private func commit() {
        let cleanName = name.trimmingCharacters(in: .whitespaces)
        let cleanURL = url.trimmingCharacters(in: .whitespaces)
        if var s = editing {
            s.name = cleanName
            s.streamURL = cleanURL
            app.update(s)
            if let coverURL { app.setCover(s, from: coverURL) }
        } else {
            let s = Station.local(name: cleanName, streamURL: cleanURL)
            app.addToLibrary(s)
            if let coverURL { app.setCover(s, from: coverURL) }
        }
        dismiss()
    }
}

struct TrackDetailSheet: View {
    @Environment(AppModel.self) private var app
    @Environment(\.dismiss) private var dismiss
    let track: Track

    var body: some View {
        @Bindable var track = track
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                StationCover(station: track.station, size: 64)
                VStack(alignment: .leading, spacing: 2) {
                    Text(track.songTitle).font(.title3.bold()).textSelection(.enabled)
                    if let artist = track.artist { Text(artist).textSelection(.enabled) }
                    Text(track.station.name).foregroundStyle(.secondary)
                }
            }
            .padding(20)

            Form {
                Section("Recording") {
                    LabeledContent("State", value: track.state.title)
                    Text(track.state.explanation).foregroundStyle(.secondary)
                    if track.state.isRecorded || track.state == .recording {
                        LabeledContent("Duration") {
                            if track.state == .recording {
                                TimelineView(.periodic(from: .now, by: 1)) { ctx in
                                    let s = Int(ctx.date.timeIntervalSince(track.startedAt))
                                    Text(String(format: "%d:%02d", s / 60, s % 60)).monospacedDigit()
                                }
                            } else {
                                Text(track.durationText).monospacedDigit()
                            }
                        }
                    }
                    if track.state == .recording && app.settings.recordingMode == .decide {
                        Toggle("Save this track when recording is complete", isOn: $track.saveWhenRecorded)
                    }
                    if let saved = track.savedTo {
                        LabeledContent("Saved") {
                            Button(saved.lastPathComponent) { NSWorkspace.shared.activateFileViewerSelecting([saved]) }
                                .buttonStyle(.link)
                        }
                    }
                    if let err = track.saveError {
                        Label(err, systemImage: "exclamationmark.triangle").foregroundStyle(.orange)
                    }
                }
                Section {
                    Text("Tracks are recognised from the station’s stream metadata. Recording behaviour can be changed in Settings.")
                        .font(.callout).foregroundStyle(.secondary)
                    SettingsLink { Text("Open Settings…") }
                }
            }
            .formStyle(.grouped)

            HStack {
                if track.state == .recording && app.player.playingTrack?.id == track.id {
                    Button("Cancel Recording", role: .destructive) { app.player.cancelRecording() }
                }
                Spacer()
                if let saved = track.savedTo {
                    Button("Play Track") { NSWorkspace.shared.open(saved) }
                }
                if track.canSave {
                    Button("Save Track") { Task { await app.player.save(track) } }.buttonStyle(.borderedProminent)
                }
                Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            .padding(16)
        }
        .frame(width: 460, height: 470)
    }
}
