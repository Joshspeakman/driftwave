import AppKit
import ShortwaveCore
import SwiftUI

/// Station artwork with a generated fallback when the station has no usable image.
struct StationCover: View {
    @Environment(AppModel.self) private var app
    let station: Station?
    var size: CGFloat
    var corner: CGFloat { size * 0.18 }

    var body: some View {
        let image = station.flatMap { app.images.cached(for: $0) }
        ZStack {
            if let image {
                Rectangle().fill(.background)
                Image(nsImage: image).resizable().interpolation(.high).aspectRatio(contentMode: .fit)
            } else {
                fallback
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: corner, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: corner, style: .continuous).strokeBorder(.quaternary, lineWidth: 0.5))
        .task(id: station.map { "\($0.id)|\($0.customCover ?? "")|\($0.faviconURL ?? "")" }) {
            if let station { await app.images.load(for: station) }
        }
        .accessibilityHidden(true)
    }

    private var fallback: some View {
        let stable = Double((station?.name ?? "").unicodeScalars.reduce(UInt32(0)) { ($1.value &+ $0 &* 31) % 360 }) / 360
        return LinearGradient(colors: [Color(hue: stable, saturation: 0.45, brightness: 0.85),
                                       Color(hue: stable, saturation: 0.65, brightness: 0.55)],
                              startPoint: .topLeading, endPoint: .bottomTrailing)
            .overlay(Image(systemName: "antenna.radiowaves.left.and.right")
                .font(.system(size: size * 0.36, weight: .medium))
                .foregroundStyle(.white.opacity(0.9)))
    }
}

/// A grid tile for a station.
struct StationCard: View {
    @Environment(AppModel.self) private var app
    let station: Station
    @ViewState private var hovering = false

    private var isCurrent: Bool { app.player.station?.id == station.id }
    private var isActive: Bool { isCurrent && app.player.isPlaying }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .bottomTrailing) {
                StationCover(station: station, size: 148)
                if hovering || isActive {
                    Button {
                        if isActive { app.player.stopPlayback() } else { app.play(station) }
                    } label: {
                        Image(systemName: isActive ? "stop.fill" : "play.fill")
                            .font(.system(size: 15, weight: .bold))
                            .frame(width: 36, height: 36)
                            .background(.regularMaterial, in: Circle())
                            .shadow(radius: 2, y: 1)
                    }
                    .buttonStyle(.plain)
                    .padding(8)
                    .help(isActive ? "Stop" : "Play")
                }
            }
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    if isActive {
                        Image(systemName: "speaker.wave.2.fill").font(.caption).foregroundStyle(.tint)
                            .symbolEffect(.variableColor.iterative, isActive: app.player.state == .playing)
                    }
                    Text(station.name).font(.system(size: 14, weight: .semibold)).lineLimit(1)
                }
                Text(cardSubtitle).font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(1)
            }
            .frame(width: 148, alignment: .leading)
        }
        .padding(8)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(hovering ? Fluent.subtle : .clear))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture(count: 2) { app.play(station) }
        .onTapGesture { app.detailStation = app.libraryVersion(of: station) }
        .contextMenu { StationMenu(station: station) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction(named: "Play") { app.play(station) }
    }

    private var cardSubtitle: String {
        if station.isLocal { return "Local station" }
        if station.isOrphaned { return "Orphaned station" }
        return station.subtitle.isEmpty ? " " : station.subtitle
    }
}

struct StationMenu: View {
    @Environment(AppModel.self) private var app
    let station: Station

    var body: some View {
        Button("Play") { app.play(station) }
        Button("Show Station Details") { app.detailStation = app.libraryVersion(of: station) }
        Divider()
        if app.isInLibrary(station) {
            Button("Remove from Library") { app.removeFromLibrary(station) }
        } else {
            Button("Add to Library") { app.addToLibrary(station) }
        }
        Button("Copy Stream URL") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(station.streamURL, forType: .string)
        }
        if let home = station.homepage, let url = URL(string: home) {
            Button("Open Homepage") { NSWorkspace.shared.open(url) }
        }
    }
}

struct StationGrid: View {
    let stations: [Station]
    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 164, maximum: 164), spacing: 8, alignment: .top)], alignment: .leading, spacing: 12) {
            ForEach(stations) { StationCard(station: $0) }
        }
    }
}
