import AVFoundation
import Foundation
import Observation
import ShortwaveCore

/// A song (or show) detected from the stream's metadata, possibly recorded.
@Observable @MainActor
final class Track: Identifiable {
    let id: UUID
    let title: String
    let station: Station
    let startedAt = Date()
    let format: RecordableFormat?
    var state: RecordingState
    var duration: TimeInterval = 0
    var file: URL?
    var savedTo: URL?
    var saveWhenRecorded = false
    var saveError: String?

    init(id: UUID = UUID(), title: String, station: Station, state: RecordingState, file: URL?, format: RecordableFormat?) {
        self.id = id; self.title = title; self.station = station; self.state = state; self.file = file; self.format = format
    }

    var artist: String? { splitArtistTitle(title).artist }
    var songTitle: String { splitArtistTitle(title).title }
    var isSaved: Bool { savedTo != nil }
    var canSave: Bool { state.isRecorded && !isSaved && file != nil }

    var durationText: String {
        let s = Int(duration.rounded())
        return String(format: "%d:%02d", s / 60, s % 60)
    }

    /// Copies the temporary recording into `directory`, tagging it with title / artist / station.
    func save(to directory: URL) async {
        guard canSave, let file, let format else { return }
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: directory, withIntermediateDirectories: true)
            let name = sanitizedFileName(title)
            switch format {
            case .mp3:
                let dest = uniqueFileURL(in: directory, name: name, ext: "mp3")
                var data = ID3.tag(title: songTitle, artist: artist, album: station.name, comment: "Recorded with Shortwave from \(station.name)")
                data.append(try Data(contentsOf: file))
                try data.write(to: dest, options: .atomic)
                savedTo = dest
            case .aac:
                // Wrap the raw ADTS stream in an .m4a so it carries tags; fall back to the raw file.
                let dest = uniqueFileURL(in: directory, name: name, ext: "m4a")
                if await Self.remux(file, to: dest, title: songTitle, artist: artist, album: station.name) {
                    savedTo = dest
                } else {
                    let raw = uniqueFileURL(in: directory, name: name, ext: "aac")
                    try fm.copyItem(at: file, to: raw)
                    savedTo = raw
                }
            }
            saveError = nil
            Diagnostics.log("saved \(savedTo?.path ?? "?")")
        } catch {
            saveError = error.localizedDescription
        }
    }

    private static func remux(_ source: URL, to dest: URL, title: String, artist: String?, album: String) async -> Bool {
        let asset = AVURLAsset(url: source)
        guard let export = AVAssetExportSession(asset: asset, presetName: AVAssetExportPresetPassthrough) else { return false }
        func item(_ id: AVMetadataIdentifier, _ value: String) -> AVMetadataItem {
            let m = AVMutableMetadataItem(); m.identifier = id; m.value = value as NSString; m.extendedLanguageTag = "und"; return m
        }
        var meta = [item(.commonIdentifierTitle, title), item(.commonIdentifierAlbumName, album)]
        if let artist { meta.append(item(.commonIdentifierArtist, artist)) }
        export.metadata = meta
        do {
            try await export.export(to: dest, as: .m4a)
            return FileManager.default.fileExists(atPath: dest.path)
        } catch {
            try? FileManager.default.removeItem(at: dest)
            return false
        }
    }
}
