using System.Net;
using System.Text.Json;

namespace Shortwave.Core;

public sealed class RadioBrowserException(string message) : Exception(message)
{
    public static RadioBrowserException NoServer() => new("Unable to reach the radio-browser.info station database.");
    public static RadioBrowserException Http(int code) => new($"The station database answered with HTTP {code}.");
}

public enum SearchOrder { Votes, Clickcount, Name, Bitrate, Random }

public static class SearchOrders
{
    public static readonly SearchOrder[] All = Enum.GetValues<SearchOrder>();
    public static string RawValue(this SearchOrder o) => o.ToString().ToLowerInvariant();
    public static string Title(this SearchOrder o) => o switch
    {
        SearchOrder.Votes => "Most Voted",
        SearchOrder.Clickcount => "Most Played",
        SearchOrder.Name => "Name",
        SearchOrder.Bitrate => "Bitrate",
        _ => "Random",
    };
    internal static bool Reverse(this SearchOrder o) => o != SearchOrder.Name;
}

public sealed record StationSearch(string Name = "", string Tag = "", string CountryCode = "", string Language = "",
    SearchOrder Order = SearchOrder.Votes, int Limit = 150)
{
    public bool IsEmpty => Name.Length == 0 && Tag.Length == 0 && CountryCode.Length == 0 && Language.Length == 0;
}

public sealed record Facet(string Name, int StationCount, string? Iso3166_1)
{
    public string Id => Iso3166_1 ?? Name;
}

/// <summary>Client for the community station database at radio-browser.info.</summary>
public sealed class RadioBrowser
{
    public const string UserAgent = "Driftwave/1.0 (Windows)";
    public const string LookupHost = "all.api.radio-browser.info";
    internal static readonly string[] FallbackServers = ["de1.api.radio-browser.info", "de2.api.radio-browser.info", "fi1.api.radio-browser.info"];

    private readonly HttpClient http;
    private List<string> servers = [];
    private readonly SemaphoreSlim gate = new(1, 1);

    public RadioBrowser(HttpClient? client = null)
    {
        http = client ?? CreateClient(TimeSpan.FromSeconds(15));
    }

    public static HttpClient CreateClient(TimeSpan timeout)
    {
        var c = new HttpClient(new SocketsHttpHandler { AutomaticDecompression = DecompressionMethods.All }) { Timeout = timeout };
        c.DefaultRequestHeaders.UserAgent.ParseAdd(UserAgent);
        return c;
    }

    // MARK: Queries

    public Task<List<Station>> Search(StationSearch s, CancellationToken ct = default)
    {
        var q = new List<(string, string)>
        {
            ("order", s.Order.RawValue()),
            ("reverse", s.Order.Reverse() ? "true" : "false"),
            ("limit", s.Limit.ToString()),
            ("hidebroken", "true"),
        };
        if (s.Name.Length > 0) q.Add(("name", s.Name));
        if (s.Tag.Length > 0) q.Add(("tag", s.Tag));
        if (s.CountryCode.Length > 0) q.Add(("countrycode", s.CountryCode));
        if (s.Language.Length > 0) q.Add(("language", s.Language));
        return Stations("/json/stations/search", q, ct);
    }

    /// <summary>Popular stations for a country, with vote-botted entries removed, shuffled down to <paramref name="count"/>.</summary>
    public async Task<List<Station>> Popular(string countryCode, int count = 12, CancellationToken ct = default)
    {
        var all = await Stations("/json/stations/search", [
            ("countrycode", countryCode), ("order", "votes"), ("reverse", "true"), ("limit", "100"), ("hidebroken", "true"),
        ], ct);
        // Anything with more than 50k votes is almost certainly botted.
        var list = all.Where(s => s.Votes < 50_000).ToArray();
        Random.Shared.Shuffle(list);
        return list.Take(count).ToList();
    }

    public Task<List<Station>> RandomStations(string countryCode, int count = 18, CancellationToken ct = default) =>
        Stations("/json/stations/search", [
            ("countrycode", countryCode), ("order", "random"), ("limit", count.ToString()), ("hidebroken", "true"),
        ], ct);

    public async Task<List<Station>> Trending(int count = 18, CancellationToken ct = default)
    {
        var all = await Stations("/json/stations/search", [
            ("order", "clicktrend"), ("reverse", "true"), ("limit", "60"), ("hidebroken", "true"),
        ], ct);
        return all.Take(count).ToList();
    }

