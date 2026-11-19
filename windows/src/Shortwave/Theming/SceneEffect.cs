using System.Numerics;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.Json.Nodes;
using Microsoft.UI;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// <c>"scene"</c>: a theme's own decoration, described in its JSON (docs/THEMES.md, "Scenes"). Layers of shapes,
/// bevels, gradients, text, pixel art, lines and paths, placed in the title bar, the navigation pane, the content
/// area, the player or the whole window, repeated or scattered, with slow clock-driven motion. Easter eggs react to
/// typed words, the Konami code, clicks on scene items or idle time: they reveal hidden layers, say something in a
/// bubble, or play a little scene (snake, pong, a card cascade, a walker). Nothing flashes; everything fades or moves.
/// </summary>
public sealed class SceneEffect : WindowEffects.Effect
{
    readonly JsonObject o;
    readonly Theme theme;
    readonly Canvas back = new(), front = new();
    readonly Dictionary<string, List<FrameworkElement>> byId = [];
    readonly List<(Rect Box, string Id)> clickable = [];
    readonly List<Egg> eggs = [];
    readonly List<Runner> runners = [];
    readonly FrameworkElement input;
    double w, h;
    string typed = "";
    int konami;
    double lastInput = Environment.TickCount64 / 1000.0;
    readonly HashSet<Egg> idleFired = [];
    readonly Microsoft.UI.Xaml.Input.KeyEventHandler keyHandler;
    readonly Microsoft.UI.Xaml.Input.PointerEventHandler pointerHandler;
    Microsoft.UI.Dispatching.DispatcherQueueTimer? idleTimer;
    bool ticking;
    static readonly Windows.System.VirtualKey[] KonamiCode =
    [
        Windows.System.VirtualKey.Up, Windows.System.VirtualKey.Up, Windows.System.VirtualKey.Down, Windows.System.VirtualKey.Down,
        Windows.System.VirtualKey.Left, Windows.System.VirtualKey.Right, Windows.System.VirtualKey.Left, Windows.System.VirtualKey.Right,
        Windows.System.VirtualKey.B, Windows.System.VirtualKey.A,
    ];

    /// <summary>The window's layout, for placing items (the navigation pane and player sizes).</summary>
    public static double PaneWidth = 200, TitleHeight = 48, PlayerWidth = 312;
    /// <summary>Where the window's own title (icon and name) ends, and how wide the caption buttons are.</summary>
    public static double TitleEnd = 160, CaptionWidth = 140;
    public static bool PlayerShown = true;
    /// <summary>The user's own background is showing: a theme's still pictures step aside (moving layers like rain stay).</summary>
    public static bool UserBackground;

    sealed record Egg(string Trigger, string? Show, string? Say, string? Run, double Seconds, string? Near);

    public SceneEffect(JsonObject o, Theme theme, FrameworkElement input)
    {
        this.o = o;
        this.theme = theme;
        this.input = input;
        if (o["eggs"] is JsonArray list)
            foreach (var e in list.OfType<JsonObject>())
                eggs.Add(new Egg(Str(e, "trigger") ?? "", Str(e, "show"), Str(e, "say"), Str(e, "run"), Num(e, "seconds", 6), Str(e, "near")));
        keyHandler = OnKey;
        pointerHandler = OnPointer;
        if (eggs.Count > 0)
        {
            input.AddHandler(UIElement.KeyDownEvent, keyHandler, true);
            input.AddHandler(UIElement.PointerPressedEvent, pointerHandler, true);
        }
        // Idle eggs: a cheap check every few seconds (the frame clock runs only while something moves).
        if (eggs.Any(e => e.Trigger.StartsWith("idle:", StringComparison.Ordinal)))
        {
            idleTimer = input.DispatcherQueue.CreateTimer();
            idleTimer.Interval = TimeSpan.FromSeconds(5);
            idleTimer.Tick += (_, _) => CheckIdle();
            idleTimer.Start();
        }
    }

    // Motion runs as clock tracks; the scene itself joins the clock only while an egg is playing.
    public override bool Animated => false;
    public override IEnumerable<(UIElement, bool)> Elements() { yield return (back, false); yield return (front, true); }

    public override void Dispose()
    {
        input.RemoveHandler(UIElement.KeyDownEvent, keyHandler);
        input.RemoveHandler(UIElement.PointerPressedEvent, pointerHandler);
        idleTimer?.Stop();
        if (ticking) { EffectsClock.Remove(this); ticking = false; }
        foreach (var r in runners) r.End();
        runners.Clear();
    }

    // MARK: building

    public override void Resize(double width, double height)
    {
        w = width; h = height;
        if (w < 1 || h < 1) return;
        back.Children.Clear();
        front.Children.Clear();
        byId.Clear();
        clickable.Clear();
        runners.Clear();
        back.Opacity = Math.Clamp(0.4 + 0.6 * Intensity, 0, 1);
        if (o["layers"] is not JsonArray layers) return;
        foreach (var item in layers.OfType<JsonObject>())
        {
            try { Place(item); }
            catch (Exception e) { Diagnostics.Log("scene item: " + e.Message); }
        }
    }

