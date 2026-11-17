namespace Shortwave.Core;

public enum RecordingMode { Everything, Decide, Nothing }

public static class RecordingModes
{
    public static readonly RecordingMode[] All = [RecordingMode.Everything, RecordingMode.Decide, RecordingMode.Nothing];

    /// <summary>The Swift rawValue, also the settings value: "everything", "decide", "nothing".</summary>
    public static string RawValue(this RecordingMode m) => m switch
    {
        RecordingMode.Everything => "everything",
        RecordingMode.Decide => "decide",
        _ => "nothing",
    };

    public static RecordingMode? FromRawValue(string? s) => s switch
    {
        "everything" => RecordingMode.Everything,
        "decide" => RecordingMode.Decide,
        "nothing" => RecordingMode.Nothing,
        _ => null,
    };

    public static string Title(this RecordingMode m) => m switch
    {
        RecordingMode.Everything => "Save All Tracks",
        RecordingMode.Decide => "Decide for Each Track",
        _ => "Record Nothing",
    };

    public static string Explanation(this RecordingMode m) => m switch
    {
        RecordingMode.Everything => "All recorded tracks are automatically saved in the track folder.",
        RecordingMode.Decide => "Tracks are temporarily recorded and can be saved if desired.",
        _ => "Stations are played without recording.",
    };
}

public enum RecordingState
{
    IdleDisabled, IdleIncomplete, IdleUnsupported,
    Recording, Recorded, RecordedReachedMaxDuration,
    DiscardedBelowMinDuration, DiscardedCancelled, DiscardedStreamFailure,
}

public static class RecordingStates
{
    public static bool IncludeInPastTracks(this RecordingState s) => s != RecordingState.DiscardedBelowMinDuration;
    public static bool IsRecorded(this RecordingState s) => s is RecordingState.Recorded or RecordingState.RecordedReachedMaxDuration;

    /// <summary>The Swift rawValue (lower camel case), used in log lines the test scripts grep.</summary>
    public static string RawValue(this RecordingState s)
    {
        string n = s.ToString();
        return char.ToLowerInvariant(n[0]) + n[1..];
    }

    public static string Title(this RecordingState s) => s switch
    {
        RecordingState.IdleDisabled or RecordingState.IdleIncomplete or RecordingState.IdleUnsupported => "Not Recorded",
        RecordingState.Recording => "Recording…",
        RecordingState.Recorded or RecordingState.RecordedReachedMaxDuration => "Recorded",
        RecordingState.DiscardedBelowMinDuration => "Below Threshold",
        RecordingState.DiscardedCancelled => "Cancelled",
        _ => "Interrupted",
    };

    public static string Explanation(this RecordingState s) => s switch
    {
        RecordingState.IdleDisabled => "Recording is turned off in Settings.",
        RecordingState.IdleIncomplete => "The track wasn’t played from the beginning, so it can’t be fully recorded.",
        RecordingState.IdleUnsupported => "This station’s stream format can’t be recorded.",
        RecordingState.Recording => "The track will be recorded until a new track gets played.",
        RecordingState.Recorded => "The track has been temporarily recorded.",
        RecordingState.RecordedReachedMaxDuration => "The maximum recording duration has been reached.",
        RecordingState.DiscardedBelowMinDuration => "The track has been discarded as it was shorter than the minimum duration.",
        RecordingState.DiscardedCancelled => "Recording has been cancelled.",
        _ => "The stream was interrupted, so the recording was discarded.",
    };
}

public sealed record RecorderConfig(RecordingMode Mode, double MinimumDuration, double MaximumDuration);

public abstract record RecorderEvent
{
    /// <summary>A new track began. <c>File</c> is set when its audio is being recorded.</summary>
    public sealed record Started(Guid Id, string Title, RecordingState State, string? File) : RecorderEvent;
    /// <summary>Recording of a track ended with <c>State</c>; <c>Duration</c> in seconds.</summary>
    public sealed record Finished(Guid Id, RecordingState State, double Duration) : RecorderEvent;
}

