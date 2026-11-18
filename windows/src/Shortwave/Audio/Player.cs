using System.Collections.ObjectModel;
using System.Text;
using Microsoft.UI.Dispatching;
using Shortwave.Core;
using Shortwave.Integration;
using Shortwave.Model;
using Windows.Media.Core;
using Windows.Media.Playback;

namespace Shortwave.Audio;

public enum PlayerState { Stopped, Loading, Playing, Failed }

/// <summary>Plays a station with Media Foundation, follows its song titles and records tracks.</summary>
public sealed class Player : Observable
{
    private readonly MediaPlayer mp = new();
    private readonly Preferences settings;
    /// <summary>Temporary recordings; test runs each get their own so parallel runs never clear each other's.</summary>
    private readonly string recordingDir;
    private readonly RadioBrowser api;
    private readonly DispatcherQueue ui;
    private readonly MediaControls? controls;

    public Func<Station, string?>? ArtworkProvider;
    public Action<Track>? OnShowTrack;
    public Func<Station, int, Station?>? StationNeighbours;
    public Action<Track, Station>? OnNewTrack;

    private IcyStream? icy;
    private bool icyHasTitles;
    private int generation;
    private Dictionary<Guid, Track> tracks = [];
    private int icyRetries;
    private int streamRetries;
    private Uri? resolvedURL;
    private MediaPlaybackItem? currentItem;
    private int currentItemGen;
    private DateTime playerRequestedAt = DateTime.UtcNow;

    public ObservableCollection<Track> PastTracks { get; } = [];
    public string RecordingDirectory => recordingDir;

    public Player(Preferences settings, AppPaths paths, RadioBrowser api, DispatcherQueue ui)
    {
        this.settings = settings;
        this.api = api;
        this.ui = ui;
        recordingDir = Diagnostics.IsTestRun ? Path.Combine(paths.Recording, Environment.ProcessId.ToString()) : paths.Recording;

        // Shortwave drives the system media controls itself (and not at all in test runs).
        mp.CommandManager.IsEnabled = false;
        mp.AudioCategory = MediaPlayerAudioCategory.Media;
        mp.RealTimePlayback = false;
        volume = settings.Volume;
        ApplyOutput();
        mp.MediaFailed += (s, e) =>
        {
            var src = s.Source;
            Diagnostics.Log($"media failed {e.Error} {e.ExtendedErrorCode?.HResult:X8} {e.ErrorMessage}");
            string message = e.Error switch
            {
                MediaPlayerError.NetworkError => "The station could not be reached.",
                MediaPlayerError.SourceNotSupported => "This stream’s format is not supported.",
                MediaPlayerError.DecodingError => "The stream could not be decoded.",
                _ => "The stream could not be played.",
            };
            ui.TryEnqueue(() => { if (src != null && ReferenceEquals(src, currentItem)) StreamFailed(message, currentItemGen); });
        };
        mp.MediaEnded += (s, _) =>
        {
            var src = s.Source;
            ui.TryEnqueue(() => { if (src != null && ReferenceEquals(src, currentItem)) StreamFailed("The station stopped sending audio.", currentItemGen); });
        };
        mp.PlaybackSession.PlaybackStateChanged += (_, _) => ui.TryEnqueue(SyncStateFromPlayer);

        if (!Diagnostics.StartMuted || Diagnostics.ForceMediaControls) controls = new MediaControls(mp, ui, this);
        try { if (Directory.Exists(recordingDir)) Directory.Delete(recordingDir, true); } catch { }
        if (Diagnostics.IsTestRun) RemoveStaleTestRecordings(paths.Recording);
    }

    /// <summary>Test runs that were killed leave their per-process folder behind; remove those whose process is gone.</summary>
    static void RemoveStaleTestRecordings(string parent)
    {
        try
        {
            if (!Directory.Exists(parent)) return;
            foreach (var dir in Directory.GetDirectories(parent))
            {
                if (!int.TryParse(Path.GetFileName(dir), out int pid) || pid == Environment.ProcessId) continue;
                bool running;
                try { using var p = System.Diagnostics.Process.GetProcessById(pid); running = !p.HasExited; } catch { running = false; }
                if (!running) { try { Directory.Delete(dir, true); } catch { } }
            }
        }
        catch { }
    }

    // MARK: Observable state