    /// <summary>The rectangle an item's coordinates are relative to.</summary>
    Rect Region(string? name)
    {
        double player = PlayerShown ? PlayerWidth : 0;
        return name switch
        {
            // "titlebar" is only the free stretch between the window's title and its caption buttons, so a theme
            // can never draw over either; "titlebar-full" is the whole strip, for backgrounds.
            "titlebar" => new Rect(TitleEnd + 16, 0, Math.Max(0, w - CaptionWidth - 12 - (TitleEnd + 16)), TitleHeight),
            "titlebar-full" => new Rect(0, 0, w, TitleHeight),
            "pane" => new Rect(0, TitleHeight, PaneWidth, Math.Max(0, h - TitleHeight)),
            "content" => new Rect(PaneWidth, TitleHeight, Math.Max(0, w - PaneWidth - player), Math.Max(0, h - TitleHeight)),
            "player" => new Rect(Math.Max(0, w - player), TitleHeight, player, Math.Max(0, h - TitleHeight)),
            _ => new Rect(0, 0, w, h),
        };
    }

    /// <summary>A coordinate: a number (negative counts from the far edge) or a percentage of the region.</summary>
    static double Coord(JsonNode? n, double size, double fallback = 0)
    {
        if (n is null) return fallback;
        if (n is JsonValue v && v.TryGetValue(out double d)) return d < 0 ? size + d : d;
        if (n is JsonValue s && s.TryGetValue(out string? t) && t.EndsWith('%') && double.TryParse(t[..^1], System.Globalization.CultureInfo.InvariantCulture, out double pct))
            return size * pct / 100;
        return fallback;
    }

    void Place(JsonObject item)
    {
        var region = Region(Str(item, "in"));
        int count = 1;
        double dx = 0, dy = 0;
        if (item["repeat"] is JsonObject rep) { count = (int)Num(rep, "n", 1); dx = Coord(rep["dx"], region.Width); dy = Coord(rep["dy"], region.Height); }
        Random? scatter = null;
        double sw = 0, sh = 0;
        string[]? palette = null;
        if (item["scatter"] is JsonObject sc)
        {
            count = (int)Num(sc, "n", 10);
            scatter = new Random((int)Num(sc, "seed", 7));
            sw = Coord(sc["w"], region.Width, region.Width);
            sh = Coord(sc["h"], region.Height, region.Height);
            palette = (sc["colors"] as JsonArray)?.Select(x => (string?)x ?? "").ToArray();
        }
        count = Math.Clamp(count, 1, 600);
        for (int i = 0; i < count; i++)
        {
            double ox = i * dx, oy = i * dy;
            string? color = null;
            if (scatter != null)
            {
                ox = scatter.NextDouble() * sw;
                oy = scatter.NextDouble() * sh;
                if (palette is { Length: > 0 }) color = palette[scatter.Next(palette.Length)];
                if (scatter.NextDouble() < Num(item, "skip", 0)) continue;
            }
            // Repeated items with "jitter" vary too, the same way every time.
            var e = Build(item, region, color, scatter ?? (item["jitter"] is not null ? new Random(i * 7919 + 13) : null));
            if (e is null) continue;
            double x = region.X + Coord(item["x"], region.Width) + ox, y = region.Y + Coord(item["y"], region.Height) + oy;
            // "anchor": "bottom" — y is where the item's bottom edge sits (skylines standing on a street).
            if (Str(item, "anchor") == "bottom" && !double.IsNaN(e.Height)) y -= e.Height;
            Canvas.SetLeft(e, x);
            Canvas.SetTop(e, y);
            e.Opacity = Num(item, "opacity", 1) * (scatter != null && item["scatter"]?["fade"] is not null ? 0.4 + 0.6 * scatter.NextDouble() : 1);
            if (Num(item, "rotate", 0) is double r && r != 0) e.RenderTransform = new RotateTransform { Angle = r };
            bool isFront = item["front"] is JsonValue fv && fv.TryGetValue(out bool fb) && fb;
            (isFront ? front : back).Children.Add(e);
            if (Str(item, "id") is { } id)
            {
                if (!byId.TryGetValue(id, out var l)) byId[id] = l = [];
                l.Add(e);
                if (item["hidden"] is JsonValue hv && hv.TryGetValue(out bool hidden) && hidden) e.Opacity = 0;
                if (item["clickable"] is JsonValue cv && cv.TryGetValue(out bool c) && c)
                    clickable.Add((new Rect(x, y, Math.Max(16, e.Width is double ew && !double.IsNaN(ew) ? ew : 32), Math.Max(16, e.Height is double eh && !double.IsNaN(eh) ? eh : 32)), id));
            }
            Animate(e, item, i, scatter);
        }
    }

