using System.Runtime.CompilerServices;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// Page title treatments: upper case, a neon glow or a hard shadow, and a block cursor. Applied to every
/// TextBlock that uses PageTitleStyle. Glow and shadow are plain XAML (copies of the text in the glow colour and a
/// soft halo), so they look the same on screen and in -screenshot renders. A composition drop shadow masked by the
/// text (GetAlphaMask) came out as a small blob under the title, on screen too.
/// </summary>
public sealed class TitleDecor
{
    sealed class Attached : EffectsClock.IAnimated
    {
        public required TextBlock Title;
        public required Canvas Host;
        /// <summary>Neon glow: a halo and a ring of faint copies of the text.</summary>
        public Canvas? Glow;
        public Ellipse? Halo;
        public readonly List<(TextBlock Text, double Dx, double Dy)> Ring = [];
        public Rectangle? Cursor;
        /// <summary>A hard shadow (blur 0): an offset copy of the title, drawn with plain XAML.</summary>
        public TextBlock? Copy;
        public double Ox, Oy;
        public double BaseOpacity = 1;
        /// <summary>Nothing here animates: no flicker and no blinking (no theme may flash or strobe).</summary>
        public bool Animated => false;

        public void Tick(double t, double dt)
        {
        }
    }

    readonly List<Attached> attached = [];
    /// <summary>Marks the hard-shadow copies, which share the title style but aren't titles.</summary>
    static readonly object ShadowTag = new();
    static readonly ConditionalWeakTable<TextBlock, string> originalText = new();
    Theme theme = ThemeManager.DefaultTheme();
    bool enabled;
    double intensity = 1;

    public void Apply(Theme t, bool effectsOn, double effectIntensity, IEnumerable<TextBlock> titles)
    {
        theme = t;
        enabled = effectsOn && effectIntensity > 0;
        intensity = effectIntensity;
        Clear();
        foreach (var title in titles.ToList()) Attach(title);
    }

    public void Clear()
    {
        foreach (var a in attached)
        {
            EffectsClock.Remove(a);
            (a.Host.Parent as Panel)?.Children.Remove(a.Host);
        }
        attached.Clear();
    }

    /// <summary>Titles in a page (call when a page appears).</summary>
    public static IEnumerable<TextBlock> FindTitles(DependencyObject root)
    {
        var style = Application.Current.Resources["PageTitleStyle"];
        var stack = new Stack<DependencyObject>();
        stack.Push(root);
        while (stack.Count > 0)
        {
            var d = stack.Pop();
            if (d is TextBlock tb && ReferenceEquals(tb.Style, style) && !ReferenceEquals(tb.Tag, ShadowTag)) yield return tb;
            int n = VisualTreeHelper.GetChildrenCount(d);
            for (int i = 0; i < n; i++) stack.Push(VisualTreeHelper.GetChild(d, i));
        }
    }