    private Station? station;
    public Station? Station { get => station; private set { if (Set(ref station, value)) Raise(nameof(HasStation)); } }
    public bool HasStation => station != null;

    private PlayerState state;
    public PlayerState State
    {
        get => state;
        private set
        {
            if (state == value && value != PlayerState.Failed) return;
            state = value;
            Diagnostics.Log(value == PlayerState.Failed ? $"state failed(\"{FailureMessage}\")" : $"state {value.ToString().ToLowerInvariant()}");
            Raise(); Raise(nameof(IsPlaying)); Raise(nameof(IsLoading));
        }
    }
    public string? FailureMessage { get; private set; }
    public bool IsPlaying => state is PlayerState.Playing or PlayerState.Loading;
    public bool IsLoading => state == PlayerState.Loading;

    private Track? playingTrack;
    public Track? PlayingTrack { get => playingTrack; private set => Set(ref playingTrack, value); }

    private double volume;
    public double Volume
    {
        get => volume;
        set
        {
            value = Math.Clamp(value, 0, 1);
            if (Set(ref volume, value)) { settings.Volume = value; ApplyOutput(); }
        }
    }

    private bool isMuted;
    public bool IsMuted { get => isMuted; set { if (Set(ref isMuted, value)) ApplyOutput(); } }

    /// <summary>
    /// What the player actually outputs. A test run without -audible stays silent whatever the controls show:
    /// Ctrl+Up and Ctrl+Alt+M change the displayed mute and volume, never the sound (the Mac does the same).
    /// </summary>
    private void ApplyOutput()
    {
        bool silent = Diagnostics.StartMuted;
        mp.IsMuted = isMuted || silent;
        mp.Volume = silent ? 0 : volume;
        Diagnostics.Log($"{OutputDescription} (shown muted={(isMuted ? "true" : "false")} volume={volume.ToString(System.Globalization.CultureInfo.InvariantCulture)})");
    }

    /// <summary>For the test log: the player's real output state.</summary>
    public string OutputDescription => $"output muted={(mp.IsMuted ? "true" : "false")} volume={mp.Volume.ToString(System.Globalization.CultureInfo.InvariantCulture)}";

    public MediaPlayer MediaPlayer => mp;

    // MARK: Control

    public void Select(Station s)
    {
        if (Station?.Id != s.Id)
        {
            StopPlayback("station change");
            Station = s;
            settings.LastStationID = s.Id;
            ResetTracks();
            StartPlayback();
        }
        else
        {
            Station = s;
            if (!IsPlaying) StartPlayback();
        }
    }

    /// <summary>Show a station without starting it (used to restore the last station at launch).</summary>
    public void Restore(Station s)
    {
        if (Station != null) return;
        Station = s;
        UpdateNowPlaying();
    }

    /// <summary>The library copy of the current station changed (renamed, new cover).</summary>
    public void StationUpdated(Station s)
    {
        if (Station?.Id == s.Id) { Station = s; UpdateNowPlaying(); }
    }

    public void TogglePlayback()
    {
        if (IsPlaying) StopPlayback("toggle"); else StartPlayback();
    }

    public void StartPlayback()
    {
        if (Station is not { } st) return;
        StopStreams();
        generation++;
        int gen = generation;
        FailureMessage = null;
        State = PlayerState.Loading;
        streamRetries = 0;
        UpdateNowPlaying();
        if (!st.IsLocal) _ = api.CountClick(st.Id);
        _ = Resolve();

        async Task Resolve()
        {
            var url = await ResolveStreamURL(st.StreamURL, st.IsLocal);
            if (url is null)
            {
                if (gen == generation) Fail("The stream address is not valid.");
                return;
            }
            if (gen != generation) return;
            resolvedURL = url;
            Play(url, gen);
        }
    }

    public void StopPlayback(string reason = "user")
    {
        Diagnostics.Log($"stop ({reason})");
        generation++;
        StopStreams();
        if (PlayingTrack is { } t && t.State.IncludeInPastTracks()) AddPast(t);
        PlayingTrack = null;
        State = PlayerState.Stopped;
        UpdateNowPlaying();
    }