    FrameworkElement? Build(JsonObject item, Rect region, string? scatterColor, Random? rnd)
    {
        string type = Str(item, "type") ?? "rect";
        Brush? Fill(string key = "fill") =>
            key == "fill" && scatterColor != null ? Solid(scatterColor) :
            item[key] is JsonArray stops ? Gradient(stops, Str(item, "angle") ?? "v") :
            Str(item, key) is { } c ? Solid(c) : null;
        double width = Coord(item["w"], region.Width, 10), height = Coord(item["h"], region.Height, 10);
        if (item["jitter"] is JsonObject j && rnd != null)
        {
            width *= 1 + (rnd.NextDouble() - 0.5) * Num(j, "w", 0);
            height *= 1 + (rnd.NextDouble() - 0.5) * Num(j, "h", 0);
        }
        switch (type)
        {
            case "rect":
                return new Rectangle
                {
                    Width = width, Height = height, Fill = Fill(), Stroke = Fill("stroke"), StrokeThickness = Num(item, "strokeWidth", 1),
                    RadiusX = Num(item, "radius", 0), RadiusY = Num(item, "radius", 0),
                };
            case "ellipse":
                return new Ellipse { Width = width, Height = height, Fill = Fill(), Stroke = Fill("stroke"), StrokeThickness = Num(item, "strokeWidth", 1) };
            case "radial":
            {
                var g = new RadialGradientBrush { MappingMode = BrushMappingMode.RelativeToBoundingBox };
                if (item["fill"] is JsonArray rs) foreach (var s in rs.OfType<JsonArray>()) g.GradientStops.Add(new GradientStop { Color = C((string?)s[0]), Offset = (double?)s[1] ?? 0 });
                return new Ellipse { Width = width, Height = height, Fill = g };
            }
            case "bevel":
            {
                // A Windows 3.x/95 or classic Mac 3D edge: light top-left, dark bottom-right (sunken: the reverse).
                bool sunken = Str(item, "style") == "sunken";
                var light = C(Str(item, "light") ?? "#FFFFFF");
                var dark = C(Str(item, "dark") ?? "#808080");
                var g = new Grid { Width = width, Height = height, Background = Fill() };
                double t = Num(item, "edge", 2);
                g.Children.Add(new Border { BorderThickness = new Thickness(t, t, 0, 0), BorderBrush = new SolidColorBrush(sunken ? dark : light) });
                g.Children.Add(new Border { BorderThickness = new Thickness(0, 0, t, t), BorderBrush = new SolidColorBrush(sunken ? light : dark) });
                if (Str(item, "outline") is { } ol) g.Children.Add(new Border { BorderThickness = new Thickness(1), BorderBrush = Solid(ol) });
                return g;
            }
            case "line":
                return new Line
                {
                    X1 = 0, Y1 = 0, X2 = Coord(item["x2"], region.Width), Y2 = Coord(item["y2"], region.Height),
                    Stroke = Fill("stroke") ?? Solid(scatterColor ?? "#FFFFFF"), StrokeThickness = Num(item, "strokeWidth", 1),
                };
            case "path":
            {
                var path = (Microsoft.UI.Xaml.Shapes.Path)Microsoft.UI.Xaml.Markup.XamlReader.Load(
                    $"<Path xmlns='http://schemas.microsoft.com/winfx/2006/xaml/presentation' Data='{System.Security.SecurityElement.Escape(Str(item, "data") ?? "")}' />");
                path.Fill = Fill();
                path.Stroke = Fill("stroke");
                path.StrokeThickness = Num(item, "strokeWidth", 1);
                double sc = Num(item, "scale", 1);
                if (sc != 1) path.RenderTransform = new ScaleTransform { ScaleX = sc, ScaleY = sc };
                return path;
            }
            case "text":
                return Text(item, scatterColor);
            case "image":
                if (UserBackground && item["loopY"] is null && item["loopX"] is null && Str(item, "in") is null or "window") return null;
                return Picture(item, region, width, height);
            case "pixels":
                return Pixels(item);
            default:
                return null;
        }
    }

