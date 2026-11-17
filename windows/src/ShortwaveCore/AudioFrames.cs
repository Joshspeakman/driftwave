namespace Shortwave.Core;

/// <summary>Header of one MP3 or ADTS (AAC) frame.</summary>
public readonly record struct FrameHeader(int Length, int HeaderLength, int Samples, int SampleRate, int Profile, int SampleRateIndex, int Channels, int RawBlocks);

/// <summary>MPEG audio (MP3) and ADTS (AAC) frame headers.</summary>
public static class AudioFrames
{
    static readonly int[] AdtsRates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];

    static readonly int[,] Mp3Rates = { { 11025, 12000, 8000 }, { 0, 0, 0 }, { 22050, 24000, 16000 }, { 44100, 48000, 32000 } };
    static readonly int[] V1L1 = [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448];
    static readonly int[] V1L2 = [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384];
    static readonly int[] V1L3 = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
    static readonly int[] V2L1 = [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256];
    static readonly int[] V2L23 = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

    /// <summary>Parses an ADTS header at the start of <paramref name="b"/>; null if it isn't one.</summary>
    public static FrameHeader? Adts(ReadOnlySpan<byte> b)
    {
        if (b.Length < 7 || b[0] != 0xFF || (b[1] & 0xF6) != 0xF0) return null;   // sync, layer 00
        bool crc = (b[1] & 0x01) == 0;
        int profile = (b[2] >> 6) & 0x3;
        int sfi = (b[2] >> 2) & 0xF;
        if (sfi >= AdtsRates.Length) return null;
        int channels = ((b[2] & 0x1) << 2) | ((b[3] >> 6) & 0x3);
        int length = ((b[3] & 0x3) << 11) | (b[4] << 3) | ((b[5] >> 5) & 0x7);
        int blocks = b[6] & 0x3;
        int head = crc ? 9 : 7;
        if (length < head) return null;
        return new FrameHeader(length, head, 1024 * (blocks + 1), AdtsRates[sfi], profile, sfi, channels, blocks);
    }

    /// <summary>Parses an MPEG audio header at the start of <paramref name="b"/>; null if it isn't one.</summary>
    public static FrameHeader? Mpeg(ReadOnlySpan<byte> b)
    {
        if (b.Length < 4 || b[0] != 0xFF || (b[1] & 0xE0) != 0xE0) return null;
        int version = (b[1] >> 3) & 0x3;     // 0: 2.5, 2: 2, 3: 1
        int layer = (b[1] >> 1) & 0x3;       // 1: III, 2: II, 3: I
        int bri = (b[2] >> 4) & 0xF;
        int sri = (b[2] >> 2) & 0x3;
        int pad = (b[2] >> 1) & 0x1;
        if (version == 1 || layer == 0 || bri == 0 || bri == 15 || sri == 3) return null;
        int rate = Mp3Rates[version, sri];
        bool v1 = version == 3;
        int kbps = (v1, layer) switch
        {
            (true, 3) => V1L1[bri],
            (true, 2) => V1L2[bri],
            (true, _) => V1L3[bri],
            (false, 3) => V2L1[bri],
            _ => V2L23[bri],
        };
        int br = kbps * 1000;
        int length, samples;
        if (layer == 3) { length = (12 * br / rate + pad) * 4; samples = 384; }
        else if (layer == 2) { length = 144 * br / rate + pad; samples = 1152; }
        else { length = (v1 ? 144 : 72) * br / rate + pad; samples = v1 ? 1152 : 576; }
        int channels = ((b[3] >> 6) & 0x3) == 3 ? 1 : 2;
        return new FrameHeader(length, 4, samples, rate, 0, sri, channels, 0);
    }

    public static FrameHeader? Header(RecordableFormat format, ReadOnlySpan<byte> b) =>
        format == RecordableFormat.Aac ? Adts(b) : Mpeg(b);
}

/// <summary>
/// Counts how many seconds of audio have passed through a stream by walking its MP3 or ADTS frames.
/// The ICY connection uses it to know where in the stream a title changed, so the title can be shown
/// when the player reaches that point (Media Foundation reports no buffered ranges for live streams).
/// </summary>
public sealed class AudioClock(RecordableFormat format)
{
    private readonly List<byte> pending = [];
    private int skip;          // bytes of the current frame still to pass
    private FrameHeader? last;

    public double Seconds { get; private set; }
    public RecordableFormat Format { get; } = format;

    public void Feed(ReadOnlySpan<byte> data)
    {
        int i = 0;
        if (skip > 0)
        {
            int n = Math.Min(skip, data.Length);
            skip -= n;
            i = n;
        }
        if (i >= data.Length) return;
        pending.AddRange(data[i..]);
        var buf = System.Runtime.InteropServices.CollectionsMarshal.AsSpan(pending);
        int p = 0;
        while (buf.Length - p >= 9)
        {
            var h = AudioFrames.Header(Format, buf[p..]);
            // Accept a frame when it matches the previous one's format, or (to lock on) when the next frame lines up too.
            bool ok = h is { } f && (last is { } l && Same(l, f)
                || (buf.Length - p >= f.Length + 9 && AudioFrames.Header(Format, buf[(p + f.Length)..]) is { } g && Same(f, g)));
            if (!ok)
            {
                if (h != null && last == null && buf.Length - p < h.Value.Length + 9) break;   // wait for more data to confirm
                p++;
                continue;
            }
            var hf = h!.Value;
            last = hf;
            Seconds += (double)hf.Samples / hf.SampleRate;
            if (buf.Length - p >= hf.Length) p += hf.Length;
            else { skip = hf.Length - (buf.Length - p); p = buf.Length; }
        }
        pending.RemoveRange(0, p);
        if (pending.Count > 1 << 16) pending.RemoveRange(0, pending.Count - 16);   // garbage: don't grow forever
    }

    static bool Same(FrameHeader a, FrameHeader b) => a.SampleRate == b.SampleRate && a.Channels == b.Channels && a.Profile == b.Profile;
}
