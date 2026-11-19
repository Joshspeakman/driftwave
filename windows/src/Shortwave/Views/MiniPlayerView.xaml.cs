using Microsoft.UI.Xaml.Controls;
using Shortwave.Audio;

namespace Shortwave.Views;

/// <summary>Compact player for the window's bottom bar and the mini player window.</summary>
public sealed partial class MiniPlayerView : UserControl
{
    private Player? player;

    public MiniPlayerView()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            player = App.Current?.Model.Player;
            if (player != null) { player.PropertyChanged -= OnChanged; player.PropertyChanged += OnChanged; }
            Refresh();
        };
        Unloaded += (_, _) => { if (player != null) player.PropertyChanged -= OnChanged; };
    }

    void OnChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e) => Refresh();

    void Refresh()
    {
        if (player is null) return;
        Cover.Station = player.Station;
        NameText.Text = player.Station?.Name ?? "No Playback";
        StatusText.Text = player.PlayingTrack?.Title ?? player.State switch
        {
            PlayerState.Stopped => player.Station is null ? "Choose a station" : "Stopped",
            PlayerState.Loading => "Buffering…",
            PlayerState.Playing => "Playing",
            _ => player.FailureMessage ?? "",
        };
        ToolTipService.SetToolTip(StatusText, StatusText.Text);
    }
}
