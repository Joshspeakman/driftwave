using System.Buffers.Binary;
using System.Text;

namespace Shortwave.Core;

/// <summary>
/// Reads song titles from an Ogg Vorbis or Opus stream's comment headers. Icecast sends a new chained
/// stream, with new headers, when the song changes. On the Mac AVPlayer reports these comments; Media
/// Foundation doesn't, so the app reads them from its own connection.
/// </summary>
public sealed class OggCommentReader
{
    private readonly List<byte> buffer = [];
    private readonly Dictionary<uint, List<byte>> partial = [];
    private readonly Dictionary<uint, (int Rate, long PreSkip)> streams = [];
    private double finishedSeconds;          // audio in chained streams that already ended
    private double currentSeconds;

    /// <summary>Seconds of audio seen so far (from granule positions).</summary>
    public double Seconds => finishedSeconds + currentSeconds;

    /// <summary>Feeds stream bytes; returns titles found, each with the stream time it starts at.</summary>
    public List<(string Title, double At)> Feed(ReadOnlySpan<byte> data)
    {
        var titles = new List<(string, double)>();
        buffer.AddRange(data);
        int p = 0;
        var buf = System.Runtime.InteropServices.CollectionsMarshal.AsSpan(buffer);
        while (true)
        {
            int start = buf[p..].IndexOf("OggS"u8);
            if (start < 0) { p = Math.Max(p, buf.Length - 3); break; }
            p += start;
            if (buf.Length - p < 27) break;
            int nseg = buf[p + 26];
            if (buf.Length - p < 27 + nseg) break;
            var lacing = buf.Slice(p + 27, nseg);
            int bodyLen = 0;
            foreach (var l in lacing) bodyLen += l;
            if (buf.Length - p < 27 + nseg + bodyLen) break;
            byte type = buf[p + 5];
            long granule = BinaryPrimitives.ReadInt64LittleEndian(buf.Slice(p + 6, 8));
            uint serial = BinaryPrimitives.ReadUInt32LittleEndian(buf.Slice(p + 14, 4));
            if (buf[p + 4] != 0) { p += 4; continue; }    // unknown version: not a page
            if ((type & 0x02) != 0)
            {
                // A new chained stream begins: the previous one's audio is complete.
                finishedSeconds += currentSeconds;
                currentSeconds = 0;
                partial.Remove(serial);
                streams.Remove(serial);
            }
            if ((type & 0x01) == 0) partial.Remove(serial);   // not a continuation: drop stale data
            var body = buf.Slice(p + 27 + nseg, bodyLen);
            int offset = 0;
            var packet = partial.TryGetValue(serial, out var pk) ? pk : [];
            foreach (var l in lacing)
            {
                packet.AddRange(body.Slice(offset, l));
                offset += l;
                if (l < 255)
                {
                    if (HandlePacket(serial, packet.ToArray()) is { } t) titles.Add((t, Seconds));
                    packet = [];
                }
            }
            if (packet.Count > 0 && packet.Count < 1 << 20) partial[serial] = packet; else partial.Remove(serial);
            if (granule > 0 && streams.TryGetValue(serial, out var info) && info.Rate > 0)
                currentSeconds = Math.Max(0, (granule - info.PreSkip) / (double)info.Rate);
            p += 27 + nseg + bodyLen;
        }
        buffer.RemoveRange(0, Math.Min(p, buffer.Count));
        if (buffer.Count > 1 << 20) buffer.Clear();
        return titles;
    }

    string? HandlePacket(uint serial, byte[] packet)
    {
        var s = packet.AsSpan();
        if (s.Length >= 16 && s[0] == 1 && s[1..7].SequenceEqual("vorbis"u8))
        {
            streams[serial] = ((int)BinaryPrimitives.ReadUInt32LittleEndian(s.Slice(12, 4)), 0);
            return null;
        }
        if (s.Length >= 12 && s[..8].SequenceEqual("OpusHead"u8))
        {
            streams[serial] = (48000, BinaryPrimitives.ReadUInt16LittleEndian(s.Slice(10, 2)));
            return null;
        }
        if (s.Length >= 7 && s[0] == 3 && s[1..7].SequenceEqual("vorbis"u8)) return Title(ParseComments(s[7..]));
        if (s.Length >= 8 && s[..8].SequenceEqual("OpusTags"u8)) return Title(ParseComments(s[8..]));
        return null;
    }

    internal static Dictionary<string, string> ParseComments(ReadOnlySpan<byte> s)
    {
        var d = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        try
        {
            int p = 0;
            int vendor = (int)BinaryPrimitives.ReadUInt32LittleEndian(s.Slice(p, 4)); p += 4 + vendor;
            int count = (int)BinaryPrimitives.ReadUInt32LittleEndian(s.Slice(p, 4)); p += 4;
            for (int i = 0; i < count && p + 4 <= s.Length; i++)
            {
                int len = (int)BinaryPrimitives.ReadUInt32LittleEndian(s.Slice(p, 4)); p += 4;
                if (len < 0 || p + len > s.Length) break;
                string kv = Encoding.UTF8.GetString(s.Slice(p, len)); p += len;
                int eq = kv.IndexOf('=');
                if (eq > 0) d.TryAdd(kv[..eq], kv[(eq + 1)..].Trim());
            }
        }
        catch (ArgumentOutOfRangeException) { }
        return d;
    }

    static string? Title(Dictionary<string, string> c)
    {
        c.TryGetValue("TITLE", out var title);
        c.TryGetValue("ARTIST", out var artist);
        if (string.IsNullOrWhiteSpace(title)) return null;
        return string.IsNullOrWhiteSpace(artist) ? title : $"{artist} - {title}";
    }
}
