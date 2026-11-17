using Microsoft.UI.Dispatching;
using Shortwave.Audio;
using Shortwave.Core;
using Shortwave.Model;
using Windows.Media;
using Windows.Media.Playback;
using Windows.Storage.Streams;

namespace Shortwave.Integration;

/// <summary>
/// The Windows media overlay and media keys (System Media Transport Controls), the counterpart of the
/// Mac's Now Playing info and remote commands. Never created in test runs.
/// </summary>
public sealed class MediaControls : IDisposable
{
    private readonly SystemMediaTransportControls smtc;
    private readonly DispatcherQueue ui;
    private string? artworkPath;

    public MediaControls(MediaPlayer mp, DispatcherQueue ui, Player player)
    {
        this.ui = ui;
        smtc = mp.SystemMediaTransportControls;
        smtc.IsEnabled = true;
        smtc.IsPlayEnabled = true;
        smtc.IsPauseEnabled = true;
        smtc.IsStopEnabled = true;
        smtc.IsNextEnabled = true;
        smtc.IsPreviousEnabled = true;
        smtc.ButtonPressed += (_, e) =>
        {
            var button = e.Button;
            ui.TryEnqueue(() =>
            {
                Diagnostics.Log($"media key {button.ToString().ToLowerInvariant()}");
                switch (button)
                {
                    case SystemMediaTransportControlsButton.Play: if (!player.IsPlaying) player.StartPlayback(); break;
                    case SystemMediaTransportControlsButton.Pause: player.StopPlayback("media key pause"); break;
                    case SystemMediaTransportControlsButton.Stop: player.StopPlayback("media key stop"); break;
                    case SystemMediaTransportControlsButton.Next: player.Skip(1); break;
                    case SystemMediaTransportControlsButton.Previous: player.Skip(-1); break;
                }
            });
        };
    }

    public void Update(Station? station, Track? track, PlayerState state, string? artwork)
    {
        var u = smtc.DisplayUpdater;
        if (station is null)
        {
            u.ClearAll();
            u.Update();
            smtc.PlaybackStatus = MediaPlaybackStatus.Closed;
            return;
        }
        u.Type = MediaPlaybackType.Music;
        u.AppMediaId = Diagnostics.AppName;
        var m = u.MusicProperties;
        m.AlbumTitle = station.Name;
        if (track != null)
        {
            m.Title = track.SongTitle;
            m.Artist = track.Artist ?? station.Name;
        }
        else
        {
            m.Title = station.Name;
            m.Artist = Diagnostics.AppName;
        }
        if (artwork != artworkPath)
        {
            artworkPath = artwork;
            u.Thumbnail = artwork != null && File.Exists(artwork) ? Thumbnail(artwork) : null;
        }
        u.Update();
        smtc.PlaybackStatus = state switch
        {
            PlayerState.Playing or PlayerState.Loading => MediaPlaybackStatus.Playing,
            PlayerState.Stopped => MediaPlaybackStatus.Stopped,
            _ => MediaPlaybackStatus.Stopped,
        };
    }

    static RandomAccessStreamReference? Thumbnail(string path)
    {
        try
        {
            var ms = new InMemoryRandomAccessStream();
            using (var w = new DataWriter(ms.GetOutputStreamAt(0)))
            {
                w.WriteBytes(File.ReadAllBytes(path));
                w.StoreAsync().AsTask().Wait();
                w.DetachStream();
            }
            return RandomAccessStreamReference.CreateFromStream(ms);
        }
        catch { return null; }
    }

    public void Dispose()
    {
        try { smtc.DisplayUpdater.ClearAll(); smtc.IsEnabled = false; } catch { }
    }
}
