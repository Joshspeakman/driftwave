using System.Buffers.Binary;
using System.Text;

namespace Shortwave.Core;

/// <summary>
/// Wraps a raw ADTS (AAC) recording in an MPEG-4 audio file (.m4a) with iTunes-style tags, without
/// re-encoding: every AAC frame is copied unchanged. The Mac app does this with AVAssetExportSession's
/// passthrough preset.
/// </summary>
public static class M4A
{
    public sealed record Frames(List<(int Offset, int Length)> Payloads, int ObjectType, int SampleRateIndex, int SampleRate, int Channels);

    /// <summary>
    /// Finds the AAC frames in an ADTS byte stream. Recordings are cut at arbitrary bytes, so leading
    /// bytes before the first real frame and a partial frame at the end are dropped. Returns null when the
    /// stream can't be passed through (no frames, multiple raw blocks, or a format change mid-stream).
    /// </summary>
    public static Frames? ParseAdts(ReadOnlySpan<byte> data)
    {
        var payloads = new List<(int, int)>();
        FrameHeader? first = null;
        int p = 0;
        while (p + 7 <= data.Length)
        {
            var h = AudioFrames.Adts(data[p..]);
            bool ok = false;
            if (h is { } f && p + f.Length <= data.Length)
            {
                if (first is { } fh) ok = Matches(fh, f);
                else
                {
                    // Lock on only when the next frame lines up too (two frames in a row), unless this frame ends the data.
                    int next = p + f.Length;
                    ok = next == data.Length || (AudioFrames.Adts(data[next..]) is { } g && Matches(f, g));
                }
            }
            if (!ok)
            {
                if (first != null && h is { } partial && p + partial.Length > data.Length) break;   // partial last frame
                if (first != null && h is { } other && !Matches(first.Value, other) && other.Length >= other.HeaderLength && p + other.Length <= data.Length
                    && (p + other.Length == data.Length || AudioFrames.Adts(data[(p + other.Length)..]) is not null))
                    return null;   // a real format change: don't produce a broken file
                p++;
                continue;
            }
            var fr = h!.Value;
            if (fr.RawBlocks != 0) return null;
            first ??= fr;
            payloads.Add((p + fr.HeaderLength, fr.Length - fr.HeaderLength));
            p += fr.Length;
        }
        if (first is not { } hdr || payloads.Count == 0 || hdr.Channels == 0) return null;
        return new Frames(payloads, hdr.Profile + 1, hdr.SampleRateIndex, hdr.SampleRate, hdr.Channels);
    }

    static bool Matches(FrameHeader a, FrameHeader b) =>
        a.Profile == b.Profile && a.SampleRateIndex == b.SampleRateIndex && a.Channels == b.Channels;

    /// <summary>Builds the .m4a file. Returns null when the ADTS data can't be passed through.</summary>
    public static byte[]? FromAdts(ReadOnlySpan<byte> adts, string title, string? artist, string? album)
    {
        var frames = ParseAdts(adts);
        if (frames is null) return null;
        int count = frames.Payloads.Count;
        long dataSize = frames.Payloads.Sum(x => (long)x.Length);
        if (dataSize > uint.MaxValue - 1024) return null;
        long samples = 1024L * count;
        int rate = frames.SampleRate;
        double seconds = (double)samples / rate;
        uint avgBitrate = seconds > 0 ? (uint)(dataSize * 8 / seconds) : 0;
        uint maxFrame = (uint)frames.Payloads.Max(x => x.Length);
        uint maxBitrate = (uint)Math.Max(avgBitrate, maxFrame * 8.0 * rate / 1024);

        byte[] ftyp = Box("ftyp", [.. Ascii("M4A "), .. U32(0), .. Ascii("M4A "), .. Ascii("mp42"), .. Ascii("isom")]);
        byte[] Moov(uint chunkOffset) => BuildMoov(frames, samples, rate, avgBitrate, maxBitrate, maxFrame, chunkOffset, title, artist, album);
        int moovSize = Moov(0).Length;
        uint offset = (uint)(ftyp.Length + moovSize + 8);
        byte[] moov = Moov(offset);

        using var ms = new MemoryStream((int)(offset + dataSize));
        ms.Write(ftyp);
        ms.Write(moov);
        ms.Write(U32((uint)(dataSize + 8)));
        ms.Write(Ascii("mdat"));
        foreach (var (o, l) in frames.Payloads) ms.Write(adts.Slice(o, l));
        return ms.ToArray();
    }

