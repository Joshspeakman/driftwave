using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>One row of the played tracks list.</summary>
public sealed partial class TrackRow : UserControl
{
    private Track? track;

    public TrackRow()
    {
        InitializeComponent();
        DataContextChanged += (_, e) => Bind(e.NewValue as Track);
        Unloaded += (_, _) => Bind(null);
        Loaded += (_, _) => Bind(DataContext as Track);
    }

    void Bind(Track? t)
    {
        if (ReferenceEquals(t, track)) return;
        if (track != null) track.PropertyChanged -= OnChanged;
        track = t;
        if (track != null) track.PropertyChanged += OnChanged;
        Refresh();
    }

    void OnChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e) => Refresh();

    void Refresh()
    {
        if (track is null) return;
        SongText.Text = track.SongTitle;
        ArtistText.Text = track.Artist ?? track.StateTitle;
        SavedIcon.Visibility = track.IsSaved ? Visibility.Visible : Visibility.Collapsed;
        SaveButton.Visibility = !track.IsSaved && track.CanSave ? Visibility.Visible : Visibility.Collapsed;
        RecordingIcon.Visibility = !track.IsSaved && !track.CanSave && track.IsRecording ? Visibility.Visible : Visibility.Collapsed;
    }

    void Body_Click(object sender, RoutedEventArgs e) { if (track != null) App.Current?.Model.ShowTrack(track); }
    async void SaveButton_Click(object sender, RoutedEventArgs e) { if (track != null && App.Current is { } app) await app.Model.Player.Save(track); }
}
