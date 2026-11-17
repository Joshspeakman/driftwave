using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Shortwave.Audio;
using Shortwave.Integration;
using Shortwave.Model;
using Shortwave.Views;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace Shortwave;

public partial class App : Application
{
    public static new App? Current => Application.Current as App;

    public AppModel Model { get; private set; } = null!;
    public DispatcherQueue DispatcherQueue { get; private set; } = null!;
    private MainWindow? main;
    private MiniPlayerWindow? mini;
    private TrayIcon? tray;
    private bool quitting;

    public App()
    {
        InitializeComponent();
        UnhandledException += (_, e) => Diagnostics.Log("unhandled: " + e.Exception);
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        DispatcherQueue = DispatcherQueue.GetForCurrentThread();
        Model = new AppModel(DispatcherQueue);
        Model.Player.OnNewTrack = (t, s) => Notifications.Show(t, s, Model.Images.Cached(s));

        main = new MainWindow(Model);
        main.Activate();

        string icon = Path.Combine(AppContext.BaseDirectory, "Assets", "Shortwave.ico");
        tray = new TrayIcon(icon, Diagnostics.AppName, TrayMenu, ShowMainWindow);
        Model.Player.PropertyChanged += (_, _) => UpdateTray();
        UpdateTray();

        if (Diagnostics.OpenMiniPlayer) ShowMiniPlayer();
        if (Diagnostics.Screenshot is { } shot) After(Diagnostics.ScreenshotDelay, async () =>
        {
            await SaveScreenshot(main, shot);
            if (mini != null) await SaveScreenshot(mini, Path.ChangeExtension(shot, null) + "-mini.png");
            if (Diagnostics.QuitAfter is null) Quit();
        });
        if (Diagnostics.QuitAfter is { } q) After(q, () => { Quit(); return Task.CompletedTask; });
    }

    private readonly HashSet<DispatcherQueueTimer> timers = [];   // keeps pending timers from being collected

    void After(double seconds, Func<Task> action)
    {
        var t = DispatcherQueue.CreateTimer();
        t.Interval = TimeSpan.FromSeconds(seconds);
        t.IsRepeating = false;
        timers.Add(t);
        t.Tick += async (s, _) => { s.Stop(); timers.Remove(s); await action(); };
        t.Start();
    }

    public void ShowMainWindow()
    {
        if (main is null) return;
        main.AppWindow.Show();
        main.Activate();
        Native.SetForegroundWindow(main.Hwnd);
    }

    public void ShowMiniPlayer()
    {
        mini ??= new MiniPlayerWindow(Model);
        mini.AppWindow.Show();
        mini.Activate();
    }

    public void ShowSettings()
    {
        ShowMainWindow();
        Model.Page = Page.Settings;
    }

    /// <summary>Called when the main window's close button is pressed.</summary>
    public bool MainWindowClosing()
    {
        if (quitting) return true;
        // Closing the window stops playback unless background playback is on. The app keeps running in the notification area.
        if (!Model.Settings.BackgroundPlayback) Model.Player.StopPlayback("window closed");
        return false;
    }

    public void Quit()
    {
        if (quitting) return;
        quitting = true;
        Diagnostics.Log("quit");
        Model.Player.Shutdown();
        tray?.Dispose();
        mini?.Close();
        main?.Close();
        Exit();
    }

    void UpdateTray()
    {
        var p = Model.Player;
        string tip = p.Station is { } s
            ? (p.PlayingTrack is { } t && p.IsPlaying ? $"{t.Title}\n{s.Name}" : $"{s.Name}{(p.IsPlaying ? "" : " (stopped)")}")
            : Diagnostics.AppName;
        tray?.SetTooltip(tip);
    }

