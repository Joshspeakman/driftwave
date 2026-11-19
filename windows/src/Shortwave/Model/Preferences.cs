using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using Shortwave.Core;

namespace Shortwave.Model;

/// <summary>
/// User preferences, saved as JSON next to the library. Keys and defaults match the Mac's UserDefaults.
/// Release and Nightly keep separate settings (as the Mac's bundle IDs do); test runs use their own folder.
/// A <c>-key value</c> launch argument overrides a setting for that run without saving it.
/// </summary>
public sealed class Preferences : Observable
{
    private readonly string path;
    private readonly JsonObject stored;
    private readonly HashSet<string> overridden = [];
    private bool loading = true;

    public Preferences(AppPaths paths)
    {
        path = Path.Combine(paths.Support, Diagnostics.IsNightly && !Diagnostics.IsTestRun ? "settings-nightly.json" : "settings.json");
        stored = Load(path);

        recordingMode = RecordingModes.FromRawValue(Get("recordingMode")) ?? RecordingMode.Decide;
        string? dir = Get("trackDirectory");
        // Test runs never save into the real track folder unless told to (-trackDirectory). The Mac's test runs
        // default to ~/Music/Shortwave; here they use ShortwaveTestRun\Tracks.
        trackDirectory = !string.IsNullOrEmpty(dir) ? dir
            : Diagnostics.IsTestRun ? Path.Combine(paths.Support, "Tracks")
            : AppPaths.DefaultTrackDirectory;
        minimumDuration = GetInt("recordingMinimumDuration") ?? 30;
        maximumDuration = GetInt("recordingMaximumDuration") ?? 900;
        backgroundPlayback = GetBool("backgroundPlayback") ?? true;
        notifications = GetBool("notifications") ?? false;
        pastTracksCount = Math.Max(1, GetInt("pastTracksCount") ?? 10);
        librarySorting = StationSortings.FromRawValue(Get("librarySorting")) ?? StationSorting.Name;
        libraryAscending = GetBool("libraryAscending") ?? true;
        volume = Math.Clamp(GetDouble("volume") ?? 1.0, 0, 1);
        lastStationID = Get("lastStation");
        discoverCountry = Get("discoverCountry") ?? DefaultCountry();
        appearance = AppThemes.FromRawValue(Get("appearance")) ?? AppTheme.System;   // -appearance light|dark overrides it
        themeId = Get("theme") ?? "default";                                             // -theme <id> overrides it
        effectsEnabled = GetBool("effects") ?? true;
        effectIntensity = Math.Clamp(GetDouble("effectIntensity") ?? 1.0, 0, 1);
        checkForUpdates = GetBool("checkForUpdates") ?? true;
        backgroundFile = Get("backgroundFile");
        backgroundDim = Math.Clamp(GetDouble("backgroundDim") ?? 0.35, 0, 0.8);
        giphyKey = Get("giphyKey");
        lastUpdateCheck = Get("lastUpdateCheck");
        loading = false;
    }

    static string DefaultCountry()
    {
        try { return new RegionInfo(CultureInfo.CurrentCulture.Name).TwoLetterISORegionName; } catch { return "US"; }
    }

    static JsonObject Load(string path)
    {
        try { if (File.Exists(path)) return JsonNode.Parse(File.ReadAllText(path)) as JsonObject ?? []; } catch { }
        return [];
    }

    // Launch arguments win over stored values.
    string? Get(string key)
    {
        if (Diagnostics.Value(key) is { } v) { overridden.Add(key); return v; }
        return stored[key] is JsonValue j && j.TryGetValue(out string? s) ? s : null;
    }
    int? GetInt(string key)
    {
        if (Diagnostics.Value(key) is { } v) { overridden.Add(key); return int.TryParse(v, out var i) ? i : null; }
        return stored[key] is JsonValue j && j.TryGetValue(out double d) ? (int)d : null;
    }
    double? GetDouble(string key)
    {
        if (Diagnostics.Value(key) is { } v) { overridden.Add(key); return double.TryParse(v, CultureInfo.InvariantCulture, out var d) ? d : null; }
        return stored[key] is JsonValue j && j.TryGetValue(out double x) ? x : null;
    }
    bool? GetBool(string key)
    {
        if (Diagnostics.Value(key) is { } v) { overridden.Add(key); return v is "1" or "true" or "YES" or "yes"; }
        return stored[key] is JsonValue j && j.TryGetValue(out bool b) ? b : null;
    }

    void Store(string key, JsonNode? value)
    {
        if (loading) return;
        overridden.Remove(key);
        if (value is null) stored.Remove(key); else stored[key] = value;
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            string tmp = path + ".tmp";
            File.WriteAllText(tmp, stored.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
            File.Move(tmp, path, overwrite: true);
        }
        catch (Exception e) { Diagnostics.Log("settings save failed: " + e.Message); }
    }

    private RecordingMode recordingMode;
    public RecordingMode RecordingMode
    {
        get => recordingMode;
        set { if (Set(ref recordingMode, value)) { Store("recordingMode", value.RawValue()); Raise(nameof(RecorderConfig)); } }
    }

    private string trackDirectory;
    public string TrackDirectory { get => trackDirectory; set { if (Set(ref trackDirectory, value)) Store("trackDirectory", value); } }

    private int minimumDuration;
    public int MinimumDuration
    {
        get => minimumDuration;
        set { if (Set(ref minimumDuration, value)) { Store("recordingMinimumDuration", value); Raise(nameof(RecorderConfig)); } }
    }

