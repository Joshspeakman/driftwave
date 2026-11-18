using Microsoft.UI.Dispatching;
using Shortwave.Audio;
using Shortwave.Core;

namespace Shortwave.Model;

public enum Page { Library, Discover, Search, Settings }

/// <summary>Library, Discover and Search state, covers, library refresh and next/previous station.</summary>
public sealed class AppModel : Observable
{
    public Preferences Settings { get; }
    public Theming.ThemeManager Themes { get; }
    public Updater Updates { get; }
    public AppPaths Paths { get; } = new(Diagnostics.IsTestRun ? "ShortwaveTestRun" : "Driftwave");
    public RadioBrowser Api { get; } = new();
    public ImageStore Images { get; }
    public Player Player { get; }

    // Requests the window acts on (the Mac's sheets).
    public event Action<Station>? ShowStationRequested;
    public event Action<Track>? ShowTrackRequested;
    public event Action<Station?>? EditStationRequested;   // null: add a new local station
    public event Action? LibraryChanged;
    /// <summary>A message to show (title, text), e.g. the result of an import or export.</summary>
    public event Action<string, string>? NoticeRequested;
    /// <summary>Import Stations… / Export Stations…: the window shows a file picker.</summary>
    public event Action? ImportFileRequested, ExportFileRequested;
    public void ChooseImportFile() => ImportFileRequested?.Invoke();
    public void ChooseExportFile() => ExportFileRequested?.Invoke();

    private readonly DispatcherQueue ui;

