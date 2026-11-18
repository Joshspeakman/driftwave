using System.Globalization;
using System.Text.Json;

namespace Shortwave.Core;

/// <summary>A radio station, either from the radio-browser.info database or added locally by the user.</summary>
public sealed class Station : IEquatable<Station>
{
    public string Id { get; set; } = "";
    public string Name { get; set; } = "";
    public string StreamURL { get; set; } = "";
    public string? Homepage { get; set; }
    public string? FaviconURL { get; set; }
    public List<string> Tags { get; set; } = [];
    public string Country { get; set; } = "";
    public string CountryCode { get; set; } = "";
    public string State { get; set; } = "";
    public string Language { get; set; } = "";
    public string Codec { get; set; } = "";
    public int Bitrate { get; set; }
    public int Votes { get; set; }
    public bool IsHLS { get; set; }
    public double? Latitude { get; set; }
    public double? Longitude { get; set; }
    /// <summary>Exists only in the user's library, not in the public database.</summary>
    public bool IsLocal { get; set; }
    /// <summary>A public station that could no longer be found in the database.</summary>
    public bool IsOrphaned { get; set; }
    /// <summary>File name of a user-chosen cover image (in the app's covers folder).</summary>
    public string? CustomCover { get; set; }

    public Station() { }

    public Station(string id, string name, string streamURL, string? homepage = null, string? faviconURL = null,
        IEnumerable<string>? tags = null, string country = "", string countryCode = "", string state = "",
        string language = "", string codec = "", int bitrate = 0, int votes = 0, bool isHLS = false,
        double? latitude = null, double? longitude = null, bool isLocal = false, bool isOrphaned = false,
        string? customCover = null)
    {
        Id = id; Name = name; StreamURL = streamURL; Homepage = homepage; FaviconURL = faviconURL;
        Tags = tags?.ToList() ?? []; Country = country; CountryCode = countryCode; State = state;
        Language = language; Codec = codec; Bitrate = bitrate; Votes = votes; IsHLS = isHLS;
        Latitude = latitude; Longitude = longitude; IsLocal = isLocal; IsOrphaned = isOrphaned; CustomCover = customCover;
    }

    public static Station Local(string name, string streamURL, string? customCover = null) =>
        new("local-" + Guid.NewGuid().ToString().ToLowerInvariant(), name, streamURL, isLocal: true, customCover: customCover);

    public bool HasLocation => Latitude != null && Longitude != null;

    /// <summary>Short human description, e.g. "MP3 · 128 kbps · Germany".</summary>
    public string Subtitle
    {
        get
        {
            var parts = new List<string>();
            if (Codec.Length > 0) parts.Add(Codec);
            if (Bitrate > 0) parts.Add($"{Bitrate} kbps");
            if (Country.Length > 0) parts.Add(Country);
            return string.Join(" · ", parts);
        }
    }

    /// <summary>Copy public metadata from a fresh database entry, keeping user-owned fields.</summary>
    public Station Refreshed(Station fresh)
    {
        var s = fresh.Copy();
        s.CustomCover = CustomCover;
        s.IsOrphaned = false;
        s.IsLocal = false;
        return s;
    }

    public Station Copy()
    {
        var s = (Station)MemberwiseClone();
        s.Tags = [.. Tags];
        return s;
    }

    public bool Equals(Station? o) =>
        o is not null && Id == o.Id && Name == o.Name && StreamURL == o.StreamURL && Homepage == o.Homepage
        && FaviconURL == o.FaviconURL && Tags.SequenceEqual(o.Tags) && Country == o.Country && CountryCode == o.CountryCode
        && State == o.State && Language == o.Language && Codec == o.Codec && Bitrate == o.Bitrate && Votes == o.Votes
        && IsHLS == o.IsHLS && Latitude == o.Latitude && Longitude == o.Longitude && IsLocal == o.IsLocal
        && IsOrphaned == o.IsOrphaned && CustomCover == o.CustomCover;

    public override bool Equals(object? obj) => Equals(obj as Station);
    public override int GetHashCode() => HashCode.Combine(Id, Name, StreamURL);
    public override string ToString() => Name;

    // MARK: library.json encoding (same keys and types as the Mac's Codable Station)

