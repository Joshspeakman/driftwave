import Foundation

public enum RecordingMode: String, CaseIterable, Codable, Sendable, Identifiable {
    case everything, decide, nothing
    public var id: String { rawValue }
    public var title: String {
        switch self {
        case .everything: "Save All Tracks"
        case .decide: "Decide for Each Track"
        case .nothing: "Record Nothing"
        }
    }
    public var explanation: String {
        switch self {
        case .everything: "All recorded tracks are automatically saved in the track folder."
        case .decide: "Tracks are temporarily recorded and can be saved if desired."
        case .nothing: "Stations are played without recording."
        }
    }
}

public enum RecordingState: String, Sendable {
    case idleDisabled, idleIncomplete, idleUnsupported
    case recording, recorded, recordedReachedMaxDuration
    case discardedBelowMinDuration, discardedCancelled, discardedStreamFailure

    public var includeInPastTracks: Bool { self != .discardedBelowMinDuration }
    public var isRecorded: Bool { self == .recorded || self == .recordedReachedMaxDuration }

    public var title: String {
        switch self {
        case .idleDisabled, .idleIncomplete, .idleUnsupported: "Not Recorded"
        case .recording: "Recording…"
        case .recorded, .recordedReachedMaxDuration: "Recorded"
        case .discardedBelowMinDuration: "Below Threshold"
        case .discardedCancelled: "Cancelled"
        case .discardedStreamFailure: "Interrupted"
        }
    }

    public var explanation: String {
        switch self {
        case .idleDisabled: "Recording is turned off in Settings."
        case .idleIncomplete: "The track wasn’t played from the beginning, so it can’t be fully recorded."
        case .idleUnsupported: "This station’s stream format can’t be recorded."
        case .recording: "The track will be recorded until a new track gets played."
        case .recorded: "The track has been temporarily recorded."
        case .recordedReachedMaxDuration: "The maximum recording duration has been reached."
        case .discardedBelowMinDuration: "The track has been discarded as it was shorter than the minimum duration."
        case .discardedCancelled: "Recording has been cancelled."
        case .discardedStreamFailure: "The stream was interrupted, so the recording was discarded."
        }
    }
}

/// Cuts a live stream into one file per track, driven by the stream's title changes.
///
/// Runs on the stream's own queue so a cut happens at the exact byte where the title changed.
/// Not thread-safe: call it from one queue only.
public final class SegmentRecorder {
    public struct Config: Sendable, Equatable {
        public var mode: RecordingMode
        public var minimumDuration: TimeInterval
        public var maximumDuration: TimeInterval
        public init(mode: RecordingMode, minimumDuration: TimeInterval, maximumDuration: TimeInterval) {
            self.mode = mode; self.minimumDuration = minimumDuration; self.maximumDuration = maximumDuration
        }
    }

    public enum Event: Equatable, Sendable {
        /// A new track began. `file` is set when its audio is being recorded.
        case started(id: UUID, title: String, state: RecordingState, file: URL?)
        /// Recording of a track ended with `state`; `duration` in seconds.
        case finished(id: UUID, state: RecordingState, duration: TimeInterval)
    }

    public var config: Config
    public let format: RecordableFormat?
    private let directory: URL
    private let clock: () -> Date

    private var hadTrack = false
    private var currentID: UUID?
    private var currentTitle: String?
    private var handle: FileHandle?
    private var fileURL: URL?
    private var startedAt = Date()

    public init(config: Config, format: RecordableFormat?, directory: URL, clock: @escaping () -> Date = Date.init) {
        self.config = config; self.format = format; self.directory = directory; self.clock = clock
    }

    public var isRecording: Bool { handle != nil }

    public func handleTitle(_ title: String) -> [Event] {
        let title = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty, title != currentTitle else { return [] }
        var events = finishCurrent(cancelled: false)

        let id = UUID()
        let state: RecordingState
        var file: URL?
        if config.mode == .nothing {
            state = .idleDisabled
        } else if format == nil {
            state = .idleUnsupported
        } else if !hadTrack {
            // The first title we see started before we tuned in.
            state = .idleIncomplete
        } else {
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let url = directory.appendingPathComponent(id.uuidString).appendingPathExtension(format!.fileExtension)
            if FileManager.default.createFile(atPath: url.path, contents: nil), let h = try? FileHandle(forWritingTo: url) {
                handle = h; fileURL = url; file = url
                state = .recording
            } else {
                state = .idleDisabled
            }
        }
        hadTrack = true
        currentID = id
        currentTitle = title
        startedAt = clock()
        events.append(.started(id: id, title: title, state: state, file: file))
        return events
    }

    public func handleAudio(_ data: Data) -> [Event] {
        guard let handle else { return [] }
        handle.write(data)
        if config.maximumDuration > 0, clock().timeIntervalSince(startedAt) >= config.maximumDuration {
            return close(state: .recordedReachedMaxDuration, discard: false)
        }
        return []
    }

    /// The user cancelled the current recording.
    public func cancel() -> [Event] { finishCurrent(cancelled: true) }

    /// The stream ended or failed: whatever was being recorded is incomplete.
    public func streamEnded() -> [Event] {
        let events = handle != nil ? close(state: .discardedStreamFailure, discard: true) : []
        hadTrack = false
        currentTitle = nil
        currentID = nil
        return events
    }

    private func finishCurrent(cancelled: Bool) -> [Event] {
        guard handle != nil else { return [] }
        if cancelled { return close(state: .discardedCancelled, discard: true) }
        let duration = clock().timeIntervalSince(startedAt)
        if duration < config.minimumDuration { return close(state: .discardedBelowMinDuration, discard: true) }
        return close(state: .recorded, discard: false)
    }

    private func close(state: RecordingState, discard: Bool) -> [Event] {
        guard let h = handle, let id = currentID else { return [] }
        try? h.close()
        handle = nil
        let duration = discard && state == .discardedCancelled ? 0 : clock().timeIntervalSince(startedAt)
        if discard, let fileURL { try? FileManager.default.removeItem(at: fileURL) }
        fileURL = nil
        return [.finished(id: id, state: state, duration: duration)]
    }
}
