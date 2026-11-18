using System.Numerics;
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
/// Page title treatments: upper case, a glow or hard shadow (a composition drop shadow masked by the text), and
/// a blinking block cursor. Applied to every TextBlock that uses PageTitleStyle.
/// </summary>
public sealed class TitleDecor
{
    sealed class Attached : EffectsClock.IAnimated
    {
        public required TextBlock Title;
        public required Canvas Host;
        public SpriteVisual? Glow;
        public DropShadow? Shadow;
        public Rectangle? Cursor;
        /// <summary>A hard shadow (blur 0): an offset copy of the title, drawn with plain XAML.</summary>
        public TextBlock? Copy;
        public double Ox, Oy;
        public double BaseOpacity = 1;
        public bool FlickerOn;
        public bool Animated => Cursor != null || FlickerOn;

        public void Tick(double t, double dt)
        {
            if (Cursor != null) Cursor.Opacity = dt == 0 || t % 1.06 < 0.6 ? 1 : 0;
            if (Shadow != null && FlickerOn)
            {
                // A tired neon tube: steady, with rare stutters.
                double v = BaseOpacity;
                if (dt > 0 && (t % 9.3 < 0.25 || t % 13.7 < 0.12)) v *= Random.Shared.NextDouble() < 0.5 ? 0.25 : 0.9;
                Shadow.Opacity = (float)v;
            }
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
            ElementCompositionPreview.SetElementChildVisual(a.Host, null);
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

        var host = new Canvas { IsHitTestVisible = false, HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top };
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
            var compositor = ElementCompositionPreview.GetElementVisual(title).Compositor;
            a.Shadow = compositor.CreateDropShadow();
            a.Shadow.Color = WindowEffects.Opt(o, "color", Microsoft.UI.Colors.White);
            a.Shadow.BlurRadius = (float)WindowEffects.Opt(o, "blur", 12);
            a.Shadow.Offset = new Vector3((float)WindowEffects.Opt(o, "offsetX", 0), (float)WindowEffects.Opt(o, "offsetY", 0), 0);
            a.BaseOpacity = Math.Clamp(WindowEffects.Opt(o, "opacity", 1) * (0.4 + 0.6 * intensity), 0, 1);
            a.Shadow.Opacity = (float)a.BaseOpacity;
            a.FlickerOn = o["flicker"] is System.Text.Json.Nodes.JsonValue f && f.TryGetValue(out bool fl) && fl;
            a.Glow = compositor.CreateSpriteVisual();
            a.Glow.Shadow = a.Shadow;
            ElementCompositionPreview.SetElementChildVisual(host, a.Glow);
        }
        if (cursor)
        {
            var c = WindowEffects.Opt(theme.Options("cursor"), "color", theme.Get("text", Microsoft.UI.Colors.White));
            a.Cursor = new Rectangle { Fill = new SolidColorBrush(c) };
            host.Children.Add(a.Cursor);
        }
        title.SizeChanged += (_, _) => Layout(a);
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
        if (a.Glow != null && a.Shadow != null)
        {
            a.Glow.Size = new Vector2((float)a.Title.ActualWidth, (float)a.Title.ActualHeight);
            a.Glow.Offset = new Vector3((float)at.X, (float)at.Y, 0);
            a.Shadow.Mask = a.Title.GetAlphaMask();
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