    static byte[] BuildMoov(Frames f, long samples, int rate, uint avgBitrate, uint maxBitrate, uint maxFrame, uint chunkOffset,
        string title, string? artist, string? album)
    {
        uint now = (uint)(DateTimeOffset.UtcNow.ToUnixTimeSeconds() + 2082844800L);   // seconds since 1904
        uint mtimescale = (uint)rate;
        uint duration = (uint)Math.Min(samples, uint.MaxValue);
        byte[] matrix = [.. U32(0x00010000), .. U32(0), .. U32(0), .. U32(0), .. U32(0x00010000), .. U32(0), .. U32(0), .. U32(0), .. U32(0x40000000)];

        byte[] mvhd = FullBox("mvhd", 0, 0, [.. U32(now), .. U32(now), .. U32(mtimescale), .. U32(duration),
            .. U32(0x00010000), .. U16(0x0100), .. new byte[10], .. matrix, .. new byte[24], .. U32(2)]);

        byte[] tkhd = FullBox("tkhd", 0, 7, [.. U32(now), .. U32(now), .. U32(1), .. U32(0), .. U32(duration),
            .. new byte[8], .. U16(0), .. U16(0), .. U16(0x0100), .. U16(0), .. matrix, .. U32(0), .. U32(0)]);

        byte[] mdhd = FullBox("mdhd", 0, 0, [.. U32(now), .. U32(now), .. U32((uint)rate), .. U32(duration), .. U16(0x55C4), .. U16(0)]);
        byte[] hdlr = FullBox("hdlr", 0, 0, [.. U32(0), .. Ascii("soun"), .. new byte[12], .. Encoding.UTF8.GetBytes("SoundHandler"), 0]);

        // AudioSpecificConfig: object type (5 bits), sampling frequency index (4), channel configuration (4), 3 zero bits.
        int asc = (f.ObjectType << 11) | (f.SampleRateIndex << 7) | (f.Channels << 3);
        byte[] decSpecific = Descriptor(0x05, [(byte)(asc >> 8), (byte)asc]);
        byte[] decConfig = Descriptor(0x04, [0x40, 0x15, (byte)(maxFrame >> 16), (byte)(maxFrame >> 8), (byte)maxFrame,
            .. U32(maxBitrate), .. U32(avgBitrate), .. decSpecific]);
        byte[] slConfig = Descriptor(0x06, [0x02]);
        byte[] es = Descriptor(0x03, [.. U16(1), 0, .. decConfig, .. slConfig]);
        byte[] esds = FullBox("esds", 0, 0, es);
        uint boxRate = rate <= 0xFFFF ? (uint)rate << 16 : 0;
        byte[] mp4a = Box("mp4a", [.. new byte[6], .. U16(1), .. new byte[8], .. U16((ushort)Math.Min(f.Channels, 2)), .. U16(16),
            .. U16(0), .. U16(0), .. U32(boxRate), .. esds]);
        byte[] stsd = FullBox("stsd", 0, 0, [.. U32(1), .. mp4a]);
        byte[] stts = FullBox("stts", 0, 0, [.. U32(1), .. U32((uint)f.Payloads.Count), .. U32(1024)]);
        byte[] stsc = FullBox("stsc", 0, 0, [.. U32(1), .. U32(1), .. U32((uint)f.Payloads.Count), .. U32(1)]);
        var sizes = new List<byte>(12 + 4 * f.Payloads.Count);
        sizes.AddRange(U32(0)); sizes.AddRange(U32((uint)f.Payloads.Count));
        foreach (var (_, l) in f.Payloads) sizes.AddRange(U32((uint)l));
        byte[] stsz = FullBox("stsz", 0, 0, [.. sizes]);
        byte[] stco = FullBox("stco", 0, 0, [.. U32(1), .. U32(chunkOffset)]);
        byte[] stbl = Box("stbl", [.. stsd, .. stts, .. stsc, .. stsz, .. stco]);
        byte[] dinf = Box("dinf", FullBox("dref", 0, 0, [.. U32(1), .. FullBox("url ", 0, 1, [])]));
        byte[] smhd = FullBox("smhd", 0, 0, [.. U16(0), .. U16(0)]);
        byte[] minf = Box("minf", [.. smhd, .. dinf, .. stbl]);
        byte[] mdia = Box("mdia", [.. mdhd, .. hdlr, .. minf]);
        byte[] trak = Box("trak", [.. tkhd, .. mdia]);

        var items = new List<byte>();
        items.AddRange(TagItem("©nam", title));
        if (!string.IsNullOrEmpty(artist)) items.AddRange(TagItem("©ART", artist));
        if (!string.IsNullOrEmpty(album)) items.AddRange(TagItem("©alb", album));
        items.AddRange(TagItem("©too", "Shortwave"));
        byte[] metaHdlr = FullBox("hdlr", 0, 0, [.. U32(0), .. Ascii("mdir"), .. Ascii("appl"), .. new byte[8], 0]);
        byte[] meta = FullBox("meta", 0, 0, [.. metaHdlr, .. Box("ilst", [.. items])]);
        byte[] udta = Box("udta", meta);

        return Box("moov", [.. mvhd, .. trak, .. udta]);
    }

    static byte[] TagItem(string key, string value) =>
        Box(key, Box("data", [.. U32(1), .. U32(0), .. Encoding.UTF8.GetBytes(value)]));

    static byte[] Descriptor(byte tag, byte[] body)
    {
        // Four-byte length form, as most muxers write it.
        int n = body.Length;
        return [tag, (byte)(0x80 | ((n >> 21) & 0x7F)), (byte)(0x80 | ((n >> 14) & 0x7F)), (byte)(0x80 | ((n >> 7) & 0x7F)), (byte)(n & 0x7F), .. body];
    }

    static byte[] Box(string type, byte[] body) => [.. U32((uint)(body.Length + 8)), .. TypeBytes(type), .. body];
    static byte[] FullBox(string type, byte version, uint flags, byte[] body) =>
        Box(type, [version, (byte)(flags >> 16), (byte)(flags >> 8), (byte)flags, .. body]);
    static byte[] TypeBytes(string type) => type.Select(c => (byte)c).ToArray();   // © is 0xA9 in Latin-1
    static byte[] Ascii(string s) => Encoding.ASCII.GetBytes(s);
    static byte[] U32(uint v) { var b = new byte[4]; BinaryPrimitives.WriteUInt32BigEndian(b, v); return b; }
    static byte[] U16(ushort v) { var b = new byte[2]; BinaryPrimitives.WriteUInt16BigEndian(b, v); return b; }
}