    FrameworkElement Text(JsonObject item, string? scatterColor)
    {
        string text = Str(item, "text") ?? "";
        if (text.Contains("{time}")) text = text.Replace("{time}", DateTime.Now.ToString("h:mm tt", System.Globalization.CultureInfo.CurrentCulture));
        if (text.Contains("{time24}")) text = text.Replace("{time24}", DateTime.Now.ToString("HH:mm", System.Globalization.CultureInfo.InvariantCulture));
        if (text.Contains("{date}")) text = text.Replace("{date}", DateTime.Now.ToString("ddd d MMM", System.Globalization.CultureInfo.CurrentCulture));
        bool vertical = item["vertical"] is JsonValue vv && vv.TryGetValue(out bool vb) && vb;
        if (vertical) text = string.Join("\n", text.EnumerateRunes().Select(r => r.ToString()));
        TextBlock Make(Color color) => new()
        {
            Text = text,
            FontSize = Num(item, "size", 14),
            FontFamily = Str(item, "font") is { } f ? ThemeManager.Font(f) : null,
            FontWeight = Str(item, "weight") switch { "bold" => Microsoft.UI.Text.FontWeights.Bold, "semibold" => Microsoft.UI.Text.FontWeights.SemiBold, "black" => Microsoft.UI.Text.FontWeights.Black, "light" => Microsoft.UI.Text.FontWeights.Light, _ => Microsoft.UI.Text.FontWeights.Normal },
            FontStyle = Str(item, "style") == "italic" ? Windows.UI.Text.FontStyle.Italic : Windows.UI.Text.FontStyle.Normal,
            Foreground = new SolidColorBrush(color),
            CharacterSpacing = (int)Num(item, "spacing", 0),
            LineHeight = vertical ? Num(item, "size", 14) * 1.05 : 0,
            TextAlignment = vertical ? TextAlignment.Center : TextAlignment.Left,
            IsTextScaleFactorEnabled = false,
        };
        var color = C(scatterColor ?? Str(item, "color") ?? "#FFFFFF");
        if (Str(item, "glow") is not { } glow) return Make(color);
        // Neon: soft copies around the text, then the text itself.
        var g = new Grid();
        var gc = C(glow);
        double radius = Num(item, "glowRadius", 2);
        foreach (var (x, y) in new[] { (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1) })
        {
            var copy = Make(Color.FromArgb((byte)(gc.A * 0.35), gc.R, gc.G, gc.B));
            copy.RenderTransform = new TranslateTransform { X = x * radius, Y = y * radius };
            g.Children.Add(copy);
        }
        g.Children.Add(Make(color));
        return g;
    }

    /// <summary>Pixel art: rows of characters, each mapped to a colour by the palette ("." is clear).</summary>
    FrameworkElement Pixels(JsonObject item)
    {
        var rows = (item["rows"] as JsonArray)?.Select(r => (string?)r ?? "").ToArray() ?? [];
        int pw = rows.Length == 0 ? 1 : rows.Max(r => r.Length), ph = Math.Max(1, rows.Length);
        var pal = new Dictionary<char, Color>();
        if (item["palette"] is JsonObject p) foreach (var (k, v) in p) if (k.Length == 1) pal[k[0]] = C((string?)v);
        var bmp = new WriteableBitmap(pw, ph);
        var px = new byte[pw * ph * 4];
        for (int y = 0; y < rows.Length; y++)
            for (int x = 0; x < rows[y].Length; x++)
                if (pal.TryGetValue(rows[y][x], out var c))
                {
                    int i = (y * pw + x) * 4;
                    double a = c.A / 255.0;
                    px[i] = (byte)(c.B * a); px[i + 1] = (byte)(c.G * a); px[i + 2] = (byte)(c.R * a); px[i + 3] = c.A;
                }
        using (var s = bmp.PixelBuffer.AsStream()) s.Write(px, 0, px.Length);
        bmp.Invalidate();
        double scale = Num(item, "scale", 4);
        return new Image { Source = bmp, Width = pw * scale, Height = ph * scale, Stretch = Stretch.Fill };
    }

    /// <summary>
    /// Rendered art from Assets\ThemeArt: <c>fit</c> "cover" (default) fills the region, cropping as needed. With
    /// <c>loopY</c> or <c>loopX</c> (seconds) the picture is a seamless tile scrolled forever (rain sheets, fog).
    /// </summary>
    FrameworkElement? Picture(JsonObject item, Rect region, double width, double height)
    {
        string? src = Str(item, "src");
        if (src is null) return null;
        string path = System.IO.Path.Combine(AppContext.BaseDirectory, "Assets", "ThemeArt", src);
        if (!System.IO.File.Exists(path)) { Diagnostics.Log("scene image missing: " + src); return null; }
        var uri = new Uri(path);
        double w0 = item["w"] is null ? region.Width : width, h0 = item["h"] is null ? region.Height : height;
        Image Make(double iw, double ih) => new()
        {
            Source = new BitmapImage(uri) { DecodePixelType = DecodePixelType.Logical },
            Width = iw, Height = ih,
            Stretch = Str(item, "fit") == "fill" ? Stretch.Fill : Stretch.UniformToFill,
        };
        double loopY = Num(item, "loopY", 0), loopX = Num(item, "loopX", 0);
        if (loopY <= 0 && loopX <= 0)
        {
            var img = Make(w0, h0);
            return new Grid { Width = w0, Height = h0, Children = { img } };
        }
        // A tile repeated to cover the region plus one tile, then scrolled by one tile per loop.
        double tile = Num(item, "tile", 512);
        int cols = (int)Math.Ceiling(w0 / tile) + (loopX > 0 ? 1 : 0), rows = (int)Math.Ceiling(h0 / tile) + (loopY > 0 ? 1 : 0);
        var sheet = new Canvas { Width = cols * tile, Height = rows * tile };
        for (int r = 0; r < rows; r++)
            for (int c = 0; c < cols; c++)
            {
                var img = Make(tile, tile);
                img.Stretch = Stretch.Fill;
                Canvas.SetLeft(img, c * tile);
                Canvas.SetTop(img, r * tile);
                sheet.Children.Add(img);
            }
        var host = new Grid { Width = w0, Height = h0, Children = { sheet } };
        host.Clip = new RectangleGeometry { Rect = new Rect(0, 0, w0, h0) };
        EffectsClock.AddTrack(sheet, (v, t) =>
        {
            float y = loopY > 0 ? (float)(-tile + tile * (t % loopY / loopY)) : 0;
            float x = loopX > 0 ? (float)(-tile * (t % loopX / loopX)) : 0;
            v.Properties.InsertVector3("Translation", new Vector3(x, y, 0));
        });
        return host;
    }

