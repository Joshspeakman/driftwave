using System.Text;

namespace Shortwave.Core;

/// <summary>Builds a minimal ID3v2.3 tag (UTF-16 text frames) to put in front of a saved MP3.</summary>
public static class ID3
{
    public static byte[] Tag(string title, string? artist, string? album, string? comment = null)
    {
        var frames = new List<byte>();
        frames.AddRange(TextFrame("TIT2", title));
        if (!string.IsNullOrEmpty(artist)) frames.AddRange(TextFrame("TPE1", artist));
        if (!string.IsNullOrEmpty(album)) frames.AddRange(TextFrame("TALB", album));
        if (!string.IsNullOrEmpty(comment)) frames.AddRange(CommentFrame(comment));

        var tag = new List<byte>(Encoding.ASCII.GetBytes("ID3"));
        tag.AddRange([3, 0, 0]);                 // v2.3.0, no flags
        tag.AddRange(Synchsafe(frames.Count));
        tag.AddRange(frames);
        return tag.ToArray();
    }

    internal static byte[] Utf16WithBom(string s)
    {
        var d = new List<byte> { 0xFF, 0xFE };
        foreach (char unit in s) { d.Add((byte)(unit & 0xFF)); d.Add((byte)(unit >> 8)); }
        return d.ToArray();
    }

    internal static byte[] TextFrame(string id, string value)
    {
        var body = new List<byte> { 1 };          // encoding: UTF-16 with BOM
        body.AddRange(Utf16WithBom(value));
        return Frame(id, body.ToArray());
    }

    internal static byte[] CommentFrame(string value)
    {
        var body = new List<byte> { 1 };
        body.AddRange(Encoding.ASCII.GetBytes("eng"));
        body.AddRange(Utf16WithBom(""));         // empty description
        body.AddRange([0, 0]);
        body.AddRange(Utf16WithBom(value));
        return Frame("COMM", body.ToArray());
    }

    internal static byte[] Frame(string id, byte[] body)
    {
        var f = new List<byte>(Encoding.ASCII.GetBytes(id));
        uint n = (uint)body.Length;
        f.AddRange([(byte)(n >> 24), (byte)((n >> 16) & 0xFF), (byte)((n >> 8) & 0xFF), (byte)(n & 0xFF)]);
        f.AddRange([0, 0]);
        f.AddRange(body);
        return f.ToArray();
    }

    internal static byte[] Synchsafe(int n) =>
        [(byte)((n >> 21) & 0x7F), (byte)((n >> 14) & 0x7F), (byte)((n >> 7) & 0x7F), (byte)(n & 0x7F)];

    /// <summary>
    /// Reads the title and artist (TIT2 / TPE1) from an ID3v2 tag, as HLS streams send them in
    /// timed metadata. On the Mac AVPlayer decodes these; on Windows the app gets the raw tag bytes.
    /// </summary>
    public static (string? Title, string? Artist) ReadTitleArtist(ReadOnlySpan<byte> tag)
    {
        string? title = null, artist = null;
        int start = tag.IndexOf("ID3"u8);
        if (start < 0 || tag.Length - start < 10) return (null, null);
        tag = tag[start..];
        int version = tag[3];
        bool unsync = (tag[5] & 0x80) != 0;
        bool extended = (tag[5] & 0x40) != 0;
        int size = (tag[6] & 0x7F) << 21 | (tag[7] & 0x7F) << 14 | (tag[8] & 0x7F) << 7 | (tag[9] & 0x7F);
        var body = tag.Slice(10, Math.Min(size, tag.Length - 10)).ToArray();
        if (unsync && version < 4) body = RemoveUnsync(body);
        int p = 0;
        if (extended && body.Length >= 4)
        {
            int ext = version >= 4
                ? (body[0] & 0x7F) << 21 | (body[1] & 0x7F) << 14 | (body[2] & 0x7F) << 7 | (body[3] & 0x7F)
                : (body[0] << 24 | body[1] << 16 | body[2] << 8 | body[3]) + 4;
            p = Math.Min(ext, body.Length);
        }
        int idLen = version == 2 ? 3 : 4, headLen = version == 2 ? 6 : 10;
        while (p + headLen <= body.Length)
        {
            string id = Encoding.ASCII.GetString(body, p, idLen);
            if (id[0] == '\0') break;
            int fsize = version switch
            {
                2 => body[p + 3] << 16 | body[p + 4] << 8 | body[p + 5],
                4 => (body[p + 4] & 0x7F) << 21 | (body[p + 5] & 0x7F) << 14 | (body[p + 6] & 0x7F) << 7 | (body[p + 7] & 0x7F),
                _ => body[p + 4] << 24 | body[p + 5] << 16 | body[p + 6] << 8 | body[p + 7],
            };
            p += headLen;
            if (fsize <= 0 || p + fsize > body.Length) break;
            var frame = body.AsSpan(p, fsize);
            if (id is "TIT2" or "TT2") title = DecodeTextFrame(frame);
            else if (id is "TPE1" or "TP1") artist = DecodeTextFrame(frame);
            p += fsize;
        }
        return (string.IsNullOrWhiteSpace(title) ? null : title.Trim(), string.IsNullOrWhiteSpace(artist) ? null : artist.Trim());
    }

    private static byte[] RemoveUnsync(byte[] data)
    {
        var o = new List<byte>(data.Length);
        for (int i = 0; i < data.Length; i++)
        {
            o.Add(data[i]);
            if (data[i] == 0xFF && i + 1 < data.Length && data[i + 1] == 0) i++;
        }
        return o.ToArray();
    }

    private static string DecodeTextFrame(ReadOnlySpan<byte> f)
    {
        if (f.Length < 1) return "";
        var text = f[1..];
        string s = f[0] switch
        {
            0 => Encoding.Latin1.GetString(text),
            1 => DecodeUtf16WithBom(text),
            2 => Encoding.BigEndianUnicode.GetString(text),
            _ => Encoding.UTF8.GetString(text),
        };
        // Multiple values are NUL-separated; keep the first.
        int nul = s.IndexOf('\0');
        return nul >= 0 ? s[..nul] : s;
    }

    private static string DecodeUtf16WithBom(ReadOnlySpan<byte> t)
    {
        if (t.Length >= 2 && t[0] == 0xFE && t[1] == 0xFF) return Encoding.BigEndianUnicode.GetString(t[2..]);
        if (t.Length >= 2 && t[0] == 0xFF && t[1] == 0xFE) return Encoding.Unicode.GetString(t[2..]);
        return Encoding.Unicode.GetString(t);
    }
}