/// <summary>
/// Cuts a live stream into one file per track, driven by the stream's title changes.
/// Runs on the stream's own thread so a cut happens at the exact byte where the title changed.
/// Not thread-safe: call it from one thread only.
/// </summary>
public sealed class SegmentRecorder
{
    public RecorderConfig Config { get; set; }
    public RecordableFormat? Format { get; }
    private readonly string directory;
    private readonly Func<DateTime> clock;

    private bool hadTrack;
    private Guid? currentId;
    private string? currentTitle;
    private FileStream? handle;
    private string? filePath;
    private DateTime startedAt = DateTime.UtcNow;

    public SegmentRecorder(RecorderConfig config, RecordableFormat? format, string directory, Func<DateTime>? clock = null)
    {
        Config = config; Format = format; this.directory = directory; this.clock = clock ?? (() => DateTime.UtcNow);
    }

    public bool IsRecording => handle != null;

    public List<RecorderEvent> HandleTitle(string rawTitle)
    {
        string title = rawTitle.Trim();
        if (title.Length == 0 || title == currentTitle) return [];
        var events = FinishCurrent(cancelled: false);

        var id = Guid.NewGuid();
        RecordingState state;
        string? file = null;
        if (Config.Mode == RecordingMode.Nothing)
        {
            state = RecordingState.IdleDisabled;
        }
        else if (Format is null)
        {
            state = RecordingState.IdleUnsupported;
        }
        else if (!hadTrack)
        {
            // The first title we see started before we tuned in.
            state = RecordingState.IdleIncomplete;
        }
        else
        {
            try
            {
                Directory.CreateDirectory(directory);
                string path = Path.Combine(directory, id.ToString().ToUpperInvariant() + "." + Format.Value.FileExtension());
                handle = new FileStream(path, FileMode.CreateNew, FileAccess.Write, FileShare.Read);
                filePath = path; file = path;
                state = RecordingState.Recording;
            }
            catch (Exception)
            {
                handle = null;
                state = RecordingState.IdleDisabled;
            }
        }
        hadTrack = true;
        currentId = id;
        currentTitle = title;
        startedAt = clock();
        events.Add(new RecorderEvent.Started(id, title, state, file));
        return events;
    }

    public List<RecorderEvent> HandleAudio(ReadOnlySpan<byte> data)
    {
        if (handle is null) return [];
        handle.Write(data);
        if (Config.MaximumDuration > 0 && (clock() - startedAt).TotalSeconds >= Config.MaximumDuration)
            return Close(RecordingState.RecordedReachedMaxDuration, discard: false);
        return [];
    }

    /// <summary>The user cancelled the current recording.</summary>
    public List<RecorderEvent> Cancel() => FinishCurrent(cancelled: true);

    /// <summary>The stream ended or failed: whatever was being recorded is incomplete.</summary>
    public List<RecorderEvent> StreamEnded()
    {
        var events = handle != null ? Close(RecordingState.DiscardedStreamFailure, discard: true) : [];
        hadTrack = false;
        currentTitle = null;
        currentId = null;
        return events;
    }

    private List<RecorderEvent> FinishCurrent(bool cancelled)
    {
        if (handle is null) return [];
        if (cancelled) return Close(RecordingState.DiscardedCancelled, discard: true);
        double duration = (clock() - startedAt).TotalSeconds;
        if (duration < Config.MinimumDuration) return Close(RecordingState.DiscardedBelowMinDuration, discard: true);
        return Close(RecordingState.Recorded, discard: false);
    }

    private List<RecorderEvent> Close(RecordingState state, bool discard)
    {
        if (handle is not { } h || currentId is not { } id) return [];
        try { h.Dispose(); } catch { }
        handle = null;
        double duration = discard && state == RecordingState.DiscardedCancelled ? 0 : (clock() - startedAt).TotalSeconds;
        if (discard && filePath != null) { try { File.Delete(filePath); } catch { } }
        filePath = null;
        return [new RecorderEvent.Finished(id, state, duration)];
    }
}
