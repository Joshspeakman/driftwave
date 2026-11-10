import AVFoundation
import AppKit
import Foundation
import MediaPlayer
import Observation
import ShortwaveCore
import UserNotifications

/// Plays a station with AVPlayer, follows its song titles and records tracks.
@Observable @MainActor
final class Player: NSObject {
    enum State: Equatable {
        case stopped, loading, playing
        case failed(String)
    }

    private(set) var station: Station?
    private(set) var state: State = .stopped { didSet { if state != oldValue { Diagnostics.log("state \(state)") } } }
    private(set) var playingTrack: Track?
    private(set) var pastTracks: [Track] = []
    var volume: Double = 1 { didSet { avPlayer.volume = Float(volume); settings.volume = volume } }
    var isMuted = false { didSet { avPlayer.isMuted = isMuted } }

    @ObservationIgnored let avPlayer = AVPlayer()
    @ObservationIgnored private let settings: Preferences
    @ObservationIgnored private let paths: AppPaths
    @ObservationIgnored private let api: RadioBrowser
    @ObservationIgnored var artworkProvider: ((Station) -> NSImage?)?
    @ObservationIgnored var stationNeighbours: ((Station, Int) -> Station?)?

    @ObservationIgnored private var icy: IcyStream?
    @ObservationIgnored private var icyHasTitles = false
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var tracks: [UUID: Track] = [:]
    @ObservationIgnored private var statusObservation: NSKeyValueObservation?
    @ObservationIgnored private var observations: [NSKeyValueObservation] = []
    @ObservationIgnored private var itemObservers: [NSObjectProtocol] = []
    @ObservationIgnored private var metadataOutput: AVPlayerItemMetadataOutput?
    @ObservationIgnored private var icyRetries = 0
    @ObservationIgnored private var streamRetries = 0
    @ObservationIgnored private var resolvedURL: URL?

    init(settings: Preferences, paths: AppPaths, api: RadioBrowser) {
        self.settings = settings
        self.paths = paths
        self.api = api
        super.init()
        volume = settings.volume
        avPlayer.volume = Float(volume)
        avPlayer.automaticallyWaitsToMinimizeStalling = true
        statusObservation = avPlayer.observe(\.timeControlStatus, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async { MainActor.assumeIsolated { self?.syncStateFromPlayer() } }
        }
        setUpRemoteCommands()
        try? FileManager.default.removeItem(at: paths.recording)
    }

    var isPlaying: Bool { state == .playing || state == .loading }

    // MARK: Control

    func select(_ station: Station) {
        if self.station?.id != station.id {
            stopPlayback()
            self.station = station
            settings.lastStationID = station.id
            resetTracks()
            startPlayback()
        } else {
            self.station = station
            if !isPlaying { startPlayback() }
        }
    }

    /// Show a station without starting it (used to restore the last station at launch).
    func restore(_ station: Station) {
        guard self.station == nil else { return }
        self.station = station
        updateNowPlaying()
    }

    func togglePlayback() {
        if isPlaying { stopPlayback() } else { startPlayback() }
    }

    func startPlayback() {
        guard let station else { return }
        stopStreams()
        generation += 1
        let gen = generation
        state = .loading
        streamRetries = 0
        updateNowPlaying()
        if !station.isLocal { Task { await api.countClick(stationID: station.id) } }
        Task {
            guard let url = await resolveStreamURL(station.streamURL) else {
                if gen == generation { state = .failed("The stream address is not valid.") }
                return
            }
            guard gen == generation else { return }
            resolvedURL = url
            play(url: url, gen: gen)
        }
    }

    func stopPlayback() {
        generation += 1
        stopStreams()
        if let t = playingTrack, t.state.includeInPastTracks { addPast(t) }
        playingTrack = nil
        state = .stopped
        updateNowPlaying()
    }

    func increaseVolume() { volume = min(1, volume + 0.05); isMuted = false }
    func decreaseVolume() { volume = max(0, volume - 0.05) }
    func toggleMute() { isMuted.toggle() }

    func cancelRecording() { icy?.cancelRecording() }

    func settingsChanged() { icy?.update(config: settings.recorderConfig) }