    private int maximumDuration;
    public int MaximumDuration
    {
        get => maximumDuration;
        set { if (Set(ref maximumDuration, value)) { Store("recordingMaximumDuration", value); Raise(nameof(RecorderConfig)); } }
    }

    private bool backgroundPlayback;
    public bool BackgroundPlayback { get => backgroundPlayback; set { if (Set(ref backgroundPlayback, value)) Store("backgroundPlayback", value); } }

    private bool notifications;
    public bool Notifications { get => notifications; set { if (Set(ref notifications, value)) Store("notifications", value); } }

    private int pastTracksCount;
    public int PastTracksCount { get => pastTracksCount; set { if (Set(ref pastTracksCount, Math.Clamp(value, 1, 50))) Store("pastTracksCount", pastTracksCount); } }

    private StationSorting librarySorting;
    public StationSorting LibrarySorting { get => librarySorting; set { if (Set(ref librarySorting, value)) Store("librarySorting", value.RawValue()); } }

    private bool libraryAscending;
    public bool LibraryAscending { get => libraryAscending; set { if (Set(ref libraryAscending, value)) Store("libraryAscending", value); } }

    private double volume;
    public double Volume { get => volume; set { if (Set(ref volume, value)) Store("volume", value); } }

    private string? lastStationID;
    public string? LastStationID { get => lastStationID; set { if (Set(ref lastStationID, value)) Store("lastStation", value); } }

    private string discoverCountry;
    public string DiscoverCountry { get => discoverCountry; set { if (Set(ref discoverCountry, value)) Store("discoverCountry", value); } }

    private AppTheme appearance;
    /// <summary>App Theme: follow Windows' light or dark setting, or always use one. Key <c>appearance</c>, like the Mac.</summary>
    public AppTheme Appearance
    {
        get => appearance;
        set { if (Set(ref appearance, value)) { Store("appearance", value.RawValue()); Diagnostics.Log("theme " + value.RawValue()); } }
    }

    private string themeId;
    /// <summary>The look (docs/THEMES.md): "default" or a theme's id.</summary>
    public string ThemeId { get => themeId; set { if (Set(ref themeId, value)) Store("theme", value); } }

    private bool effectsEnabled;
    /// <summary>Animated and decorative theme effects (also off while Windows' animation effects are off).</summary>
    public bool EffectsEnabled { get => effectsEnabled; set { if (Set(ref effectsEnabled, value)) Store("effects", value); } }

    private double effectIntensity;
    public double EffectIntensity { get => effectIntensity; set { if (Set(ref effectIntensity, Math.Clamp(value, 0, 1))) Store("effectIntensity", effectIntensity); } }

    private string? backgroundFile;
    /// <summary>Your own background (a file in the data folder's Backgrounds folder), or none.</summary>
    public string? BackgroundFile { get => backgroundFile; set { if (Set(ref backgroundFile, value)) Store("backgroundFile", value); } }

    private double backgroundDim;
    /// <summary>How much your background is darkened (0 to 0.8), so text stays readable.</summary>
    public double BackgroundDim { get => backgroundDim; set { if (Set(ref backgroundDim, Math.Clamp(value, 0, 0.8))) Store("backgroundDim", backgroundDim); } }

    private string? giphyKey;
    /// <summary>Your GIPHY API key for background search (else the app's own, if it was built with one).</summary>
    public string? GiphyKey { get => giphyKey; set { if (Set(ref giphyKey, string.IsNullOrWhiteSpace(value) ? null : value)) Store("giphyKey", giphyKey); } }

    private bool checkForUpdates;
    /// <summary>Look for a new release at launch (at most once a day).</summary>
    public bool CheckForUpdates { get => checkForUpdates; set { if (Set(ref checkForUpdates, value)) Store("checkForUpdates", value); } }

    private string? lastUpdateCheck;
    public string? LastUpdateCheck { get => lastUpdateCheck; set { if (Set(ref lastUpdateCheck, value)) Store("lastUpdateCheck", value); } }

    public RecorderConfig RecorderConfig => new(RecordingMode, MinimumDuration, MaximumDuration);
}

/// <summary>The app's light or dark look: Windows' own setting (the default), or always light or dark.</summary>
public enum AppTheme { System, Light, Dark }

public static class AppThemes
{
    public static readonly AppTheme[] All = [AppTheme.System, AppTheme.Light, AppTheme.Dark];
    public static string RawValue(this AppTheme t) => t.ToString().ToLowerInvariant();
    public static AppTheme? FromRawValue(string? raw) => All.Cast<AppTheme?>().FirstOrDefault(t => t!.Value.RawValue() == raw);
    public static string Title(this AppTheme t) => t switch
    {
        AppTheme.Light => "Light",
        AppTheme.Dark => "Dark",
        _ => "Use System Setting",
    };
    public static Microsoft.UI.Xaml.ElementTheme ElementTheme(this AppTheme t) => t switch
    {
        AppTheme.Light => Microsoft.UI.Xaml.ElementTheme.Light,
        AppTheme.Dark => Microsoft.UI.Xaml.ElementTheme.Dark,
        _ => Microsoft.UI.Xaml.ElementTheme.Default,
    };
    public static Microsoft.UI.Windowing.TitleBarTheme TitleBarTheme(this AppTheme t) => t switch
    {
        AppTheme.Light => Microsoft.UI.Windowing.TitleBarTheme.Light,
        AppTheme.Dark => Microsoft.UI.Windowing.TitleBarTheme.Dark,
        _ => Microsoft.UI.Windowing.TitleBarTheme.UseDefaultAppMode,
    };
}
