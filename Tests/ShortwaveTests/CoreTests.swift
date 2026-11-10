import Foundation
import Testing
@testable import ShortwaveCore

@Suite struct ICYTests {
    func block(_ text: String) -> Data {
        var bytes = Array(text.utf8)
        while bytes.count % 16 != 0 { bytes.append(0) }
        return Data([UInt8(bytes.count / 16)] + bytes)
    }

    @Test func splitsAudioAndMetadataAcrossChunks() {
        let metaInt = 8
        var stream = Data(repeating: 0xAA, count: 8)
        stream += block("StreamTitle='A - B';")
        stream += Data(repeating: 0xBB, count: 8)
        stream += Data([0])                          // empty metadata block
        stream += Data(repeating: 0xCC, count: 5)

        for chunkSize in [1, 3, 7, 100] {
            var parser = ICYParser(metaInt: metaInt)
            var audio = Data(); var titles: [String] = []
            var i = 0
            while i < stream.count {
                let chunk = stream[i..<min(i + chunkSize, stream.count)]
                for e in parser.feed(Data(chunk)) {
                    switch e {
                    case .audio(let d): audio += d
                    case .metadata(let m): titles.append(ICYParser.streamTitle(in: m) ?? "?")
                    }
                }
                i += chunkSize
            }
            #expect(audio == Data(repeating: 0xAA, count: 8) + Data(repeating: 0xBB, count: 8) + Data(repeating: 0xCC, count: 5), "chunk \(chunkSize)")
            #expect(titles == ["A - B"], "chunk \(chunkSize)")
        }
    }

    @Test func streamTitleParsing() {
        #expect(ICYParser.streamTitle(in: "StreamTitle='Artist - Song';StreamUrl='';") == "Artist - Song")
        #expect(ICYParser.streamTitle(in: "StreamTitle='Guns N' Roses - Don't Cry';StreamUrl='x';") == "Guns N' Roses - Don't Cry")
        #expect(ICYParser.streamTitle(in: "StreamTitle='It's';") == "It's")
        #expect(ICYParser.streamTitle(in: "StreamTitle='';") == "")
        #expect(ICYParser.streamTitle(in: "StreamUrl='x';") == nil)
    }

    @Test func latin1Fallback() {
        let data = Data([0x53, 0x74, 0x72, 0x65, 0x61, 0x6D, 0x54, 0x69, 0x74, 0x6C, 0x65, 0x3D, 0x27, 0x43, 0x61, 0x66, 0xE9, 0x27, 0x3B, 0, 0])
        #expect(ICYParser.streamTitle(in: ICYParser.decodeText(data)) == "Café")
    }

    @Test func artistTitle() {
        #expect(splitArtistTitle("Daft Punk - One More Time").artist == "Daft Punk")
        #expect(splitArtistTitle("Daft Punk - One More Time").title == "One More Time")
        #expect(splitArtistTitle("News").artist == nil)
    }

    @Test func formats() {
        #expect(RecordableFormat(contentType: "audio/mpeg") == .mp3)
        #expect(RecordableFormat(contentType: "audio/aacp; charset=x") == .aac)
        #expect(RecordableFormat(contentType: "application/ogg") == nil)
        #expect(RecordableFormat(contentType: nil) == nil)
    }
}

@Suite struct PlaylistTests {
    let base = URL(string: "http://example.com/radio/listen.pls")!

    @Test func pls() {
        let text = "[playlist]\nNumberOfEntries=2\nFile1=http://stream.example.com:8000/live\nTitle1=Live\n"
        #expect(Playlist.firstStream(in: text, base: base)?.absoluteString == "http://stream.example.com:8000/live")
    }

    @Test func m3u() {
        let text = "#EXTM3U\n#EXTINF:-1,Station\nhttps://a.example.com/stream.mp3\n"
        #expect(Playlist.firstStream(in: text, base: base)?.absoluteString == "https://a.example.com/stream.mp3")
    }

    @Test func relative() {
        #expect(Playlist.firstStream(in: "stream.mp3\n", base: base)?.absoluteString == "http://example.com/radio/stream.mp3")
    }

    @Test func detection() {
        #expect(Playlist.looksLikePlaylist(url: URL(string: "http://x/a.pls")!, contentType: nil))
        #expect(Playlist.looksLikePlaylist(url: URL(string: "http://x/a")!, contentType: "audio/x-scpls"))
        #expect(!Playlist.looksLikePlaylist(url: URL(string: "http://x/a.m3u8")!, contentType: "application/x-mpegurl"))
        #expect(!Playlist.looksLikePlaylist(url: URL(string: "http://x/a")!, contentType: "audio/mpeg"))
    }

    @Test func fileNames() {
        #expect(sanitizedFileName("AC/DC: Back in Black?") == "AC_DC_ Back in Black_")
        #expect(sanitizedFileName("...") == "Track")
    }
}

@Suite struct ID3Tests {
    @Test func header() {
        let tag = ID3.tag(title: "Song", artist: "Band", album: "Station")
        #expect(Array(tag.prefix(3)) == Array("ID3".utf8))
        #expect(tag[3] == 3)
        let size = Int(tag[6]) << 21 | Int(tag[7]) << 14 | Int(tag[8]) << 7 | Int(tag[9])
        #expect(size == tag.count - 10)
        #expect(tag.range(of: Data("TIT2".utf8)) != nil)
        #expect(tag.range(of: Data("TPE1".utf8)) != nil)
    }
}

