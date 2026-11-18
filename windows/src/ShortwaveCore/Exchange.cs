using System.Globalization;
using System.Text;
using System.Text.Json;

namespace Shortwave.Core;

/// <summary>
/// Moving saved stations between the Windows and Mac apps (and between machines). A 1:1 translation of the Mac's
/// <c>StationExchange</c> (Sources/ShortwaveCore/Exchange.swift); the format is in docs/STATION-EXCHANGE.md.
/// An exchange file is also a valid <c>library.json</c>, and a bare <c>library.json</c> can be imported.
/// </summary>
public static class StationExchange
{
    public const string Format = "shortwave-stations";
    public const int Version = 1;
    /// <summary>Suggested file name for an export.</summary>
    public const string DefaultFileName = "Driftwave Stations.json";

    public enum ReadError { NotJSON, NoStations }

    public sealed class ReadException(ReadError error) : Exception(Describe(error))
    {
        public ReadError Error { get; } = error;
    }

    static string Describe(ReadError e) => e switch
    {
        ReadError.NotJSON => "This file isn’t a station file.",
        _ => "This file doesn’t contain a list of stations.",
    };

    /// <summary>What an exchange file holds.</summary>
    public sealed record Contents(List<Station> Stations, Dictionary<string, byte[]> Covers, int Unreadable)
    {
        public Contents(List<Station> stations) : this(stations, [], 0) { }
    }

