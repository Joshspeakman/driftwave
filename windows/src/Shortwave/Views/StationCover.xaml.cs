using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Shortwave.Core;
using Shortwave.Model;
using Windows.UI;

namespace Shortwave.Views;

/// <summary>Station artwork with a generated fallback when the station has no usable image.</summary>
public sealed partial class StationCover : UserControl
{
    private Station? station;
    private string? shownPath;
    private ImageStore? images;

    public StationCover()
    {
        InitializeComponent();
        Loaded += (_, _) =>
        {
            images = App.Current?.Model.Images;
            if (images != null) images.Changed += OnImageChanged;
            Apply();
        };
        Unloaded += (_, _) => { if (images != null) images.Changed -= OnImageChanged; };
    }

    public static readonly DependencyProperty SizeProperty = DependencyProperty.Register(nameof(Size), typeof(double), typeof(StationCover),
        new PropertyMetadata(148.0, (d, _) => ((StationCover)d).Layout()));

    public double Size { get => (double)GetValue(SizeProperty); set => SetValue(SizeProperty, value); }

    public Station? Station
    {
        get => station;
        set
        {
            string Key(Station? s) => s is null ? "" : $"{s.Id}|{s.CustomCover}|{s.FaviconURL}|{s.Name}";
            if (Key(station) == Key(value)) { station = value; return; }
            station = value;
            Apply();
        }
    }

    void Layout()
    {
        Width = Height = Size;
        var r = new CornerRadius(Size * 0.18);
        Frame.CornerRadius = r;
        Glyph.FontSize = Size * 0.36;
    }

    void OnImageChanged(string key)
    {
        if (station != null && ImageStore.Key(station) == key) Apply();
    }

    async void Apply()
    {
        Layout();
        var s = station;
        SetFallbackColors(s?.Name ?? "");
        var store = App.Current?.Model.Images;
        if (s is null || store is null) { Show(null); return; }
        string? path = store.Cached(s);
        if (path is null)
        {
            Show(null);
            path = await store.Load(s);
            if (!ReferenceEquals(s, station)) return;
        }
        Show(path);
    }

    void Show(string? path)
    {
        if (path == shownPath) return;
        shownPath = path;
        if (path is null)
        {
            Picture.Source = null;
            ImageHost.Visibility = Visibility.Collapsed;
            Fallback.Visibility = Visibility.Visible;
            return;
        }
        var bmp = new BitmapImage { DecodePixelWidth = (int)Math.Ceiling(Size * 2), DecodePixelType = DecodePixelType.Logical };
        bmp.UriSource = new Uri(path);
        Picture.Source = bmp;
        ImageHost.Visibility = Visibility.Visible;
        Fallback.Visibility = Visibility.Collapsed;
    }

    /// <summary>Same hue as the Mac app for a given station name.</summary>
    void SetFallbackColors(string name)
    {
        uint acc = 0;
        foreach (var rune in name.EnumerateRunes()) acc = unchecked((uint)rune.Value + acc * 31) % 360;
        double hue = acc / 360.0;
        Stop1.Color = Hsb(hue, 0.45, 0.85);
        Stop2.Color = Hsb(hue, 0.65, 0.55);
    }

    static Color Hsb(double h, double s, double v)
    {
        double i = Math.Floor(h * 6), f = h * 6 - i;
        double p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s);
        var (r, g, b) = ((int)i % 6) switch
        {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        return Color.FromArgb(255, (byte)Math.Round(r * 255), (byte)Math.Round(g * 255), (byte)Math.Round(b * 255));
    }
}
