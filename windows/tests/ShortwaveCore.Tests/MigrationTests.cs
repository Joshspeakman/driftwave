using System.Text.Json.Nodes;

namespace Shortwave.Core.Tests;

/// <summary>The Shortwave → Driftwave data copy, on temporary folders only (never the real data).</summary>
public class MigrationTests : IDisposable
{
    readonly string root = Path.Combine(Path.GetTempPath(), "dw-migration-" + Guid.NewGuid().ToString("N")[..8]);
    string Old => Path.Combine(root, "Shortwave");
    string New => Path.Combine(root, "Driftwave");
    string Music => Path.Combine(root, "Music", "Shortwave");

    public MigrationTests()
    {
        Directory.CreateDirectory(Path.Combine(Old, "Covers"));
        Directory.CreateDirectory(Path.Combine(Old, "Themes"));
        File.WriteAllText(Path.Combine(Old, "library.json"), "{\"stations\":[],\"version\":1}");
        File.WriteAllText(Path.Combine(Old, "settings-nightly.json"), "{\"volume\":0.5}");
        File.WriteAllText(Path.Combine(Old, "settings.json"), "{\"trackDirectory\":\"D:\\\\Mine\"}");
        File.WriteAllBytes(Path.Combine(Old, "Covers", "a.png"), [0x89, 0x50, 0x4E, 0x47]);
        File.WriteAllText(Path.Combine(Old, "Themes", "custom-1.json"), "{}");
    }

    public void Dispose() { try { Directory.Delete(root, recursive: true); } catch { } }

    [Fact]
    public void CopiesEverythingAndKeepsTheOldFolder()
    {
        Directory.CreateDirectory(Music);
        Assert.True(DataMigration.CopyIfFirstRun(Old, New, Music));
        foreach (var f in new[] { "library.json", "settings.json", "settings-nightly.json", @"Covers\a.png", @"Themes\custom-1.json" })
        {
            Assert.True(File.Exists(Path.Combine(New, f)), f);
            Assert.True(File.Exists(Path.Combine(Old, f)), "old " + f);
        }
        Assert.False(Directory.Exists(New + ".migrating"));
        // Recordings keep going to the old default folder, unless a folder was chosen.
        Assert.Equal(Music, (string?)JsonNode.Parse(File.ReadAllText(Path.Combine(New, "settings-nightly.json")))!["trackDirectory"]);
        Assert.Equal(0.5, (double?)JsonNode.Parse(File.ReadAllText(Path.Combine(New, "settings-nightly.json")))!["volume"]);
        Assert.Equal(@"D:\Mine", (string?)JsonNode.Parse(File.ReadAllText(Path.Combine(New, "settings.json")))!["trackDirectory"]);
        // The old folder's settings are untouched.
        Assert.Null(JsonNode.Parse(File.ReadAllText(Path.Combine(Old, "settings-nightly.json")))!["trackDirectory"]);
    }

    [Fact]
    public void NeverOverwritesAnExistingNewFolder()
    {
        Directory.CreateDirectory(New);
        File.WriteAllText(Path.Combine(New, "library.json"), "new");
        Assert.False(DataMigration.CopyIfFirstRun(Old, New, null));
        Assert.Equal("new", File.ReadAllText(Path.Combine(New, "library.json")));
    }

    [Fact]
    public void NothingToDoWithoutAnOldFolder()
    {
        Directory.Delete(Old, recursive: true);
        Assert.False(DataMigration.CopyIfFirstRun(Old, New, null));
        Assert.False(Directory.Exists(New));
    }

    [Fact]
    public void NoTrackFolderAddedWhenTheOldOneDoesntExist()
    {
        Assert.True(DataMigration.CopyIfFirstRun(Old, New, Music));   // Music\Shortwave was never created
        Assert.Null(JsonNode.Parse(File.ReadAllText(Path.Combine(New, "settings-nightly.json")))!["trackDirectory"]);
    }
}
