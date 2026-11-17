// Tests for code that exists only in the Windows port.
using System.Buffers.Binary;
using System.Diagnostics;
using System.Text;

namespace Shortwave.Core.Tests;

public class MacLibraryTests
{
    /// <summary>
    /// A library.json in the Mac's format. The repo has no file captured from the Mac, so this fixture was
    /// written by hand to match Swift's JSONEncoder output ([.prettyPrinted, .sortedKeys]: escaped slashes,
    /// " : " separators, optional values left out).
    /// </summary>
    [Fact]
    public void LoadsMacLibrary()
    {
        var list = LibraryFile.Load(Path.Combine(AppContext.BaseDirectory, "Fixtures", "mac-library.json"));
        Assert.Equal(2, list.Count);
        var soma = list[0];
        Assert.Equal("960cfd3b-0601-11e8-ae97-52543be04c81", soma.Id);
        Assert.Equal("http://ice1.somafm.com/groovesalad-128-mp3", soma.StreamURL);
        Assert.Equal("https://somafm.com/groovesalad/", soma.Homepage);
        Assert.Equal(["ambient", "chillout"], soma.Tags);
        Assert.Equal(37.775, soma.Latitude);
        Assert.Equal(-122.419, soma.Longitude);
        Assert.Equal(4321, soma.Votes);
        Assert.Equal("5C1A7F0E-2B7D-4F64-9E1B-1D2C3B4A5F60.png", soma.CustomCover);
        var local = list[1];
        Assert.True(local.IsLocal);
        Assert.Null(local.Homepage);
        Assert.Null(local.Latitude);
        Assert.Empty(local.Tags);
        Assert.Equal("Café Radio été", local.Name);
    }

    [Fact]
    public void WritesMacCompatibleLayout()
    {
        var s = new Station("id1", "A/B", "http://x/y", tags: ["t"], latitude: 1.5);
        string json = Encoding.UTF8.GetString(LibraryFile.Encode([s]));
        // Sorted keys, optionals left out, stations before version.
        Assert.DoesNotContain("null", json);
        Assert.DoesNotContain("homepage", json);
        Assert.True(json.IndexOf("\"stations\"") < json.IndexOf("\"version\""));
        var keys = System.Text.RegularExpressions.Regex.Matches(json, "\"(\\w+)\":").Select(m => m.Groups[1].Value)
            .Where(k => k != "stations" && k != "version").ToList();
        Assert.Equal(keys.OrderBy(k => k, StringComparer.Ordinal), keys);
        Assert.Equal(["bitrate", "codec", "country", "countryCode", "id", "isHLS", "isLocal", "isOrphaned", "language",
            "latitude", "name", "state", "streamURL", "tags", "votes"], keys);
        // And it reads back.
        string path = Path.Combine(Path.GetTempPath(), $"swlib-{Guid.NewGuid()}.json");
        try { LibraryFile.Save([s], path); Assert.Equal([s], LibraryFile.Load(path)); } finally { File.Delete(path); }
    }
}

public class WindowsFileNameTests
{
    [Fact]
    public void ReservedNamesAndTrailingDots()
    {
        Assert.Equal("_CON", FileNames.Sanitized("CON"));
        Assert.Equal("_nul", FileNames.Sanitized("nul"));
        Assert.Equal("_COM1.remix", FileNames.Sanitized("COM1.remix"));
        Assert.Equal("_LPT9", FileNames.Sanitized("LPT9 "));
        Assert.Equal("Console", FileNames.Sanitized("Console"));
        Assert.Equal("Song", FileNames.Sanitized("Song. . "));
        Assert.Equal("a_b", FileNames.Sanitized("a\tb"));
        Assert.Equal(150, FileNames.Sanitized(new string('x', 400)).Length);
    }

