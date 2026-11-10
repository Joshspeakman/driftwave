import AVKit
import ShortwaveCore
import SwiftUI

/// The player sidebar: station, current track, controls and recently played tracks.
struct PlayerPanel: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        let player = app.player
        if let station = player.station {
            ScrollView {
                VStack(spacing: 16) {
                    Button { app.detailStation = app.libraryVersion(of: station) } label: {
                        StationCover(station: station, size: 200).shadow(color: .black.opacity(0.15), radius: 8, y: 4)
                    }
                    .buttonStyle(.plain)
                    .help("Show Station Details")

                    VStack(spacing: 4) {
                        Text(station.name).font(.title3.bold()).multilineTextAlignment(.center).lineLimit(2)
                        nowPlaying
                    }

                    PlaybackControls(large: true)

                    if let track = player.playingTrack {
                        RecordingIndicator(track: track)
                    }

                    if !player.pastTracks.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Played Tracks").font(.headline).padding(.horizontal, 4)
                            ForEach(player.pastTracks) { TrackRow(track: $0) }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .padding(16)
                .frame(maxWidth: .infinity)
            }
        } else {
            ContentUnavailableView("No Playback", systemImage: "radio",
                                   description: Text("Choose a station from your library or discover new ones."))
        }
    }

    @ViewBuilder private var nowPlaying: some View {
        let player = app.player
        switch player.state {
        case .failed(let message):
            Label(message, systemImage: "exclamationmark.triangle.fill")
                .foregroundStyle(.orange).font(.callout).multilineTextAlignment(.center)
        case .loading:
            Text("Buffering…").foregroundStyle(.secondary)
        case .stopped:
            Text(player.station?.subtitle ?? "").foregroundStyle(.secondary).font(.callout)
        case .playing:
            if let track = player.playingTrack {
                VStack(spacing: 2) {
                    Text(track.songTitle).font(.headline).multilineTextAlignment(.center).textSelection(.enabled)
                    if let artist = track.artist {
                        Text(artist).foregroundStyle(.secondary).multilineTextAlignment(.center).textSelection(.enabled)
                    }
                }
            } else {
                Text("Playing").foregroundStyle(.secondary)
            }
        }
    }
}

struct PlaybackControls: View {
    @Environment(AppModel.self) private var app
    var large = false

    var body: some View {
        let player = app.player
        HStack(spacing: large ? 14 : 8) {
            Button { player.togglePlayback() } label: {
                ZStack {
                    if player.state == .loading {
                        ProgressView().controlSize(large ? .regular : .small)
                    }
                    Image(systemName: player.isPlaying ? "stop.fill" : "play.fill")
                        .font(.system(size: large ? 20 : 13, weight: .bold))
                        .opacity(player.state == .loading ? 0.25 : 1)
                }
                .frame(width: large ? 48 : 30, height: large ? 48 : 30)
                .background(Circle().fill(.tint.opacity(0.18)))
            }
            .buttonStyle(.plain)
            .help(player.isPlaying ? "Stop" : "Play")
            .disabled(player.station == nil)

            VolumeControl()
            if large {
                RoutePicker().frame(width: 24, height: 24).help("AirPlay")
            }
        }
    }
}

struct VolumeControl: View {
    @Environment(AppModel.self) private var app
    var body: some View {
        @Bindable var player = app.player
        HStack(spacing: 6) {
            Button { player.toggleMute() } label: {
                Image(systemName: speakerSymbol).frame(width: 18)
            }
            .buttonStyle(.plain)
            .help(player.isMuted ? "Unmute" : "Mute")
            Slider(value: $player.volume, in: 0...1)
                .frame(minWidth: 80, maxWidth: 140)
                .controlSize(.small)
                .accessibilityLabel("Volume")
        }
    }

    private var speakerSymbol: String {
        let p = app.player
        if p.isMuted || p.volume == 0 { return "speaker.slash.fill" }
        return p.volume < 0.34 ? "speaker.wave.1.fill" : p.volume < 0.67 ? "speaker.wave.2.fill" : "speaker.wave.3.fill"
    }
}

/// AirPlay output picker (the Mac counterpart of Shortwave's cast button).
struct RoutePicker: NSViewRepresentable {
    @Environment(AppModel.self) private var app
    func makeNSView(context: Context) -> AVRoutePickerView {
        let v = AVRoutePickerView()
        v.player = app.player.avPlayer
        v.isRoutePickerButtonBordered = false
        return v
    }
    func updateNSView(_ nsView: AVRoutePickerView, context: Context) {}
}

struct RecordingIndicator: View {
    @Environment(AppModel.self) private var app
    let track: Track

    var body: some View {
        Button { app.detailTrack = track } label: {
            HStack(spacing: 8) {
                Image(systemName: symbol)
                    .foregroundStyle(track.state == .recording ? .red : .secondary)
                    .symbolEffect(.pulse, isActive: track.state == .recording)
                VStack(alignment: .leading, spacing: 1) {
                    Text(track.state.title).font(.callout.weight(.semibold))
                    Text(track.saveWhenRecorded && track.state == .recording ? "Will be saved when complete" : track.state.explanation)
                        .font(.caption).foregroundStyle(.secondary).lineLimit(2)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10, style: .continuous).fill(.quaternary.opacity(0.5)))
        }
        .buttonStyle(.plain)
        .help("Show track details")
    }

    private var symbol: String {
        switch track.state {
        case .recording: "record.circle.fill"
        case .recorded, .recordedReachedMaxDuration: "checkmark.circle"
        default: "record.circle"
        }
    }
}

struct TrackRow: View {
    @Environment(AppModel.self) private var app
    let track: Track

    var body: some View {
        HStack(spacing: 8) {
            Button { app.detailTrack = track } label: {
                VStack(alignment: .leading, spacing: 1) {
                    Text(track.songTitle).lineLimit(1)
                    Text(track.artist ?? track.state.title).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if track.isSaved {
                Image(systemName: "checkmark.circle.fill").foregroundStyle(.green).help("Saved")
            } else if track.canSave {
                Button { Task { await app.player.save(track) } } label: {
                    Image(systemName: "square.and.arrow.down")
                }
                .buttonStyle(.borderless)
                .help("Save Track")
            } else if track.state == .recording {
                Image(systemName: "record.circle.fill").foregroundStyle(.red).help("Recording…")
            }
        }
        .padding(.vertical, 5)
        .padding(.horizontal, 6)
        .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(.quaternary.opacity(0.35)))
    }
}

/// Compact player for the window bottom bar, the mini player and the menu bar.
struct MiniPlayer: View {
    @Environment(AppModel.self) private var app
    var body: some View {
        let player = app.player
        HStack(spacing: 10) {
            StationCover(station: player.station, size: 44)
            VStack(alignment: .leading, spacing: 2) {
                Text(player.station?.name ?? "No Playback").font(.headline).lineLimit(1)
                Text(player.playingTrack?.title ?? statusText).font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            PlaybackControls()
        }
        .padding(10)
    }

    private var statusText: String {
        switch app.player.state {
        case .stopped: app.player.station == nil ? "Choose a station" : "Stopped"
        case .loading: "Buffering…"
        case .playing: "Playing"
        case .failed(let m): m
        }
    }
}