    Brush Gradient(JsonArray stops, string angle)
    {
        var g = new LinearGradientBrush
        {
            StartPoint = new Point(0, 0),
            EndPoint = angle switch { "h" => new Point(1, 0), "d" => new Point(1, 1), _ => new Point(0, 1) },
        };
        int n = stops.Count;
        for (int i = 0; i < n; i++)
        {
            if (stops[i] is JsonArray pair) g.GradientStops.Add(new GradientStop { Color = C((string?)pair[0]), Offset = (double?)pair[1] ?? 0 });
            else g.GradientStops.Add(new GradientStop { Color = C((string?)stops[i]), Offset = n == 1 ? 0 : i / (double)(n - 1) });
        }
        return g;
    }

    /// <summary>Slow movement: drift (there and back), scroll (a loop), breathe (opacity), sway (rotation), spin.</summary>
    void Animate(FrameworkElement e, JsonObject item, int index, Random? rnd)
    {
        if (item["motion"] is not JsonObject m) return;
        double seconds = Math.Max(2, Num(m, "seconds", 12));
        double offset = Num(m, "offset", 0) * index + (rnd?.NextDouble() ?? 0) * Num(m, "randomOffset", 0);
        switch (Str(m, "kind"))
        {
            case "drift":
            {
                var to = new Vector3((float)Num(m, "x", 0), (float)Num(m, "y", 0), 0);
                EffectsClock.AddTrack(e, (v, t) =>
                {
                    double s = (1 - Math.Cos((t + offset) / seconds * 2 * Math.PI)) / 2;
                    v.Properties.InsertVector3("Translation", to * (float)s);
                });
                break;
            }
            case "scroll":
            {
                float x = (float)Num(m, "x", 0), y = (float)Num(m, "y", 0);
                EffectsClock.AddTrack(e, (v, t) =>
                {
                    float f = (float)((t + offset) % seconds / seconds);
                    v.Properties.InsertVector3("Translation", new Vector3(x * f, y * f, 0));
                });
                break;
            }
            case "breathe":
            {
                // Slow, gentle opacity (never under 4 s a cycle: a glow, not a blink).
                seconds = Math.Max(4, seconds);
                double lo = Num(m, "min", 0.6), hi = Num(m, "max", 1);
                EffectsClock.AddTrack(e, (v, t) => v.Opacity = (float)(lo + (hi - lo) * (1 - Math.Cos((t + offset) / seconds * 2 * Math.PI)) / 2));
                break;
            }
            case "sway":
            {
                double deg = Num(m, "angle", 3);
                EffectsClock.AddTrack(e, (v, t) =>
                {
                    v.CenterPoint = new Vector3((float)(e.ActualWidth / 2), (float)e.ActualHeight, 0);
                    v.RotationAngleInDegrees = (float)(deg * Math.Sin((t + offset) / seconds * 2 * Math.PI));
                });
                break;
            }
            case "spin":
                EffectsClock.AddTrack(e, (v, t) =>
                {
                    v.CenterPoint = new Vector3((float)(e.ActualWidth / 2), (float)(e.ActualHeight / 2), 0);
                    v.RotationAngleInDegrees = (float)((t + offset) / seconds * 360 % 360);
                });
                break;
        }
    }

    // MARK: easter eggs

    void OnKey(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        lastInput = Environment.TickCount64 / 1000.0;
        idleFired.Clear();
        konami = e.Key == KonamiCode[konami] ? konami + 1 : e.Key == KonamiCode[0] ? 1 : 0;
        if (konami == KonamiCode.Length) { konami = 0; Fire("konami", null); }
        // Typed words count only outside text fields (a search for "snake" shouldn't set one off).
        if (e.OriginalSource is TextBox or AutoSuggestBox or PasswordBox) return;
        if (e.Key is >= Windows.System.VirtualKey.A and <= Windows.System.VirtualKey.Z)
        {
            typed = (typed + (char)('a' + (e.Key - Windows.System.VirtualKey.A)))[^Math.Min(typed.Length + 1, 24)..];
            foreach (var egg in eggs)
                if (egg.Trigger.StartsWith("type:", StringComparison.Ordinal) && typed.EndsWith(egg.Trigger[5..], StringComparison.Ordinal))
                {
                    typed = "";
                    Run(egg, null);
                }
        }
    }

