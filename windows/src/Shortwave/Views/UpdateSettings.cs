using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>Settings > Updates: version, check, release notes, download and install, and the automatic check.</summary>
public sealed class UpdateSettings : StackPanel
{
    readonly AppModel app;
    readonly TextBlock status = new() { TextWrapping = TextWrapping.Wrap };
    readonly TextBlock notes = new() { TextWrapping = TextWrapping.Wrap, MaxLines = 10, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly Button check = new() { Content = "Check for Updates" };
    readonly Button install = new() { Content = "Download and Install" };
    readonly HyperlinkButton page = new() { Content = "Release notes on GitHub" };
    readonly ProgressBar progress = new() { Minimum = 0, Maximum = 1, Width = 240, VerticalAlignment = VerticalAlignment.Center };
    readonly ToggleSwitch auto = new() { OnContent = "", OffContent = "", MinWidth = 0 };
    bool updating;

    public UpdateSettings(AppModel app, Style cardStyle)
    {
        this.app = app;
        var u = app.Updates;
        install.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        AutomationProperties.SetName(auto, "Check for updates automatically");
        var stack = new StackPanel { Spacing = 8 };
        stack.Children.Add(new TextBlock { Text = $"{Diagnostics.AppName} {Updater.CurrentVersion}" });
        stack.Children.Add(new TextBlock
        {
            Text = $"New versions are published on github.com/{Updater.Repo}. Updating downloads the new version, checks it, and restarts {Diagnostics.AppName}; a station that's playing carries on.",
            TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"],
        });
        stack.Children.Add(status);
        stack.Children.Add(notes);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        buttons.Children.Add(check);
        buttons.Children.Add(install);
        buttons.Children.Add(progress);
        buttons.Children.Add(page);
        stack.Children.Add(buttons);
        var autoRow = new Grid { ColumnSpacing = 12, Margin = new Thickness(0, 4, 0, 0) };
        autoRow.ColumnDefinitions.Add(new ColumnDefinition());
        autoRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        autoRow.Children.Add(new TextBlock { Text = "Check for updates automatically (once a day, at launch)", VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap });
        Grid.SetColumn(auto, 1);
        autoRow.Children.Add(auto);
        stack.Children.Add(autoRow);
        Children.Add(new Border { Style = cardStyle, Child = stack });

        check.Click += async (_, _) => await u.Check();
        install.Click += async (_, _) => await u.DownloadAndInstall();
        page.Click += (_, _) => Ui.Open(u.Latest?.Page ?? Updater.RepoPage + "/releases");
        auto.Toggled += (_, _) => { if (!updating) app.Settings.CheckForUpdates = auto.IsOn; };
        u.PropertyChanged += (_, _) => DispatcherQueue.TryEnqueue(Refresh);
        Refresh();
    }

    void Refresh()
    {
        updating = true;
        var u = app.Updates;
        status.Text = u.State == UpdateState.Idle ? (Updater.CanCheck ? "" : "Test runs don’t check for updates.") : u.Message;
        status.Visibility = status.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        bool available = u.State == UpdateState.Available || (u.State == UpdateState.Failed && u.Latest != null);
        install.Visibility = available && Updater.CanInstall ? Visibility.Visible : Visibility.Collapsed;
        notes.Text = available && u.Latest is { } r && r.Notes.Length > 0 ? r.Notes : "";
        notes.Visibility = notes.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        page.Visibility = u.Latest != null ? Visibility.Visible : Visibility.Collapsed;
        bool busy = u.State is UpdateState.Checking or UpdateState.Downloading or UpdateState.Installing;
        check.IsEnabled = !busy && Updater.CanCheck;
        install.IsEnabled = !busy;
        progress.Visibility = u.State is UpdateState.Downloading or UpdateState.Installing ? Visibility.Visible : Visibility.Collapsed;
        progress.IsIndeterminate = u.State == UpdateState.Installing;
        progress.Value = u.Progress;
        auto.IsOn = app.Settings.CheckForUpdates;
        updating = false;
    }
}