    public void Attach(TextBlock title)
    {
        if (attached.Any(a => a.Title == title)) return;
        // Upper case is part of the theme's typography, so it applies even with effects off.
        if (!originalText.TryGetValue(title, out var text)) { text = title.Text; originalText.Add(title, text); }
        title.Text = theme.Fonts.TitleUppercase ? text.ToUpperInvariant() : text;
        if (!enabled || title.Parent is not Panel panel) return;
        bool glow = theme.Has("titleShadow"), cursor = theme.Has("cursor");
        if (!glow && !cursor) return;

        // The host covers the title's cell. Positions are measured again once it has been laid out: measured before,
        // they put every glow, shadow and cursor 24 px right and 12 px down.
        var host = new Canvas { IsHitTestVisible = false, HorizontalAlignment = HorizontalAlignment.Stretch, VerticalAlignment = VerticalAlignment.Stretch };
        Grid.SetRow(host, Grid.GetRow(title));
        Grid.SetColumn(host, Grid.GetColumn(title));
        panel.Children.Insert(0, host);   // behind the text
        var a = new Attached { Title = title, Host = host };
        var shadowOptions = theme.Options("titleShadow");
        if (glow && WindowEffects.Opt(shadowOptions, "blur", 12) <= 0)
        {
            // A hard shadow is exact as a copy of the text, and shows in -screenshot renders too.
            a.Ox = WindowEffects.Opt(shadowOptions, "offsetX", 3);
            a.Oy = WindowEffects.Opt(shadowOptions, "offsetY", 3);
            a.Copy = new TextBlock
            {
                Style = title.Style, Tag = ShadowTag, Text = title.Text, TextWrapping = TextWrapping.NoWrap,
                Foreground = new SolidColorBrush(WindowEffects.Opt(shadowOptions, "color", Microsoft.UI.Colors.Black)),
                Opacity = Math.Clamp(WindowEffects.Opt(shadowOptions, "opacity", 1) * (0.4 + 0.6 * intensity), 0, 1),
            };
            host.Children.Add(a.Copy);
            glow = false;
        }
        if (glow)
        {
            var o = shadowOptions;
            var color = WindowEffects.Opt(o, "color", Microsoft.UI.Colors.White);
            double blur = WindowEffects.Opt(o, "blur", 12);
            a.Ox = WindowEffects.Opt(o, "offsetX", 0);
            a.Oy = WindowEffects.Opt(o, "offsetY", 0);
            a.BaseOpacity = Math.Clamp(WindowEffects.Opt(o, "opacity", 1) * (0.4 + 0.6 * intensity), 0, 1);
            a.Glow = new Canvas { Opacity = a.BaseOpacity };
            // A soft halo behind the word...
            a.Halo = new Ellipse
            {
                Fill = new RadialGradientBrush
                {
                    GradientStops =
                    {
                        new GradientStop { Color = WindowEffects.A(color, 0.42), Offset = 0 },
                        new GradientStop { Color = WindowEffects.A(color, 0.16), Offset = 0.55 },
                        new GradientStop { Color = WindowEffects.A(color, 0), Offset = 1 },
                    },
                },
            };
            a.Glow.Children.Add(a.Halo);
            // ...and the letters themselves bleeding light: faint copies in two rings around them.
            foreach (var (radius, alpha) in new[] { (blur * 0.32, 0.30), (blur * 0.16, 0.45) })
                for (int k = 0; k < 12; k++)
                {
                    double angle = k * Math.PI / 6;
                    var copy = new TextBlock
                    {
                        Style = title.Style, Tag = ShadowTag, Text = title.Text, TextWrapping = TextWrapping.NoWrap,
                        Foreground = new SolidColorBrush(color), Opacity = alpha,
                    };
                    a.Glow.Children.Add(copy);
                    a.Ring.Add((copy, radius * Math.Cos(angle), radius * Math.Sin(angle)));
                }
            host.Children.Add(a.Glow);
        }
        if (cursor)
        {
            var c = WindowEffects.Opt(theme.Options("cursor"), "color", theme.Get("text", Microsoft.UI.Colors.White));
            a.Cursor = new Rectangle { Fill = new SolidColorBrush(c) };
            host.Children.Add(a.Cursor);
        }
        title.SizeChanged += (_, _) => Layout(a);
        host.SizeChanged += (_, _) => Layout(a);   // the first measurement comes before the host is laid out
        title.Loaded += (_, _) => Layout(a);
        Layout(a);
        attached.Add(a);
        if (a.Animated) EffectsClock.Add(a); else a.Tick(EffectsClock.StillTime, 0);
    }

    static void Layout(Attached a)
    {
        if (a.Title.ActualWidth < 1 || a.Host.XamlRoot is null) return;
        Windows.Foundation.Point at;
        try { at = a.Title.TransformToVisual(a.Host).TransformPoint(new Windows.Foundation.Point(0, 0)); } catch { return; }
        if (a.Glow != null)
        {
            double w = a.Title.ActualWidth, h = a.Title.ActualHeight;
            if (a.Halo != null)
            {
                a.Halo.Width = w * 1.25; a.Halo.Height = h * 2.4;
                Canvas.SetLeft(a.Halo, at.X + a.Ox + (w - a.Halo.Width) / 2);
                Canvas.SetTop(a.Halo, at.Y + a.Oy + (h - a.Halo.Height) / 2);
            }
            foreach (var (text, dx, dy) in a.Ring)
            {
                text.Text = a.Title.Text;
                Canvas.SetLeft(text, at.X + a.Ox + dx);
                Canvas.SetTop(text, at.Y + a.Oy + dy);
            }
        }
        if (a.Copy != null)
        {
            a.Copy.Text = a.Title.Text;
            Canvas.SetLeft(a.Copy, at.X + a.Ox);
            Canvas.SetTop(a.Copy, at.Y + a.Oy);
        }
        if (a.Cursor != null)
        {
            double size = a.Title.FontSize;
            a.Cursor.Width = Math.Round(size * 0.62);
            a.Cursor.Height = Math.Round(size * 0.78);
            Canvas.SetLeft(a.Cursor, at.X + a.Title.ActualWidth + size * 0.35);
            Canvas.SetTop(a.Cursor, at.Y + (a.Title.ActualHeight - a.Cursor.Height) / 2);
        }
    }
}
