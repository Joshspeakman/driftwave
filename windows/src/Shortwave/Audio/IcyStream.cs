using Shortwave.Core;

namespace Shortwave.Audio;

public abstract record IcyMessage
{
    /// <summary>Connected. <c>HasTitles</c> is false when the server sends no ICY metadata.</summary>
    public sealed record Connected(bool HasTitles, RecordableFormat? Format, bool OggComments) : IcyMessage;
    /// <summary>Recorder events, each with the stream time (seconds of audio on this connection) it happened at.</summary>
    public sealed record Recorder(List<(RecorderEvent Event, double At)> Events) : IcyMessage;
    /// <summary>A title from an Ogg stream's comments (not recordable).</summary>
    public sealed record OggTitle(string Title, double At) : IcyMessage;
    /// <summary>The connection ended without being asked to.</summary>
    public sealed record Ended(string? Error) : IcyMessage;
}

/// <summary>
/// A second connection to a Shoutcast/Icecast stream that reads its in-band song titles and records
/// tracks. Media Foundation plays the audio but doesn't report ICY titles. For Ogg streams it reads the
/// Vorbis/Opus comments instead.
/// </summary>
public sealed class IcyStream
{
    private readonly Uri url;
    private readonly string directory;
    private readonly Action<IcyMessage> post;
    private readonly CancellationTokenSource cts = new();
    private readonly object gate = new();

    // Guarded by `gate`.
    private SegmentRecorder? recorder;
    private RecorderConfig config;
    private bool stopped;

    /// <summary>When the response headers arrived (the start of this connection's stream time).</summary>
    public DateTime ConnectedAt { get; private set; } = DateTime.UtcNow;
    public DateTime RequestedAt { get; private set; } = DateTime.UtcNow;

    /// <param name="post">Called on a background thread; the caller marshals to the UI thread, in order.</param>
    public IcyStream(Uri url, RecorderConfig config, string directory, Action<IcyMessage> post)
    {
        this.url = url; this.config = config; this.directory = directory; this.post = post;
    }

    public void Start()
    {
        RequestedAt = DateTime.UtcNow;
        _ = Task.Run(Run);
    }

    public void Stop()
    {
        lock (gate)
        {
            if (stopped) return;
            stopped = true;
            if (recorder?.StreamEnded() is { Count: > 0 } events) post(new IcyMessage.Recorder(events.Select(e => (e, 0.0)).ToList()));
        }
        cts.Cancel();
    }

    public void Update(RecorderConfig config)
    {
        lock (gate)
        {
            this.config = config;
            if (recorder != null) recorder.Config = config;
        }
    }

    public void CancelRecording()
    {
        lock (gate)
        {
            if (recorder?.Cancel() is { Count: > 0 } events) post(new IcyMessage.Recorder(events.Select(e => (e, 0.0)).ToList()));
        }
    }

    async Task Run()
    {
        StreamResponse? resp = null;
        string? error = null;
        try
        {
            resp = await StreamHttp.Open(url, new Dictionary<string, string> { ["Icy-MetaData"] = "1" }, cts.Token);
            ConnectedAt = DateTime.UtcNow;
            if (!resp.IsSuccess) throw new HttpRequestException($"HTTP {resp.Status}");
            var format = RecordableFormats.FromContentType(resp.ContentType);
            string ct = (resp.ContentType ?? "").ToLowerInvariant();
            int metaInt = int.TryParse(resp.Header("icy-metaint")?.Trim(), out var mi) ? mi : 0;
            if (metaInt <= 0)
            {
                bool ogg = ct.Contains("ogg") || ct.Contains("opus");
                lock (gate)
                {
                    if (stopped) return;
                    post(new IcyMessage.Connected(false, format, ogg));
                    if (!ogg) { stopped = true; return; }
                }
                await ReadOgg(resp.Body);
            }
            else
            {
                await ReadIcy(resp.Body, metaInt, format, resp.Header("icy-br"));
            }
        }
        catch (OperationCanceledException) when (cts.IsCancellationRequested) { }
        catch (Exception e) { error = e.Message; }
        finally
        {
            resp?.Dispose();
            lock (gate)
            {
                if (!stopped)
                {
                    stopped = true;
                    if (recorder?.StreamEnded() is { Count: > 0 } events) post(new IcyMessage.Recorder(events.Select(e => (e, 0.0)).ToList()));
                    post(new IcyMessage.Ended(error ?? "The connection was closed."));
                }
            }
        }
    }

    async Task ReadIcy(Stream body, int metaInt, RecordableFormat? format, string? icyBitrate)
    {
        var parser = new IcyParser(metaInt);
        var clock = format is { } f ? new AudioClock(f) : null;
        // Without frames to count, estimate stream time from the advertised bitrate.
        double bytesPerSecond = int.TryParse(icyBitrate?.Split(',')[0].Trim(), out var kbps) && kbps > 0 ? kbps * 125.0 : 0;
        long audioBytes = 0;
        double Now() => clock?.Seconds ?? (bytesPerSecond > 0 ? audioBytes / bytesPerSecond : 0);
        lock (gate)
        {
            if (stopped) return;
            recorder = new SegmentRecorder(config, format, directory);
            post(new IcyMessage.Connected(true, format, false));
        }
        var buf = new byte[16384];
        while (true)
        {
            int n = await body.ReadAsync(buf, cts.Token);
            if (n == 0) return;
            lock (gate)
            {
                if (stopped || recorder is null) return;
                var events = new List<(RecorderEvent, double)>();
                foreach (var e in parser.Feed(buf.AsSpan(0, n)))
                {
                    switch (e)
                    {
                        case IcyEvent.Audio a:
                            clock?.Feed(a.Data);
                            audioBytes += a.Data.Length;
                            events.AddRange(recorder.HandleAudio(a.Data).Select(x => (x, Now())));
                            break;
                        case IcyEvent.Metadata m:
                            if (IcyParser.StreamTitle(m.Text) is { } title)
                                events.AddRange(recorder.HandleTitle(title).Select(x => (x, Now())));
                            break;
                    }
                }
                if (events.Count > 0) post(new IcyMessage.Recorder(events));
            }
        }
    }

    async Task ReadOgg(Stream body)
    {
        var reader = new OggCommentReader();
        var buf = new byte[16384];
        while (true)
        {
            int n = await body.ReadAsync(buf, cts.Token);
            if (n == 0) return;
            var titles = reader.Feed(buf.AsSpan(0, n));
            lock (gate)
            {
                if (stopped) return;
                foreach (var (t, at) in titles) post(new IcyMessage.OggTitle(t, at));
            }
        }
    }
}
