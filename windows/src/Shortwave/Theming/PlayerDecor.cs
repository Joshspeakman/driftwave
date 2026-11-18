using System.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Shortwave.Audio;
using Windows.Foundation;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// Decorations for the player pane: an LCD track display and spectrum bars (Classic Skin), a backlit tuning
/// dial and VU meters (Analog 70s). The bars and meters are decorative: they move while a station plays, but
/// Media Foundation doesn't hand the app the audio, so they don't follow the sound.
/// </summary>
public abstract class PlayerDecor : EffectsClock.IAnimated, IDisposable
{
    protected readonly Player Player;
    protected readonly Theme Theme;
    public abstract FrameworkElement Root { get; }
    /// <summary>Shown above the cover (true) or below the track (false).</summary>
    public virtual bool Top => false;
    /// <summary>Takes over from the normal song and artist text.</summary>
    public virtual bool ReplacesTrackText => false;

    protected PlayerDecor(Player player, Theme theme)
    {
        Player = player;
        Theme = theme;
        Player.PropertyChanged += OnPlayerChanged;
    }

    void OnPlayerChanged(object? s, PropertyChangedEventArgs e) => PlayerChanged();
    protected virtual void PlayerChanged() { }
    public abstract void Tick(double t, double dt);
    public void Dispose() => Player.PropertyChanged -= OnPlayerChanged;

    public static List<PlayerDecor> For(Theme theme, Player player, bool enabled)
    {
        var list = new List<PlayerDecor>();
        if (!enabled || theme.IsDefault) return list;
        if (theme.Has("dial")) list.Add(new TuningDial(player, theme));
        if (theme.Has("lcd")) list.Add(new Lcd(player, theme));
        if (theme.Has("analyzer")) list.Add(new Analyzer(player, theme));
        if (theme.Has("vu")) list.Add(new VuMeters(player, theme));
        return list;
    }

    protected bool Playing => Player.State == PlayerState.Playing;

    // MARK: LCD

    sealed class Lcd : PlayerDecor
    {
        readonly Border root;
        readonly Canvas strip = new() { Height = 28 };
        readonly TextBlock text, copy, info;
        double width;
        public override FrameworkElement Root => root;
        public override bool ReplacesTrackText => true;

        public Lcd(Player player, Theme theme) : base(player, theme)
        {
            var o = theme.Options("lcd");
            var fg = WindowEffects.Opt(o, "color", Color.FromArgb(255, 0, 224, 0));
            var bg = WindowEffects.Opt(o, "background", Color.FromArgb(255, 5, 8, 5));
            var font = ThemeManager.Font(theme.Fonts.Lcd ?? "Consolas");
            TextBlock Make(double size) => new() { FontFamily = font, FontSize = size, Foreground = new SolidColorBrush(fg), TextWrapping = TextWrapping.NoWrap };
            text = Make(26); copy = Make(26);
            info = Make(17);
            info.Foreground = new SolidColorBrush(Theme.WithAlpha(fg, 0.7));
            strip.Children.Add(text);
            strip.Children.Add(copy);
            var grid = new Grid { RowSpacing = 0 };
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            grid.Children.Add(strip);
            Grid.SetRow(info, 1);
            grid.Children.Add(info);
            root = new Border
            {
                Background = new SolidColorBrush(bg), BorderBrush = new SolidColorBrush(theme.Get("controlStroke", Microsoft.UI.Colors.Gray)),
                BorderThickness = new Thickness(1), CornerRadius = new CornerRadius(2), Padding = new Thickness(10, 4, 10, 6), Child = grid,
            };
            strip.SizeChanged += (_, e) =>
            {
                width = e.NewSize.Width;
                strip.Clip = new RectangleGeometry { Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height) };
            };
            PlayerChanged();
        }