    [Fact]
    public void UniquePaths()
    {
        string dir = Path.Combine(Path.GetTempPath(), "swnames-" + Guid.NewGuid());
        Directory.CreateDirectory(dir);
        try
        {
            string a = FileNames.UniqueFilePath(dir, "Song", "mp3");
            Assert.Equal(Path.Combine(dir, "Song.mp3"), a);
            File.WriteAllText(a, "");
            Assert.Equal(Path.Combine(dir, "Song (2).mp3"), FileNames.UniqueFilePath(dir, "Song", "mp3"));
        }
        finally { Directory.Delete(dir, true); }
    }
}

public class ID3ReaderTests
{
    [Fact]
    public void ReadsBackOwnTag()
    {
        var (title, artist) = ID3.ReadTitleArtist(ID3.Tag("Parlons-nous", "Caroline Dublanche", "RTL"));
        Assert.Equal("Parlons-nous", title);
        Assert.Equal("Caroline Dublanche", artist);
    }

    [Fact]
    public void ReadsV4Latin1()
    {
        byte[] frame(string id, string v) { var body = new byte[] { 0 }.Concat(Encoding.Latin1.GetBytes(v)).ToArray();
            int n = body.Length; return [.. Encoding.ASCII.GetBytes(id), 0, (byte)(n >> 14 & 0x7F), (byte)(n >> 7 & 0x7F), (byte)(n & 0x7F), 0, 0, .. body]; }
        byte[] frames = [.. frame("TIT2", "Café"), .. frame("TPE1", "Band")];
        int s = frames.Length;
        byte[] tag = [.. "ID3"u8.ToArray(), 4, 0, 0, (byte)(s >> 21 & 0x7F), (byte)(s >> 14 & 0x7F), (byte)(s >> 7 & 0x7F), (byte)(s & 0x7F), .. frames];
        Assert.Equal(("Café", "Band"), ID3.ReadTitleArtist(tag));
    }
}

public class AudioFrameTests
{
    /// <summary>A fake ADTS frame: valid 7-byte header (AAC LC, 44.1 kHz, stereo) and a payload.</summary>
    internal static byte[] AdtsFrame(int payloadLength, byte fill)
    {
        int len = payloadLength + 7;
        byte[] h = [0xFF, 0xF1, (byte)((1 << 6) | (4 << 2) | 0), (byte)((2 << 6) | ((len >> 11) & 0x3)), (byte)(len >> 3), (byte)(((len & 0x7) << 5) | 0x1F), 0xFC];
        return [.. h, .. Enumerable.Repeat(fill, payloadLength)];
    }

    [Fact]
    public void ClockCountsAdtsFramesAcrossChunks()
    {
        var stream = new List<byte> { 0x12, 0x34, 0xFF };    // junk before the first frame
        for (int i = 0; i < 43; i++) stream.AddRange(AdtsFrame(100 + i % 7, (byte)i));
        foreach (int chunk in new[] { 1, 5, 64, 100000 })
        {
            var clock = new AudioClock(RecordableFormat.Aac);
            for (int i = 0; i < stream.Count; i += chunk) clock.Feed(stream.Skip(i).Take(chunk).ToArray());
            // The last frame can't be confirmed by a following one until more data arrives, but it is counted
            // once the previous frame has locked the format.
            Assert.InRange(clock.Seconds, 42 * 1024 / 44100.0 - 1e-9, 43 * 1024 / 44100.0 + 1e-9);
        }
    }

    [Fact]
    public void ClockCountsMp3Frames()
    {
        // MPEG-1 Layer III, 128 kbps, 44.1 kHz, no padding: 417 bytes, 1152 samples.
        byte[] frame = [0xFF, 0xFB, 0x90, 0x64, .. new byte[413]];
        Assert.Equal(417, AudioFrames.Mpeg(frame)!.Value.Length);
        var clock = new AudioClock(RecordableFormat.Mp3);
        for (int i = 0; i < 100; i++) clock.Feed(frame);
        Assert.InRange(clock.Seconds, 99 * 1152 / 44100.0 - 1e-9, 100 * 1152 / 44100.0 + 1e-9);
    }
}

