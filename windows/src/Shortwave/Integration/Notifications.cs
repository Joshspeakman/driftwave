using Microsoft.Windows.AppNotifications;
using Microsoft.Windows.AppNotifications.Builder;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Integration;

/// <summary>A toast when a new track starts (off by default), like the Mac's user notifications.</summary>
public static class Notifications
{
    static bool registered;
    static bool failed;

    static bool EnsureRegistered()
    {
        if (registered) return true;
        if (failed) return false;
        try
        {
            string icon = Path.Combine(AppContext.BaseDirectory, "Assets", "Shortwave.png");
            AppNotificationManager.Default.NotificationInvoked += (_, _) => App.Current?.ShowMainWindow();
            AppNotificationManager.Default.Register(Diagnostics.AppName, new Uri(icon));
            registered = true;
        }
        catch (Exception e)
        {
            failed = true;
            Diagnostics.Log("notifications unavailable: " + e.Message);
        }
        return registered;
    }

    /// <summary>Called when the setting is turned on. Returns false when Windows has notifications turned off for the app.</summary>
    public static bool RequestPermission()
    {
        if (!EnsureRegistered()) return false;
        try { return AppNotificationManager.Default.Setting == AppNotificationSetting.Enabled; } catch { return true; }
    }

    public static void Show(Track track, Station station, string? artwork)
    {
        if (!EnsureRegistered()) return;
        try
        {
            var b = new AppNotificationBuilder()
                .AddText(track.SongTitle)
                .SetTag("track").SetGroup("shortwave");
            if (track.Artist is { } artist) b.AddText(artist);
            b.SetAttributionText(station.Name);
            if (artwork != null && File.Exists(artwork)) b.SetAppLogoOverride(new Uri(artwork), AppNotificationImageCrop.Default);
            var n = b.BuildNotification();
            n.ExpiresOnReboot = true;
            AppNotificationManager.Default.Show(n);
            Diagnostics.Log($"notification \"{track.Title}\"");
        }
        catch (Exception e) { Diagnostics.Log("notification failed: " + e.Message); }
    }

    public static void Unregister()
    {
        if (!registered) return;
        try { AppNotificationManager.Default.Unregister(); } catch { }
    }
}
