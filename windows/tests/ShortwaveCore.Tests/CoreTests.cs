// 1:1 translation of Tests/ShortwaveTests/CoreTests.swift (19 tests), followed by Windows-only tests.
using System.Text;

namespace Shortwave.Core.Tests;

static class Bytes
{
    public static byte[] Repeat(byte b, int n) => Enumerable.Repeat(b, n).ToArray();
}

public class ICYTests
{
    static byte[] Block(string text)
    {
        var bytes = Encoding.UTF8.GetBytes(text).ToList();
        while (bytes.Count % 16 != 0) bytes.Add(0);
        return [(byte)(bytes.Count / 16), .. bytes];
    }

    [Fact]
    public void SplitsAudioAndMetadataAcrossChunks()
    {
        int metaInt = 8;
        byte[] stream = [.. Bytes.Repeat(0xAA, 8), .. Block("StreamTitle='A - B';"), .. Bytes.Repeat(0xBB, 8), 0, .. Bytes.Repeat(0xCC, 5)];

        foreach (int chunkSize in new[] { 1, 3, 7, 100 })
        {
            var parser = new IcyParser(metaInt);
            var audio = new List<byte>(); var titles = new List<string>();
            for (int i = 0; i < stream.Length; i += chunkSize)
            {
                var chunk = stream.AsSpan(i, Math.Min(chunkSize, stream.Length - i));
                foreach (var e in parser.Feed(chunk))
                {
                    switch (e)
                    {
                        case IcyEvent.Audio a: audio.AddRange(a.Data); break;
                        case IcyEvent.Metadata m: titles.Add(IcyParser.StreamTitle(m.Text) ?? "?"); break;
                    }
                }
            }
            Assert.Equal([.. Bytes.Repeat(0xAA, 8), .. Bytes.Repeat(0xBB, 8), .. Bytes.Repeat(0xCC, 5)], audio.ToArray());
            Assert.Equal(["A - B"], titles);
        }
    }

    [Fact]
    public void StreamTitleParsing()
    {
        Assert.Equal("Artist - Song", IcyParser.StreamTitle("StreamTitle='Artist - Song';StreamUrl='';"));
        Assert.Equal("Guns N' Roses - Don't Cry", IcyParser.StreamTitle("StreamTitle='Guns N' Roses - Don't Cry';StreamUrl='x';"));
        Assert.Equal("It's", IcyParser.StreamTitle("StreamTitle='It's';"));
        Assert.Equal("", IcyParser.StreamTitle("StreamTitle='';"));
        Assert.Null(IcyParser.StreamTitle("StreamUrl='x';"));
    }

    [Fact]
    public void Latin1Fallback()
    {
        byte[] data = [0x53, 0x74, 0x72, 0x65, 0x61, 0x6D, 0x54, 0x69, 0x74, 0x6C, 0x65, 0x3D, 0x27, 0x43, 0x61, 0x66, 0xE9, 0x27, 0x3B, 0, 0];
        Assert.Equal("Café", IcyParser.StreamTitle(IcyParser.DecodeText(data)));
    }

    [Fact]
    public void ArtistTitle()
    {
        Assert.Equal("Daft Punk", TrackTitles.SplitArtistTitle("Daft Punk - One More Time").Artist);
        Assert.Equal("One More Time", TrackTitles.SplitArtistTitle("Daft Punk - One More Time").Title);
        Assert.Null(TrackTitles.SplitArtistTitle("News").Artist);
    }

    [Fact]
    public void Formats()
    {
        Assert.Equal(RecordableFormat.Mp3, RecordableFormats.FromContentType("audio/mpeg"));
        Assert.Equal(RecordableFormat.Aac, RecordableFormats.FromContentType("audio/aacp; charset=x"));
        Assert.Null(RecordableFormats.FromContentType("application/ogg"));
        Assert.Null(RecordableFormats.FromContentType(null));
    }
}

public class PlaylistTests
{
    static readonly Uri Base = new("http://example.com/radio/listen.pls");

    [Fact]
    public void Pls()
    {
        string text = "[playlist]\nNumberOfEntries=2\nFile1=http://stream.example.com:8000/live\nTitle1=Live\n";
        Assert.Equal("http://stream.example.com:8000/live", Playlist.FirstStream(text, Base)?.AbsoluteUri);
    }

