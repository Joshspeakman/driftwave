using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Core;
using Shortwave.Integration;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>General and Recording settings (the Mac's Settings window), plus About.</summary>
public sealed partial class SettingsPage : UserControl
{
    private readonly AppModel app;
    private readonly Func<nint> hwnd;
    private bool updating;

    public SettingsPage(AppModel app, Func<nint> hwnd)
    {
        this.app = app;
        this.hwnd = hwnd;
        InitializeComponent();
        ModeButtons.ItemsSource = RecordingModes.All.Select(m => m.Title()).ToList();
        ThemeBox.ItemsSource = AppThemes.All.Select(t => t.Title()).ToList();
        ThemeHost.Children.Add(new ThemeSettings(app, hwnd, () => new Border { Style = (Style)Resources["Card"] }));
        AboutTitle.Text = $"{Diagnostics.AppName} {Diagnostics.Version.Split('+')[0]}{(Diagnostics.Commit is { Length: > 0 } c ? $" ({c})" : "")}";
        app.Settings.PropertyChanged += (_, _) => Refresh();
        Refresh();
    }

    static string Minutes(int seconds)
    {
        if (seconds < 60) return $"{seconds}s";
        return seconds % 60 == 0 ? $"{seconds / 60}m" : $"{seconds / 60}m {seconds % 60}s";
    }

    static string Hours(int seconds)
    {
        int h = seconds / 3600, m = seconds % 3600 / 60;
        return h == 0 ? $"{m}m" : m == 0 ? $"{h}h" : $"{h}h {m}m";
    }

    void Refresh()
    {
        updating = true;
        var s = app.Settings;
        ThemeBox.SelectedIndex = Array.IndexOf(AppThemes.All, s.Appearance);
        BackgroundToggle.IsOn = s.BackgroundPlayback;
        NotificationsToggle.IsOn = s.Notifications;
        PastTracksBox.Value = s.PastTracksCount;
        ModeButtons.SelectedIndex = Array.IndexOf(RecordingModes.All, s.RecordingMode);
        ModeExplanation.Text = s.RecordingMode.Explanation();
        RecordingOptions.Opacity = s.RecordingMode == RecordingMode.Nothing ? 0.5 : 1;
        RecordingOptions.IsHitTestVisible = s.RecordingMode != RecordingMode.Nothing;
        FolderText.Text = s.TrackDirectory;
        ToolTipService.SetToolTip(FolderText, s.TrackDirectory);
        OpenFolderButton.IsEnabled = Directory.Exists(s.TrackDirectory);
        MinBox.Value = s.MinimumDuration;
        MinText.Text = Minutes(s.MinimumDuration);
        MaxBox.Value = s.MaximumDuration;
        MaxText.Text = Hours(s.MaximumDuration);
        updating = false;
    }

    void ThemeBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (updating || ThemeBox.SelectedIndex < 0) return;
        app.Settings.Appearance = AppThemes.All[ThemeBox.SelectedIndex];
    }

    void BackgroundToggle_Toggled(object sender, RoutedEventArgs e) { if (!updating) app.Settings.BackgroundPlayback = BackgroundToggle.IsOn; }

    void NotificationsToggle_Toggled(object sender, RoutedEventArgs e)
    {
        if (updating) return;
        bool on = NotificationsToggle.IsOn;
        if (on && !Notifications.RequestPermission())
        {
            NotificationsText.Text = "Notifications are turned off for this app in Windows Settings > System > Notifications.";
            on = false;
        }
        app.Settings.Notifications = on;
        Refresh();
    }

    void PastTracksBox_ValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (updating || double.IsNaN(args.NewValue)) { Refresh(); return; }
        app.Settings.PastTracksCount = (int)Math.Round(args.NewValue);
    }

    void ModeButtons_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (updating || ModeButtons.SelectedIndex < 0) return;
        app.Settings.RecordingMode = RecordingModes.All[ModeButtons.SelectedIndex];
    }

    void MinBox_ValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (updating || double.IsNaN(args.NewValue)) { Refresh(); return; }
        app.Settings.MinimumDuration = Math.Clamp((int)Math.Round(args.NewValue), 0, 600);
    }

    void MaxBox_ValueChanged(NumberBox sender, NumberBoxValueChangedEventArgs args)
    {
        if (updating || double.IsNaN(args.NewValue)) { Refresh(); return; }
        app.Settings.MaximumDuration = Math.Clamp((int)Math.Round(args.NewValue), 60, 7200);
    }

    async void ChooseFolder_Click(object sender, RoutedEventArgs e)
    {
        if (await Ui.PickFolder(hwnd()) is { } path) app.Settings.TrackDirectory = path;
    }

    void OpenFolder_Click(object sender, RoutedEventArgs e) => Ui.Open(app.Settings.TrackDirectory);
}
