using System.Reflection;

namespace Shortwave;

/// <summary>
/// Test hooks, used by scripts\smoke-test.ps1 and scripts\recording-test.ps1 (same flags as the Mac app):
///   SHORTWAVE_LOG=1            log player events to stderr
///   SHORTWAVE_TEST=1           test run with no other effect
///   -playURL &lt;url&gt;            play a stream at launch (as a temporary local station)
///   -page library|discover|search|settings, -searchText &lt;text&gt;
///   -openFirstResult           show details of the first search result
///   -openTrackDetail           show details of the first track being recorded
///   -muted / -audible          start muted (the default in test runs) / allow sound in a test run
///   -&lt;settingKey&gt; &lt;value&gt;     override a setting for this run (e.g. -recordingMode everything)
/// Windows-only: -miniPlayer (open the mini player), -screenshot &lt;png&gt; [-screenshotDelay &lt;s&gt;] (save the
/// main window and exit), -quitAfter &lt;s&gt; (exit after that many seconds).
/// A test run uses its own data folder and settings, and stays muted and off the system media controls.
/// </summary>
public static class Diagnostics
{
    static readonly string[] Args = Environment.GetCommandLineArgs().Skip(1).ToArray();
    public static readonly bool Logging = Environment.GetEnvironmentVariable("SHORTWAVE_LOG") == "1";
    static readonly object logLock = new();

    public static void Log(string message)
    {
        if (!Logging) return;
        lock (logLock)
        {
            try { Console.Error.WriteLine("[shortwave] " + message); Console.Error.Flush(); } catch { }
        }
    }

    static bool Has(string flag) => Args.Any(a => string.Equals(a, flag, StringComparison.OrdinalIgnoreCase));

    /// <summary>The value after <c>-key</c>, like the Mac's launch-argument defaults.</summary>
    public static string? Value(string key)
    {
        for (int i = 0; i < Args.Length - 1; i++)
            if (string.Equals(Args[i], "-" + key, StringComparison.OrdinalIgnoreCase)) return Args[i + 1];
        return null;
    }

    public static string? PlayURL => Value("playURL");
    public static string? StartPage => Value("page");
    public static string? SearchText => Value("searchText");
    public static bool OpenFirstResult => Has("-openFirstResult");
    public static bool OpenTrackDetail => Has("-openTrackDetail");
    public static bool OpenMiniPlayer => Has("-miniPlayer");
    public static string? Screenshot => Value("screenshot");
    public static double ScreenshotDelay => double.TryParse(Value("screenshotDelay"), System.Globalization.CultureInfo.InvariantCulture, out var d) ? d : 4;
    public static double? QuitAfter => double.TryParse(Value("quitAfter"), System.Globalization.CultureInfo.InvariantCulture, out var d) ? d : null;

    /// <summary>Test runs use their own data folder and settings, so they never touch the real app's data.</summary>
    public static readonly bool IsTestRun =
        Has("-muted") || PlayURL != null || StartPage != null || SearchText != null || OpenFirstResult || OpenTrackDetail
        || OpenMiniPlayer || Screenshot != null || QuitAfter != null
        || Environment.GetEnvironmentVariable("SHORTWAVE_TEST") == "1";

    /// <summary>Test runs are always muted and keep off the system media controls (pass -audible for a sound test).</summary>
    public static readonly bool StartMuted = Has("-muted") || (IsTestRun && !Has("-audible"));

#if NIGHTLY
    public const bool IsNightly = true;
    public const string AppName = "Shortwave Nightly";
    public const string AppUserModelId = "Joshspeakman.Shortwave.Nightly";
#else
    public const bool IsNightly = false;
    public const string AppName = "Shortwave";
    public const string AppUserModelId = "Joshspeakman.Shortwave";
#endif

    public static string Version =>
        typeof(Diagnostics).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "1.0";

    public static string? Commit =>
        typeof(Diagnostics).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>().FirstOrDefault(a => a.Key == "ShortwaveCommit")?.Value;
}