    public void IncreaseVolume() { Volume = Math.Min(1, Math.Round((Volume + 0.05) * 100) / 100); IsMuted = false; }
    public void DecreaseVolume() { Volume = Math.Max(0, Math.Round((Volume - 0.05) * 100) / 100); }
    public void ToggleMute() => IsMuted = !IsMuted;

    public void CancelRecording() => icy?.CancelRecording();

    public void SettingsChanged() => icy?.Update(settings.RecorderConfig);

    public Task Save(Track track) => track.Save(settings.TrackDirectory);

    /// <summary>Next / previous: move through the library.</summary>
    public void Skip(int delta)
    {
        if (Station is { } s && StationNeighbours?.Invoke(s, delta) is { } next) Select(next);
    }

    public void SetAudioDevice(Windows.Devices.Enumeration.DeviceInformation? device)
    {
        try { mp.AudioDevice = device; } catch (Exception e) { Diagnostics.Log("audio device: " + e.Message); }
    }

    public Windows.Devices.Enumeration.DeviceInformation? AudioDevice => mp.AudioDevice;

    // MARK: Streams

    private void Play(Uri url, int gen)
    {
        var source = MediaSource.CreateFromUri(url);
        var item = new MediaPlaybackItem(source);
        var watched = new HashSet<TimedMetadataTrack>();
        item.TimedMetadataTracksChanged += (s, _) =>
        {
            for (int i = 0; i < s.TimedMetadataTracks.Count; i++)
            {
                var t = s.TimedMetadataTracks[i];
                if (!watched.Add(t)) continue;
                s.TimedMetadataTracks.SetPresentationMode((uint)i, TimedMetadataTrackPresentationMode.ApplicationPresented);
                t.CueEntered += (_, ce) =>
                {
                    if (ce.Cue is not DataCue { Data: { } data }) return;
                    var bytes = new byte[data.Length];
                    using (var r = Windows.Storage.Streams.DataReader.FromBuffer(data)) r.ReadBytes(bytes);
                    var (title, artist) = ID3.ReadTitleArtist(bytes);
                    if (title is null) return;
                    ui.TryEnqueue(() => { if (gen == generation) PlayerMetadata(title, artist); });
                };
            }
        };
        currentItem = item;
        currentItemGen = gen;
        playerRequestedAt = DateTime.UtcNow;
        ApplyOutput();
        mp.Source = item;
        mp.Play();

        string ext = url.AbsolutePath.Split('/').Last();
        if (Station?.IsHLS != true && !ext.EndsWith(".m3u8", StringComparison.OrdinalIgnoreCase))
            StartIcy(url, gen);
    }

    private void StartIcy(Uri url, int gen)
    {
        IcyStream? stream = null;
        stream = new IcyStream(url, settings.RecorderConfig, recordingDir, message => ui.TryEnqueue(() =>
        {
            if (gen != generation)
            {
                // A stream we already left: still let finished recordings land on their tracks.
                if (message is IcyMessage.Recorder rec) Apply(rec.Events, live: false, stream!);
                return;
            }
            Handle(message, url, gen, stream!);
        }));
        icy = stream;
        stream.Start();
    }

    private void Handle(IcyMessage message, Uri url, int gen, IcyStream stream)
    {
        switch (message)
        {
            case IcyMessage.Connected c:
                Diagnostics.Log($"icy connected titles={(c.HasTitles ? "true" : "false")} format={c.Format?.RawValue() ?? "none"}{(c.OggComments ? " ogg-comments" : "")}");
                icyHasTitles = c.HasTitles;
                if (c.HasTitles || c.OggComments) icyRetries = 0; else icy = null;
                break;
            case IcyMessage.Recorder r:
                Apply(r.Events, live: true, stream);
                break;
            case IcyMessage.OggTitle o:
                double lead = Lead(o.At, stream);
                After(lead, () => { if (gen == generation) PlayerMetadata(o.Title, null, fromOgg: true); });
                break;
            case IcyMessage.Ended e:
                Diagnostics.Log($"icy ended: {e.Error}");
                icy = null;
                icyHasTitles = false;
                // Keep following titles while the audio still plays.
                if (!IsPlaying || icyRetries >= 5) return;
                icyRetries++;
                After(3, () =>
                {
                    if (gen == generation && IsPlaying && icy == null) StartIcy(url, gen);
                });
                break;
        }
    }

