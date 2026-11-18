// 1:1 with the Mac's ExchangeTests (Tests/ShortwaveTests/CoreTests.swift), plus the shared fixtures.
using System.Runtime.CompilerServices;
using System.Text;

namespace Shortwave.Core.Tests;

public class ExchangeTests
{
    static readonly byte[] Png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    /// <summary>Shared fixtures at the repo root, written by each app's export (see docs/STATION-EXCHANGE.md).</summary>
    static string Fixtures([CallerFilePath] string here = "") =>
        Path.GetFullPath(Path.Combine(Path.GetDirectoryName(here)!, "..", "..", "..", "fixtures", "exchange"));

    static Func<string> Names() { int n = 0; return () => $"new{++n}"; }

    [Fact]
    public void RoundTripWithCover()
    {
        var local = Station.Local("Mine", "http://x/live", customCover: "a.png");
        var pub = new Station("uuid-1", "Public", "http://p", tags: ["jazz"], bitrate: 128, latitude: 1.5);
        var data = StationExchange.Encode([local, pub], "test", cover: n => n == "a.png" ? Png : null);
        string text = Encoding.UTF8.GetString(data);
        Assert.Contains("\"format\": \"shortwave-stations\"", text);
        var c = StationExchange.Decode(data);
        Assert.Equal([local, pub], c.Stations);
        Assert.Equal(["a.png"], c.Covers.Keys);
        Assert.Equal(Png, c.Covers["a.png"]);
        Assert.Equal(0, c.Unreadable);
        // An exchange file is also a library file.
        string path = Path.Combine(Path.GetTempPath(), $"swx-{Guid.NewGuid()}.json");
        try
        {
            File.WriteAllBytes(path, data);
            Assert.Equal([local, pub], LibraryFile.Load(path));
        }
        finally { File.Delete(path); }
    }

    [Fact]
    public void ReadsLibraryFileAndBareArray()
    {
        var s = Station.Local("Mine", "http://x");
        Assert.Equal([s], StationExchange.Decode(LibraryFile.Encode([s])).Stations);
        using var array = new MemoryStream();
        using (var w = new System.Text.Json.Utf8JsonWriter(array))
        {
            w.WriteStartArray();
            s.WriteJson(w);
            w.WriteEndArray();
        }
        Assert.Equal([s], StationExchange.Decode(array.ToArray()).Stations);
    }

    [Fact]
    public void LenientStations()
    {
        const string json = """
        {"stations": [
          {"id": "a", "name": "Only the required keys", "streamURL": "http://a"},
          {"id": "b", "name": "Wrong types", "streamURL": "http://b", "bitrate": "96", "isHLS": 1, "tags": ["x", null], "votes": null},
          {"name": "no id", "streamURL": "http://c"},
          {"id": "d", "name": "empty url", "streamURL": " "},
          "not a station"
        ]}
        """;
        var c = StationExchange.Decode(Encoding.UTF8.GetBytes(json));
        Assert.Equal(["a", "b"], c.Stations.Select(s => s.Id));
        Assert.True(c.Stations[1].Bitrate == 96 && c.Stations[1].IsHLS && c.Stations[1].Tags.SequenceEqual(["x"]));
        Assert.Equal(3, c.Unreadable);
    }

    [Fact]
    public void RejectsOtherFiles()
    {
        Assert.Equal(StationExchange.ReadError.NotJSON,
            Assert.Throws<StationExchange.ReadException>(() => StationExchange.Decode(Encoding.UTF8.GetBytes("not json"))).Error);
        Assert.Equal(StationExchange.ReadError.NoStations,
            Assert.Throws<StationExchange.ReadException>(() => StationExchange.Decode(Encoding.UTF8.GetBytes("{\"a\": 1}"))).Error);
    }

    [Fact]
    public void MergeAddsOnlyNewStations()
    {
        Station[] have =
        [
            new("uuid-1", "Kept", "http://p", customCover: "mine.png"),
            new("local-1", "Mine", "http://X/Live ", isLocal: true),
        ];
        var incoming = new StationExchange.Contents(
        [
            new("uuid-1", "Renamed elsewhere", "http://p"),                 // same id: keep ours
            new("local-2", "Same stream", "http://x/live", isLocal: true),  // same local stream
            new("uuid-2", "New", "http://n", customCover: "c.png"),
            new("uuid-3", "Cover missing", "http://m", customCover: "gone.png"),
            new("uuid-2", "Twice in the file", "http://n"),
        ], new() { ["c.png"] = Png }, 1);
        var r = StationExchange.Merge(incoming, have, Names());
        Assert.Equal(["uuid-1", "local-1", "uuid-2", "uuid-3"], r.Library.Select(s => s.Id));
        Assert.True(r.Library[0].Name == "Kept" && r.Library[0].CustomCover == "mine.png");
        Assert.Equal(["uuid-2", "uuid-3"], r.Added.Select(s => s.Id));
        Assert.True(r.Added[0].CustomCover == "new1.png" && r.Added[1].CustomCover == null);
        Assert.Equal(["new1.png"], r.NewCovers.Keys);
        Assert.Equal(Png, r.NewCovers["new1.png"]);
        Assert.Equal(3, r.AlreadyPresent);
        Assert.Equal(1, r.Unreadable);
        // The incoming stations aren't changed.
        Assert.Equal("c.png", incoming.Stations[2].CustomCover);
    }

    [Fact]
    public void ReadsMacExport() => CheckFixture("mac-export.json");

    [Fact]
    public void ReadsWindowsExport() => CheckFixture("windows-export.json");

    static void CheckFixture(string name)
    {
        var c = StationExchange.Decode(File.ReadAllBytes(Path.Combine(Fixtures(), name)));
        // Both fixtures hold the same three stations (docs/STATION-EXCHANGE.md, "Test fixtures").
        Assert.Equal(0, c.Unreadable);
        Assert.Equal(3, c.Stations.Count);
        var pub = Assert.Single(c.Stations, s => s.Id == "960cfd3b-0601-11e8-ae97-52543be04c81");
        Assert.True(pub.Name == "SomaFM Groove Salad" && pub.Tags.SequenceEqual(["ambient", "chillout"]) && pub.Bitrate == 128);
        Assert.True(pub.Latitude == 37.775 && pub.Longitude == -122.419);
        var local = Assert.Single(c.Stations, s => s.IsLocal);
        Assert.True(local.Name == "Exchange Test Local" && local.StreamURL == "http://127.0.0.1:8765/mp3");
        Assert.NotNull(local.CustomCover);
        Assert.True(c.Covers.TryGetValue(local.CustomCover!, out var image) && StationExchange.IsImage(image));
        var orphan = Assert.Single(c.Stations, s => s.IsOrphaned);
        Assert.True(orphan.Name == "Exchange Test Orphan" && orphan.Homepage == "https://example.com/");
        var r = StationExchange.Merge(c, [], Names());
        Assert.True(r.Added.Count == 3 && r.NewCovers.Count == 1);
    }
}