    /// <summary>Writes the station with sorted keys; optional values are left out rather than written as null.</summary>
    public void WriteJson(Utf8JsonWriter w)
    {
        w.WriteStartObject();
        w.WriteNumber("bitrate", Bitrate);
        w.WriteString("codec", Codec);
        w.WriteString("country", Country);
        w.WriteString("countryCode", CountryCode);
        if (CustomCover != null) w.WriteString("customCover", CustomCover);
        if (FaviconURL != null) w.WriteString("faviconURL", FaviconURL);
        if (Homepage != null) w.WriteString("homepage", Homepage);
        w.WriteString("id", Id);
        w.WriteBoolean("isHLS", IsHLS);
        w.WriteBoolean("isLocal", IsLocal);
        w.WriteBoolean("isOrphaned", IsOrphaned);
        w.WriteString("language", Language);
        if (Latitude is { } lat) w.WriteNumber("latitude", lat);
        if (Longitude is { } lon) w.WriteNumber("longitude", lon);
        w.WriteString("name", Name);
        w.WriteString("state", State);
        w.WriteString("streamURL", StreamURL);
        w.WriteStartArray("tags");
        foreach (var t in Tags) w.WriteStringValue(t);
        w.WriteEndArray();
        w.WriteNumber("votes", Votes);
        w.WriteEndObject();
    }

    /// <summary>Reads a library.json station (written by this app or by the Mac app).</summary>
    public static Station? FromJson(JsonElement e)
    {
        if (e.ValueKind != JsonValueKind.Object) return null;
        string? id = Str(e, "id"), name = Str(e, "name"), url = Str(e, "streamURL");
        if (id is null || name is null || url is null) return null;
        return new Station(
            id, name, url,
            homepage: Str(e, "homepage"),
            faviconURL: Str(e, "faviconURL"),
            tags: TagList(e),
            country: Str(e, "country") ?? "",
            countryCode: Str(e, "countryCode") ?? "",
            state: Str(e, "state") ?? "",
            language: Str(e, "language") ?? "",
            codec: Str(e, "codec") ?? "",
            bitrate: Int(e, "bitrate"),
            votes: Int(e, "votes"),
            isHLS: Bool(e, "isHLS"),
            latitude: Num(e, "latitude"),
            longitude: Num(e, "longitude"),
            isLocal: Bool(e, "isLocal"),
            isOrphaned: Bool(e, "isOrphaned"),
            customCover: Str(e, "customCover"));
    }

    // Like the Mac's lenient decoder, which reads tags as [String?]: nulls are dropped, and any other kind of
    // element makes the whole list empty.
    static List<string>? TagList(JsonElement e)
    {
        if (!e.TryGetProperty("tags", out var t) || t.ValueKind != JsonValueKind.Array) return null;
        var list = new List<string>();
        foreach (var x in t.EnumerateArray())
        {
            if (x.ValueKind == JsonValueKind.String) list.Add(x.GetString()!);
            else if (x.ValueKind != JsonValueKind.Null) return null;
        }
        return list;
    }

    static int Int(JsonElement e, string key) =>
        Num(e, key) is { } d && double.IsFinite(d) && Math.Abs(d) < int.MaxValue ? (int)d : 0;