    void OnPointer(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        lastInput = Environment.TickCount64 / 1000.0;
        idleFired.Clear();
        var p = e.GetCurrentPoint(input).Position;
        foreach (var (box, id) in clickable)
            if (box.Contains(p)) { Fire("click:" + id, new Point(box.X + box.Width / 2, box.Y)); break; }
    }

    void Fire(string trigger, Point? at)
    {
        foreach (var egg in eggs) if (egg.Trigger == trigger) Run(egg, at);
    }

    void Run(Egg egg, Point? at)
    {
        Diagnostics.Log($"scene egg {egg.Trigger}");
        double now = Environment.TickCount64 / 1000.0;
        if (egg.Show is { } show && byId.TryGetValue(show, out var items))
            foreach (var it in items) runners.Add(new Reveal(it, now, egg.Seconds));
        if (egg.Say is { } say)
        {
            var near = at ?? (egg.Near is { } n && byId.TryGetValue(n, out var ni) && ni.Count > 0
                ? new Point(Canvas.GetLeft(ni[0]) + ni[0].ActualWidth / 2, Canvas.GetTop(ni[0]))
                : new Point(Region("content").Right - 160, Region("content").Bottom - 120));
            runners.Add(new Bubble(front, say, near, now, egg.Seconds, theme));
        }
        if (egg.Run is { } run)
        {
            var area = egg.Near is { } a && byId.TryGetValue(a, out var ai) && ai.Count > 0
                ? new Rect(Canvas.GetLeft(ai[0]), Canvas.GetTop(ai[0]), ai[0].ActualWidth, ai[0].ActualHeight)
                : Region("content");
            Runner? r = run switch
            {
                "snake" => new Snake(front, area, now, egg.Seconds, theme),
                "pong" => new Pong(front, area, now, egg.Seconds, theme),
                "cascade" => new Cascade(front, Region("content"), now, egg.Seconds),
                _ when run.StartsWith("walk:", StringComparison.Ordinal) && byId.TryGetValue(run[5..], out var wk) && wk.Count > 0
                    => new Walk(wk[0], Region("content"), now, egg.Seconds),
                _ => null,
            };
            if (r != null) runners.Add(r);
        }
        StartTicking();
    }

    void CheckIdle()
    {
        double now = Environment.TickCount64 / 1000.0;
        foreach (var egg in eggs)
            if (egg.Trigger.StartsWith("idle:", StringComparison.Ordinal) && double.TryParse(egg.Trigger[5..], System.Globalization.CultureInfo.InvariantCulture, out double idle)
                && now - lastInput > idle && idleFired.Add(egg))
                Run(egg, null);
    }

    void StartTicking()
    {
        if (ticking || runners.Count == 0) return;
        ticking = true;
        EffectsClock.Add(this);
    }

    public override void Tick(double t, double dt)
    {
        double now = Environment.TickCount64 / 1000.0;
        foreach (var r in runners.ToArray())
            if (!r.Step(now)) { r.End(); runners.Remove(r); }
        if (runners.Count == 0 && ticking) { ticking = false; EffectsClock.Remove(this); }
    }

    // MARK: little scenes

    abstract class Runner
    {
        public abstract bool Step(double now);
        public virtual void End() { }
    }

    /// <summary>Fades a hidden item in, holds it, fades it out.</summary>
    sealed class Reveal(FrameworkElement e, double start, double seconds) : Runner
    {
        public override bool Step(double now)
        {
            double t = now - start;
            e.Opacity = Math.Clamp(Math.Min(t / 0.6, (seconds - t) / 0.8), 0, 1);
            return t < seconds;
        }
        public override void End() => e.Opacity = 0;
    }

    /// <summary>A speech bubble that rises in, waits, and fades away.</summary>
    sealed class Bubble : Runner
    {
        readonly Border bubble;
        readonly Canvas host;
        readonly double start, seconds, y;
        public Bubble(Canvas host, string text, Point near, double start, double seconds, Theme theme)
        {
            this.host = host; this.start = start; this.seconds = seconds;
            var bg = theme.Get("dialog") ?? theme.Get("card") ?? Colors.White;
            bubble = new Border
            {
                Background = new SolidColorBrush(Theme.WithAlpha(bg, 0.96)),
                BorderBrush = new SolidColorBrush(theme.Get("accent") ?? Colors.Gray),
                BorderThickness = new Thickness(1.5),
                CornerRadius = new CornerRadius(10),
                Padding = new Thickness(12, 8, 12, 8),
                MaxWidth = 300,
                Opacity = 0,
                Child = new TextBlock { Text = text, TextWrapping = TextWrapping.Wrap, Foreground = new SolidColorBrush(theme.Get("text") ?? Colors.Black), FontSize = 13 },
            };
            host.Children.Add(bubble);
            bubble.Measure(new Size(300, 400));
            double x = Math.Clamp(near.X - bubble.DesiredSize.Width / 2, 8, Math.Max(8, host.ActualWidth - bubble.DesiredSize.Width - 8));
            y = Math.Max(52, near.Y - bubble.DesiredSize.Height - 10);
            Canvas.SetLeft(bubble, x);
            Canvas.SetTop(bubble, y + 8);
        }
        public override bool Step(double now)
        {
            double t = now - start;
            double show = Math.Clamp(Math.Min(t / 0.35, (seconds - t) / 0.6), 0, 1);
            bubble.Opacity = show;
            Canvas.SetTop(bubble, y + 8 * (1 - Math.Min(1, t / 0.35)));
            return t < seconds;
        }
        public override void End() => host.Children.Remove(bubble);
    }