public class M4ATests
{
    [Fact]
    public void DropsPartialFramesAndKeepsPayloads()
    {
        var frames = Enumerable.Range(0, 20).Select(i => AudioFrameTests.AdtsFrame(50 + i, (byte)(i + 1))).ToList();
        // Cut in the middle of a frame at both ends, as a recording is.
        byte[] all = [.. frames.SelectMany(f => f)];
        byte[] cut = all[(frames[0].Length - 10)..^25];
        var m4a = M4A.FromAdts(cut, "Song", "Band", "Station");
        Assert.NotNull(m4a);
        // Frames 1..18 survive (frame 0 starts before the cut, frame 19 ends after it).
        var expected = frames.Skip(1).Take(18).SelectMany(f => f.Skip(7)).ToArray();
        int mdat = m4a!.AsSpan().IndexOf("mdat"u8);
        Assert.Equal(expected, m4a[(mdat + 4)..]);
        int stsz = m4a.AsSpan().IndexOf("stsz"u8);
        Assert.Equal(18u, BinaryPrimitives.ReadUInt32BigEndian(m4a.AsSpan(stsz + 12)));
        int stco = m4a.AsSpan().IndexOf("stco"u8);
        Assert.Equal((uint)(mdat + 4), BinaryPrimitives.ReadUInt32BigEndian(m4a.AsSpan(stco + 12)));
        Assert.True(m4a.AsSpan().IndexOf(Encoding.UTF8.GetBytes("Station")) > 0);
    }

    [Fact]
    public void RejectsNonAdts()
    {
        Assert.Null(M4A.FromAdts(new byte[5000], "x", null, null));
    }

    /// <summary>
    /// With ffmpeg installed: encode real AAC, cut it at odd bytes, mux it, and check with ffprobe/ffmpeg that the
    /// result plays, carries the tags and holds the same AAC frames (passthrough, no re-encoding).
    /// </summary>
    [Fact]
    public void RealAacPassthroughWithFfmpeg()
    {
        if (!Tool.Exists("ffmpeg") || !Tool.Exists("ffprobe")) return;
        string dir = Path.Combine(Path.GetTempPath(), "swm4a-" + Guid.NewGuid());
        Directory.CreateDirectory(dir);
        try
        {
            string src = Path.Combine(dir, "src.aac");
            Tool.Run("ffmpeg", $"-loglevel error -f lavfi -i sine=frequency=500:duration=8 -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts \"{src}\"");
            byte[] all = File.ReadAllBytes(src);
            byte[] cut = all[1234..^777];
            string m4a = Path.Combine(dir, "out.m4a");
            File.WriteAllBytes(m4a, M4A.FromAdts(cut, "Test Song", "Test Artist", "Test Station")!);

            string probe = Tool.Run("ffprobe", $"-v error -show_entries stream=codec_name,sample_rate,channels:format_tags=title,artist,album -of default=nw=1 \"{m4a}\"");
            Assert.Contains("codec_name=aac", probe);
            Assert.Contains("sample_rate=44100", probe);
            Assert.Contains("channels=2", probe);
            Assert.Contains("TAG:title=Test Song", probe);
            Assert.Contains("TAG:artist=Test Artist", probe);
            Assert.Contains("TAG:album=Test Station", probe);

            // Decodes without errors.
            Tool.Run("ffmpeg", $"-v error -xerror -i \"{m4a}\" -f null -");

            // Copy the frames back out to ADTS: the payloads must equal the source's whole frames.
            string back = Path.Combine(dir, "back.aac");
            Tool.Run("ffmpeg", $"-loglevel error -i \"{m4a}\" -c copy -f adts \"{back}\"");
            var a = M4A.ParseAdts(cut)!; var b = M4A.ParseAdts(File.ReadAllBytes(back))!;
            byte[] backBytes = File.ReadAllBytes(back);
            Assert.Equal(a.Payloads.Count, b.Payloads.Count);
            for (int i = 0; i < a.Payloads.Count; i++)
                Assert.True(cut.AsSpan(a.Payloads[i].Offset, a.Payloads[i].Length).SequenceEqual(backBytes.AsSpan(b.Payloads[i].Offset, b.Payloads[i].Length)), $"frame {i}");
        }
        finally { Directory.Delete(dir, true); }
    }
}