    /// <summary>Writes an exchange file. <paramref name="cover"/> returns the image data for a <c>customCover</c> file name.</summary>
    public static byte[] Encode(IEnumerable<Station> stations, string app, DateTimeOffset? date = null, Func<string, byte[]?>? cover = null)
    {
        var list = stations.ToList();
        var covers = new SortedDictionary<string, string>(StringComparer.Ordinal);
        foreach (var s in list)
        {
            if (s.CustomCover is { } name && !covers.ContainsKey(name) && cover?.Invoke(name) is { } data && IsImage(data))
                covers[name] = Convert.ToBase64String(data);
        }
        string exported = (date ?? DateTimeOffset.UtcNow).ToUniversalTime().ToString("yyyy-MM-dd'T'HH:mm:ss'Z'", CultureInfo.InvariantCulture);

        using var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms, new JsonWriterOptions { Indented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }))
        {
            // Keys in sorted order, like the Mac's JSONEncoder with .sortedKeys.
            w.WriteStartObject();
            w.WriteString("app", app);
            w.WriteStartObject("covers");
            foreach (var (name, b64) in covers) w.WriteString(name, b64);
            w.WriteEndObject();
            w.WriteString("exported", exported);
            w.WriteString("format", Format);
            w.WriteStartArray("stations");
            foreach (var s in list) s.WriteJson(w);
            w.WriteEndArray();
            w.WriteNumber("version", Version);
            w.WriteEndObject();
        }
        return ms.ToArray();
    }

    /// <summary>Reads an exchange file, a <c>library.json</c>, or a bare JSON array of stations.</summary>
    public static Contents Decode(byte[] data)
    {
        JsonDocument doc;
        try { doc = JsonDocument.Parse(data); }
        catch (JsonException) { throw new ReadException(ReadError.NotJSON); }
        using (doc)
        {
            var root = doc.RootElement;
            JsonElement list;
            var covers = new Dictionary<string, byte[]>();
            if (root.ValueKind == JsonValueKind.Array)
            {
                list = root;
            }
            else if (root.ValueKind == JsonValueKind.Object && root.TryGetProperty("stations", out var st) && st.ValueKind == JsonValueKind.Array)
            {
                list = st;
                if (root.TryGetProperty("covers", out var c) && c.ValueKind == JsonValueKind.Object)
                {
                    foreach (var p in c.EnumerateObject())
                    {
                        if (p.Value.ValueKind == JsonValueKind.String && Base64(p.Value.GetString()!) is { } d && IsImage(d))
                            covers[p.Name] = d;
                    }
                }
            }
            else
            {
                throw new ReadException(ReadError.NoStations);
            }
            int count = list.GetArrayLength();
            var stations = list.EnumerateArray().Select(Station.FromJson).OfType<Station>()
                .Where(s => s.Id.Length > 0 && !IsBlank(s.StreamURL)).ToList();
            return new Contents(stations, covers, count - stations.Count);
        }
    }

    // Swift's .whitespaces: spaces (category Zs) and tab, but not newlines.
    static bool IsBlank(string s) => s.All(ch => ch == '\t' || CharUnicodeInfo.GetUnicodeCategory(ch) == UnicodeCategory.SpaceSeparator);

    // Like Data(base64Encoded:options: .ignoreUnknownCharacters).
    static byte[]? Base64(string s)
    {
        var sb = new StringBuilder(s.Length);
        foreach (char ch in s)
            if (ch is >= 'A' and <= 'Z' or >= 'a' and <= 'z' or >= '0' and <= '9' or '+' or '/' or '=') sb.Append(ch);
        try { return Convert.FromBase64String(sb.ToString()); } catch (FormatException) { return null; }
    }

    /// <param name="Library">The library with the new stations appended.</param>
    /// <param name="Added">The stations that were added.</param>
    /// <param name="AlreadyPresent">Stations skipped because the library already has them.</param>
    /// <param name="Unreadable">Entries in the file's station list that couldn't be read.</param>
    /// <param name="NewCovers">Cover images to write to the covers folder, by their new file names.</param>
    public sealed record MergeResult(List<Station> Library, List<Station> Added, int AlreadyPresent, int Unreadable,
        Dictionary<string, byte[]> NewCovers);

    /// <summary>
    /// Adds the stations the library doesn't have yet. Never changes or removes existing ones.
    /// A station is already present when its <c>id</c> matches, or, for local stations, when a local station with
    /// the same stream address exists. Imported covers get new file names from <paramref name="newCoverName"/>
    /// (without extension: <c>.png</c> or <c>.jpg</c> is added); a station whose cover image isn't in the file
    /// loses its <c>customCover</c>.
    /// </summary>
    public static MergeResult Merge(Contents contents, IEnumerable<Station> library, Func<string> newCoverName)
    {
        static string Key(string url) => url.Trim().ToLowerInvariant();
        var result = new MergeResult(library.ToList(), [], 0, contents.Unreadable, []);
        int present = 0;
        var ids = result.Library.Select(s => s.Id).ToHashSet();
        var localURLs = result.Library.Where(s => s.IsLocal).Select(s => Key(s.StreamURL)).ToHashSet();
        foreach (var incoming in contents.Stations)
        {
            if (ids.Contains(incoming.Id) || (incoming.IsLocal && localURLs.Contains(Key(incoming.StreamURL))))
            {
                present++;
                continue;
            }
            var s = incoming.Copy();
            if (s.CustomCover is { } name)
            {
                if (contents.Covers.TryGetValue(name, out var data))
                {
                    bool isPNG = data.AsSpan().StartsWith(Png);
                    string fresh = newCoverName() + (isPNG ? ".png" : ".jpg");
                    result.NewCovers[fresh] = data;
                    s.CustomCover = fresh;
                }
                else
                {
                    s.CustomCover = null;
                }
            }
            ids.Add(s.Id);
            if (s.IsLocal) localURLs.Add(Key(s.StreamURL));
            result.Library.Add(s);
            result.Added.Add(s);
        }
        return result with { AlreadyPresent = present };
    }

    static ReadOnlySpan<byte> Png => [0x89, 0x50, 0x4E, 0x47];
    static ReadOnlySpan<byte> Jpeg => [0xFF, 0xD8, 0xFF];

    /// <summary>PNG or JPEG, by their signatures.</summary>
    public static bool IsImage(byte[] d) => d.AsSpan().StartsWith(Png) || d.AsSpan().StartsWith(Jpeg);
}