    private void StopStreams()
    {
        icy?.Stop();
        icy = null;
        icyHasTitles = false;
        currentItem = null;
        try { mp.Pause(); mp.Source = null; } catch { }
    }

    private void StreamFailed(string message, int gen)
    {
        if (gen != generation) return;
        // Live streams sometimes drop; reconnect a couple of times before giving up.
        if (streamRetries < 2 && resolvedURL is { } url)
        {
            streamRetries++;
            int retries = streamRetries;
            Diagnostics.Log($"stream failed ({message}), retry {retries}");
            StopStreams();
            State = PlayerState.Loading;
            After(1.5, () =>
            {
                if (gen != generation) return;
                Play(url, gen);
                streamRetries = retries;
            });
            return;
        }
        StopStreams();
        if (PlayingTrack is { } t && t.State.IncludeInPastTracks()) AddPast(t);
        PlayingTrack = null;
        Fail(message);
    }

    private void Fail(string message)
    {
        FailureMessage = message;
        State = PlayerState.Failed;
        Raise(nameof(FailureMessage));
        UpdateNowPlaying();
    }

    private void SyncStateFromPlayer()
    {
        if (currentItem == null) return;
        switch (mp.PlaybackSession.PlaybackState)
        {
            case MediaPlaybackState.Playing:
                State = PlayerState.Playing;
                streamRetries = 0;
                break;
            case MediaPlaybackState.Buffering:
            case MediaPlaybackState.Opening:
                State = PlayerState.Loading;
                break;
        }
        UpdateNowPlaying();
    }