    /// <summary>Snake playing itself on a pixel grid: it steers for the food and grows.</summary>
    sealed class Snake : Runner
    {
        readonly Canvas host, board = new();
        readonly double start, seconds, cell;
        readonly int cols, rows;
        readonly List<(int X, int Y)> body = [];
        (int X, int Y) food;
        readonly Random rnd = new();
        double next;
        readonly Brush ink;
        public Snake(Canvas host, Rect area, double start, double seconds, Theme theme)
        {
            this.host = host; this.start = start; this.seconds = seconds;
            cell = 10;
            cols = Math.Max(8, (int)(Math.Min(area.Width, 360) / cell));
            rows = Math.Max(6, (int)(Math.Min(area.Height, 220) / cell));
            ink = new SolidColorBrush(theme.Get("text") ?? Colors.Black);
            board.Width = cols * cell; board.Height = rows * cell;
            board.Background = new SolidColorBrush(Theme.WithAlpha(theme.Get("surface") ?? Colors.White, 0.9));
            Canvas.SetLeft(board, area.X + (area.Width - board.Width) / 2);
            Canvas.SetTop(board, area.Y + (area.Height - board.Height) / 2);
            host.Children.Add(board);
            for (int i = 0; i < 4; i++) body.Add((cols / 2 - i, rows / 2));
            Food();
            next = start;
        }
        void Food() { do food = (rnd.Next(cols), rnd.Next(rows)); while (body.Contains(food)); }
        public override bool Step(double now)
        {
            if (now - start > seconds) return false;
            board.Opacity = Math.Clamp(Math.Min((now - start) / 0.4, (seconds - (now - start)) / 0.6), 0, 1);
            if (now < next) return true;
            next = now + 0.12;
            var head = body[0];
            // Head for the food; never back onto itself (or wrap round the edges).
            (int dx, int dy)[] dirs = [(Math.Sign(food.X - head.X), 0), (0, Math.Sign(food.Y - head.Y)), (1, 0), (0, 1), (-1, 0), (0, -1)];
            foreach (var (dx, dy) in dirs)
            {
                if (dx == 0 && dy == 0) continue;
                var n = ((head.X + dx + cols) % cols, (head.Y + dy + rows) % rows);
                if (body.Take(body.Count - 1).Contains(n)) continue;
                body.Insert(0, n);
                if (n == food) Food(); else body.RemoveAt(body.Count - 1);
                break;
            }
            board.Children.Clear();
            foreach (var (x, y) in body) board.Children.Add(Cell(x, y, 1));
            board.Children.Add(Cell(food.X, food.Y, 0.6));
            return true;
        }
        Rectangle Cell(int x, int y, double scale)
        {
            var r = new Rectangle { Width = cell * scale - 1, Height = cell * scale - 1, Fill = ink };
            Canvas.SetLeft(r, x * cell + cell * (1 - scale) / 2);
            Canvas.SetTop(r, y * cell + cell * (1 - scale) / 2);
            return r;
        }
        public override void End() => host.Children.Remove(board);
    }

    /// <summary>Two paddles keeping a square ball in play.</summary>
    sealed class Pong : Runner
    {
        readonly Canvas host, board = new();
        readonly Rectangle left, right, ball;
        readonly double start, seconds, bw, bh;
        double x, y, vx = 190, vy = 120, last;
        public Pong(Canvas host, Rect area, double start, double seconds, Theme theme)
        {
            this.host = host; this.start = start; this.seconds = seconds; last = start;
            bw = Math.Min(area.Width - 40, 420); bh = Math.Min(area.Height - 40, 240);
            var ink = new SolidColorBrush(theme.Get("text") ?? Colors.White);
            board.Width = bw; board.Height = bh;
            board.Background = new SolidColorBrush(Color.FromArgb(230, 0, 0, 0));
            Canvas.SetLeft(board, area.X + (area.Width - bw) / 2);
            Canvas.SetTop(board, area.Y + (area.Height - bh) / 2);
            left = new Rectangle { Width = 8, Height = 44, Fill = ink };
            right = new Rectangle { Width = 8, Height = 44, Fill = ink };
            ball = new Rectangle { Width = 8, Height = 8, Fill = ink };
            var net = new Line { X1 = bw / 2, X2 = bw / 2, Y1 = 0, Y2 = bh, Stroke = ink, StrokeThickness = 2, StrokeDashArray = [3, 3], Opacity = 0.5 };
            board.Children.Add(net); board.Children.Add(left); board.Children.Add(right); board.Children.Add(ball);
            host.Children.Add(board);
            x = bw / 2; y = bh / 2;
        }
        public override bool Step(double now)
        {
            if (now - start > seconds) return false;
            board.Opacity = Math.Clamp(Math.Min((now - start) / 0.4, (seconds - (now - start)) / 0.6), 0, 1);
            double dt = Math.Min(0.1, now - last); last = now;
            x += vx * dt; y += vy * dt;
            if (y < 0 || y > bh - 8) { vy = -vy; y = Math.Clamp(y, 0, bh - 8); }
            if (x < 18 || x > bw - 26) { vx = -vx; x = Math.Clamp(x, 18, bw - 26); }
            Canvas.SetLeft(ball, x); Canvas.SetTop(ball, y);
            Canvas.SetLeft(left, 8); Canvas.SetTop(left, Math.Clamp(y - 18, 0, bh - 44));
            Canvas.SetLeft(right, bw - 16); Canvas.SetTop(right, Math.Clamp(y - 18 + Math.Sin(now) * 6, 0, bh - 44));
            return true;
        }
        public override void End() => host.Children.Remove(board);
    }

