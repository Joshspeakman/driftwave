using System.Net;
using System.Net.Security;
using System.Net.Sockets;
using System.Text;
using Shortwave.Core;

namespace Shortwave.Audio;

/// <summary>An open HTTP response whose body is read as a stream.</summary>
public sealed class StreamResponse(int status, Uri url, Dictionary<string, string> headers, Stream body, IDisposable? owner) : IDisposable
{
    public int Status { get; } = status;
    public Uri Url { get; } = url;
    public Stream Body { get; } = body;
    public bool IsSuccess => Status is >= 200 and < 300;
    public string? Header(string name) => headers.TryGetValue(name, out var v) ? v : null;
    public string? ContentType => Header("Content-Type");
    public void Dispose() { try { Body.Dispose(); } catch { } try { owner?.Dispose(); } catch { } }
}

/// <summary>
/// Opens radio streams. Differences from a plain HttpClient call, each needed by real stations:
/// redirects are followed by hand (HttpClient won't follow https to http, which some stations do),
/// and old Shoutcast servers that answer <c>ICY 200 OK</c> instead of <c>HTTP/1.0 200 OK</c> are read
/// over a raw socket.
/// </summary>
public static class StreamHttp
{
    static readonly HttpClient Client = new(new SocketsHttpHandler
    {
        AllowAutoRedirect = false,
        AutomaticDecompression = DecompressionMethods.None,
        ConnectTimeout = TimeSpan.FromSeconds(20),
        PooledConnectionLifetime = TimeSpan.FromMinutes(2),
    })
    { Timeout = Timeout.InfiniteTimeSpan };

    public static async Task<StreamResponse> Open(Uri url, IReadOnlyDictionary<string, string> headers, CancellationToken ct, int maxRedirects = 5)
    {
        for (int hop = 0; ; hop++)
        {
            StreamResponse r;
            try
            {
                r = await OpenOnce(url, headers, ct);
            }
            catch (HttpRequestException e) when (e.HttpRequestError is HttpRequestError.InvalidResponse or HttpRequestError.ResponseEnded)
            {
                r = await RawOpen(url, headers, ct);
            }
            if (r.Status is 301 or 302 or 303 or 307 or 308 && hop < maxRedirects && r.Header("Location") is { } loc
                && Uri.TryCreate(url, loc, out var next) && (next.Scheme == Uri.UriSchemeHttp || next.Scheme == Uri.UriSchemeHttps))
            {
                r.Dispose();
                url = next;
                continue;
            }
            return r;
        }
    }

    static async Task<StreamResponse> OpenOnce(Uri url, IReadOnlyDictionary<string, string> headers, CancellationToken ct)
    {
        var req = new HttpRequestMessage(HttpMethod.Get, url) { Version = HttpVersion.Version11 };
        req.Headers.TryAddWithoutValidation("User-Agent", RadioBrowser.UserAgent);
        foreach (var (k, v) in headers) req.Headers.TryAddWithoutValidation(k, v);
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(TimeSpan.FromSeconds(20));
        var resp = await Client.SendAsync(req, HttpCompletionOption.ResponseHeadersRead, timeout.Token);
        var h = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (var kv in resp.Headers) h[kv.Key] = string.Join(", ", kv.Value);
        foreach (var kv in resp.Content.Headers) h[kv.Key] = string.Join(", ", kv.Value);
        if (resp.Headers.Location is { } loc) h["Location"] = loc.OriginalString;
        var body = await resp.Content.ReadAsStreamAsync(ct);
        return new StreamResponse((int)resp.StatusCode, url, h, body, resp);
    }

    /// <summary>HTTP/1.0 over a plain socket (TLS if https), accepting an <c>ICY</c> status line.</summary>
    static async Task<StreamResponse> RawOpen(Uri url, IReadOnlyDictionary<string, string> headers, CancellationToken ct)
    {
        var tcp = new TcpClient { NoDelay = true };
        try
        {
            using (var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct))
            {
                timeout.CancelAfter(TimeSpan.FromSeconds(20));
                await tcp.ConnectAsync(url.Host, url.Port, timeout.Token);
            }
            Stream s = tcp.GetStream();
            if (url.Scheme == Uri.UriSchemeHttps)
            {
                var ssl = new SslStream(s, false);
                await ssl.AuthenticateAsClientAsync(new SslClientAuthenticationOptions { TargetHost = url.Host }, ct);
                s = ssl;
            }
            var sb = new StringBuilder();
            sb.Append($"GET {url.PathAndQuery} HTTP/1.0\r\nHost: {url.Authority}\r\nUser-Agent: {RadioBrowser.UserAgent}\r\nAccept: */*\r\n");
            foreach (var (k, v) in headers) sb.Append($"{k}: {v}\r\n");
            sb.Append("\r\n");
            await s.WriteAsync(Encoding.ASCII.GetBytes(sb.ToString()), ct);

            // Read the header block byte by byte so nothing of the body is consumed.
            var head = new List<byte>();
            var one = new byte[1];
            while (head.Count < 65536)
            {
                int n = await s.ReadAsync(one, ct);
                if (n == 0) break;
                head.Add(one[0]);
                int c = head.Count;
                if (c >= 4 && head[c - 4] == '\r' && head[c - 3] == '\n' && head[c - 2] == '\r' && head[c - 1] == '\n') break;
                if (c >= 2 && head[c - 2] == '\n' && head[c - 1] == '\n') break;
            }
            var lines = Encoding.Latin1.GetString(head.ToArray()).Split('\n').Select(l => l.TrimEnd('\r')).ToList();
            var status = lines.Count > 0 ? lines[0].Split(' ', 3) : [];
            if (status.Length < 2 || !int.TryParse(status[1], out int code)) throw new HttpRequestException("The server's answer is not HTTP or ICY.");
            var h = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (var l in lines.Skip(1))
            {
                int colon = l.IndexOf(':');
                if (colon > 0) h[l[..colon].Trim()] = l[(colon + 1)..].Trim();
            }
            return new StreamResponse(code, url, h, s, tcp);
        }
        catch
        {
            tcp.Dispose();
            throw;
        }
    }

    /// <summary>Reads at most <paramref name="max"/> bytes of a response body.</summary>
    public static async Task<byte[]> ReadUpTo(Stream s, int max, CancellationToken ct)
    {
        var ms = new MemoryStream();
        var buf = new byte[8192];
        while (ms.Length <= max)
        {
            int n = await s.ReadAsync(buf, ct);
            if (n == 0) break;
            ms.Write(buf, 0, n);
        }
        return ms.ToArray();
    }
}