public class OggTests
{
    static byte[] Page(uint serial, byte type, long granule, uint seq, params byte[][] packets)
    {
        var lacing = new List<byte>(); var body = new List<byte>();
        foreach (var p in packets)
        {
            int n = p.Length;
            while (n >= 255) { lacing.Add(255); n -= 255; }
            lacing.Add((byte)n);
            body.AddRange(p);
        }
        var h = new byte[27];
        "OggS"u8.CopyTo(h); h[4] = 0; h[5] = type;
        BinaryPrimitives.WriteInt64LittleEndian(h.AsSpan(6), granule);
        BinaryPrimitives.WriteUInt32LittleEndian(h.AsSpan(14), serial);
        BinaryPrimitives.WriteUInt32LittleEndian(h.AsSpan(18), seq);
        h[26] = (byte)lacing.Count;
        return [.. h, .. lacing, .. body];
    }

    static byte[] VorbisId(int rate) { var b = new byte[30]; b[0] = 1; "vorbis"u8.CopyTo(b.AsSpan(1)); b[11] = 2; BinaryPrimitives.WriteUInt32LittleEndian(b.AsSpan(12), (uint)rate); return b; }
    static byte[] VorbisComments(params string[] kv)
    {
        var b = new List<byte> { 3 }; b.AddRange("vorbis"u8.ToArray());
        byte[] vendor = Encoding.UTF8.GetBytes("test");
        b.AddRange(BitConverter.GetBytes(vendor.Length)); b.AddRange(vendor);
        b.AddRange(BitConverter.GetBytes(kv.Length));
        foreach (var s in kv) { var x = Encoding.UTF8.GetBytes(s); b.AddRange(BitConverter.GetBytes(x.Length)); b.AddRange(x); }
        b.Add(1);
        return [.. b];
    }

    [Fact]
    public void ReadsChainedVorbisTitles()
    {
        byte[] stream = [
            .. Page(1, 2, 0, 0, VorbisId(44100)),
            .. Page(1, 0, 0, 1, VorbisComments("ARTIST=Band", "TITLE=First Song")),
            .. Page(1, 0, 441000, 2, new byte[300]),
            .. Page(2, 2, 0, 0, VorbisId(44100)),
            .. Page(2, 0, 0, 1, VorbisComments("title=Only Title")),
        ];
        foreach (int chunk in new[] { 1, 17, 100000 })
        {
            var r = new OggCommentReader(); var titles = new List<(string, double)>();
            for (int i = 0; i < stream.Length; i += chunk) titles.AddRange(r.Feed(stream.AsSpan(i, Math.Min(chunk, stream.Length - i))));
            Assert.Equal(2, titles.Count);
            Assert.Equal("Band - First Song", titles[0].Item1);
            Assert.Equal(0, titles[0].Item2, 3);
            Assert.Equal("Only Title", titles[1].Item1);
            Assert.Equal(10, titles[1].Item2, 3);
        }
    }
}

static class Tool
{
    public static bool Exists(string name) =>
        (Environment.GetEnvironmentVariable("PATH") ?? "").Split(Path.PathSeparator)
            .Any(d => File.Exists(Path.Combine(d, name + ".exe")) || File.Exists(Path.Combine(d, name)));

    public static string Run(string exe, string args)
    {
        var p = Process.Start(new ProcessStartInfo(exe, args) { RedirectStandardOutput = true, RedirectStandardError = true, UseShellExecute = false })!;
        var err = p.StandardError.ReadToEndAsync();
        string o = p.StandardOutput.ReadToEnd();
        p.WaitForExit();
        if (p.ExitCode != 0) throw new Exception($"{exe} {args} failed: {err.Result}");
        return o + err.Result;
    }
}