    [Fact]
    public void M3u()
    {
        string text = "#EXTM3U\n#EXTINF:-1,Station\nhttps://a.example.com/stream.mp3\n";
        Assert.Equal("https://a.example.com/stream.mp3", Playlist.FirstStream(text, Base)?.AbsoluteUri);
    }

    [Fact]
    public void Relative()
    {
        Assert.Equal("http://example.com/radio/stream.mp3", Playlist.FirstStream("stream.mp3\n", Base)?.AbsoluteUri);
    }

    [Fact]
    public void Detection()
    {
        Assert.True(Playlist.LooksLikePlaylist(new Uri("http://x/a.pls"), null));
        Assert.True(Playlist.LooksLikePlaylist(new Uri("http://x/a"), "audio/x-scpls"));
        Assert.False(Playlist.LooksLikePlaylist(new Uri("http://x/a.m3u8"), "application/x-mpegurl"));
        Assert.False(Playlist.LooksLikePlaylist(new Uri("http://x/a"), "audio/mpeg"));
    }

    [Fact]
    public void FileNames_()
    {
        Assert.Equal("AC_DC_ Back in Black_", FileNames.Sanitized("AC/DC: Back in Black?"));
        Assert.Equal("Track", FileNames.Sanitized("..."));
    }
}

public class ID3Tests
{
    [Fact]
    public void Header()
    {
        var tag = ID3.Tag("Song", "Band", "Station");
        Assert.Equal("ID3"u8.ToArray(), tag[..3]);
        Assert.Equal(3, tag[3]);
        int size = tag[6] << 21 | tag[7] << 14 | tag[8] << 7 | tag[9];
        Assert.Equal(tag.Length - 10, size);
        Assert.True(tag.AsSpan().IndexOf("TIT2"u8) >= 0);
        Assert.True(tag.AsSpan().IndexOf("TPE1"u8) >= 0);
    }
}

public class RecorderTests
{
    sealed class Clock { public DateTime Now = DateTime.UnixEpoch; }

    static (SegmentRecorder, Clock, string) Make(RecordingMode mode = RecordingMode.Decide, double min = 30, double max = 900, RecordableFormat? format = RecordableFormat.Mp3)
    {
        string dir = Path.Combine(Path.GetTempPath(), "swtest-" + Guid.NewGuid());
        var clock = new Clock();
        var r = new SegmentRecorder(new RecorderConfig(mode, min, max), format, dir, () => clock.Now);
        return (r, clock, dir);
    }

    static RecordingState? StartState(List<RecorderEvent> events) => events.OfType<RecorderEvent.Started>().FirstOrDefault()?.State;
    static RecordingState? FinishState(List<RecorderEvent> events) => events.OfType<RecorderEvent.Finished>().FirstOrDefault()?.State;
    static void Remove(string dir) { try { Directory.Delete(dir, true); } catch { } }

    [Fact]
    public void FirstTrackIsIncompleteThenRecords()
    {
        var (r, clock, dir) = Make();
        try
        {
            Assert.Equal(RecordingState.IdleIncomplete, StartState(r.HandleTitle("One")));
            Assert.Empty(r.HandleTitle("One"));            // repeated title is not a change
            r.HandleAudio(new byte[] { 1, 2, 3 });
            var e = r.HandleTitle("Two");
            Assert.Equal(RecordingState.Recording, StartState(e));
            var file = Assert.IsType<RecorderEvent.Started>(e.Last()).File;
            Assert.NotNull(file);
            r.HandleAudio(new byte[] { 4, 5 });
            clock.Now = clock.Now.AddSeconds(60);
            r.HandleAudio(new byte[] { 6 });
            Assert.Equal(RecordingState.Recorded, FinishState(r.HandleTitle("Three")));
            Assert.Equal(new byte[] { 4, 5, 6 }, File.ReadAllBytes(file!));
        }
        finally { r.StreamEnded(); Remove(dir); }
    }

