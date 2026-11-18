using System.Text.Json;
using System.Text.Json.Nodes;

namespace Shortwave.Core;

/// <summary>
/// The Windows app was renamed from Shortwave to Driftwave (2026-11-18). On the first launch under the new name, the
/// old data folder (library, settings, covers, the user's themes) is copied to the new one. Never moves, never
/// overwrites: the old folder stays as it was, and an existing new folder is left alone.
/// </summary>
public static class DataMigration
{
    /// <summary>
    /// Copies <paramref name="oldFolder"/> to <paramref name="newFolder"/> if the new one doesn't exist yet and the
    /// old one does. Settings files that didn't choose a track folder get <paramref name="oldTrackFolder"/> (if it
    /// exists), so recordings keep going where they always went. Returns true if it copied.
    /// </summary>
    public static bool CopyIfFirstRun(string oldFolder, string newFolder, string? oldTrackFolder)
    {
        if (Directory.Exists(newFolder) || !Directory.Exists(oldFolder)) return false;
        // Copy into a staging folder first, so a failure halfway never leaves a half-made data folder.
        string staging = newFolder.TrimEnd(Path.DirectorySeparatorChar) + ".migrating";
        if (Directory.Exists(staging)) Directory.Delete(staging, recursive: true);
        CopyTree(oldFolder, staging);
        if (oldTrackFolder != null && Directory.Exists(oldTrackFolder))
        {
            foreach (var settings in Directory.GetFiles(staging, "settings*.json"))
            {
                try
                {
                    var json = JsonNode.Parse(File.ReadAllText(settings)) as JsonObject ?? [];
                    if (json["trackDirectory"] is null)
                    {
                        json["trackDirectory"] = oldTrackFolder;
                        File.WriteAllText(settings, json.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
                    }
                }
                catch (Exception) { /* an unreadable settings file is copied as it was */ }
            }
        }
        Directory.Move(staging, newFolder);
        return true;
    }

    static void CopyTree(string from, string to)
    {
        Directory.CreateDirectory(to);
        foreach (var file in Directory.GetFiles(from)) File.Copy(file, Path.Combine(to, Path.GetFileName(file)));
        foreach (var dir in Directory.GetDirectories(from)) CopyTree(dir, Path.Combine(to, Path.GetFileName(dir)));
    }
}