    /// <summary>Follow <c>.pls</c>/<c>.m3u</c>/<c>.xspf</c> links to the real stream (up to 3 hops, 64 KB each).</summary>
    private static async Task<Uri?> ResolveStreamURL(string s, bool isLocal)
    {
        if (!Uri.TryCreate(s.Trim(), UriKind.Absolute, out var url) || (url.Scheme != Uri.UriSchemeHttp && url.Scheme != Uri.UriSchemeHttps)) return null;
        for (int hop = 0; hop < 3; hop++)
        {
            bool byExtension = Playlist.LooksLikePlaylist(url, null);
            var headers = new Dictionary<string, string>();
            if (!byExtension)
            {
                // Only peek at unknown content types for local stations: most database links are plain streams.
                if (!isLocal) return url;
                headers["Range"] = "bytes=0-4095";
            }
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(10));
            StreamResponse r;
            try { r = await StreamHttp.Open(url, headers, cts.Token); }
            catch { return url; }
            using (r)
            {
                if (!Playlist.LooksLikePlaylist(url, r.ContentType)) return url;
                byte[] data;
                try { data = await StreamHttp.ReadUpTo(r.Body, 65536, cts.Token); } catch { data = []; }
                string text = Encoding.UTF8.GetString(data);
                if (Playlist.FirstStream(text, r.Url) is not { } next) return url;
                Diagnostics.Log($"playlist {url} -> {next}");
                url = next;
            }
        }
        return url;
    }

    // MARK: Tracks

    private void ResetTracks()
    {
        PlayingTrack = null;
        PastTracks.Clear();
        tracks = tracks.Where(kv => kv.Value.State == RecordingState.Recording || kv.Value.State.IsRecorded() && !kv.Value.IsSaved)
            .ToDictionary(kv => kv.Key, kv => kv.Value);
    }

    private void Apply(List<(RecorderEvent Event, double At)> events, bool live, IcyStream stream)
    {
        foreach (var (ev, at) in events)
        {
            switch (ev)
            {
                case RecorderEvent.Started st:
                {
                    Diagnostics.Log($"track started \"{st.Title}\" {st.State.RawValue()} {st.File ?? "-"}");
                    if (!live || Station is not { } s) continue;
                    var format = st.File is { } f ? RecordableFormats.FromRawValue(Path.GetExtension(f).TrimStart('.')) : null;
                    var track = new Track(st.Id, st.Title, s, st.State, st.File, format);
                    tracks[st.Id] = track;
                    // After a title-stream reconnect the same song is announced again: keep the one we have.
                    if (st.State == RecordingState.IdleIncomplete && st.Title == PlayingTrack?.Title) continue;
                    // The title connection runs ahead of what is audible by the player's buffer.
                    double lead = Lead(at, stream);
                    int gen = generation;
                    After(lead, () => { if (gen == generation) Promote(track); });
                    break;
                }
                case RecorderEvent.Finished fin:
                {
                    Diagnostics.Log($"track finished {fin.State.RawValue()} {(int)fin.Duration}s");
                    if (!tracks.TryGetValue(fin.Id, out var track)) continue;
                    track.State = fin.State;
                    track.Duration = fin.Duration;
                    if (!fin.State.IsRecorded()) track.File = null;
                    else if (settings.RecordingMode == RecordingMode.Everything || track.SaveWhenRecorded) _ = Save(track);
                    if (!fin.State.IncludeInPastTracks())
                    {
                        var p = PastTracks.FirstOrDefault(x => x.Id == fin.Id);
                        if (p != null) PastTracks.Remove(p);
                    }
                    break;
                }
            }
        }
    }

    /// <summary>
    /// How long until the player reaches a point <paramref name="at"/> seconds into the title connection's
    /// audio. Both connections start at the server's burst point, so the difference is their start times.
    /// </summary>
    private double Lead(double at, IcyStream stream)
    {
        double position = mp.PlaybackSession.Position.TotalSeconds;
        double lead = at + (stream.RequestedAt - playerRequestedAt).TotalSeconds - position;
        Diagnostics.Log($"title lead {lead:F1}s (stream {at:F1}s, player {position:F1}s)");
        return double.IsFinite(lead) ? Math.Clamp(lead, 0, 20) : 0;
    }

    private void Promote(Track track)
    {
        if (PlayingTrack?.Id == track.Id) return;
        if (PlayingTrack is { } old && old.State.IncludeInPastTracks()) AddPast(old);
        PlayingTrack = track;
        if (Diagnostics.OpenTrackDetail && track.State == RecordingState.Recording) OnShowTrack?.Invoke(track);
        Diagnostics.Log($"now playing \"{track.Title}\"");
        UpdateNowPlaying();
        if (settings.Notifications && Station is { } s) OnNewTrack?.Invoke(track, s);
    }

    private void AddPast(Track track)
    {
        var existing = PastTracks.FirstOrDefault(t => t.Id == track.Id);
        if (existing != null) PastTracks.Remove(existing);
        PastTracks.Insert(0, track);
        int limit = Math.Max(1, settings.PastTracksCount);
        while (PastTracks.Count > limit)
        {
            var dropped = PastTracks[^1];
            PastTracks.RemoveAt(PastTracks.Count - 1);
            if (!dropped.IsSaved && dropped.State != RecordingState.Recording)
            {
                if (dropped.File is { } f) { try { File.Delete(f); } catch { } }
                tracks.Remove(dropped.Id);
            }
        }
    }

    /// <summary>Titles from the player (HLS ID3 tags) or from Ogg comments, for streams without ICY titles.</summary>
    private void PlayerMetadata(string title, string? artist, bool fromOgg = false)
    {
        if ((icyHasTitles && !fromOgg) || Station is not { } s || !IsPlaying) return;
        string full = artist != null ? $"{artist} - {title}" : title;
        if (full == PlayingTrack?.Title) return;
        var st = settings.RecordingMode == RecordingMode.Nothing ? RecordingState.IdleDisabled : RecordingState.IdleUnsupported;
        Promote(new Track(Guid.NewGuid(), full, s, st, null, null));
    }

    // Timers are kept here until they fire: a DispatcherQueueTimer nothing references can be collected before it ticks.
    private readonly HashSet<DispatcherQueueTimer> timers = [];

    private void After(double seconds, Action action)
    {
        if (seconds <= 0.01) { ui.TryEnqueue(() => action()); return; }
        var timer = ui.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(seconds);
        timer.IsRepeating = false;
        timers.Add(timer);
        timer.Tick += (t, _) => { t.Stop(); timers.Remove(t); action(); };
        timer.Start();
    }

    // MARK: System media controls

    public void UpdateNowPlaying() => controls?.Update(Station, PlayingTrack, State, Station is { } s ? ArtworkProvider?.Invoke(s) : null);

    /// <summary>Test runs remove their own temporary recordings on exit.</summary>
    public void Shutdown()
    {
        StopStreams();
        controls?.Dispose();
        if (Diagnostics.IsTestRun) { try { Directory.Delete(recordingDir, true); } catch { } }
    }
}