    func save(_ track: Track) async {
        await track.save(to: settings.trackDirectory)
    }

    // MARK: Streams

    private func play(url: URL, gen: Int) {
        let item = AVPlayerItem(url: url)
        item.preferredForwardBufferDuration = 4
        let output = AVPlayerItemMetadataOutput(identifiers: nil)
        output.setDelegate(self, queue: .main)
        item.add(output)
        metadataOutput = output
        observations.append(item.observe(\.status, options: [.new]) { [weak self] item, _ in
            DispatchQueue.main.async { MainActor.assumeIsolated {
                guard let self, gen == self.generation else { return }
                if item.status == .failed { self.streamFailed(item.error?.localizedDescription ?? "The stream could not be played.", gen: gen) }
            } }
        })
        let nc = NotificationCenter.default
        itemObservers.append(nc.addObserver(forName: AVPlayerItem.failedToPlayToEndTimeNotification, object: item, queue: .main) { [weak self] n in
            let msg = (n.userInfo?[AVPlayerItemFailedToPlayToEndTimeErrorKey] as? Error)?.localizedDescription
            MainActor.assumeIsolated { self?.streamFailed(msg ?? "The stream was interrupted.", gen: gen) }
        })
        itemObservers.append(nc.addObserver(forName: AVPlayerItem.didPlayToEndTimeNotification, object: item, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.streamFailed("The station stopped sending audio.", gen: gen) }
        })
        avPlayer.replaceCurrentItem(with: item)
        avPlayer.play()