    [Fact]
    public void BelowMinimumIsDiscarded()
    {
        var (r, clock, dir) = Make();
        try
        {
            r.HandleTitle("One");
            var e = r.HandleTitle("Two");
            var file = Assert.IsType<RecorderEvent.Started>(e.Last()).File;
            Assert.NotNull(file);
            clock.Now = clock.Now.AddSeconds(5);
            Assert.Equal(RecordingState.DiscardedBelowMinDuration, FinishState(r.HandleTitle("Three")));
            Assert.False(File.Exists(file));
        }
        finally { r.StreamEnded(); Remove(dir); }
    }

    [Fact]
    public void MaximumDurationStopsRecording()
    {
        var (r, clock, dir) = Make(max: 100);
        try
        {
            r.HandleTitle("One"); r.HandleTitle("Two");
            clock.Now = clock.Now.AddSeconds(101);
            Assert.Equal(RecordingState.RecordedReachedMaxDuration, FinishState(r.HandleAudio(new byte[] { 1 })));
            Assert.False(r.IsRecording);
            Assert.Null(FinishState(r.HandleTitle("Three")));
        }
        finally { r.StreamEnded(); Remove(dir); }
    }

    [Fact]
    public void CancelAndStreamFailure()
    {
        var (r, _, dir) = Make();
        try
        {
            r.HandleTitle("One"); r.HandleTitle("Two");
            Assert.Equal(RecordingState.DiscardedCancelled, FinishState(r.Cancel()));
            r.HandleTitle("Three");
            Assert.Equal(RecordingState.DiscardedStreamFailure, FinishState(r.StreamEnded()));
            Assert.Equal(RecordingState.IdleIncomplete, StartState(r.HandleTitle("Four")));
        }
        finally { r.StreamEnded(); Remove(dir); }
    }

    [Fact]
    public void DisabledAndUnsupported()
    {
        var (r, _, dir) = Make(RecordingMode.Nothing);
        r.HandleTitle("One");
        Assert.Equal(RecordingState.IdleDisabled, StartState(r.HandleTitle("Two")));
        var (u, _, dir2) = Make(format: null);
        u.HandleTitle("One");
        Assert.Equal(RecordingState.IdleUnsupported, StartState(u.HandleTitle("Two")));
        Remove(dir); Remove(dir2);
    }
}

public class StationTests
{
    [Fact]
    public void DecodeAPI()
    {
        string json = """
        [{"stationuuid":"abc","name":" Test FM ","url":"http://a","url_resolved":"http://b","homepage":"","favicon":"http://f.png",
          "tags":"jazz, blues,","country":"Germany","countrycode":"DE","state":"","language":"german,english","votes":5,
          "codec":"MP3","bitrate":128,"hls":0,"lastcheckok":1,"geo_lat":null,"geo_long":null,"extra":"ignored"}]
        """;
        var s = APIStation.DecodeList(json)[0];
        Assert.Equal("Test FM", s.Name);
        Assert.Equal("http://b", s.StreamURL);
        Assert.Null(s.Homepage);
        Assert.Equal(["jazz", "blues"], s.Tags);
        Assert.Equal("German, English", s.Language);
        Assert.False(s.HasLocation);
        Assert.Equal("MP3 · 128 kbps · Germany", s.Subtitle);
    }

    [Fact]
    public void Sorting()
    {
        var a = new Station("1", "Bravo", "x", bitrate: 64, votes: 10);
        var b = new Station("2", "alpha", "x", bitrate: 128, votes: 30);
        Assert.Equal(["2", "1"], new[] { a, b }.SortedBy(StationSorting.Name, true).Select(s => s.Id));
        Assert.Equal(["2", "1"], new[] { a, b }.SortedBy(StationSorting.Votes, false).Select(s => s.Id));
        Assert.Equal(["1", "2"], new[] { a, b }.SortedBy(StationSorting.Bitrate, true).Select(s => s.Id));
    }

    [Fact]
    public void LibraryRoundTrip()
    {
        string path = Path.Combine(Path.GetTempPath(), $"swlib-{Guid.NewGuid()}.json");
        try
        {
            var s = Station.Local("Mine", "http://x");
            LibraryFile.Save([s], path);
            Assert.Equal([s], LibraryFile.Load(path));
        }
        finally { File.Delete(path); }
    }
}
