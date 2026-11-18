using System.Globalization;
using System.IO.Compression;
using System.Security.Cryptography;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Shortwave.Core;

namespace Shortwave.Model;

public enum UpdateState { Idle, Checking, UpToDate, Available, Downloading, Installing, Failed }

/// <summary>A release on the public Driftwave repo.</summary>
public sealed record UpdateRelease(string Tag, Version Version, string Title, string Notes, DateTimeOffset Published,
    string ZipUrl, long ZipSize, string? ShaUrl, string Page);

/// <summary>
/// Settings > Updates: looks for newer releases on github.com/Joshspeakman/driftwave (tags <c>nightly-1.0.N</c> for
/// Driftwave Nightly, <c>v1.0.N</c> for Driftwave), downloads the zip, checks its SHA-256, and hands over to a small
/// script that waits for the app to quit, replaces the installed files (with a backup it restores on failure) and
/// starts it again. The checksum catches a broken download; it doesn't prove who published the release (it sits
/// next to the zip), so the app only downloads from that repo's release assets over HTTPS.
/// Test runs never contact GitHub: they check only a -updateFeed (a file or URL with the same JSON) when given one,
/// and never touch the Settings > Apps entry.
/// </summary>
public sealed class Updater : Observable
{
    public const string Repo = "Joshspeakman/driftwave";
    public static string RepoPage => $"https://github.com/{Repo}";
    static readonly HttpClient http = CreateClient();
    readonly AppPaths paths;
    readonly Preferences settings;
    readonly DispatcherQueue ui;

    public Updater(AppPaths paths, Preferences settings, DispatcherQueue ui)
    {
        this.paths = paths;
        this.settings = settings;
        this.ui = ui;
    }

    static HttpClient CreateClient()
    {
        var c = new HttpClient { Timeout = TimeSpan.FromMinutes(5) };
        c.DefaultRequestHeaders.UserAgent.ParseAdd(RadioBrowser.UserAgent);
        c.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        return c;
    }

    private UpdateState state;
    public UpdateState State { get => state; private set => Set(ref state, value); }
    private string message = "";
    public string Message { get => message; private set => Set(ref message, value); }
    private double progress;
    public double Progress { get => progress; private set => Set(ref progress, value); }
    private UpdateRelease? latest;
    public UpdateRelease? Latest { get => latest; private set => Set(ref latest, value); }

    public static Version CurrentVersion =>
        Version.TryParse(Diagnostics.Version.Split('+')[0], out var v) ? v : new Version(0, 0);
    static string TagPrefix => Diagnostics.IsNightly ? "nightly-" : "v";

    /// <summary>GitHub's releases list; in test runs only the -updateFeed override, if any.</summary>
    static string? Feed => Diagnostics.IsTestRun ? Diagnostics.Value("updateFeed") : $"https://api.github.com/repos/{Repo}/releases?per_page=30";
    public static bool CanCheck => Feed != null;

