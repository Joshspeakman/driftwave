using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// Neon tubes round the window's panels ("neonEdges"): a bright tube with a white-hot core on the edge of the
/// page, the player card and the bottom bar, and a soft glow spilling round it.
///
/// The glow is a stack of thin rounded rings whose opacity falls away like light does, behind the panels (so it
/// shines through their tinted glass); the tube lies on top. Plain XAML, so screenshots show it, and it only
/// moves when the panels do (no per-frame work). Never animated: neon here hums, it doesn't flicker.
/// Options: "content", "player", "bar" (colours), "glow" (reach in px, default 16), "opacity" (default 0.8).
/// </summary>
public sealed class NeonEdges(JsonObject o, FrameworkElement root) : WindowEffects.Effect
{
    sealed record Target(string Name, FrameworkElement Element, Color Color);

    readonly Canvas glowLayer = new(), tubeLayer = new();
    readonly List<(Target T, List<Border> Glow, Border Tube, Border Core)> parts = [];
    readonly List<(FrameworkElement E, SizeChangedEventHandler H)> sizeHooks = [];
    readonly List<(FrameworkElement E, long Token)> visibilityHooks = [];
    bool built;

    public override IEnumerable<(UIElement, bool)> Elements()
    {
        yield return (glowLayer, false);
        yield return (tubeLayer, true);
    }

    void Build()
    {
        if (built) return;
        built = true;
        var pane = root.FindName("PlayerPane") as FrameworkElement;
        var targets = new List<Target>();
        // The page's own card: the grid holding the page and the player (the content area's edge).
        if (pane?.Parent is FrameworkElement content)
            targets.Add(new Target("content", content, WindowEffects.Opt(o, "content", Color.FromArgb(255, 55, 224, 255))));
        if (root.FindName("PlayerCard") is FrameworkElement card)
            targets.Add(new Target("player", card, WindowEffects.Opt(o, "player", Color.FromArgb(255, 255, 79, 139))));
        if (root.FindName("BottomBar") is FrameworkElement bar)
            targets.Add(new Target("bar", bar, WindowEffects.Opt(o, "bar", Color.FromArgb(255, 255, 79, 139))));

        double reach = WindowEffects.Opt(o, "glow", 16.0);
        int rings = (int)Math.Clamp(reach / 1.5, 4, 24);
        foreach (var t in targets)
        {
            var glow = new List<Border>();
            // Rings outward from the edge (and a few inward, the light on the glass), each 1.5 px wide.
            for (int i = -3; i < rings; i++)
            {
                double d = (i + 0.5) * 1.5;
                double a = Math.Exp(-Math.Pow(d / (reach * 0.55), 2)) * (i < 0 ? 0.5 : 1);
                var ring = new Border { BorderThickness = new Thickness(1.5), BorderBrush = new SolidColorBrush(WindowEffects.A(t.Color, a * 0.55)), Tag = d };
                glow.Add(ring);
                glowLayer.Children.Add(ring);
            }
            var tube = new Border { BorderThickness = new Thickness(2), BorderBrush = new SolidColorBrush(t.Color) };
            var white = Color.FromArgb(255, (byte)(t.Color.R + (255 - t.Color.R) * 0.75), (byte)(t.Color.G + (255 - t.Color.G) * 0.75), (byte)(t.Color.B + (255 - t.Color.B) * 0.75));
            var core = new Border { BorderThickness = new Thickness(0.75), BorderBrush = new SolidColorBrush(white) };
            tubeLayer.Children.Add(tube);
            tubeLayer.Children.Add(core);
            parts.Add((t, glow, tube, core));
            SizeChangedEventHandler h = (_, _) => Layout();
            t.Element.SizeChanged += h;
            sizeHooks.Add((t.Element, h));
            visibilityHooks.Add((t.Element, t.Element.RegisterPropertyChangedCallback(UIElement.VisibilityProperty, (_, _) => Layout())));
        }
        // The player card moves when its pane slides.
        if (pane != null)
        {
            SizeChangedEventHandler h = (_, _) => Layout();
            pane.SizeChanged += h;
            sizeHooks.Add((pane, h));
            visibilityHooks.Add((pane, pane.RegisterPropertyChangedCallback(UIElement.VisibilityProperty, (_, _) => Layout())));
        }
        double opacity = WindowEffects.Opt(o, "opacity", 0.8) * Intensity;
        glowLayer.Opacity = opacity;
        tubeLayer.Opacity = Math.Min(1, opacity + 0.15);
    }

    public override void Resize(double w, double h)
    {
        Build();
        Layout();
    }

    static bool Shown(FrameworkElement e)
    {
        for (DependencyObject? x = e; x is UIElement u; x = VisualTreeHelper.GetParent(x))
            if (u.Visibility != Visibility.Visible) return false;
        return e.ActualWidth > 24 && e.ActualHeight > 24;
    }

    void Layout()
    {
        foreach (var (t, glow, tube, core) in parts)
        {
            bool show = t.Element.XamlRoot != null && Shown(t.Element);
            Rect r = default;
            if (show)
            {
                try { r = t.Element.TransformToVisual(root).TransformBounds(new Rect(0, 0, t.Element.ActualWidth, t.Element.ActualHeight)); }
                catch { show = false; }
            }
            // The page's edge meets the window's right and bottom edges: bring the tube in so none of it is cut off.
            if (show && t.Name == "content")
            {
                var win = new Rect(0, 0, root.ActualWidth, root.ActualHeight);
                double right = Math.Min(r.Right, win.Right - 3), bottom = Math.Min(r.Bottom, win.Bottom - 3);
                r = new Rect(r.X, r.Y, Math.Max(0, right - r.X), Math.Max(0, bottom - r.Y));
            }
            // The card's own clip: a sliding player card shows only the part inside its pane.
            if (show && t.Name == "player" && t.Element.Parent is FrameworkElement pane)
            {
                var p = pane.TransformToVisual(root).TransformBounds(new Rect(0, 0, pane.ActualWidth, pane.ActualHeight));
                if (p.Width < r.Width * 0.9) show = false;   // mid-slide: the tube would hang outside the window
            }
            double radius = t.Element switch
            {
                Border b => b.CornerRadius.TopLeft,
                Control c => c.CornerRadius.TopLeft,
                _ => (Application.Current.Resources.TryGetValue("ShortwaveCardRadius", out var cr) && cr is CornerRadius rr ? rr.TopLeft : 4),
            };
            foreach (var ring in glow) Place(ring, r, (double)ring.Tag, radius, show);
            Place(tube, r, 0, radius, show);
            Place(core, r, -0.6, radius, show);
        }
    }

    /// <summary>A rounded ring <paramref name="outset"/> px outside the rect (negative: inside), centred on that line.</summary>
    static void Place(Border b, Rect r, double outset, double radius, bool show)
    {
        b.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        if (!show) return;
        double half = b.BorderThickness.Left / 2;
        double x = r.X - outset - half, y = r.Y - outset - half;
        Canvas.SetLeft(b, x);
        Canvas.SetTop(b, y);
        b.Width = Math.Max(0, r.Width + 2 * (outset + half));
        b.Height = Math.Max(0, r.Height + 2 * (outset + half));
        b.CornerRadius = new CornerRadius(Math.Max(0, radius + outset + half));
    }

    public override void Dispose()
    {
        foreach (var (e, h) in sizeHooks) e.SizeChanged -= h;
        foreach (var (e, token) in visibilityHooks) e.UnregisterPropertyChangedCallback(UIElement.VisibilityProperty, token);
        sizeHooks.Clear();
        visibilityHooks.Clear();
        parts.Clear();
    }
}