        protected override void PlayerChanged()
        {
            var s = Player.Station;
            string line = Player.State switch
            {
                PlayerState.Loading => "BUFFERING...",
                PlayerState.Failed => "STREAM ERROR",
                PlayerState.Playing when Player.PlayingTrack is { } t => t.Title,
                PlayerState.Playing => s?.Name ?? "",
                _ => s?.Name ?? "",
            };
            line = line.ToUpperInvariant();
            if (text.Text != line + "  ***  ") { text.Text = copy.Text = line + "  ***  "; }
            var parts = new List<string>();
            if (s is { Bitrate: > 0 }) parts.Add($"{s.Bitrate} KBPS");
            if (s is { Codec.Length: > 0 }) parts.Add(s.Codec.ToUpperInvariant());
            parts.Add(Player.State switch { PlayerState.Playing => "PLAY", PlayerState.Loading => "LOAD", PlayerState.Failed => "ERR", _ => "STOP" });
            info.Text = string.Join("   ", parts);
        }

        public override void Tick(double t, double dt)
        {
            text.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
            double tw = text.DesiredSize.Width;
            if (tw <= width + 1 || dt == 0)
            {
                Canvas.SetLeft(text, 0);
                copy.Visibility = Visibility.Collapsed;
                return;
            }
            copy.Visibility = Visibility.Visible;
            double x = -(t * 38 % tw);
            Canvas.SetLeft(text, x);
            Canvas.SetLeft(copy, x + tw);
        }
    }

    // MARK: spectrum bars

    sealed class Analyzer : PlayerDecor
    {
        const int Bars = 19;
        const double H = 54;
        readonly Border root;
        readonly Canvas canvas = new() { Height = H };
        readonly Rectangle[] masks = new Rectangle[Bars], peaks = new Rectangle[Bars];
        readonly double[] level = new double[Bars], target = new double[Bars], peak = new double[Bars];
        double nextTarget;
        double barW = 10;
        public override FrameworkElement Root => root;

        public Analyzer(Player player, Theme theme) : base(player, theme)
        {
            var bg = WindowEffects.Opt(theme.Options("lcd"), "background", Color.FromArgb(255, 5, 8, 5));
            root = new Border
            {
                Background = new SolidColorBrush(bg), BorderBrush = new SolidColorBrush(theme.Get("controlStroke", Microsoft.UI.Colors.Gray)),
                BorderThickness = new Thickness(1), CornerRadius = new CornerRadius(2), Padding = new Thickness(6), Child = canvas,
            };
            canvas.SizeChanged += (_, e) => Build(e.NewSize.Width);
        }

        void Build(double w)
        {
            canvas.Children.Clear();
            if (w < 40) return;
            var bg = ((SolidColorBrush)root.Background).Color;
            barW = (w - (Bars - 1) * 3) / Bars;
            var fill = new LinearGradientBrush
            {
                StartPoint = new Point(0, 1), EndPoint = new Point(0, 0),
                GradientStops =
                {
                    new GradientStop { Color = Color.FromArgb(255, 0, 200, 0), Offset = 0 },
                    new GradientStop { Color = Color.FromArgb(255, 120, 230, 0), Offset = 0.45 },
                    new GradientStop { Color = Color.FromArgb(255, 240, 220, 0), Offset = 0.7 },
                    new GradientStop { Color = Color.FromArgb(255, 255, 60, 20), Offset = 1 },
                },
            };
            for (int i = 0; i < Bars; i++)
            {
                double x = i * (barW + 3);
                var bar = new Rectangle { Width = barW, Height = H, Fill = fill };
                Canvas.SetLeft(bar, x);
                canvas.Children.Add(bar);
                masks[i] = new Rectangle { Width = barW, Fill = new SolidColorBrush(bg) };
                Canvas.SetLeft(masks[i], x);
                canvas.Children.Add(masks[i]);
                peaks[i] = new Rectangle { Width = barW, Height = 2, Fill = new SolidColorBrush(Color.FromArgb(255, 200, 255, 200)) };
                Canvas.SetLeft(peaks[i], x);
                canvas.Children.Add(peaks[i]);
            }
            // Segment gaps across all bars.
            canvas.Children.Add(new Rectangle
            {
                Width = w, Height = H,
                Fill = new LinearGradientBrush
                {
                    MappingMode = BrushMappingMode.Absolute, StartPoint = new Point(0, 0), EndPoint = new Point(0, 3), SpreadMethod = GradientSpreadMethod.Repeat,
                    GradientStops = { new GradientStop { Color = bg, Offset = 0 }, new GradientStop { Color = bg, Offset = 0.34 }, new GradientStop { Color = Theme.WithAlpha(bg, 0), Offset = 0.34 }, new GradientStop { Color = Theme.WithAlpha(bg, 0), Offset = 1 } },
                },
            });
            if (EffectsClock.Frozen) Tick(EffectsClock.StillTime, 0);
        }

        public override void Tick(double t, double dt)
        {
            if (masks[0] is null) return;
            bool still = dt == 0;
            if (still)
            {
                // A fixed, plausible frame for renders.
                for (int i = 0; i < Bars; i++) level[i] = peak[i] = Playing || Player.Station != null ? 0.25 + 0.6 * Math.Abs(Math.Sin(i * 0.9 + 1.3)) * (1 - i / (Bars * 1.6)) : 0;
            }
            else
            {
                if (t > nextTarget)
                {
                    nextTarget = t + 0.07;
                    for (int i = 0; i < Bars; i++)
                        target[i] = Playing ? Math.Clamp((0.3 + Random.Shared.NextDouble() * 0.75) * (1.05 - i / (Bars * 1.5)), 0, 1) : 0;
                }
                for (int i = 0; i < Bars; i++)
                {
                    level[i] += (target[i] - level[i]) * Math.Min(1, dt * (target[i] > level[i] ? 30 : 9));
                    peak[i] = Math.Max(level[i], peak[i] - dt * 0.35);
                }
            }
            for (int i = 0; i < Bars; i++)
            {
                double lit = level[i] * H;
                masks[i].Height = Math.Max(0, H - lit);
                Canvas.SetTop(peaks[i], Math.Clamp(H - peak[i] * H - 2, 0, H - 2));
                peaks[i].Opacity = peak[i] > 0.02 ? 1 : 0;
            }
        }
    }

    // MARK: tuning dial

    sealed class TuningDial : PlayerDecor
    {
        const double H = 58;
        readonly Border root;
        readonly Canvas canvas = new() { Height = H };
        readonly Rectangle needle = new() { Width = 2.5 }, needleGlow = new() { Width = 10 };
        double width, x, velocity, targetX;
        public override FrameworkElement Root => root;
        public override bool Top => true;

        public TuningDial(Player player, Theme theme) : base(player, theme)
        {
            var amber = WindowEffects.Opt(theme.Options("dial"), "color", Color.FromArgb(255, 242, 166, 90));
            root = new Border
            {
                CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(1),
                BorderBrush = new SolidColorBrush(Color.FromArgb(255, 20, 12, 6)),
                Background = new LinearGradientBrush
                {
                    StartPoint = new Point(0, 0), EndPoint = new Point(0, 1),
                    GradientStops = { new GradientStop { Color = Color.FromArgb(255, 14, 10, 7), Offset = 0 }, new GradientStop { Color = Color.FromArgb(255, 30, 21, 13), Offset = 1 } },
                },
                Child = canvas,
            };
            needle.Fill = new SolidColorBrush(Color.FromArgb(255, 255, 90, 40));
            needleGlow.Fill = new LinearGradientBrush
            {
                StartPoint = new Point(0, 0), EndPoint = new Point(1, 0),
                GradientStops = { new GradientStop { Color = Color.FromArgb(0, 255, 90, 40), Offset = 0 }, new GradientStop { Color = Color.FromArgb(110, 255, 90, 40), Offset = 0.5 }, new GradientStop { Color = Color.FromArgb(0, 255, 90, 40), Offset = 1 } },
            };
            canvas.SizeChanged += (_, e) => Build(e.NewSize.Width, amber);
        }

        void Build(double w, Color amber)
        {
            width = w;
            canvas.Children.Clear();
            if (w < 60) return;
            // Backlight.
            canvas.Children.Add(new Rectangle
            {
                Width = w, Height = H,
                Fill = new RadialGradientBrush
                {
                    RadiusX = 0.6, RadiusY = 0.9,
                    GradientStops = { new GradientStop { Color = Theme.WithAlpha(amber, 0.28), Offset = 0 }, new GradientStop { Color = Theme.WithAlpha(amber, 0), Offset = 1 } },
                },
            });
            var cream = Color.FromArgb(255, 243, 227, 195);
            double left = 22, right = w - 22;
            for (int mhz = 88; mhz <= 108; mhz++)
            {
                double px = left + (mhz - 88) / 20.0 * (right - left);
                bool major = mhz % 4 == 0;
                var tick = new Rectangle { Width = 1, Height = major ? 12 : 6, Fill = new SolidColorBrush(Theme.WithAlpha(cream, major ? 0.9 : 0.55)) };
                Canvas.SetLeft(tick, px); Canvas.SetTop(tick, H - 18 - tick.Height);
                canvas.Children.Add(tick);
                if (major)
                {
                    var label = new TextBlock { Text = mhz.ToString(), FontFamily = new FontFamily("Bahnschrift"), FontSize = 11, Foreground = new SolidColorBrush(cream) };
                    label.Measure(new Size(100, 100));
                    Canvas.SetLeft(label, px - label.DesiredSize.Width / 2); Canvas.SetTop(label, H - 17);
                    canvas.Children.Add(label);
                }
            }
            var fm = new TextBlock { Text = "FM", FontFamily = new FontFamily("Bahnschrift"), FontSize = 10, FontWeight = Microsoft.UI.Text.FontWeights.Bold, Foreground = new SolidColorBrush(amber) };
            Canvas.SetLeft(fm, 6); Canvas.SetTop(fm, 4);
            canvas.Children.Add(fm);
            var mhzLabel = new TextBlock { Text = "MHz", FontFamily = new FontFamily("Bahnschrift"), FontSize = 10, Foreground = new SolidColorBrush(Theme.WithAlpha(cream, 0.7)) };
            Canvas.SetLeft(mhzLabel, w - 28); Canvas.SetTop(mhzLabel, 4);
            canvas.Children.Add(mhzLabel);
            needle.Height = H - 10; needleGlow.Height = H - 10;
            Canvas.SetTop(needle, 5); Canvas.SetTop(needleGlow, 5);
            canvas.Children.Add(needleGlow);
            canvas.Children.Add(needle);
            PlayerChanged();
            x = targetX;
            Place();
        }

        protected override void PlayerChanged()
        {
            if (width < 60) return;
            double left = 22, right = width - 22;
            // Each station gets its own spot on the dial (a stable hash: string.GetHashCode changes per run).
            double f = Player.Station is { } st ? StableHash(st.Id) % 10000 / 10000.0 : 0.02;
            targetX = left + f * (right - left);
        }

        static uint StableHash(string s) { uint h = 2166136261; foreach (char c in s) h = (h ^ c) * 16777619; return h; }

        void Place()
        {
            Canvas.SetLeft(needle, x - needle.Width / 2);
            Canvas.SetLeft(needleGlow, x - needleGlow.Width / 2);
        }

        public override void Tick(double t, double dt)
        {
            if (width < 60) return;
            if (dt == 0) { x = targetX; Place(); return; }
            // A slightly springy pointer, like a dial cord.
            double a = (targetX - x) * 40 - velocity * 11;
            velocity += a * dt;
            x += velocity * dt;
            if (Playing) x += Math.Sin(t * 3.1) * 0.03;
            Place();
        }
    }

    // MARK: VU meters

    sealed class VuMeters : PlayerDecor
    {
        readonly Grid root = new() { ColumnSpacing = 10, Height = 82 };
        readonly RotateTransform[] needles = new RotateTransform[2];
        readonly double[] angle = [-48, -48], vel = new double[2], target = [-48, -48];
        double next;
        public override FrameworkElement Root => root;

        public VuMeters(Player player, Theme theme) : base(player, theme)
        {
            root.ColumnDefinitions.Add(new ColumnDefinition());
            root.ColumnDefinitions.Add(new ColumnDefinition());
            for (int i = 0; i < 2; i++)
            {
                var meter = Meter(i == 0 ? "L" : "R", out needles[i]);
                Grid.SetColumn(meter, i);
                root.Children.Add(meter);
            }
        }

        static Border Meter(string channel, out RotateTransform rotate)
        {
            const double W = 124, Hgt = 80, cx = W / 2, cy = Hgt - 8, r = 54;
            var canvas = new Canvas { Width = W, Height = Hgt };
            var ink = Color.FromArgb(255, 40, 28, 18);
            for (int deg = -48; deg <= 48; deg += 8)
            {
                double rad = deg * Math.PI / 180;
                bool red = deg >= 24;
                double inner = r - (deg % 24 == 0 ? 9 : 5);
                canvas.Children.Add(new Line
                {
                    X1 = cx + inner * Math.Sin(rad), Y1 = cy - inner * Math.Cos(rad), X2 = cx + r * Math.Sin(rad), Y2 = cy - r * Math.Cos(rad),
                    Stroke = new SolidColorBrush(red ? Color.FromArgb(255, 200, 40, 30) : ink), StrokeThickness = red ? 2 : 1.3,
                });
            }
            var arc = new Microsoft.UI.Xaml.Shapes.Path
            {
                Stroke = new SolidColorBrush(Color.FromArgb(255, 200, 40, 30)), StrokeThickness = 3,
                Data = new PathGeometry
                {
                    Figures =
                    {
                        new PathFigure
                        {
                            StartPoint = new Point(cx + (r + 3) * Math.Sin(24 * Math.PI / 180), cy - (r + 3) * Math.Cos(24 * Math.PI / 180)),
                            Segments = { new ArcSegment { Point = new Point(cx + (r + 3) * Math.Sin(48 * Math.PI / 180), cy - (r + 3) * Math.Cos(48 * Math.PI / 180)), Size = new Size(r + 3, r + 3), SweepDirection = SweepDirection.Clockwise } },
                        },
                    },
                },
            };
            canvas.Children.Add(arc);
            var label = new TextBlock { Text = "VU", FontFamily = new FontFamily("Bahnschrift"), FontSize = 12, FontWeight = Microsoft.UI.Text.FontWeights.Bold, Foreground = new SolidColorBrush(ink) };
            Canvas.SetLeft(label, cx - 9); Canvas.SetTop(label, cy - 30);
            canvas.Children.Add(label);
            var ch = new TextBlock { Text = channel, FontFamily = new FontFamily("Bahnschrift"), FontSize = 10, Foreground = new SolidColorBrush(Theme.WithAlpha(ink, 0.7)) };
            Canvas.SetLeft(ch, 6); Canvas.SetTop(ch, Hgt - 18);
            canvas.Children.Add(ch);
            rotate = new RotateTransform { CenterX = 0, CenterY = r + 4, Angle = -48 };
            var needle = new Rectangle { Width = 1.6, Height = r + 4, Fill = new SolidColorBrush(Color.FromArgb(255, 20, 14, 10)), RenderTransform = rotate };
            Canvas.SetLeft(needle, cx); Canvas.SetTop(needle, cy - r - 4);
            canvas.Children.Add(needle);
            var pivot = new Ellipse { Width = 9, Height = 9, Fill = new SolidColorBrush(ink) };
            Canvas.SetLeft(pivot, cx - 4.5); Canvas.SetTop(pivot, cy - 4.5);
            canvas.Children.Add(pivot);
            return new Border
            {
                CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(1), BorderBrush = new SolidColorBrush(Color.FromArgb(255, 20, 12, 6)),
                Background = new LinearGradientBrush
                {
                    StartPoint = new Point(0, 0), EndPoint = new Point(0, 1),
                    GradientStops = { new GradientStop { Color = Color.FromArgb(255, 248, 236, 205), Offset = 0 }, new GradientStop { Color = Color.FromArgb(255, 226, 202, 150), Offset = 1 } },
                },
                Child = new Viewbox { Child = canvas, Stretch = Stretch.Uniform },
            };
        }

        public override void Tick(double t, double dt)
        {
            if (dt == 0)
            {
                for (int i = 0; i < 2; i++) needles[i].Angle = Player.Station != null ? (i == 0 ? 2 : -6) : -48;
                return;
            }
            if (t > next)
            {
                next = t + 0.11;
                for (int i = 0; i < 2; i++) target[i] = Playing ? -30 + Random.Shared.NextDouble() * 62 : -48;
            }
            for (int i = 0; i < 2; i++)
            {
                double a = (target[i] - angle[i]) * 60 - vel[i] * 12;
                vel[i] += a * dt;
                angle[i] = Math.Clamp(angle[i] + vel[i] * dt, -50, 50);
                needles[i].Angle = angle[i];
            }
        }
    }
}