    /// <summary>The folder this copy runs from. Updates install only into an installed copy (its folder has uninstall.ps1).</summary>
    public static string InstallFolder => AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar);
    public static bool CanInstall => File.Exists(Path.Combine(InstallFolder, "uninstall.ps1"));

    /// <summary>The automatic check at launch: at most once every 20 hours (test runs with a feed: always).</summary>
    public async Task CheckAtLaunch()
    {
        if (!settings.CheckForUpdates || !CanCheck) return;
        if (!Diagnostics.IsTestRun && DateTimeOffset.TryParse(settings.LastUpdateCheck, CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var last)
            && DateTimeOffset.UtcNow - last < TimeSpan.FromHours(20)) return;
        await Check();
    }

    public async Task Check()
    {
        if (State is UpdateState.Checking or UpdateState.Downloading or UpdateState.Installing) return;
        if (Feed is not { } feed)
        {
            State = UpdateState.Failed;
            Message = "Test runs don’t check for updates (give them -updateFeed to try it).";
            return;
        }
        State = UpdateState.Checking;
        Message = "Checking for updates…";
        try
        {
            string json = IsUrl(feed) ? await http.GetStringAsync(feed) : await File.ReadAllTextAsync(feed);
            var best = Parse(json).Where(r => r.Version > CurrentVersion).OrderByDescending(r => r.Version).FirstOrDefault();
            settings.LastUpdateCheck = DateTimeOffset.UtcNow.ToString("o", CultureInfo.InvariantCulture);
            Latest = best;
            if (best is null)
            {
                State = UpdateState.UpToDate;
                Message = $"You’re up to date: {Diagnostics.AppName} {CurrentVersion}.";
            }
            else
            {
                State = UpdateState.Available;
                string name = best.Title.StartsWith("Driftwave", StringComparison.Ordinal) ? best.Title : $"{Diagnostics.AppName} {best.Version}";
                Message = $"{name} is available (you have {CurrentVersion}).";
            }
            Diagnostics.Log($"update check: current {CurrentVersion}, latest {(best?.Version.ToString() ?? "none newer")}");
        }
        catch (Exception e)
        {
            State = UpdateState.Failed;
            Message = "Couldn’t check for updates: " + Short(e);
            Diagnostics.Log("update check failed: " + e.Message);
        }
    }

    static bool IsUrl(string s) => s.StartsWith("http://", StringComparison.OrdinalIgnoreCase) || s.StartsWith("https://", StringComparison.OrdinalIgnoreCase);
    static string Short(Exception e) => e is HttpRequestException h && h.StatusCode is { } code ? $"GitHub answered {(int)code}." : e.Message;

    /// <summary>The releases for this app's channel, from GitHub's releases JSON.</summary>
    IEnumerable<UpdateRelease> Parse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        if (doc.RootElement.ValueKind != JsonValueKind.Array) yield break;
        string flavor = Diagnostics.IsNightly ? "Nightly" : "";
        foreach (var r in doc.RootElement.EnumerateArray())
        {
            if (r.TryGetProperty("draft", out var d) && d.GetBoolean()) continue;
            string tag = r.GetProperty("tag_name").GetString() ?? "";
            if (!tag.StartsWith(TagPrefix, StringComparison.Ordinal)) continue;
            // v1.0.41 or v1.0.41-beta.1: the number decides what's newer; the label is only shown.
            string versionText = tag[TagPrefix.Length..];
            int dash = versionText.IndexOf('-');
            if (dash >= 0) versionText = versionText[..dash];
            if (!Version.TryParse(versionText, out var version)) continue;
            string? zip = null, sha = null;
            long size = 0;
            foreach (var a in r.GetProperty("assets").EnumerateArray())
            {
                string name = a.GetProperty("name").GetString() ?? "";
                string url = a.GetProperty("browser_download_url").GetString() ?? "";
                bool ours = name.StartsWith("Driftwave-" + (flavor.Length > 0 ? flavor + "-" : ""), StringComparison.Ordinal)
                    && (flavor.Length > 0 || !name.Contains("-Nightly-"));
                if (!ours) continue;
                if (name.EndsWith("-win-x64.zip", StringComparison.Ordinal)) { zip = url; size = a.GetProperty("size").GetInt64(); }
                else if (name.EndsWith("-win-x64.zip.sha256", StringComparison.Ordinal)) sha = url;
            }
            if (zip is null || !Allowed(zip) || (sha != null && !Allowed(sha))) continue;
            DateTimeOffset.TryParse(r.TryGetProperty("published_at", out var p) ? p.GetString() : null, CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var published);
            yield return new UpdateRelease(tag, version, r.TryGetProperty("name", out var n) ? n.GetString() ?? tag : tag,
                r.TryGetProperty("body", out var b) ? b.GetString() ?? "" : "", published, zip, size, sha,
                r.TryGetProperty("html_url", out var h) ? h.GetString() ?? RepoPage : RepoPage);
        }
    }

    /// <summary>Real runs download only this repo's release assets over HTTPS; test feeds may point anywhere.</summary>
    static bool Allowed(string url)
    {
        if (Diagnostics.IsTestRun) return true;
        return Uri.TryCreate(url, UriKind.Absolute, out var u) && u.Scheme == Uri.UriSchemeHttps && u.Host == "github.com"
            && u.AbsolutePath.StartsWith($"/{Repo}/releases/download/", StringComparison.OrdinalIgnoreCase);
    }

    /// <summary>Downloads, verifies and installs the latest release, then quits; the update script starts the new version.</summary>
    public async Task DownloadAndInstall()
    {
        if (Latest is not { } release || State is UpdateState.Downloading or UpdateState.Installing) return;
        if (!CanInstall)
        {
            State = UpdateState.Failed;
            Message = $"This copy runs from {InstallFolder}, which isn’t an installed copy, so it can’t update itself.";
            return;
        }
        string dir = Path.Combine(paths.Caches, "Updates");
        try
        {
            State = UpdateState.Downloading;
            Progress = 0;
            Message = $"Downloading {Diagnostics.AppName} {release.Version}…";
            if (Directory.Exists(dir)) Directory.Delete(dir, recursive: true);
            Directory.CreateDirectory(dir);
            string zip = Path.Combine(dir, $"Driftwave-{release.Version}.zip");
            await Download(release.ZipUrl, zip, release.ZipSize);

            if (release.ShaUrl is null) throw new InvalidDataException("The release has no checksum file.");
            string expected = (await ReadText(release.ShaUrl)).Trim().Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries)[0].ToLowerInvariant();
            string actual;
            await using (var f = File.OpenRead(zip)) actual = Convert.ToHexString(await SHA256.HashDataAsync(f)).ToLowerInvariant();
            if (actual != expected) throw new InvalidDataException("The download is damaged (its checksum doesn’t match). Try again.");

            State = UpdateState.Installing;
            Message = "Installing…";
            string staged = Path.Combine(dir, "new");
            await Task.Run(() => ZipFile.ExtractToDirectory(zip, staged));
            string exe = Path.GetFileName(Environment.ProcessPath!);
            if (!File.Exists(Path.Combine(staged, exe))) throw new InvalidDataException($"The download doesn’t contain {exe}.");

            string script = Path.Combine(dir, "apply-update.ps1");
            await File.WriteAllTextAsync(script, ApplyScript);
            var args = App.Current!.RelaunchArguments();
            string argsFile = Path.Combine(dir, "args.json");
            await File.WriteAllTextAsync(argsFile, JsonSerializer.Serialize(args));
            string registry = Diagnostics.IsTestRun ? "" : $@"HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{Diagnostics.AppUserModelId}";
            var psi = new System.Diagnostics.ProcessStartInfo("powershell.exe") { UseShellExecute = false, CreateNoWindow = true };
            foreach (var a in new[] { "-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File", script,
                "-WaitPid", Environment.ProcessId.ToString(), "-Source", staged, "-Dest", InstallFolder, "-Exe", exe,
                "-Version", release.Version.ToString(), "-RegistryKey", registry, "-ArgsFile", argsFile, "-Log", Path.Combine(dir, "update.log") })
                psi.ArgumentList.Add(a);
            System.Diagnostics.Process.Start(psi);
            Diagnostics.Log($"update {CurrentVersion} -> {release.Version}: installing, quitting");
            ui.TryEnqueue(() => App.Current?.Quit());
        }
        catch (Exception e)
        {
            State = UpdateState.Failed;
            Message = "Couldn’t update: " + Short(e);
            Diagnostics.Log("update failed: " + e);
        }
    }

    async Task Download(string url, string to, long expectedSize)
    {
        if (!IsUrl(url)) { File.Copy(url, to); Progress = 1; return; }
        using var resp = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead);
        resp.EnsureSuccessStatusCode();
        long total = resp.Content.Headers.ContentLength ?? expectedSize;
        await using var src = await resp.Content.ReadAsStreamAsync();
        await using var dst = File.Create(to);
        var buf = new byte[81920];
        long done = 0;
        int n;
        while ((n = await src.ReadAsync(buf)) > 0)
        {
            await dst.WriteAsync(buf.AsMemory(0, n));
            done += n;
            if (total > 0) Progress = Math.Min(1, (double)done / total);
        }
    }

    static async Task<string> ReadText(string url) => IsUrl(url) ? await http.GetStringAsync(url) : await File.ReadAllTextAsync(url);

    /// <summary>Runs after the app quits: back up, copy the new files in, restore on failure, start the app again.</summary>
    const string ApplyScript = """
        param([int]$WaitPid, [string]$Source, [string]$Dest, [string]$Exe, [string]$Version, [string]$RegistryKey, [string]$ArgsFile, [string]$Log)
        function Write-Log([string]$m) { Add-Content -LiteralPath $Log -Value ("{0:u} {1}" -f (Get-Date).ToUniversalTime(), $m) }
        Write-Log "update to ${Version}: waiting for ${WaitPid}"
        try { Wait-Process -Id $WaitPid -Timeout 30 -ErrorAction SilentlyContinue } catch { }
        # Only ever touch an installed copy, and only with a complete new one.
        if (-not (Test-Path -LiteralPath (Join-Path $Dest 'uninstall.ps1')) -or -not (Test-Path -LiteralPath (Join-Path $Dest $Exe))) { Write-Log "not an install folder: $Dest"; exit 1 }
        if (-not (Test-Path -LiteralPath (Join-Path $Source 'uninstall.ps1')) -or -not (Test-Path -LiteralPath (Join-Path $Source $Exe))) { Write-Log "incomplete download: $Source"; exit 1 }
        $inside = $Dest.TrimEnd('\') + '\'
        Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($inside, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 500
        $backup = "$Dest.backup"
        $ok = $false
        if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Recurse -Force }
        robocopy $Dest $backup /E /R:2 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
        if ($LASTEXITCODE -ge 8) { Write-Log "couldn't back up the current version (robocopy $LASTEXITCODE); leaving it as it is" }
        else {
            try {
                # Mirror: files the new version no longer has are removed (the release zip has everything, uninstall.ps1 too).
                robocopy $Source $Dest /MIR /XF Install.ps1 flavor.txt README.txt /R:3 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
                if ($LASTEXITCODE -ge 8) { throw "copy failed (robocopy $LASTEXITCODE)" }
                if (-not (Test-Path -LiteralPath (Join-Path $Dest $Exe))) { throw "$Exe missing after the copy" }
                if ($RegistryKey -and (Test-Path $RegistryKey)) { Set-ItemProperty -Path $RegistryKey -Name DisplayVersion -Value $Version }
                $ok = $true
                Write-Log "updated to $Version"
            }
            catch {
                Write-Log "update failed: $_ (restoring the previous version)"
                robocopy $backup $Dest /MIR /R:3 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
                Write-Log "restore: robocopy $LASTEXITCODE"
            }
            if ($ok) { Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue }
        }
        $arguments = @(Get-Content -Raw -LiteralPath $ArgsFile | ConvertFrom-Json)
        $quoted = $arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }
        if ($quoted) { Start-Process -FilePath (Join-Path $Dest $Exe) -ArgumentList $quoted } else { Start-Process -FilePath (Join-Path $Dest $Exe) }
        Write-Log "started $Exe"
        Remove-Item -LiteralPath $Source -Recurse -Force -ErrorAction SilentlyContinue
        """;
}