@Suite struct RecorderTests {
    final class Clock { var now = Date(timeIntervalSince1970: 0) }

    func make(_ mode: RecordingMode = .decide, min: TimeInterval = 30, max: TimeInterval = 900, format: RecordableFormat? = .mp3) -> (SegmentRecorder, Clock, URL) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("swtest-" + UUID().uuidString)
        let clock = Clock()
        let r = SegmentRecorder(config: .init(mode: mode, minimumDuration: min, maximumDuration: max), format: format, directory: dir, clock: { clock.now })
        return (r, clock, dir)
    }

    func startState(_ events: [SegmentRecorder.Event]) -> RecordingState? {
        for case let .started(_, _, state, _) in events { return state }
        return nil
    }

    func finishState(_ events: [SegmentRecorder.Event]) -> RecordingState? {
        for case let .finished(_, state, _) in events { return state }
        return nil
    }

    @Test func firstTrackIsIncompleteThenRecords() throws {
        let (r, clock, dir) = make()
        defer { try? FileManager.default.removeItem(at: dir) }
        #expect(startState(r.handleTitle("One")) == .idleIncomplete)
        #expect(r.handleTitle("One").isEmpty)        // repeated title is not a change
        _ = r.handleAudio(Data([1, 2, 3]))
        let e = r.handleTitle("Two")
        #expect(startState(e) == .recording)
        guard case let .started(_, _, _, file?) = e.last else { Issue.record("no file"); return }
        _ = r.handleAudio(Data([4, 5]))
        clock.now += 60
        _ = r.handleAudio(Data([6]))
        #expect(finishState(r.handleTitle("Three")) == .recorded)
        #expect(try Data(contentsOf: file) == Data([4, 5, 6]))
    }

    @Test func belowMinimumIsDiscarded() {
        let (r, clock, dir) = make()
        defer { try? FileManager.default.removeItem(at: dir) }
        _ = r.handleTitle("One")
        let e = r.handleTitle("Two")
        guard case let .started(_, _, _, file?) = e.last else { Issue.record("no file"); return }
        clock.now += 5
        #expect(finishState(r.handleTitle("Three")) == .discardedBelowMinDuration)
        #expect(!FileManager.default.fileExists(atPath: file.path))
    }

    @Test func maximumDurationStopsRecording() {
        let (r, clock, dir) = make(max: 100)
        defer { try? FileManager.default.removeItem(at: dir) }
        _ = r.handleTitle("One"); _ = r.handleTitle("Two")
        clock.now += 101
        #expect(finishState(r.handleAudio(Data([1]))) == .recordedReachedMaxDuration)
        #expect(!r.isRecording)
        #expect(finishState(r.handleTitle("Three")) == nil)
    }

    @Test func cancelAndStreamFailure() {
        let (r, _, dir) = make()
        defer { try? FileManager.default.removeItem(at: dir) }
        _ = r.handleTitle("One"); _ = r.handleTitle("Two")
        #expect(finishState(r.cancel()) == .discardedCancelled)
        _ = r.handleTitle("Three")
        #expect(finishState(r.streamEnded()) == .discardedStreamFailure)
        #expect(startState(r.handleTitle("Four")) == .idleIncomplete)
    }

    @Test func disabledAndUnsupported() {
        let (r, _, dir) = make(.nothing)
        _ = r.handleTitle("One")
        #expect(startState(r.handleTitle("Two")) == .idleDisabled)
        let (u, _, dir2) = make(format: nil)
        _ = u.handleTitle("One")
        #expect(startState(u.handleTitle("Two")) == .idleUnsupported)
        try? FileManager.default.removeItem(at: dir); try? FileManager.default.removeItem(at: dir2)
    }
}

@Suite struct StationTests {
    @Test func decodeAPI() throws {
        let json = """
        [{"stationuuid":"abc","name":" Test FM ","url":"http://a","url_resolved":"http://b","homepage":"","favicon":"http://f.png",
          "tags":"jazz, blues,","country":"Germany","countrycode":"DE","state":"","language":"german,english","votes":5,
          "codec":"MP3","bitrate":128,"hls":0,"lastcheckok":1,"geo_lat":null,"geo_long":null,"extra":"ignored"}]
        """
        let s = try JSONDecoder().decode([APIStation].self, from: Data(json.utf8))[0].station
        #expect(s.name == "Test FM")
        #expect(s.streamURL == "http://b")
        #expect(s.homepage == nil)
        #expect(s.tags == ["jazz", "blues"])
        #expect(s.language == "German, English")
        #expect(!s.hasLocation)
        #expect(s.subtitle == "MP3 · 128 kbps · Germany")
    }

    @Test func sorting() {
        let a = Station(id: "1", name: "Bravo", streamURL: "x", bitrate: 64, votes: 10)
        let b = Station(id: "2", name: "alpha", streamURL: "x", bitrate: 128, votes: 30)
        #expect([a, b].sorted(by: .name, ascending: true).map(\.id) == ["2", "1"])
        #expect([a, b].sorted(by: .votes, ascending: false).map(\.id) == ["2", "1"])
        #expect([a, b].sorted(by: .bitrate, ascending: true).map(\.id) == ["1", "2"])
    }

    @Test func libraryRoundTrip() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("swlib-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        let s = Station.local(name: "Mine", streamURL: "http://x")
        try LibraryFile.save([s], to: url)
        #expect(LibraryFile.load(from: url) == [s])
    }
}
