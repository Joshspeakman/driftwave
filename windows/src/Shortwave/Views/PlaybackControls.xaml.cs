using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Shortwave.Audio;
using Windows.Devices.Enumeration;
using Windows.Media.Devices;

namespace Shortwave.Views;

/// <summary>Play/stop, mute and volume; the large variant adds the audio output picker (the Mac's AirPlay button).</summary>
public sealed partial class PlaybackControls : UserControl
{
    private Player? player;
    private bool updating;

    public PlaybackControls()
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

    public static readonly DependencyProperty LargeProperty = DependencyProperty.Register(nameof(Large), typeof(bool), typeof(PlaybackControls),
        new PropertyMetadata(false, (d, _) => ((PlaybackControls)d).ApplySize()));

    public bool Large { get => (bool)GetValue(LargeProperty); set => SetValue(LargeProperty, value); }

    void ApplySize()
    {
        double s = Large ? 48 : 30;
        PlayButton.Width = PlayButton.Height = s;
        PlayButton.CornerRadius = new CornerRadius(s / 2);
        PlayGlyph.FontSize = Large ? 20 : 13;
        StopGlyph.Width = StopGlyph.Height = Large ? 16 : 10;
        Spinner.Width = Spinner.Height = Large ? 34 : 22;
        Panel.Spacing = Large ? 12 : 6;
        VolumeSlider.Width = Large ? 120 : 90;
        OutputButton.Visibility = Large ? Visibility.Visible : Visibility.Collapsed;
    }

    void OnChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e) => Refresh();

    void Refresh()
    {
        if (player is null) return;
        ApplySize();
        bool playing = player.IsPlaying;
        PlayGlyph.Visibility = playing ? Visibility.Collapsed : Visibility.Visible;
        StopGlyph.Visibility = playing ? Visibility.Visible : Visibility.Collapsed;
        StopGlyph.Opacity = player.IsLoading ? 0.35 : 1;
        Spinner.IsActive = player.IsLoading;
        PlayButton.IsEnabled = player.HasStation;
        string label = playing ? "Stop" : "Play";
        ToolTipService.SetToolTip(PlayButton, label + " (Ctrl+Enter)");
        AutomationProperties.SetName(PlayButton, label);

        double v = player.Volume;
        MuteGlyph.Glyph = player.IsMuted || v == 0 ? "\uE74F" : v < 0.34 ? "\uE993" : v < 0.67 ? "\uE994" : "\uE995";
        ToolTipService.SetToolTip(MuteButton, player.IsMuted ? "Unmute" : "Mute");
        AutomationProperties.SetName(MuteButton, player.IsMuted ? "Unmute" : "Mute");
        updating = true;
        VolumeSlider.Value = Math.Round(v * 100);
        updating = false;
    }

    void PlayButton_Click(object sender, RoutedEventArgs e) => player?.TogglePlayback();
    void MuteButton_Click(object sender, RoutedEventArgs e) => player?.ToggleMute();

    void VolumeSlider_ValueChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (updating || player is null) return;
        player.Volume = e.NewValue / 100.0;
    }

    async void OutputMenu_Opening(object sender, object e)
    {
        OutputMenu.Items.Clear();
        if (player is null) return;
        string? current = player.AudioDevice?.Id;
        var def = new ToggleMenuFlyoutItem { Text = "System Default", IsChecked = current is null };
        def.Click += (_, _) => player.SetAudioDevice(null);
        OutputMenu.Items.Add(def);
        OutputMenu.Items.Add(new MenuFlyoutSeparator());
        try
        {
            var devices = await DeviceInformation.FindAllAsync(MediaDevice.GetAudioRenderSelector());
            foreach (var d in devices.Where(d => d.IsEnabled))
            {
                var item = new ToggleMenuFlyoutItem { Text = d.Name, IsChecked = d.Id == current };
                item.Click += (_, _) => player.SetAudioDevice(d);
                OutputMenu.Items.Add(item);
            }
        }
        catch (Exception ex) { Diagnostics.Log("audio devices: " + ex.Message); }
    }
}