    internal static string? Str(JsonElement e, string key) =>
        e.TryGetProperty(key, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    /// <summary>A number, or a string holding one.</summary>
    internal static double? Num(JsonElement e, string key)
    {
        if (!e.TryGetProperty(key, out var v)) return null;
        if (v.ValueKind == JsonValueKind.Number) return v.GetDouble();
        if (v.ValueKind == JsonValueKind.String && double.TryParse(v.GetString(),
                NumberStyles.AllowLeadingSign | NumberStyles.AllowDecimalPoint | NumberStyles.AllowExponent,
                CultureInfo.InvariantCulture, out var d)) return d;
        return null;
    }

    /// <summary>true, or any non-zero number (also as a string).</summary>
    internal static bool Bool(JsonElement e, string key) =>
        e.TryGetProperty(key, out var v) && (v.ValueKind == JsonValueKind.True || (Num(e, key) ?? 0) != 0);
}

/// <summary>Raw station JSON as returned by radio-browser.info.</summary>
internal static class APIStation
{
    public static Station ToStation(JsonElement e)
    {
        string Clean(string? s) => (s ?? "").Trim();
        string? url = Station.Str(e, "url"), resolvedRaw = Station.Str(e, "url_resolved");
        string resolved = !string.IsNullOrEmpty(resolvedRaw) ? resolvedRaw : (url ?? "");
        var tagList = Clean(Station.Str(e, "tags")).Split(',').Select(x => x.Trim()).Where(x => x.Length > 0);
        string name = Clean(Station.Str(e, "name"));
        string homepage = Clean(Station.Str(e, "homepage")), favicon = Clean(Station.Str(e, "favicon"));
        var language = string.Join(", ", Clean(Station.Str(e, "language")).Split(',', StringSplitOptions.RemoveEmptyEntries)
            .Select(x => Capitalized(x.Trim())));
        string codec = Clean(Station.Str(e, "codec"));
        return new Station(
            id: Station.Str(e, "stationuuid") ?? "",
            name: name.Length == 0 ? "Unnamed Station" : name,
            streamURL: resolved,
            homepage: homepage.Length == 0 ? null : homepage,
            faviconURL: favicon.Length == 0 ? null : favicon,
            tags: tagList,
            country: Clean(Station.Str(e, "country")),
            countryCode: Clean(Station.Str(e, "countrycode")),
            state: Clean(Station.Str(e, "state")),
            language: language,
            codec: codec.ToUpperInvariant() == "UNKNOWN" ? "" : codec,
            bitrate: (int)(Station.Num(e, "bitrate") ?? 0),
            votes: (int)(Station.Num(e, "votes") ?? 0),
            isHLS: (Station.Num(e, "hls") ?? 0) != 0,
            latitude: Station.Num(e, "geo_lat"),
            longitude: Station.Num(e, "geo_long"));
    }

    public static List<Station> DecodeList(string json)
    {
        using var doc = JsonDocument.Parse(json);
        return doc.RootElement.EnumerateArray()
            .Where(e => e.ValueKind == JsonValueKind.Object && Station.Str(e, "stationuuid") != null)
            .Select(ToStation).ToList();
    }

    /// <summary>Like Swift's <c>String.capitalized</c>: first letter of each word upper case, the rest lower case.</summary>
    internal static string Capitalized(string s)
    {
        var chars = s.ToCharArray();
        bool start = true;
        for (int i = 0; i < chars.Length; i++)
        {
            if (char.IsLetterOrDigit(chars[i]) || chars[i] == '\'')
            {
                chars[i] = start ? char.ToUpperInvariant(chars[i]) : char.ToLowerInvariant(chars[i]);
                start = false;
            }
            else start = true;
        }
        return new string(chars);
    }
}

public enum StationSorting { Name, Language, Country, State, Codec, Votes, Bitrate }

public static class StationSortings
{
    public static readonly StationSorting[] All = Enum.GetValues<StationSorting>();
    public static string RawValue(this StationSorting s) => s.ToString().ToLowerInvariant();
    public static StationSorting? FromRawValue(string? raw) =>
        All.Cast<StationSorting?>().FirstOrDefault(s => s!.Value.RawValue() == raw);
    public static string Title(this StationSorting s) => s.ToString();

    /// <summary>Like Foundation's <c>localizedStandardCompare</c>: case-insensitive, numbers compared by value.</summary>
    public static int LocalizedStandardCompare(string a, string b) =>
        CultureInfo.CurrentCulture.CompareInfo.Compare(a, b, CompareOptions.IgnoreCase | CompareOptions.NumericOrdering);

    public static List<Station> SortedBy(this IEnumerable<Station> stations, StationSorting sorting, bool ascending)
    {
        int Primary(Station a, Station b) => sorting switch
        {
            StationSorting.Name => LocalizedStandardCompare(a.Name, b.Name),
            StationSorting.Language => LocalizedStandardCompare(a.Language, b.Language),
            StationSorting.Country => LocalizedStandardCompare(a.Country, b.Country),
            StationSorting.State => LocalizedStandardCompare(a.State, b.State),
            StationSorting.Codec => LocalizedStandardCompare(a.Codec, b.Codec),
            StationSorting.Votes => a.Votes.CompareTo(b.Votes),
            _ => a.Bitrate.CompareTo(b.Bitrate),
        };
        int Compare(Station a, Station b)
        {
            int p = Math.Sign(Primary(a, b));
            if (p == 0) return Math.Sign(LocalizedStandardCompare(a.Name, b.Name));
            return ascending ? p : -p;
        }
        // OrderBy is stable, like Swift's sort.
        return stations.OrderBy(s => s, Comparer<Station>.Create(Compare)).ToList();
    }
}
