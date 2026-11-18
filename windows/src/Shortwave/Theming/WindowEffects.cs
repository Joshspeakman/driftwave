using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// The window-wide effects of a theme, drawn with ordinary XAML shapes (so -screenshot renders show them):
/// "back" effects sit behind the navigation pane and the (translucent) content area, "front" effects lie over
/// everything without taking clicks.
/// </summary>
public sealed class WindowEffects
{
    readonly Grid back, front;
    readonly FrameworkElement pointerSource;
    readonly List<Effect> effects = [];
    Size size;

    public WindowEffects(Grid back, Grid front, FrameworkElement pointerSource)
    {
        this.back = back;
        this.front = front;
        this.pointerSource = pointerSource;
        back.SizeChanged += (_, e) => Resize(e.NewSize);
        // Effects draw past the edges (grid lines, rain); keep them inside the window.
        foreach (var host in new[] { back, front })
            host.SizeChanged += (s, e) => ((Grid)s).Clip = new RectangleGeometry { Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height) };
    }

    public void Apply(Theme theme, bool enabled, double intensity)
    {
        Clear();
        if (!enabled || intensity <= 0 || theme.IsDefault) return;
        foreach (var (key, _) in Theme.EffectKinds)
        {
            if (!theme.Effects.TryGetValue(key, out var o)) continue;
            Effect? e = key switch
            {
                "synthwave" => new Synthwave(o, theme),
                "rain" => new Rain(o),
                "aurora" => new Aurora(o),
                "wood" => new Wood(theme),
                "concrete" => new Concrete(o, theme),
                "grain" => new Grain(o),
                "vignette" => new Vignette(o),
                "scanlines" => new Scanlines(o),
                "spotlight" => new Spotlight(o, pointerSource),
                _ => null,
            };
            if (e is null) continue;
            e.Intensity = intensity;
            foreach (var (element, isFront) in e.Elements())
            {
                element.IsHitTestVisible = false;
                (isFront ? front : back).Children.Add(element);
            }
            effects.Add(e);
            if (size.Width > 0) e.Resize(size.Width, size.Height);
            if (e.Animated) EffectsClock.Add(e); else e.Tick(EffectsClock.StillTime, 0);
        }
        Diagnostics.Log($"effects {string.Join(",", effects.Select(x => x.GetType().Name.ToLowerInvariant()))}");
    }

    public void Clear()
    {
        foreach (var e in effects) { EffectsClock.Remove(e); e.Dispose(); }
        effects.Clear();
        back.Children.Clear();
        front.Children.Clear();
    }

    void Resize(Size s)
    {
        size = s;
        foreach (var e in effects)
        {
            e.Resize(s.Width, s.Height);
            if (!e.Animated || EffectsClock.Frozen) e.Tick(EffectsClock.StillTime, 0);
        }
    }

    // MARK: helpers shared by the effects

    internal static Color Opt(JsonObject o, string key, Color fallback) =>
        o[key] is JsonValue v && v.TryGetValue(out string? s) && Theme.ParseColor(s) is { } c ? c : fallback;
    internal static double Opt(JsonObject o, string key, double fallback) =>
        o[key] is JsonValue v && v.TryGetValue(out double d) ? d : fallback;
    internal static SolidColorBrush Brush(Color c) => new(c);
    internal static Color A(Color c, double alpha) => Theme.WithAlpha(c, alpha);

    /// <summary>
    /// A looping composition animation on an element's Translation: it runs on the compositor, so it costs the UI
    /// thread nothing per frame. Not started when effects are frozen (screenshots, animations off).
    /// </summary>
    internal static void Loop(UIElement e, string property, float from, float to, double seconds)
    {
        if (EffectsClock.Frozen || seconds <= 0) return;
        Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.SetIsTranslationEnabled(e, true);
        var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(e);
        var c = visual.Compositor;
        var anim = c.CreateScalarKeyFrameAnimation();
        anim.InsertKeyFrame(0f, from);
        anim.InsertKeyFrame(1f, to, c.CreateLinearEasingFunction());
        anim.Duration = TimeSpan.FromSeconds(seconds);
        anim.IterationBehavior = Microsoft.UI.Composition.AnimationIterationBehavior.Forever;
        visual.StartAnimation(property, anim);
    }

    /// <summary>A slow composition loop through <paramref name="points"/> (translations) and back to the start.</summary>
    internal static void LoopPath(UIElement e, IList<System.Numerics.Vector3> points, double seconds)
    {
        if (EffectsClock.Frozen || points.Count == 0) return;
        Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.SetIsTranslationEnabled(e, true);
        var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(e);
        var c = visual.Compositor;
        var anim = c.CreateVector3KeyFrameAnimation();
        anim.InsertKeyFrame(0f, System.Numerics.Vector3.Zero);
        for (int i = 0; i < points.Count; i++) anim.InsertKeyFrame((i + 1f) / (points.Count + 1), points[i]);
        anim.InsertKeyFrame(1f, System.Numerics.Vector3.Zero);
        anim.Duration = TimeSpan.FromSeconds(seconds);
        anim.IterationBehavior = Microsoft.UI.Composition.AnimationIterationBehavior.Forever;
        visual.StartAnimation("Translation", anim);
    }

    /// <summary>A grey noise image, premultiplied BGRA; <paramref name="smooth"/> adds soft blotches (concrete).</summary>
    internal static WriteableBitmap Noise(int w, int h, int seed, byte maxAlpha, bool dark, bool smooth = false)
    {
        var rnd = new Random(seed);
        var px = new byte[w * h * 4];
        // Coarse value noise for blotches.
        int gw = w / 48 + 2, gh = h / 48 + 2;
        var grid = new double[gw * gh];
        for (int i = 0; i < grid.Length; i++) grid[i] = rnd.NextDouble();
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                double n = rnd.NextDouble();
                if (smooth)
                {
                    double gx = x / 48.0, gy = y / 48.0;
                    int ix = (int)gx, iy = (int)gy;
                    double fx = gx - ix, fy = gy - iy;
                    double a = grid[iy * gw + ix], b = grid[iy * gw + ix + 1], c = grid[(iy + 1) * gw + ix], d = grid[(iy + 1) * gw + ix + 1];
                    double v = (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy;
                    n = n * 0.45 + v * 0.55;
                }
                byte alpha = (byte)(maxAlpha * n);
                byte shade = dark ? (byte)0 : (byte)255;
                if (smooth) shade = n > 0.5 ? (byte)255 : (byte)0;
                byte pre = (byte)(shade * alpha / 255);
                int i = (y * w + x) * 4;
                px[i] = px[i + 1] = px[i + 2] = pre;
                px[i + 3] = alpha;
            }
        var bmp = new WriteableBitmap(w, h);
        using (var s = bmp.PixelBuffer.AsStream()) s.Write(px, 0, px.Length);
        bmp.Invalidate();
        return bmp;
    }

    // MARK: the effects

    public abstract class Effect : EffectsClock.IAnimated, IDisposable
    {
        public double Intensity { get; set; } = 1;
        public virtual bool Animated => false;
        public abstract IEnumerable<(UIElement Element, bool Front)> Elements();
        public virtual void Resize(double w, double h) { }
        public virtual void Tick(double t, double dt) { }
        public virtual void Dispose() { }
    }

    /// <summary>Retrowave: a striped setting sun over a perspective grid rolling towards you.</summary>
    sealed class Synthwave(JsonObject o, Theme theme) : Effect
    {
        readonly Canvas canvas = new();
        readonly List<Line> rows = [];
        double w, h, horizon;
        readonly Color grid = Opt(o, "grid", Color.FromArgb(255, 255, 46, 151));
        public override bool Animated => true;
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (canvas, false); }

        public override void Resize(double width, double height)
        {
            w = width; h = height;
            if (w < 1 || h < 1) return;
            canvas.Children.Clear();
            rows.Clear();
            canvas.Opacity = Math.Clamp(0.35 + 0.65 * Intensity, 0, 1);
            horizon = h * 0.6;
            var bg = theme.Get("background", Color.FromArgb(255, 18, 6, 43));
            var sky = Opt(o, "sky", Color.FromArgb(255, 59, 15, 92));
            var sunTop = Opt(o, "sunTop", Color.FromArgb(255, 255, 228, 94));
            var sunBottom = Opt(o, "sunBottom", Color.FromArgb(255, 255, 46, 151));
            var skyRect = new Rectangle
            {
                Width = w, Height = horizon,
                Fill = new LinearGradientBrush
                {
                    StartPoint = new Point(0, 0), EndPoint = new Point(0, 1),
                    GradientStops = { new GradientStop { Color = A(bg, 1), Offset = 0 }, new GradientStop { Color = sky, Offset = 0.7 }, new GradientStop { Color = Theme.Mix(sky, sunBottom, 0.45), Offset = 1 } },
                },
            };
            canvas.Children.Add(skyRect);
            // The sun: horizontal slices, with gaps that widen towards the horizon.
            double r = Math.Min(w, h) * 0.2, cx = w * 0.5, cy = horizon - r * 0.3;
            for (double y = cy - r; y < horizon; y += 6)
            {
                double frac = (y - (cy - r)) / (2 * r);
                double gap = frac < 0.42 ? 0 : Math.Min(4.5, (frac - 0.42) * 14);
                double dy = Math.Abs(y + 3 - cy);
                if (dy >= r) continue;
                double half = Math.Sqrt(r * r - dy * dy);
                double bandH = Math.Max(0, Math.Min(6 - gap, horizon - y));
                if (bandH <= 0.5) continue;
                var slice = new Rectangle { Width = 2 * half, Height = bandH, Fill = Brush(Theme.Mix(sunTop, sunBottom, Math.Clamp(frac, 0, 1))) };
                Canvas.SetLeft(slice, cx - half);
                Canvas.SetTop(slice, y);
                canvas.Children.Add(slice);
            }
            var floor = new Rectangle
            {
                Width = w, Height = h - horizon,
                Fill = new LinearGradientBrush
                {
                    StartPoint = new Point(0, 0), EndPoint = new Point(0, 1),
                    GradientStops = { new GradientStop { Color = Theme.Mix(bg, grid, 0.18), Offset = 0 }, new GradientStop { Color = A(bg, 1), Offset = 1 } },
                },
            };
            Canvas.SetTop(floor, horizon);
            canvas.Children.Add(floor);
            for (int i = -14; i <= 14; i++)
            {
                var line = new Line
                {
                    X1 = cx + i * w * 0.012, Y1 = horizon, X2 = cx + i * w * 0.11, Y2 = h,
                    Stroke = Brush(A(grid, 0.55)), StrokeThickness = 1.2,
                };
                canvas.Children.Add(line);
            }
            for (int i = 0; i < 16; i++)
            {
                var line = new Line { X1 = 0, X2 = w, Stroke = Brush(grid), StrokeThickness = 1.4 };
                rows.Add(line);
                canvas.Children.Add(line);
            }
            var glow = new Rectangle { Width = w, Height = 2, Fill = Brush(Theme.Mix(grid, Microsoft.UI.Colors.White, 0.4)) };
            Canvas.SetTop(glow, horizon - 1);
            canvas.Children.Add(glow);
        }

        public override void Tick(double t, double dt)
        {
            if (rows.Count == 0) return;
            double phase = t * 0.45 % 1;
            for (int i = 0; i < rows.Count; i++)
            {
                double z = i + 1 - phase;
                double y = horizon + (h - horizon) / (1 + 0.55 * z) * 1.0;
                var line = rows[i];
                line.Y1 = line.Y2 = y;
                line.Opacity = Math.Clamp((y - horizon) / (h - horizon) * 3, 0, 1);
            }
        }
    }

    /// <summary>
    /// Neon Noir: rain over a hazy amber-and-cyan city glow. The drops are three sheets scrolled by composition
    /// animations (no per-frame work on the UI thread); each sheet holds its drops twice, so the loop is seamless.
    /// </summary>
    sealed class Rain(JsonObject o) : Effect
    {
        readonly Canvas glow = new(), drops = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (glow, false); yield return (drops, false); }   // behind the UI: rain never crosses text or controls

        public override void Resize(double w, double h)
        {
            if (w < 1 || h < 1) return;
            glow.Children.Clear(); drops.Children.Clear();
            var a = Opt(o, "glowA", Color.FromArgb(255, 255, 138, 30));
            var b = Opt(o, "glowB", Color.FromArgb(255, 0, 229, 255));
            var rnd = new Random(7);
            for (int i = 0; i < 7; i++)
            {
                var c = i % 2 == 0 ? a : b;
                double d = 180 + rnd.NextDouble() * 380;
                var e = new Ellipse
                {
                    Width = d, Height = d * 0.7,
                    Fill = new RadialGradientBrush { GradientStops = { new GradientStop { Color = A(c, 0.22 * Intensity), Offset = 0 }, new GradientStop { Color = A(c, 0), Offset = 1 } } },
                };
                Canvas.SetLeft(e, rnd.NextDouble() * w - d / 2);
                Canvas.SetTop(e, h * (0.45 + rnd.NextDouble() * 0.55) - d * 0.35);
                glow.Children.Add(e);
            }
            var color = Opt(o, "color", Color.FromArgb(255, 159, 216, 255));
            int n = (int)Opt(o, "drops", 90);
            double[] speeds = [820, 1080, 1400];
            const double slant = 0.18;
            for (int layer = 0; layer < speeds.Length; layer++)
            {
                var sheet = new Canvas { Width = w + h * slant + 40, Height = 2 * h };
                for (int k = 0; k < n / speeds.Length; k++)
                {
                    double len = 12 + rnd.NextDouble() * 22 + layer * 4;
                    double x = rnd.NextDouble() * sheet.Width, y = rnd.NextDouble() * h;
                    var brush = Brush(A(color, (0.10 + rnd.NextDouble() * 0.22 + layer * 0.05) * Intensity));
                    double thick = layer == 2 ? 1.5 : 1;
                    foreach (double yy in new[] { y, y + h })
                        sheet.Children.Add(new Line { X1 = x, Y1 = yy, X2 = x - len * slant, Y2 = yy + len, Stroke = brush, StrokeThickness = thick });
                }
                Canvas.SetLeft(sheet, 0);
                Canvas.SetTop(sheet, -h);
                drops.Children.Add(sheet);
                double seconds = h / speeds[layer];
                Loop(sheet, "Translation.Y", 0, (float)h, seconds);
                Loop(sheet, "Translation.X", 0, (float)(-h * slant), seconds);   // the streaks lean down-left
            }
        }
    }

    /// <summary>Frosted Glass: big soft colour blobs drifting behind the translucent panes (composition animations).</summary>
    sealed class Aurora(JsonObject o) : Effect
    {
        readonly Canvas canvas = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (canvas, false); }

        public override void Resize(double w, double h)
        {
            canvas.Children.Clear();
            if (w < 1 || h < 1) return;
            var colors = (o["colors"] as JsonArray)?.Select(n => Theme.ParseColor((string?)n)).OfType<Color>().ToList() ?? [];
            if (colors.Count == 0) colors = [Color.FromArgb(255, 125, 211, 252), Color.FromArgb(255, 196, 181, 253), Color.FromArgb(255, 249, 168, 212)];
            double op = Opt(o, "opacity", 0.55) * Intensity;
            for (int i = 0; i < colors.Count; i++)
            {
                var c = colors[i];
                double d = Math.Max(w, h) * 0.75;
                var e = new Ellipse
                {
                    Width = d, Height = d,
                    Fill = new RadialGradientBrush { GradientStops = { new GradientStop { Color = A(c, op), Offset = 0 }, new GradientStop { Color = A(c, 0), Offset = 1 } } },
                };
                int index = i;
                // Start where the still frame has it, then wander through a few points and back.
                Point P(double k) => new(w * (0.5 + 0.38 * Math.Sin(k + index * 2.1)) - d / 2, h * (0.5 + 0.34 * Math.Cos(k * 1.3 + index * 1.7)) - d / 2);
                double k0 = EffectsClock.StillTime * (0.05 + i * 0.02);
                var start = P(k0);
                Canvas.SetLeft(e, start.X);
                Canvas.SetTop(e, start.Y);
                canvas.Children.Add(e);
                var path = Enumerable.Range(1, 4).Select(k => P(k0 + k * 1.4))
                    .Select(p => new System.Numerics.Vector3((float)(p.X - start.X), (float)(p.Y - start.Y), 0)).ToList();
                LoopPath(e, path, 60 + i * 17);
            }
        }
    }

    /// <summary>Analog 70s: walnut veneer, with wavy grain lines.</summary>
    sealed class Wood(Theme theme) : Effect
    {
        readonly Canvas canvas = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (canvas, false); }

        public override void Resize(double w, double h)
        {
            canvas.Children.Clear();
            if (w < 1 || h < 1) return;
            var baseColor = Theme.WithAlpha(theme.Get("background", Color.FromArgb(255, 59, 36, 22)), 1);
            canvas.Children.Add(new Rectangle
            {
                Width = w, Height = h,
                Fill = new LinearGradientBrush
                {
                    StartPoint = new Point(0, 0), EndPoint = new Point(1, 0.3),
                    GradientStops =
                    {
                        new GradientStop { Color = Theme.Mix(baseColor, Microsoft.UI.Colors.Black, 0.25), Offset = 0 },
                        new GradientStop { Color = baseColor, Offset = 0.45 },
                        new GradientStop { Color = Theme.Mix(baseColor, Color.FromArgb(255, 120, 72, 40), 0.25), Offset = 0.7 },
                        new GradientStop { Color = Theme.Mix(baseColor, Microsoft.UI.Colors.Black, 0.3), Offset = 1 },
                    },
                },
            });
            var rnd = new Random(1970);
            int count = (int)(h / 9);
            var dark = Color.FromArgb(255, 30, 16, 8);
            var light = Color.FromArgb(255, 140, 86, 50);
            for (int k = 0; k < count; k++)
            {
                double y0 = k * h / count + rnd.NextDouble() * 6;
                double amp = 2 + rnd.NextDouble() * 7, f1 = 0.004 + rnd.NextDouble() * 0.006, f2 = 0.017 + rnd.NextDouble() * 0.02, ph = rnd.NextDouble() * 6.28;
                var pts = new PointCollection();
                for (double x = -20; x <= w + 40; x += 24)
                    pts.Add(new Point(x, y0 + amp * Math.Sin(x * f1 + ph) + amp * 0.35 * Math.Sin(x * f2 + ph * 2)));
                bool isDark = rnd.NextDouble() < 0.65;
                canvas.Children.Add(new Polyline
                {
                    Points = pts,
                    Stroke = Brush(A(isDark ? dark : light, (isDark ? 0.35 : 0.18) * Intensity)),
                    StrokeThickness = 0.8 + rnd.NextDouble() * (isDark ? 2.6 : 1.2),
                });
            }
        }
    }

    /// <summary>Brutalist: board-marked concrete.</summary>
    sealed class Concrete(JsonObject o, Theme theme) : Effect
    {
        readonly Image image = new() { Stretch = Stretch.UniformToFill };
        readonly Canvas boards = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (image, false); yield return (boards, false); }

        public override void Resize(double w, double h)
        {
            if (image.Source == null) image.Source = Noise(720, 480, 1952, 70, dark: !theme.IsDark, smooth: true);
            image.Opacity = Opt(o, "opacity", 0.5) * Intensity;
            boards.Children.Clear();
            // Formwork lines every 120 px.
            for (double y = 120; y < h; y += 120)
            {
                var r = new Rectangle { Width = w, Height = 1, Fill = Brush(A(Microsoft.UI.Colors.Black, 0.12 * Intensity)) };
                Canvas.SetTop(r, y);
                boards.Children.Add(r);
            }
        }
    }

    /// <summary>
    /// Film grain over everything: a still texture. Never animated: swapping noise frames read as flicker, and
    /// nothing in a theme may flash or strobe (docs/THEMES.md).
    /// </summary>
    sealed class Grain(JsonObject o) : Effect
    {
        readonly Image image = new() { Stretch = Stretch.UniformToFill, Source = Noise(640, 400, 11, 255, dark: false) };
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (image, true); }
        public override void Resize(double w, double h) => image.Opacity = Opt(o, "opacity", 0.06) * Intensity;
    }

    sealed class Vignette(JsonObject o) : Effect
    {
        readonly Rectangle rect = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (rect, true); }

        public override void Resize(double w, double h)
        {
            rect.Width = w; rect.Height = h;
            var c = Opt(o, "color", Microsoft.UI.Colors.Black);
            rect.Fill = new RadialGradientBrush
            {
                RadiusX = 0.75, RadiusY = 0.75,
                GradientStops =
                {
                    new GradientStop { Color = A(c, 0), Offset = 0 },
                    new GradientStop { Color = A(c, 0), Offset = 0.6 },
                    new GradientStop { Color = A(c, Opt(o, "opacity", 0.4) * Intensity), Offset = 1 },
                },
            };
        }
    }

    /// <summary>CRT scanlines, with a faint bright band rolling down (a composition animation).</summary>
    sealed class Scanlines(JsonObject o) : Effect
    {
        readonly Rectangle lines = new(), band = new() { Height = 140 };
        readonly Canvas canvas = new();
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (canvas, true); }

        public override void Resize(double w, double height)
        {
            canvas.Children.Clear();
            double a = Opt(o, "opacity", 0.12) * Intensity;
            lines.Width = w; lines.Height = height;
            lines.Fill = new LinearGradientBrush
            {
                MappingMode = BrushMappingMode.Absolute, StartPoint = new Point(0, 0), EndPoint = new Point(0, 3), SpreadMethod = GradientSpreadMethod.Repeat,
                GradientStops =
                {
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, a), Offset = 0 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, a), Offset = 0.34 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, 0), Offset = 0.34 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, 0), Offset = 1 },
                },
            };
            band.Width = w;
            band.Fill = new LinearGradientBrush
            {
                StartPoint = new Point(0, 0), EndPoint = new Point(0, 1),
                GradientStops =
                {
                    new GradientStop { Color = A(Microsoft.UI.Colors.White, 0), Offset = 0 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.White, 0.035 * Intensity), Offset = 0.5 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.White, 0), Offset = 1 },
                },
            };
            Canvas.SetTop(band, -140);
            canvas.Children.Add(lines);
            canvas.Children.Add(band);
            Loop(band, "Translation.Y", 0, (float)(height + 280), 7);
        }
    }

    /// <summary>Stealth: the dark closes in except where the pointer is.</summary>
    sealed class Spotlight : Effect
    {
        readonly Rectangle rect = new();
        readonly RadialGradientBrush brush;
        readonly FrameworkElement source;
        readonly PointerEventHandler moved;
        Point target, pos;
        bool hasPointer;
        double w, h;
        public override bool Animated => true;
        public override IEnumerable<(UIElement, bool)> Elements() { yield return (rect, true); }

        public Spotlight(JsonObject o, FrameworkElement source)
        {
            this.source = source;
            double op = Opt(o, "opacity", 0.6), radius = Opt(o, "radius", 360);
            brush = new RadialGradientBrush
            {
                MappingMode = BrushMappingMode.Absolute, RadiusX = radius, RadiusY = radius,
                GradientStops =
                {
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, 0), Offset = 0 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, op * 0.45), Offset = 0.6 },
                    new GradientStop { Color = A(Microsoft.UI.Colors.Black, op), Offset = 1 },
                },
            };
            rect.Fill = brush;
            moved = (_, e) => { target = e.GetCurrentPoint(source).Position; hasPointer = true; };
            source.AddHandler(UIElement.PointerMovedEvent, moved, handledEventsToo: true);
        }

        public override void Resize(double width, double height)
        {
            w = width; h = height;
            rect.Width = w; rect.Height = h;
            if (!hasPointer) target = pos = new Point(w * 0.42, h * 0.35);
            rect.Opacity = Intensity;
        }

        public override void Tick(double t, double dt)
        {
            double k = dt <= 0 ? 1 : 1 - Math.Exp(-dt * 10);
            pos = new Point(pos.X + (target.X - pos.X) * k, pos.Y + (target.Y - pos.Y) * k);
            brush.Center = pos;
            brush.GradientOrigin = pos;
        }

        public override void Dispose() => source.RemoveHandler(UIElement.PointerMovedEvent, moved);
    }
}
