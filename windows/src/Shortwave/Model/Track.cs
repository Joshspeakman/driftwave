using Shortwave.Core;

namespace Shortwave.Model;

/// <summary>A song (or show) detected from the stream's metadata, possibly recorded.</summary>
public sealed class Track : Observable
{
    public Guid Id { get; }
    public string Title { get; }
    public Station Station { get; }
    public DateTime StartedAt { get; } = DateTime.Now;
    public RecordableFormat? Format { get; }

    public Track(Guid id, string title, Station station, RecordingState state, string? file, RecordableFormat? format)
    {
        Id = id; Title = title; Station = station; this.state = state; this.file = file; Format = format;
    }

    private RecordingState state;
    public RecordingState State
    {
        get => state;
        set { if (Set(ref state, value)) { Raise(nameof(CanSave)); Raise(nameof(StateTitle)); Raise(nameof(StateExplanation)); Raise(nameof(IsRecording)); } }
    }

    private double duration;
    public double Duration { get => duration; set { if (Set(ref duration, value)) Raise(nameof(DurationText)); } }

    private string? file;
    public string? File { get => file; set { if (Set(ref file, value)) Raise(nameof(CanSave)); } }

    private string? savedTo;
    public string? SavedTo { get => savedTo; set { if (Set(ref savedTo, value)) { Raise(nameof(IsSaved)); Raise(nameof(CanSave)); } } }

    private bool saveWhenRecorded;
    public bool SaveWhenRecorded { get => saveWhenRecorded; set => Set(ref saveWhenRecorded, value); }

    private string? saveError;
    public string? SaveError { get => saveError; set => Set(ref saveError, value); }

    public string? Artist => TrackTitles.SplitArtistTitle(Title).Artist;
    public string SongTitle => TrackTitles.SplitArtistTitle(Title).Title;
    public bool IsSaved => SavedTo != null;
    public bool CanSave => State.IsRecorded() && !IsSaved && File != null;
    public bool IsRecording => State == RecordingState.Recording;
    public string StateTitle => State.Title();
    public string StateExplanation => State.Explanation();

    public string DurationText
    {
        get { int s = (int)Math.Round(Duration); return $"{s / 60}:{s % 60:00}"; }
    }

    private bool saving;

    /// <summary>Copies the temporary recording into <paramref name="directory"/>, tagging it with title / artist / station.</summary>
    public async Task Save(string directory)
    {
        if (!CanSave || saving || File is not { } source || Format is not { } format) return;
        saving = true;
        string title = Title, song = SongTitle, station = Station.Name;
        string? artist = Artist;
        try
        {
            string dest = await Task.Run(() =>
            {
                Directory.CreateDirectory(directory);
                string name = FileNames.Sanitized(title);
                byte[] audio = System.IO.File.ReadAllBytes(source);
                if (format == RecordableFormat.Mp3)
                {
                    string d = FileNames.UniqueFilePath(directory, name, "mp3");
                    byte[] tag = ID3.Tag(song, artist, station, $"Recorded with Driftwave from {station}");
                    WriteAtomic(d, [.. tag, .. audio]);
                    return d;
                }
                // Wrap the raw ADTS stream in an .m4a so it carries tags; fall back to the raw file.
                if (M4A.FromAdts(audio, song, artist, station) is { } m4a)
                {
                    string d = FileNames.UniqueFilePath(directory, name, "m4a");
                    WriteAtomic(d, m4a);
                    return d;
                }
                string raw = FileNames.UniqueFilePath(directory, name, "aac");
                System.IO.File.Copy(source, raw);
                return raw;
            });
            SavedTo = dest;
            SaveError = null;
            Diagnostics.Log($"saved {dest}");
        }
        catch (Exception e)
        {
            SaveError = e.Message;
            Diagnostics.Log($"save failed: {e.Message}");
        }
        finally { saving = false; }
    }

    static void WriteAtomic(string path, byte[] data)
    {
        string tmp = path + ".part";
        System.IO.File.WriteAllBytes(tmp, data);
        System.IO.File.Move(tmp, path);
    }
}