    /// <summary>Playing cards bouncing off the bottom, each leaving a gentle trail (a solitaire win, in spirit).</summary>
    sealed class Cascade : Runner
    {
        readonly Canvas host, layer = new();
        readonly double start, seconds;
        readonly Rect area;
        readonly List<(Border Card, double X, double Y, double Vx, double Vy)> cards = [];
        readonly Random rnd = new(52);
        double last, trailAt;
        static readonly string[] Suits = ["♠", "♥", "♦", "♣"];
        public Cascade(Canvas host, Rect area, double start, double seconds)
        {
            this.host = host; this.area = area; this.start = start; this.seconds = seconds; last = start;
            host.Children.Add(layer);
        }
        Border Card(string suit) => new()
        {
            Width = 42, Height = 58, CornerRadius = new CornerRadius(4), Background = new SolidColorBrush(Colors.White),
            BorderBrush = new SolidColorBrush(Colors.Black), BorderThickness = new Thickness(1),
            Child = new TextBlock { Text = suit, FontSize = 22, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
                Foreground = new SolidColorBrush(suit is "♥" or "♦" ? Colors.Red : Colors.Black) },
        };
        public override bool Step(double now)
        {
            double t = now - start;
            if (t > seconds) return false;
            layer.Opacity = Math.Clamp((seconds - t) / 0.8, 0, 1);
            double dt = Math.Min(0.1, now - last); last = now;
            if (cards.Count < 4 && t > cards.Count * 1.2)
            {
                var c = Card(Suits[cards.Count % 4]);
                layer.Children.Add(c);
                cards.Add((c, area.Right - 70 - cards.Count * 50, area.Y + 10, -(80 + rnd.NextDouble() * 90), 0));
            }
            bool trail = now - trailAt > 0.08 && layer.Children.Count < 220;
            if (trail) trailAt = now;
            for (int i = 0; i < cards.Count; i++)
            {
                var (c, x, y, vx, vy) = cards[i];
                vy += 900 * dt; x += vx * dt; y += vy * dt;
                if (y > area.Bottom - 58) { y = area.Bottom - 58; vy = -vy * 0.78; }
                if (trail && x > area.X - 40)
                {
                    var copy = Card(((TextBlock)c.Child).Text);
                    Canvas.SetLeft(copy, x); Canvas.SetTop(copy, y);
                    layer.Children.Insert(0, copy);
                }
                Canvas.SetLeft(c, x); Canvas.SetTop(c, y);
                cards[i] = (c, x, y, vx, vy);
            }
            return true;
        }
        public override void End() => host.Children.Remove(layer);
    }

    /// <summary>A hidden item walks (with a little bob) across the bottom of the content area.</summary>
    sealed class Walk(FrameworkElement e, Rect area, double start, double seconds) : Runner
    {
        readonly double baseLeft = Canvas.GetLeft(e), baseTop = Canvas.GetTop(e);
        public override bool Step(double now)
        {
            double t = (now - start) / seconds;
            if (t > 1) return false;
            e.Opacity = Math.Clamp(Math.Min(t * 12, (1 - t) * 12), 0, 1);
            Canvas.SetLeft(e, area.X - 60 + (area.Width + 120) * t);
            Canvas.SetTop(e, area.Bottom - e.ActualHeight - 12 - Math.Abs(Math.Sin(t * seconds * 7)) * 5);
            return true;
        }
        public override void End() { e.Opacity = 0; Canvas.SetLeft(e, baseLeft); Canvas.SetTop(e, baseTop); }
    }

    // MARK: helpers

    static string? Str(JsonObject o, string k) => o[k] is JsonValue v && v.TryGetValue(out string? s) ? s : null;
    static double Num(JsonObject o, string k, double fallback) => o[k] is JsonValue v && v.TryGetValue(out double d) ? d : fallback;
    static Color C(string? s) => Theme.ParseColor(s) ?? Colors.Transparent;
    static SolidColorBrush Solid(string s) => new(C(s));
}
