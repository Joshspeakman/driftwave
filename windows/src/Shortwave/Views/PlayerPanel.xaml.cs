using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Shortwave.Audio;
using Shortwave.Core;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>The player side panel: station, current track, controls and recently played tracks.</summary>
public sealed partial class PlayerPanel : UserControl
{
    private Player? player;
    private Track? track;
    private List<Theming.PlayerDecor> decor = [];
    private bool hideTrackText;

    /// <summary>Shows the theme's player decorations (replacing any earlier ones).</summary>
    public void SetDecor(List<Theming.PlayerDecor> items)
    {
        foreach (var d in decor) { Theming.EffectsClock.Remove(d); d.Dispose(); }
        DecorTop.Children.Clear();
        DecorMiddle.Children.Clear();
        decor = items;
        foreach (var d in items)
        {
            (d.Top ? DecorTop : DecorMiddle).Children.Add(d.Root);
            Theming.EffectsClock.Add(d);
        }
        DecorTop.Visibility = DecorTop.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        DecorMiddle.Visibility = DecorMiddle.Children.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        hideTrackText = items.Any(d => d.ReplacesTrackText);
        Refresh();
    }

    public PlayerPanel()
    {
        InitializeComponent();
        PastList.ElementPrepared += (_, e) =>
        {
            if (player != null && enteringTrack != null && e.Index >= 0 && e.Index < player.PastTracks.Count && ReferenceEquals(player.PastTracks[e.Index], enteringTrack))
            {
                enteringTrack = null;
                Motion.Entrance(e.Element, rise: -10);
            }
            else Motion.Reset(e.Element);
        };
        PastList.ElementClearing += (_, e) => Motion.Glide(e.Element, false);
        Loaded += (_, _) =>
        {
            player = App.Current?.Model.Player;
            if (player is null) return;
            player.PropertyChanged -= OnChanged;
            player.PropertyChanged += OnChanged;
            player.PastTracks.CollectionChanged -= OnPastChanged;
            player.PastTracks.CollectionChanged += OnPastChanged;
            PastList.ItemsSource = player.PastTracks;
            Refresh();
        };
        Unloaded += (_, _) =>
        {
            if (player is null) return;
            player.PropertyChanged -= OnChanged;
            player.PastTracks.CollectionChanged -= OnPastChanged;
            BindTrack(null);
        };
    }

    void OnChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e) => Refresh();
    void OnPastChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
    {
        // Runs before the list updates (it subscribed first): the rows already there glide down to make room.
        if (e.Action == System.Collections.Specialized.NotifyCollectionChangedAction.Add && e.NewItems?[0] is Track t)
        {
            Motion.GlideDuring(PastList);
            enteringTrack = t;
        }
        Refresh();
    }

    Track? enteringTrack;
    string? shownStationId;
    string? shownSong;

    void BindTrack(Track? t)
    {
        if (ReferenceEquals(t, track)) return;
        if (track != null) track.PropertyChanged -= OnChanged;
        track = t;
        if (track != null) track.PropertyChanged += OnChanged;
    }

    void Refresh()
    {
        if (player is null) return;
        var station = player.Station;
        Empty.Visibility = station is null ? Visibility.Visible : Visibility.Collapsed;
        ContentScroller.Visibility = station is null ? Visibility.Collapsed : Visibility.Visible;
        BindTrack(player.PlayingTrack);
        if (station is null) return;

        Cover.Station = station;
        StationName.Text = station.Name;
        if (station.Id != shownStationId)
        {
            bool first = shownStationId is null;
            shownStationId = station.Id;
            if (!first) { Motion.FadeIn(CoverButton); Motion.FadeIn(TitleBlock, 1f); }
        }
        string song = "", artist = "";
        bool failed = player.State == PlayerState.Failed;
        switch (player.State)
        {
            case PlayerState.Loading: artist = "Buffering…"; break;
            case PlayerState.Stopped: artist = station.Subtitle; break;
            case PlayerState.Playing:
                if (track != null) { song = track.SongTitle; artist = track.Artist ?? ""; }
                else artist = "Playing";
                break;
        }
        if (song.Length > 0 && song != shownSong && shownSong != null) Motion.FadeIn(SongText, 1f);
        shownSong = song;
        SongText.Text = song;
        SongText.Visibility = song.Length > 0 && !hideTrackText ? Visibility.Visible : Visibility.Collapsed;
        ArtistText.Text = artist;
        ArtistText.Visibility = artist.Length > 0 && !hideTrackText ? Visibility.Visible : Visibility.Collapsed;
        FailurePanel.Visibility = failed ? Visibility.Visible : Visibility.Collapsed;
        FailureText.Text = player.FailureMessage ?? "";

        if (track != null)
        {
            RecordingIndicator.Visibility = Visibility.Visible;
            bool recording = track.State == RecordingState.Recording;
            RecordingGlyph.Glyph = recording ? "\uE7C8" : track.State.IsRecorded() ? "\uE930" : "\uEA3A";
            RecordingGlyph.Foreground = recording
                ? Ui.ThemeBrush("SystemFillColorCriticalBrush")
                : Ui.ThemeBrush("TextFillColorSecondaryBrush");
            RecordingTitle.Text = track.StateTitle;
            RecordingText.Text = track.SaveWhenRecorded && recording ? "Will be saved when complete" : track.StateExplanation;
        }
        else RecordingIndicator.Visibility = Visibility.Collapsed;

        PastPanel.Visibility = player.PastTracks.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    void CoverButton_Click(object sender, RoutedEventArgs e)
    {
        if (player?.Station is { } s) App.Current?.Model.ShowStation(s);
    }

    void RecordingIndicator_Click(object sender, RoutedEventArgs e)
    {
        if (track != null) App.Current?.Model.ShowTrack(track);
    }
}