    public async Task<List<Station>> StationsByUUIDs(IReadOnlyList<string> uuids, CancellationToken ct = default)
    {
        var result = new List<Station>();
        for (int start = 0; start < uuids.Count; start += 50)
        {
            var chunk = uuids.Skip(start).Take(50);
            result.AddRange(await Stations("/json/stations/byuuid", [("uuids", string.Join(",", chunk))], ct));
        }
        return result;
    }

    public async Task<List<Facet>> Countries(CancellationToken ct = default) =>
        (await Facets("/json/countries", [("hidebroken", "true")], ct))
            .Where(f => f.StationCount > 0 && !string.IsNullOrEmpty(f.Iso3166_1))
            .OrderBy(f => f.Name, Comparer<string>.Create(StationSortings.LocalizedStandardCompare)).ToList();

    public async Task<List<Facet>> Languages(CancellationToken ct = default) =>
        (await Facets("/json/languages", [("hidebroken", "true"), ("order", "stationcount"), ("reverse", "true")], ct))
            .Where(f => f.StationCount >= 5 && f.Name.Length > 0).ToList();

    public async Task<List<Facet>> Tags(CancellationToken ct = default) =>
        (await Facets("/json/tags", [("hidebroken", "true"), ("order", "stationcount"), ("reverse", "true"), ("limit", "300")], ct))
            .Where(f => f.Name.Length > 0).ToList();

    /// <summary>Tell the database a station was played (it uses this for its popularity statistics).</summary>
    public async Task CountClick(string stationID)
    {
        try { await Data("/json/url/" + Uri.EscapeDataString(stationID), [], default); } catch { }
    }

    // MARK: Transport

    private async Task<List<Station>> Stations(string path, List<(string, string)> query, CancellationToken ct)
    {
        string json = await Data(path, query, ct);
        return APIStation.DecodeList(json).Where(s => s.StreamURL.Length > 0).ToList();
    }

    private async Task<List<Facet>> Facets(string path, List<(string, string)> query, CancellationToken ct)
    {
        string json = await Data(path, query, ct);
        using var doc = JsonDocument.Parse(json);
        return doc.RootElement.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.Object).Select(e =>
            new Facet(Station.Str(e, "name") ?? "", (int)(Station.Num(e, "stationcount") ?? 0), Station.Str(e, "iso_3166_1"))).ToList();
    }

    private async Task<string> Data(string path, List<(string, string)> query, CancellationToken ct)
    {
        List<string> list;
        await gate.WaitAsync(ct);
        try
        {
            if (servers.Count == 0) servers = await DiscoverServers(ct);
            list = [.. servers];
        }
        finally { gate.Release(); }

        Exception lastError = RadioBrowserException.NoServer();
        for (int index = 0; index < list.Count; index++)
        {
            string host = list[index];
            string qs = query.Count == 0 ? "" : "?" + string.Join("&", query.Select(p => Uri.EscapeDataString(p.Item1) + "=" + Uri.EscapeDataString(p.Item2)));
            try
            {
                using var resp = await http.GetAsync($"https://{host}{path}{qs}", ct);
                if (!resp.IsSuccessStatusCode) throw RadioBrowserException.Http((int)resp.StatusCode);
                string body = await resp.Content.ReadAsStringAsync(ct);
                if (index > 0)
                {
                    await gate.WaitAsync(CancellationToken.None);
                    try { servers.Remove(host); servers.Insert(0, host); } finally { gate.Release(); }
                }
                return body;
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested) { throw; }
            catch (Exception e) { lastError = e; }
        }
        throw lastError is RadioBrowserException ? lastError : new RadioBrowserException(lastError.Message);
    }

    private async Task<List<string>> DiscoverServers(CancellationToken ct)
    {
        try
        {
            string json = await http.GetStringAsync($"https://{LookupHost}/json/servers", ct);
            using var doc = JsonDocument.Parse(json);
            var names = doc.RootElement.EnumerateArray().Select(e => Station.Str(e, "name")).OfType<string>().Distinct().ToArray();
            Random.Shared.Shuffle(names);
            if (names.Length > 0) return [.. names, .. FallbackServers.Where(f => !names.Contains(f))];
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { throw; }
        catch { }
        return [.. FallbackServers];
    }
}
