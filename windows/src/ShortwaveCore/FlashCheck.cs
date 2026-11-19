namespace Shortwave.Core;

/// <summary>
/// Decides whether an animation flashes, so a background GIF can be refused before anyone sees it. Follows the
/// spirit of WCAG 2.3.1 (three flashes): a flash is a pair of opposing changes in average brightness, each at least
/// <see cref="MinChange"/> (with the darker state below <see cref="DarkLimit"/>), and more than
/// <see cref="MaxFlashesPerSecond"/> flashes in any one second fails. Brightness is relative luminance, 0 to 1.
/// Saturated red flashes count at a lower threshold, as red is the most provoking colour.
/// </summary>
public static class FlashCheck
{
    public const double MinChange = 0.10;
    public const double DarkLimit = 0.80;
    public const int MaxFlashesPerSecond = 3;

    /// <summary>One frame: its average relative luminance, its share of saturated red (0..1), and how long it shows.</summary>
    public readonly record struct Frame(double Luminance, double Red, double Seconds);

    public static bool IsFlashing(IReadOnlyList<Frame> frames)
    {
        if (frames.Count < 3) return false;
        // Two loops of the animation, so a flash across the loop point counts too.
        var seq = frames.Concat(frames).ToList();
        var changes = new List<(double At, int Direction)>();
        double t = 0;
        for (int i = 1; i < seq.Count; i++)
        {
            t += Math.Max(0.01, seq[i - 1].Seconds);
            double a = seq[i - 1].Luminance, b = seq[i].Luminance;
            double delta = b - a;
            bool bright = Math.Abs(delta) >= MinChange && Math.Min(a, b) < DarkLimit;
            bool red = Math.Abs(seq[i].Red - seq[i - 1].Red) >= 0.2;
            if (bright || red) changes.Add((t, bright ? Math.Sign(delta) : (seq[i].Red > seq[i - 1].Red ? 1 : -1)));
        }
        // A flash is a change followed by the opposite change; count flashes inside every one-second window.
        var flashes = new List<double>();
        for (int i = 1; i < changes.Count; i++)
            if (changes[i].Direction != changes[i - 1].Direction) { flashes.Add(changes[i].At); i++; }
        for (int i = 0; i < flashes.Count; i++)
        {
            int inWindow = flashes.Skip(i).TakeWhile(f => f - flashes[i] < 1.0).Count();
            if (inWindow > MaxFlashesPerSecond) return true;
        }
        return false;
    }

    /// <summary>Relative luminance of an sRGB colour (0..255 channels).</summary>
    public static double Luminance(byte r, byte g, byte b)
    {
        static double Lin(byte c) { double s = c / 255.0; return s <= 0.04045 ? s / 12.92 : Math.Pow((s + 0.055) / 1.055, 2.4); }
        return 0.2126 * Lin(r) + 0.7152 * Lin(g) + 0.0722 * Lin(b);
    }

    /// <summary>Whether a pixel counts as saturated red (WCAG's red-flash definition, simplified).</summary>
    public static bool IsSaturatedRed(byte r, byte g, byte b) => r > 128 && r / (double)(r + g + b + 1) >= 0.8;
}
