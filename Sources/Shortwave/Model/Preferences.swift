import Foundation
import Observation
import SwiftUI
import ShortwaveCore

/// `@State` is a macro in the macOS 27 SDK whose plugin ships only with Xcode; the property
/// wrapper type itself still works when reached through an alias.
typealias ViewState<V> = SwiftUICore.State<V>

/// User preferences, persisted in UserDefaults.
@Observable @MainActor
final class Preferences {
    private let d = Diagnostics.isTestRun ? UserDefaults(suiteName: "io.github.joshspeakman.Shortwave.TestRun")! : UserDefaults.standard

    var recordingMode: RecordingMode { didSet { d.set(recordingMode.rawValue, forKey: "recordingMode") } }
    var trackDirectory: URL { didSet { d.set(trackDirectory.path, forKey: "trackDirectory") } }
    var minimumDuration: Int { didSet { d.set(minimumDuration, forKey: "recordingMinimumDuration") } }
    var maximumDuration: Int { didSet { d.set(maximumDuration, forKey: "recordingMaximumDuration") } }
    var backgroundPlayback: Bool { didSet { d.set(backgroundPlayback, forKey: "backgroundPlayback") } }
    var notifications: Bool { didSet { d.set(notifications, forKey: "notifications") } }
    var pastTracksCount: Int { didSet { d.set(pastTracksCount, forKey: "pastTracksCount") } }
    var librarySorting: StationSorting { didSet { d.set(librarySorting.rawValue, forKey: "librarySorting") } }
    var libraryAscending: Bool { didSet { d.set(libraryAscending, forKey: "libraryAscending") } }
    var volume: Double { didSet { d.set(volume, forKey: "volume") } }
    var lastStationID: String? { didSet { d.set(lastStationID, forKey: "lastStation") } }
    var discoverCountry: String { didSet { d.set(discoverCountry, forKey: "discoverCountry") } }

    init() {
        d.register(defaults: [
            "recordingMode": RecordingMode.decide.rawValue,
            "recordingMinimumDuration": 30,
            "recordingMaximumDuration": 900,
            "backgroundPlayback": true,
            "notifications": false,
            "pastTracksCount": 10,
            "librarySorting": StationSorting.name.rawValue,
            "libraryAscending": true,
            "volume": 1.0,
        ])
        recordingMode = RecordingMode(rawValue: d.string(forKey: "recordingMode") ?? "") ?? .decide
        if let p = d.string(forKey: "trackDirectory"), !p.isEmpty {
            trackDirectory = URL(fileURLWithPath: p, isDirectory: true)
        } else {
            trackDirectory = AppPaths.defaultTrackDirectory
        }
        minimumDuration = d.integer(forKey: "recordingMinimumDuration")
        maximumDuration = d.integer(forKey: "recordingMaximumDuration")
        backgroundPlayback = d.bool(forKey: "backgroundPlayback")
        notifications = d.bool(forKey: "notifications")
        pastTracksCount = max(1, d.integer(forKey: "pastTracksCount"))
        librarySorting = StationSorting(rawValue: d.string(forKey: "librarySorting") ?? "") ?? .name
        libraryAscending = d.bool(forKey: "libraryAscending")
        volume = min(1, max(0, d.double(forKey: "volume")))
        lastStationID = d.string(forKey: "lastStation")
        discoverCountry = d.string(forKey: "discoverCountry") ?? (Locale.current.region?.identifier ?? "US")
    }

    var recorderConfig: SegmentRecorder.Config {
        .init(mode: recordingMode, minimumDuration: TimeInterval(minimumDuration), maximumDuration: TimeInterval(maximumDuration))
    }
}
