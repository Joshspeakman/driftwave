using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Shortwave.Audio;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>A grid tile for a station: click for details, double-click to play, hover for a play button.</summary>
public sealed partial class StationCard : UserControl
{
    private Station? station;
    private bool hovering;
    private Player? player;
    private DispatcherTimer? singleTap;

    public StationCard()
    {
        InitializeComponent();
        DataContextChanged += (_, e) => { if (e.NewValue is Station s) SetStation(s); };
        Card.Invoked += () => { if (station != null) App.Current?.Model.ShowStation(station); };
        Loaded += (_, _) =>
        {
            player = App.Current?.Model.Player;
            if (player != null) player.PropertyChanged += OnPlayerChanged;
            Refresh();
        };
        Unloaded += (_, _) => { if (player != null) player.PropertyChanged -= OnPlayerChanged; };
    }

    void SetStation(Station s)
    {
        station = s;
        Cover.Station = s;
        NameText.Text = s.Name;
        SubtitleText.Text = s.IsLocal ? "Local station" : s.IsOrphaned ? "Orphaned station" : s.Subtitle.Length == 0 ? " " : s.Subtitle;
        ToolTipService.SetToolTip(Card, s.Name);
        AutomationProperties.SetName(Card, s.Name);
        Refresh();
    }

    void OnPlayerChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (e.PropertyName is "" or nameof(Player.State) or nameof(Player.Station) or nameof(Player.IsPlaying)) Refresh();
    }

    bool IsCurrent => station != null && player?.Station?.Id == station.Id;
    bool IsActive => IsCurrent && player!.IsPlaying;

    void Refresh()
    {
        bool active = IsActive;
        PlayButton.Visibility = hovering || active ? Visibility.Visible : Visibility.Collapsed;
        PlayGlyph.Visibility = active ? Visibility.Collapsed : Visibility.Visible;
        StopGlyph.Visibility = active ? Visibility.Visible : Visibility.Collapsed;
        ToolTipService.SetToolTip(PlayButton, active ? "Stop" : "Play");
        ActiveIcon.Visibility = active ? Visibility.Visible : Visibility.Collapsed;
        Card.Background = hovering ? (Brush)Application.Current.Resources["SubtleFillColorSecondaryBrush"] : new SolidColorBrush(Microsoft.UI.Colors.Transparent);
    }

    void Card_PointerEntered(object sender, PointerRoutedEventArgs e) { hovering = true; Refresh(); }
    void Card_PointerExited(object sender, PointerRoutedEventArgs e) { hovering = false; Refresh(); }

    void PlayButton_Click(object sender, RoutedEventArgs e)
    {
        if (station is null || App.Current is not { } app) return;
        if (IsActive) app.Model.Player.StopPlayback(); else app.Model.Play(station);
    }

    // A single click opens the details, a double click plays: wait for the double-click time before acting on one click.
    void Card_Tapped(object sender, TappedRoutedEventArgs e)
    {
        singleTap?.Stop();
        singleTap = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(GetDoubleClickTime()) };
        singleTap.Tick += (t, _) => { ((DispatcherTimer)t!).Stop(); if (station != null) App.Current?.Model.ShowStation(station); };
        singleTap.Start();
    }

    void Card_DoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        singleTap?.Stop();
        if (station != null) App.Current?.Model.Play(station);
    }

    void Card_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (station is null) return;
        if (e.Key == Windows.System.VirtualKey.Enter) { App.Current?.Model.Play(station); e.Handled = true; }
        else if (e.Key == Windows.System.VirtualKey.Space) { App.Current?.Model.ShowStation(station); e.Handled = true; }
    }

    void Menu_Opening(object sender, object e)
    {
        Menu.Items.Clear();
        if (station is null || App.Current?.Model is not { } app) return;
        var s = station;
        MenuFlyoutItem Item(string text, string glyph, Action a)
        {
            var i = new MenuFlyoutItem { Text = text, Icon = new FontIcon { Glyph = glyph } };
            i.Click += (_, _) => a();
            return i;
        }
        Menu.Items.Add(Item("Play", "\uF5B0", () => app.Play(s)));
        Menu.Items.Add(Item("Show Station Details", "\uE946", () => app.ShowStation(s)));
        Menu.Items.Add(new MenuFlyoutSeparator());
        if (app.IsInLibrary(s)) Menu.Items.Add(Item("Remove from Library", "\uE738", () => app.RemoveFromLibrary(s)));
        else Menu.Items.Add(Item("Add to Library", "\uE710", () => app.AddToLibrary(s)));
        Menu.Items.Add(Item("Copy Stream URL", "\uE8C8", () => Ui.Copy(s.StreamURL)));
        if (s.Homepage is { } home && Uri.TryCreate(home, UriKind.Absolute, out var url))
            Menu.Items.Add(Item("Open Homepage", "\uE774", () => Ui.Open(url.AbsoluteUri)));
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")] static extern uint GetDoubleClickTime();
}
