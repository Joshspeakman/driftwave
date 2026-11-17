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

    public PlayerPanel()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            player = App.Current?.Model.Player;
            if (player is null) return;
            player.PropertyChanged += OnChanged;
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
    void OnPastChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e) => Refresh();

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
        SongText.Text = song;
        SongText.Visibility = song.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        ArtistText.Text = artist;
        ArtistText.Visibility = artist.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        FailurePanel.Visibility = failed ? Visibility.Visible : Visibility.Collapsed;
        FailureText.Text = player.FailureMessage ?? "";

        if (track != null)
        {
            RecordingIndicator.Visibility = Visibility.Visible;
            bool recording = track.State == RecordingState.Recording;
            RecordingGlyph.Glyph = recording ? "" : track.State.IsRecorded() ? "" : "";
            RecordingGlyph.Foreground = recording
                ? (Brush)Application.Current.Resources["SystemFillColorCriticalBrush"]
                : (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];
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
