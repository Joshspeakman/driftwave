using System.Numerics;
using System.Text.Json.Nodes;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Shortwave.Theming;

/// <summary>
/// Rain on the window ("drips"): drops cling to the glass, then run down in fits and starts, slowing as they
/// shed water, and leave a wet trail that dries slowly. Each drop is a little lens of the street (drop-N.png).
///
/// A small simulation moves them; it sets composition properties only (no layout per frame) and runs with the
/// effects clock, so it rests when the window isn't active. Screenshots show it after a minute of rain.
/// Options: "density" (drops per 100,000 px², default 2.4), "opacity", "sprites" (default 6), "trail" (default 1).
/// </summary>
public sealed class GlassDrops(JsonObject o) : WindowEffects.Effect
{
    sealed class Drop
    {
        public double X, Y, R, V, Wait, Slide, TrailTop, TrailAlpha, Alpha, Phase;
        public bool Gone;   // ran off the bottom: its trail dries before it comes back
        public Vector4 Shown = new(float.NaN);   // what the visuals show now (x, y, trail length, trail alpha)
        public Image Sprite = null!, Trail = null!;
        public Visual SpriteVisual = null!, TrailVisual = null!;
    }

    const double TrailTile = 256;
    readonly Canvas canvas = new();
    readonly List<Drop> drops = [];
    readonly Random rnd = new(20261118);
    double w, h;

    public override bool Animated => true;
    public override IEnumerable<(UIElement, bool)> Elements() { yield return (canvas, false); }

    static BitmapImage Art(string name) =>
        new(new Uri(System.IO.Path.Combine(AppContext.BaseDirectory, "Assets", "ThemeArt", name))) { DecodePixelType = DecodePixelType.Logical };

    public override void Resize(double width, double height)
    {
        if (Math.Abs(width - w) < 1 && Math.Abs(height - h) < 1 && drops.Count > 0) return;
        w = width; h = height;
        canvas.Children.Clear();
        drops.Clear();
        canvas.Opacity = WindowEffects.Opt(o, "opacity", 1.0) * Intensity;
        int sprites = (int)WindowEffects.Opt(o, "sprites", 6);
        var art = Enumerable.Range(0, Math.Max(1, sprites)).Select(i => Art($"drop-{i}.png")).ToArray();
        var trail = Art("drop-trail.png");
        int count = Math.Clamp((int)(w * h / 100_000 * WindowEffects.Opt(o, "density", 2.4)), 4, 70);
        for (int i = 0; i < count; i++)
        {
            var d = new Drop();
            d.Trail = new Image { Source = trail, Stretch = Stretch.Fill, Height = TrailTile };
            d.Sprite = new Image { Source = art[i % art.Length], Stretch = Stretch.Fill };
            canvas.Children.Add(d.Trail);
            drops.Add(d);
        }
        // Drops over every trail.
        foreach (var d in drops) canvas.Children.Add(d.Sprite);
        foreach (var d in drops)
        {
            d.SpriteVisual = VisualOf(d.Sprite);
            d.TrailVisual = VisualOf(d.Trail);
            Spawn(d, anywhere: true);
        }
        // A minute of rain before the first frame, so the glass is already wet (and screenshots look lived in).
        for (int i = 0; i < 1500; i++) Step(0.04);
        foreach (var d in drops) Show(d);
    }

    static Visual VisualOf(UIElement e)
    {
        ElementCompositionPreview.SetIsTranslationEnabled(e, true);
        return ElementCompositionPreview.GetElementVisual(e);
    }

    void Spawn(Drop d, bool anywhere)
    {
        // Mostly small drops, the odd big one.
        d.R = 4 + Math.Pow(rnd.NextDouble(), 2.2) * 9;
        d.X = rnd.NextDouble() * w;
        d.Y = anywhere ? rnd.NextDouble() * h : rnd.NextDouble() * h * 0.45;
        d.V = 0;
        d.Slide = 0;
        d.Wait = 0.5 + rnd.NextDouble() * 6;
        d.TrailTop = d.Y;
        d.TrailAlpha = 0;
        d.Alpha = anywhere ? 1 : 0;
        d.Phase = rnd.NextDouble() * 100;
        d.Gone = false;
        d.Shown = new Vector4(float.NaN);
        double size = d.R * 2 + 3;
        d.Sprite.Width = size;
        d.Sprite.Height = size * 1.18;
        d.Trail.Width = Math.Max(3, d.R * 1.1);
    }

    void Step(double dt)
    {
        foreach (var d in drops)
        {
            if (d.Gone)
            {
                d.TrailAlpha -= dt * 0.08;
                if (d.TrailAlpha <= 0) Spawn(d, anywhere: false);
                continue;
            }
            d.Alpha = Math.Min(1, d.Alpha + dt * 0.6);   // new drops gather, never pop in
            if (d.Wait > 0)
            {
                d.Wait -= dt;
                // Clinging: the trail dries; once it's gone, the next run starts a new one.
                d.TrailAlpha = Math.Max(0, d.TrailAlpha - dt * 0.035);
                if (d.TrailAlpha <= 0) d.TrailTop = d.Y;
                continue;
            }
            if (d.Slide <= 0) d.Slide = (25 + rnd.NextDouble() * 240) * (d.R / 8);
            double vmax = 30 + d.R * 16;
            d.V = Math.Min(vmax, d.V + vmax * 2.5 * dt);
            double dy = d.V * dt;
            d.Y += dy;
            d.Slide -= dy;
            d.X += Math.Sin(d.Y / 27 + d.Phase) * 0.18;   // runs wander a little
            d.TrailAlpha = Math.Min(0.9, d.TrailAlpha + dt * 1.5);
            if (d.Slide <= 0)
            {
                // It snags on the glass, a little smaller for the water it left behind.
                d.V = 0;
                d.Wait = 0.3 + rnd.NextDouble() * 4.5;
                d.R = Math.Max(3.5, d.R * 0.96);
            }
            if (d.Y > h + 40) { d.V = 0; d.Gone = true; }
        }
    }

    /// <summary>
    /// Moves the drop's visuals, but only when something visibly changed: a clinging drop costs nothing, and the
    /// window redraws only while drops run (trail fading goes in 1/50 steps).
    /// </summary>
    void Show(Drop d)
    {
        var now = new Vector4((float)Math.Round(d.X, 1), (float)Math.Round(d.Y, 1), (float)Math.Round(d.Alpha, 2), (float)Math.Round(d.TrailAlpha * 50) / 50);
        if (now == d.Shown) return;
        d.Shown = now;
        double sw = d.Sprite.Width, sh = d.Sprite.Height;
        d.SpriteVisual.Properties.InsertVector3("Translation", new Vector3((float)(d.X - sw / 2), (float)(d.Y - sh / 2), 0));
        d.SpriteVisual.Opacity = (float)d.Alpha;
        double length = Math.Max(0, d.Y - d.TrailTop);
        d.TrailVisual.Properties.InsertVector3("Translation", new Vector3((float)(d.X - d.Trail.Width / 2), (float)d.TrailTop, 0));
        d.TrailVisual.Scale = new Vector3(1, (float)(length / TrailTile), 1);
        d.TrailVisual.Opacity = (float)(d.TrailAlpha * WindowEffects.Opt(o, "trail", 1.0));
    }

    public override void Tick(double t, double dt)
    {
        if (dt > 0) Step(dt);
        foreach (var d in drops) Show(d);
    }
}
