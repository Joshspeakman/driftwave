using System.Text;
using System.Text.Json;

namespace Shortwave.Core;

/// <summary>Where the app keeps its files.</summary>
public sealed class AppPaths
{
    /// <summary>%APPDATA%\&lt;appFolder&gt;: library, covers, settings.</summary>
    public string Support { get; }
    /// <summary>%LOCALAPPDATA%\&lt;appFolder&gt;: favicon cache, temporary recordings.</summary>
    public string Caches { get; }

    public AppPaths(string appFolder = "Shortwave")
    {
        Support = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), appFolder);
        Caches = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), appFolder);
    }

    public AppPaths(string support, string caches) { Support = support; Caches = caches; }

    public string Library => Path.Combine(Support, "library.json");
    public string Covers => Path.Combine(Support, "Covers");
    public string Favicons => Path.Combine(Caches, "Favicons");
    public string Recording => Path.Combine(Caches, "Recording");

    public static string DefaultTrackDirectory =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyMusic), "Shortwave");
}

/// <summary>The saved stations, stored as JSON so they survive the station vanishing from the database.</summary>
public static class LibraryFile
{
    public const int Version = 1;

    public static List<Station> Load(string path)
    {
        try
        {
            if (!File.Exists(path)) return [];
            using var doc = JsonDocument.Parse(File.ReadAllBytes(path));
            if (!doc.RootElement.TryGetProperty("stations", out var list) || list.ValueKind != JsonValueKind.Array) return [];
            return list.EnumerateArray().Select(Station.FromJson).OfType<Station>().ToList();
        }
        catch (Exception) { return []; }
    }

    /// <summary>Same layout as the Mac: <c>{"stations": [...], "version": 1}</c>, pretty-printed with sorted keys.</summary>
    public static byte[] Encode(IEnumerable<Station> stations)
    {
        using var ms = new MemoryStream();
        using (var w = new Utf8JsonWriter(ms, new JsonWriterOptions { Indented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping }))
        {
            w.WriteStartObject();
            w.WriteStartArray("stations");
            foreach (var s in stations) s.WriteJson(w);
            w.WriteEndArray();
            w.WriteNumber("version", Version);
            w.WriteEndObject();
        }
        return ms.ToArray();
    }

    public static void Save(IEnumerable<Station> stations, string path)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        string tmp = path + ".tmp";
        File.WriteAllBytes(tmp, Encode(stations));
        File.Move(tmp, path, overwrite: true);
    }
}