    public AppModel(DispatcherQueue ui)
    {
        this.ui = ui;
        // Renamed from Shortwave (2026-11-18): the first real launch copies the old data folder. Test runs never do.
        if (!Diagnostics.IsTestRun)
        {
            try
            {
                string oldSupport = new AppPaths("Shortwave").Support;
                string oldMusic = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyMusic), "Shortwave");
                if (DataMigration.CopyIfFirstRun(oldSupport, Paths.Support, oldMusic)) Diagnostics.Log($"migrated {oldSupport} -> {Paths.Support}");
            }
            catch (Exception e) { Diagnostics.Log("migration failed: " + e.Message); }
        }
        Settings = new Preferences(Paths);
        Themes = new Theming.ThemeManager(Settings, Paths);
        Updates = new Updater(Paths, Settings, ui);
        Images = new ImageStore(Paths);
        Player = new Player(Settings, Paths, Api, ui);
        library = LibraryFile.Load(Paths.Library);
        Player.ArtworkProvider = s => Images.Cached(s);
        Player.OnShowTrack = t => ShowTrackRequested?.Invoke(t);
        Player.StationNeighbours = Neighbour;
        Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(Preferences.LibrarySorting) or nameof(Preferences.LibraryAscending)) { Raise(nameof(SortedLibrary)); LibraryChanged?.Invoke(); }
            if (e.PropertyName == nameof(Preferences.RecorderConfig)) Player.SettingsChanged();
        };
        if (Settings.LastStationID is { } id && library.FirstOrDefault(s => s.Id == id) is { } last)
        {
            Player.Restore(last);
            _ = LoadArtworkThenUpdate(last);
        }
        _ = RefreshLibrary();
        if (Diagnostics.StartMuted) Player.IsMuted = true;
        Diagnostics.Log($"launch testRun={(Diagnostics.IsTestRun ? "true" : "false")} muted={(Player.IsMuted ? "true" : "false")}");
        page = Diagnostics.StartPage?.ToLowerInvariant() switch
        {
            "discover" => Page.Discover,
            "search" => Page.Search,
            "settings" => Page.Settings,
            _ => Page.Library,
        };
        if (Diagnostics.SearchText is { } q) { search = search with { Name = q }; SearchChanged(); }
        if (Diagnostics.ImportStations is { } importPath)
        {
            try
            {
                var r = ImportStations(importPath);
                Diagnostics.Log($"imported added={r.Added.Count} present={r.AlreadyPresent} unreadable={r.Unreadable} covers={r.NewCovers.Count}");
            }
            catch (Exception e) { Diagnostics.Log("import failed: " + e.Message); }
        }
        if (Diagnostics.ExportStations is { } exportPath)
        {
            try
            {
                ExportStations(exportPath);
                Diagnostics.Log($"exported {library.Count} stations to {exportPath}");
            }
            catch (Exception e) { Diagnostics.Log("export failed: " + e.Message); }
        }
        if (Diagnostics.Resume) ResumeAfterRestart();
        if (Diagnostics.PlayURL is { } url) Player.Select(Station.Local("Test Stream", url));
    }

    /// <summary>Plays the station that was playing when App.Restart started this copy.</summary>
    void ResumeAfterRestart()
    {
        string file = Path.Combine(Paths.Caches, "resume.json");
        try
        {
            var station = LibraryFile.Load(file).FirstOrDefault();
            File.Delete(file);
            if (station is null) return;
            Diagnostics.Log("resume " + station.Name);
            Play(station);
        }
        catch (Exception e) { Diagnostics.Log("resume: " + e.Message); }
    }

    private Page page;
    public Page Page { get => page; set => Set(ref page, value); }

    private bool showPlayer = true;
    public bool ShowPlayer { get => showPlayer; set => Set(ref showPlayer, value); }

    // MARK: Library

    private List<Station> library;
    public IReadOnlyList<Station> Library => library;
    public List<Station> SortedLibrary => library.SortedBy(Settings.LibrarySorting, Settings.LibraryAscending);

    public bool IsInLibrary(Station s) => library.Any(x => x.Id == s.Id);
    public Station LibraryVersion(Station s) => library.FirstOrDefault(x => x.Id == s.Id) ?? s;

    public void AddToLibrary(Station s)
    {
        if (IsInLibrary(s)) return;
        library.Add(s);
        SaveLibrary();
    }

    public void RemoveFromLibrary(Station s)
    {
        if (library.FirstOrDefault(x => x.Id == s.Id)?.CustomCover is { } cover)
        {
            try { File.Delete(Path.Combine(Paths.Covers, cover)); } catch { }
        }
        library.RemoveAll(x => x.Id == s.Id);
        SaveLibrary();
    }

    public void Update(Station s)
    {
        int i = library.FindIndex(x => x.Id == s.Id);
        if (i >= 0) { library[i] = s; SaveLibrary(); }
        Images.Invalidate(s);
        Player.StationUpdated(s);
    }

    public void ToggleLibrary(Station s)
    {
        if (IsInLibrary(s)) RemoveFromLibrary(s); else AddToLibrary(s);
    }

    /// <summary>Copies an image into the covers folder and uses it as the station's cover (null: back to the default).</summary>
    public async Task SetCover(Station s, string? imagePath)
    {
        var station = LibraryVersion(s).Copy();
        if (station.CustomCover is { } old) { try { File.Delete(Path.Combine(Paths.Covers, old)); } catch { } }
        Images.Invalidate(station);
        station.CustomCover = null;
        if (imagePath != null)
        {
            try
            {
                if (await ImageStore.PngData(await File.ReadAllBytesAsync(imagePath), 512) is { } png)
                {
                    Directory.CreateDirectory(Paths.Covers);
                    string name = Guid.NewGuid().ToString().ToUpperInvariant() + ".png";
                    await File.WriteAllBytesAsync(Path.Combine(Paths.Covers, name), png);
                    station.CustomCover = name;
                }
            }
            catch (Exception e) { Diagnostics.Log("cover: " + e.Message); }
        }
        Update(station);
        _ = LoadArtworkThenUpdate(station);
    }

    // MARK: Import and export (docs/STATION-EXCHANGE.md)

    /// <summary>Writes the whole library, with custom covers, to an exchange file.</summary>
    public void ExportStations(string path)
    {
        var data = StationExchange.Encode(SortedLibrary, Diagnostics.AppDescription, cover: name =>
        {
            try { return File.ReadAllBytes(Path.Combine(Paths.Covers, name)); } catch { return null; }
        });
        string full = Path.GetFullPath(path);
        Directory.CreateDirectory(Path.GetDirectoryName(full)!);
        string tmp = full + ".tmp";
        File.WriteAllBytes(tmp, data);
        File.Move(tmp, full, overwrite: true);
    }

    /// <summary>Adds the stations from an exchange file (or a library.json) that the library doesn't have.</summary>
    public StationExchange.MergeResult ImportStations(string path)
    {
        var contents = StationExchange.Decode(File.ReadAllBytes(path));
        var result = StationExchange.Merge(contents, library, () => Guid.NewGuid().ToString().ToUpperInvariant());
        if (result.NewCovers.Count > 0) { try { Directory.CreateDirectory(Paths.Covers); } catch { } }
        foreach (var (name, data) in result.NewCovers)
        {
            try { File.WriteAllBytes(Path.Combine(Paths.Covers, name), data); }
            catch (Exception e)
            {
                // Couldn't store the cover: fall back to the station's favicon.
                Diagnostics.Log($"cover {name}: {e.Message}");
                foreach (var s in result.Library.Where(s => s.CustomCover == name)) s.CustomCover = null;
            }
        }
        if (result.Added.Count > 0)
        {
            library = result.Library;
            SaveLibrary();
            _ = RefreshLibrary();
        }
        return result;
    }

    /// <summary>Exports and tells the user how it went.</summary>
    public void ExportReporting(string path)
    {
        try
        {
            ExportStations(path);
            int n = library.Count;
            Notice("Stations Exported",
                $"{n} {(n == 1 ? "station was" : "stations were")} saved to “{Path.GetFileName(path)}”. Open it with Import Stations… in Driftwave on another computer, or in Shortwave on a Mac.");
        }
        catch (Exception e) { Notice("Couldn’t Export Stations", e.Message); }
    }

    /// <summary>Imports and tells the user how it went.</summary>
    public void ImportReporting(string path)
    {
        try
        {
            var r = ImportStations(path);
            var parts = new List<string>
            {
                r.Added.Count == 0 ? "No new stations."
                    : $"{r.Added.Count} {(r.Added.Count == 1 ? "station was" : "stations were")} added to your library.",
            };
            if (r.AlreadyPresent > 0) parts.Add($"{r.AlreadyPresent} {(r.AlreadyPresent == 1 ? "was" : "were")} already there.");
            if (r.Unreadable > 0) parts.Add($"{r.Unreadable} couldn’t be read.");
            Notice(r.Added.Count == 0 ? "Nothing Imported" : "Stations Imported", string.Join(" ", parts));
            if (r.Added.Count > 0) Page = Page.Library;
        }
        catch (Exception e) { Notice("Couldn’t Import Stations", e.Message); }
    }

    public void Notice(string title, string message)
    {
        Diagnostics.Log($"notice {title}: {message}");
        NoticeRequested?.Invoke(title, message);
    }

    private void SaveLibrary()
    {
        try { LibraryFile.Save(library, Paths.Library); } catch (Exception e) { Diagnostics.Log("library save failed: " + e.Message); }
        Raise(nameof(Library));
        Raise(nameof(SortedLibrary));
        LibraryChanged?.Invoke();
    }

    /// <summary>Refresh public stations from the database; mark those it no longer knows as orphaned.</summary>
    public async Task RefreshLibrary()
    {
        var ids = library.Where(s => !s.IsLocal).Select(s => s.Id).ToList();
        if (ids.Count == 0) return;
        List<Station> fresh;
        try { fresh = await Api.StationsByUUIDs(ids); } catch { return; }
        var byID = new Dictionary<string, Station>();
        foreach (var f in fresh) byID.TryAdd(f.Id, f);
        var asked = ids.ToHashSet();
        bool changed = false;
        library = library.Select(s =>
        {
            // Stations added (e.g. imported) while this refresh was waiting get their own refresh.
            if (s.IsLocal || !asked.Contains(s.Id)) return s;
            if (byID.TryGetValue(s.Id, out var f))
            {
                var r = s.Refreshed(f);
                if (!r.Equals(s)) changed = true;
                return r;
            }
            if (!s.IsOrphaned) changed = true;
            var o = s.Copy(); o.IsOrphaned = true; return o;
        }).ToList();
        if (changed) SaveLibrary();
    }

    private Station? Neighbour(Station station, int delta)
    {
        var list = SortedLibrary;
        if (list.Count == 0) return null;
        int i = list.FindIndex(s => s.Id == station.Id);
        if (i < 0) return list[0];
        return list[((i + delta) % list.Count + list.Count) % list.Count];
    }

    // MARK: Playback

    public void Play(Station s)
    {
        var station = LibraryVersion(s);
        Player.Select(station);
        _ = LoadArtworkThenUpdate(station);
    }

    private async Task LoadArtworkThenUpdate(Station s)
    {
        await Images.Load(s);
        Player.UpdateNowPlaying();
    }

    public void ShowStation(Station s) => ShowStationRequested?.Invoke(LibraryVersion(s));
    public void ShowTrack(Track t) => ShowTrackRequested?.Invoke(t);
    public void AddLocalStation() => EditStationRequested?.Invoke(null);
    public void EditStation(Station s) => EditStationRequested?.Invoke(s);

    // MARK: Discover

    private List<Station> popular = [], trending = [], randomStations = [];
    public List<Station> Popular { get => popular; private set => Set(ref popular, value); }
    public List<Station> Trending { get => trending; private set => Set(ref trending, value); }
    public List<Station> RandomStations { get => randomStations; private set => Set(ref randomStations, value); }

    private bool discoverLoading;
    public bool DiscoverLoading { get => discoverLoading; private set => Set(ref discoverLoading, value); }

    private string? discoverError;
    public string? DiscoverError { get => discoverError; private set => Set(ref discoverError, value); }

    public async Task LoadDiscover(bool force = false)
    {
        if (!(force || Popular.Count == 0) || DiscoverLoading) return;
        DiscoverLoading = true;
        DiscoverError = null;
        try
        {
            string cc = Settings.DiscoverCountry;
            var p = Api.Popular(cc);
            var t = Api.Trending();
            var r = Api.RandomStations(cc);
            await Task.WhenAll(p, t, r);
            Popular = p.Result; Trending = t.Result; RandomStations = r.Result;
        }
        catch (Exception e)
        {
            DiscoverError = (e as AggregateException)?.InnerException?.Message ?? e.Message;
        }
        finally { DiscoverLoading = false; }
    }

    // MARK: Search

    private StationSearch search = new();
    public StationSearch Search
    {
        get => search;
        set { if (Set(ref search, value)) SearchChanged(); }
    }

    private List<Station> searchResults = [];
    public List<Station> SearchResults { get => searchResults; private set => Set(ref searchResults, value); }

    private bool searchLoading;
    public bool SearchLoading { get => searchLoading; private set => Set(ref searchLoading, value); }

    private string? searchError;
    public string? SearchError { get => searchError; private set => Set(ref searchError, value); }

    private List<Facet> countries = [], languages = [], tags = [];
    public List<Facet> Countries { get => countries; private set => Set(ref countries, value); }
    public List<Facet> Languages { get => languages; private set => Set(ref languages, value); }
    public List<Facet> Tags { get => tags; private set => Set(ref tags, value); }

    private CancellationTokenSource? searchCts;
    private bool facetsLoading;

    public async Task LoadFacets()
    {
        if (Countries.Count > 0 || facetsLoading) return;
        facetsLoading = true;
        async Task<List<Facet>> Try(Task<List<Facet>> t) { try { return await t; } catch { return []; } }
        var c = Try(Api.Countries());
        var l = Try(Api.Languages());
        var t = Try(Api.Tags());
        Countries = await c;
        Languages = await l;
        Tags = await t;
        facetsLoading = false;
    }

    public void SearchChanged()
    {
        searchCts?.Cancel();
        var query = search;
        if (query.IsEmpty)
        {
            SearchResults = [];
            SearchLoading = false;
            SearchError = null;
            return;
        }
        SearchLoading = true;
        var cts = new CancellationTokenSource();
        searchCts = cts;
        _ = Run();

        async Task Run()
        {
            try
            {
                await Task.Delay(350, cts.Token);
                var results = await Api.Search(query, cts.Token);
                if (cts.IsCancellationRequested) return;
                SearchResults = results;
                SearchError = null;
                if (Diagnostics.OpenFirstResult && results.Count > 0 && !openedFirstResult)
                {
                    openedFirstResult = true;
                    ShowStation(results[0]);
                }
            }
            catch (OperationCanceledException) when (cts.IsCancellationRequested) { return; }
            catch (Exception e)
            {
                if (cts.IsCancellationRequested) return;
                SearchError = e.Message;
            }
            SearchLoading = false;
        }
    }

    private bool openedFirstResult;
}