    List<TrayMenuItem> TrayMenu()
    {
        var p = Model.Player;
        var items = new List<TrayMenuItem>();
        if (p.Station is { } st)
        {
            items.Add(new(st.Name, Enabled: false));
            if (p.PlayingTrack is { } t) items.Add(new(t.Title, Enabled: false));
            items.Add(new(p.IsPlaying ? "Stop" : "Play", p.TogglePlayback));
        }
        else items.Add(new("No Playback", Enabled: false));
        if (Model.Library.Count > 0)
        {
            items.Add(TrayMenuItem.Separator);
            items.Add(new("Library", Children: Model.SortedLibrary.Select(s =>
                new TrayMenuItem(s.Name, () => Model.Play(s), Checked: p.Station?.Id == s.Id)).ToList()));
        }
        items.Add(TrayMenuItem.Separator);
        items.Add(new($"Show {Diagnostics.AppName}", ShowMainWindow));
        items.Add(new("Mini Player", ShowMiniPlayer));
        items.Add(new("Settings…", ShowSettings));
        items.Add(TrayMenuItem.Separator);
        items.Add(new($"Quit {Diagnostics.AppName}", Quit));
        return items;
    }

    /// <summary>
    /// The -screenshot test hook: renders a window's XAML, plus any open dialogs and flyouts, over the theme
    /// background and saves it as PNG. (Rendering the XAML works even when the screen can't be captured.)
    /// </summary>
    public static async Task SaveScreenshot(Window window, string path)
    {
        try
        {
            if (window.Content is not FrameworkElement root || root.XamlRoot is null) return;
            Diagnostics.Log("screenshot: rendering window");
            var (bg, w, h) = await Render(root);
            bool dark = root.ActualTheme == ElementTheme.Dark;
            var canvas = new byte[w * h * 4];
            byte shade = dark ? (byte)0x20 : (byte)0xF3;
            for (int i = 0; i < canvas.Length; i += 4) { canvas[i] = canvas[i + 1] = canvas[i + 2] = shade; canvas[i + 3] = 255; }
            Blend(canvas, w, h, bg, w, h, 0, 0);
            double scale = root.XamlRoot.RasterizationScale;
            foreach (var popup in Microsoft.UI.Xaml.Media.VisualTreeHelper.GetOpenPopupsForXamlRoot(root.XamlRoot).Reverse())   // topmost comes first
            {
                if (popup.Child is not FrameworkElement child || child.ActualWidth < 1) continue;
                var at = child.TransformToVisual(null).TransformPoint(new Windows.Foundation.Point(0, 0));
                Diagnostics.Log($"screenshot: rendering popup {child.GetType().Name}");
                var (px, pw, ph) = await Render(child);
                Blend(canvas, w, h, px, pw, ph, (int)Math.Round(at.X * scale), (int)Math.Round(at.Y * scale));
            }
            using var stream = new InMemoryRandomAccessStream();
            var enc = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, stream);
            enc.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Ignore, (uint)w, (uint)h, 96, 96, canvas);
            await enc.FlushAsync();
            var bytes = new byte[stream.Size];
            stream.Seek(0);
            await stream.ReadAsync(bytes.AsBuffer(), (uint)bytes.Length, InputStreamOptions.None);
            Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
            await File.WriteAllBytesAsync(path, bytes);
            Diagnostics.Log($"screenshot {path}");
        }
        catch (Exception e) { Diagnostics.Log("screenshot failed: " + e.Message); }
    }

    static async Task<(byte[] Pixels, int Width, int Height)> Render(UIElement element)
    {
        // RenderAsync never completes while the window isn't being drawn (e.g. minimized), so don't wait forever.
        var rtb = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
        var render = rtb.RenderAsync(element).AsTask();
        if (await Task.WhenAny(render, Task.Delay(10000)) != render) throw new TimeoutException("the window is not being drawn");
        await render;
        return ((await rtb.GetPixelsAsync()).ToArray(), rtb.PixelWidth, rtb.PixelHeight);
    }

    /// <summary>Draws premultiplied BGRA pixels over the canvas.</summary>
    static void Blend(byte[] dst, int dw, int dh, byte[] src, int sw, int sh, int ox, int oy)
    {
        for (int y = 0; y < sh; y++)
        {
            int ty = y + oy;
            if (ty < 0 || ty >= dh) continue;
            for (int x = 0; x < sw; x++)
            {
                int tx = x + ox;
                if (tx < 0 || tx >= dw) continue;
                int s = (y * sw + x) * 4, d = (ty * dw + tx) * 4;
                int a = src[s + 3];
                if (a == 0) continue;
                for (int c = 0; c < 3; c++) dst[d + c] = (byte)Math.Min(255, src[s + c] + dst[d + c] * (255 - a) / 255);
            }
        }
    }
}
