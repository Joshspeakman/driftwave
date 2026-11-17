using System.Text;

namespace Shortwave.Core;

/// <summary>One piece of a parsed ICY stream: either audio bytes or a metadata block's text.</summary>
public abstract record IcyEvent
{
    public sealed record Audio(byte[] Data) : IcyEvent;
    public sealed record Metadata(string Text) : IcyEvent;
}

/// <summary>
/// Splits a Shoutcast/Icecast byte stream (sent with <c>Icy-MetaData: 1</c>) into audio and metadata.
/// Every <c>metaInt</c> audio bytes the server inserts one length byte (n) followed by n*16 bytes of
/// metadata text, e.g. <c>StreamTitle='Artist - Title';StreamUrl='';</c> padded with NULs.
/// </summary>
public sealed class IcyParser
{
    public int MetaInt { get; }
    private int audioLeft;
    private int metaLeft;
    private bool expectingLength;
    private readonly MemoryStream metaBuffer = new();

    public IcyParser(int metaInt)
    {
        if (metaInt <= 0) throw new ArgumentOutOfRangeException(nameof(metaInt));
        MetaInt = metaInt;
        audioLeft = metaInt;
    }

    public List<IcyEvent> Feed(ReadOnlySpan<byte> data)
    {
        var events = new List<IcyEvent>();
        var audio = new MemoryStream();
        int i = 0;
        while (i < data.Length)
        {
            if (expectingLength)
            {
                metaLeft = data[i] * 16;
                i += 1;
                expectingLength = false;
                if (metaLeft == 0) audioLeft = MetaInt;
            }
            else if (metaLeft > 0)
            {
                int n = Math.Min(metaLeft, data.Length - i);
                metaBuffer.Write(data.Slice(i, n));
                i += n;
                metaLeft -= n;
                if (metaLeft == 0)
                {
                    if (audio.Length > 0) { events.Add(new IcyEvent.Audio(audio.ToArray())); audio.SetLength(0); }
                    events.Add(new IcyEvent.Metadata(DecodeText(metaBuffer.ToArray())));
                    metaBuffer.SetLength(0);
                    audioLeft = MetaInt;
                }
            }
            else
            {
                int n = Math.Min(audioLeft, data.Length - i);
                audio.Write(data.Slice(i, n));
                i += n;
                audioLeft -= n;
                if (audioLeft == 0) expectingLength = true;
            }
        }
        if (audio.Length > 0) events.Add(new IcyEvent.Audio(audio.ToArray()));
        return events;
    }

    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    /// <summary>Metadata text is UTF-8, falling back to Latin-1. Trailing NULs are stripped.</summary>
    public static string DecodeText(ReadOnlySpan<byte> data)
    {
        int end = data.Length;
        while (end > 0 && data[end - 1] == 0) end--;
        var bytes = data[..end];
        try { return StrictUtf8.GetString(bytes); }
        catch (DecoderFallbackException) { return Encoding.Latin1.GetString(bytes); }
    }

    /// <summary>
    /// Pulls the <c>StreamTitle</c> value out of an ICY metadata block. Titles may contain apostrophes,
    /// so the value ends at the first <c>';</c> that is followed by another key or the end of the block.
    /// </summary>
    public static string? StreamTitle(string metadata)
    {
        const string key = "StreamTitle='";
        int start = metadata.IndexOf(key, StringComparison.Ordinal);
        if (start < 0) return null;
        string rest = metadata[(start + key.Length)..];
        int end = rest.Length;
        int search = 0;
        while (search <= rest.Length)
        {
            int r = rest.IndexOf("';", search, StringComparison.Ordinal);
            if (r < 0) break;
            string after = rest[(r + 2)..];
            if (after.Length == 0 || char.IsLetter(after[0]) || after.All(c => c == '\0' || c == ' '))
            {
                end = r;
                break;
            }
            search = r + 2;
        }
        if (end == rest.Length && rest.EndsWith('\'')) end = rest.Length - 1;
        return rest[..end].Trim();
    }
}

public static class TrackTitles
{
    /// <summary>Splits "Artist - Title" into its parts; when there is no separator the whole string is the title.</summary>
    public static (string? Artist, string Title) SplitArtistTitle(string s)
    {
        int r = s.IndexOf(" - ", StringComparison.Ordinal);
        if (r >= 0)
        {
            string artist = s[..r].Trim(' ', '\t');
            string title = s[(r + 3)..].Trim(' ', '\t');
            if (artist.Length > 0 && title.Length > 0) return (artist, title);
        }
        return (null, s);
    }
}

/// <summary>Raw stream formats the recorder can store without re-encoding.</summary>
public enum RecordableFormat { Mp3, Aac }

public static class RecordableFormats
{
    public static RecordableFormat? FromContentType(string? contentType)
    {
        if (contentType is null) return null;
        string ct = contentType.ToLowerInvariant().Split(';')[0].Trim();
        return ct switch
        {
            "audio/mpeg" or "audio/mp3" or "audio/mpeg3" or "audio/x-mpeg" => RecordableFormat.Mp3,
            "audio/aac" or "audio/aacp" or "audio/x-aac" or "audio/x-aacp" or "audio/aac-adts" => RecordableFormat.Aac,
            _ => null,
        };
    }

    public static RecordableFormat? FromRawValue(string? raw) => raw?.ToLowerInvariant() switch
    {
        "mp3" => RecordableFormat.Mp3,
        "aac" => RecordableFormat.Aac,
        _ => null,
    };

    /// <summary>Same as the Swift rawValue: "mp3" / "aac".</summary>
    public static string RawValue(this RecordableFormat f) => f == RecordableFormat.Mp3 ? "mp3" : "aac";
    public static string FileExtension(this RecordableFormat f) => f.RawValue();
}