        if station?.isHLS != true, url.pathExtension.lowercased() != "m3u8" {
            startIcy(url: url, gen: gen)
        }
    }

    private func startIcy(url: URL, gen: Int) {
        let stream = IcyStream(url: url, config: settings.recorderConfig, directory: paths.recording) { [weak self] message in
            guard let self, gen == self.generation else {
                // A stream we already left: still let finished recordings land on their tracks.
                if case .recorder(let events) = message { self?.apply(events, live: false) }
                return
            }
            self.handle(message, url: url, gen: gen)
        }
        icy = stream
        stream.start()
    }

    private func handle(_ message: IcyStream.Message, url: URL, gen: Int) {
        switch message {
        case .connected(let hasTitles, let format):
            Diagnostics.log("icy connected titles=\(hasTitles) format=\(format?.rawValue ?? "none")")
            icyHasTitles = hasTitles
            if hasTitles { icyRetries = 0 } else { icy = nil }
        case .recorder(let events):
            apply(events, live: true)
        case .ended:
            icy = nil
            icyHasTitles = false
            // Keep following titles while the audio still plays.
            guard isPlaying, icyRetries < 5 else { return }
            icyRetries += 1
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
                guard let self, gen == self.generation, self.isPlaying, self.icy == nil else { return }
                self.startIcy(url: url, gen: gen)
            }
        }
    }

    private func stopStreams() {
        icy?.stop()
        icy = nil
        icyHasTitles = false
        avPlayer.pause()
        avPlayer.replaceCurrentItem(with: nil)
        observations.removeAll()
        itemObservers.forEach(NotificationCenter.default.removeObserver)
        itemObservers.removeAll()
        metadataOutput = nil
    }

    private func streamFailed(_ message: String, gen: Int) {
        guard gen == generation else { return }
        // Live streams sometimes drop; reconnect a couple of times before giving up.
        if streamRetries < 2, let url = resolvedURL {
            streamRetries += 1
            let retries = streamRetries
            stopStreams()
            state = .loading
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in
                guard let self, gen == self.generation else { return }
                self.play(url: url, gen: gen)
                self.streamRetries = retries
            }
            return
        }
        stopStreams()
        if let t = playingTrack, t.state.includeInPastTracks { addPast(t) }
        playingTrack = nil
        state = .failed(message)
        updateNowPlaying()
    }

    private func syncStateFromPlayer() {
        guard avPlayer.currentItem != nil else { return }
        switch avPlayer.timeControlStatus {
        case .playing:
            state = .playing
            streamRetries = 0
        case .waitingToPlayAtSpecifiedRate:
            if state != .playing || avPlayer.reasonForWaitingToPlay == .toMinimizeStalls { state = .loading }
        case .paused: break
        @unknown default: break
        }
        updateNowPlaying()
    }

    /// Follow `.pls`/`.m3u` links to the real stream.
    private func resolveStreamURL(_ string: String) async -> URL? {
        guard var url = URL(string: string.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else { return nil }
        for _ in 0..<3 {
            var probe = URLRequest(url: url)
            probe.setValue(RadioBrowser.userAgent, forHTTPHeaderField: "User-Agent")
            probe.timeoutInterval = 10
            let byExtension = Playlist.looksLikePlaylist(url: url, contentType: nil)
            if !byExtension {
                // Only a cheap HEAD-like peek for unknown content types: most links are plain streams.
                guard station?.isLocal == true else { return url }
                probe.setValue("bytes=0-4095", forHTTPHeaderField: "Range")
            }
            guard let (bytes, response) = try? await URLSession.shared.bytes(for: probe) else { return url }
            let ct = (response as? HTTPURLResponse)?.value(forHTTPHeaderField: "Content-Type")
            guard Playlist.looksLikePlaylist(url: url, contentType: ct) else { bytes.task.cancel(); return url }
            var data = Data()
            do {
                for try await b in bytes {
                    data.append(b)
                    if data.count > 65536 { break }
                }
            } catch {}
            bytes.task.cancel()
            let text = String(decoding: data, as: UTF8.self)
            guard let next = Playlist.firstStream(in: text, base: url) else { return url }
            url = next
        }
        return url
    }

    // MARK: Tracks

    private func resetTracks() {
        playingTrack = nil
        pastTracks.removeAll()
        tracks = tracks.filter { $0.value.state == .recording || $0.value.state.isRecorded && !$0.value.isSaved }
    }

    private func apply(_ events: [SegmentRecorder.Event], live: Bool) {
        for event in events {
            switch event {
            case let .started(id, title, state, file):
                Diagnostics.log("track started \"\(title)\" \(state.rawValue) \(file?.path ?? "-")")
                guard live, let station else { continue }
                let track = Track(id: id, title: title, station: station, state: state, file: file, format: icyFormat(file))
                tracks[id] = track
                // The sidecar runs ahead of what is audible by AVPlayer's buffer.
                let lead = bufferedLead()
                let gen = generation
                DispatchQueue.main.asyncAfter(deadline: .now() + lead) { [weak self] in
                    guard let self, gen == self.generation else { return }
                    self.promote(track)
                }
            case let .finished(id, state, duration):
                Diagnostics.log("track finished \(state.rawValue) \(Int(duration))s")
                guard let track = tracks[id] else { continue }
                track.state = state
                track.duration = duration
                if !state.isRecorded {
                    track.file = nil
                } else if settings.recordingMode == .everything || track.saveWhenRecorded {
                    Task { await self.save(track) }
                }
                if !state.includeInPastTracks { pastTracks.removeAll { $0.id == id } }
            }
        }
    }

    private func icyFormat(_ file: URL?) -> RecordableFormat? {
        guard let ext = file?.pathExtension else { return nil }
        return RecordableFormat(rawValue: ext)
    }

    private func bufferedLead() -> TimeInterval {
        guard let item = avPlayer.currentItem, let range = item.loadedTimeRanges.last?.timeRangeValue else { return 0 }
        let lead = CMTimeGetSeconds(CMTimeRangeGetEnd(range)) - CMTimeGetSeconds(item.currentTime())
        return lead.isFinite ? min(20, max(0, lead)) : 0
    }

    private func promote(_ track: Track) {
        guard playingTrack?.id != track.id else { return }
        if let old = playingTrack, old.state.includeInPastTracks { addPast(old) }
        playingTrack = track
        Diagnostics.log("now playing \"\(track.title)\"")
        updateNowPlaying()
        notify(track)
    }

    private func addPast(_ track: Track) {
        pastTracks.removeAll { $0.id == track.id }
        pastTracks.insert(track, at: 0)
        let limit = max(1, settings.pastTracksCount)
        if pastTracks.count > limit {
            for dropped in pastTracks[limit...] where !dropped.isSaved && dropped.state != .recording {
                if let f = dropped.file { try? FileManager.default.removeItem(at: f) }
                tracks[dropped.id] = nil
            }
            pastTracks = Array(pastTracks.prefix(limit))
        }
    }

    /// Titles from AVPlayer, for streams without ICY titles (HLS ID3 tags, Ogg comments).
    fileprivate func playerMetadata(_ groups: [AVTimedMetadataGroup]) {
        guard !icyHasTitles, let station, isPlaying else { return }
        var title: String?
        var artist: String?
        for item in groups.flatMap(\.items) {
            let id = item.identifier?.rawValue.lowercased() ?? ""
            guard let value = (item.value as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else { continue }
            if id.hasSuffix("/tit2") || id.hasSuffix("/title") || item.commonKey == .commonKeyTitle || id == "icy/streamtitle" {
                title = value
            } else if id.hasSuffix("/tpe1") || id.hasSuffix("/artist") || item.commonKey == .commonKeyArtist {
                artist = value
            }
        }
        guard let title else { return }
        let full = artist.map { "\($0) - \(title)" } ?? title
        guard full != playingTrack?.title else { return }
        let state: RecordingState = settings.recordingMode == .nothing ? .idleDisabled : .idleUnsupported
        let track = Track(title: full, station: station, state: state, file: nil, format: nil)
        promote(track)
    }

    // MARK: Notifications

    private func notify(_ track: Track) {
        guard settings.notifications, let station else { return }
        let content = UNMutableNotificationContent()
        content.title = track.songTitle
        content.subtitle = track.artist ?? ""
        content.body = station.name
        let request = UNNotificationRequest(identifier: "track", content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }

    // MARK: Now Playing / media keys

    func updateNowPlaying() {
        let center = MPNowPlayingInfoCenter.default()
        guard let station else {
            center.nowPlayingInfo = nil
            center.playbackState = .stopped
            return
        }
        var info: [String: Any] = [
            MPNowPlayingInfoPropertyIsLiveStream: true,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.audio.rawValue,
            MPNowPlayingInfoPropertyPlaybackRate: state == .playing ? 1.0 : 0.0,
            MPMediaItemPropertyAlbumTitle: station.name,
        ]
        if let track = playingTrack {
            info[MPMediaItemPropertyTitle] = track.songTitle
            info[MPMediaItemPropertyArtist] = track.artist ?? station.name
        } else {
            info[MPMediaItemPropertyTitle] = station.name
            info[MPMediaItemPropertyArtist] = "Shortwave"
        }
        if let image = artworkProvider?(station) {
            info[MPMediaItemPropertyArtwork] = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
        }
        center.nowPlayingInfo = info
        center.playbackState = switch state {
        case .playing, .loading: .playing
        case .stopped: .stopped
        case .failed: .interrupted
        }
    }

    private func setUpRemoteCommands() {
        let c = MPRemoteCommandCenter.shared()
        c.togglePlayPauseCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { self?.togglePlayback() }
            return .success
        }
        c.playCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { if self?.isPlaying == false { self?.startPlayback() } }
            return .success
        }
        c.pauseCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { self?.stopPlayback() }
            return .success
        }
        c.stopCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { self?.stopPlayback() }
            return .success
        }
        c.nextTrackCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { self?.skip(1) }
            return .success
        }
        c.previousTrackCommand.addTarget { [weak self] _ in
            MainActor.assumeIsolated { self?.skip(-1) }
            return .success
        }
    }

    /// Next / previous media key: move through the library.
    func skip(_ delta: Int) {
        guard let station, let next = stationNeighbours?(station, delta) else { return }
        select(next)
    }
}

extension Player: AVPlayerItemMetadataOutputPushDelegate {
    nonisolated func metadataOutput(_ output: AVPlayerItemMetadataOutput, didOutputTimedMetadataGroups groups: [AVTimedMetadataGroup], from track: AVPlayerItemTrack?) {
        MainActor.assumeIsolated { self.playerMetadata(groups) }
    }
}
