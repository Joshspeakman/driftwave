namespace Shortwave.Core.Tests;

public class FlashCheckTests
{
    static FlashCheck.Frame F(double lum, double seconds, double red = 0) => new(lum, red, seconds);

    [Fact]
    public void StrobeIsFlashing()
    {
        // Black and white, 10 frames a second.
        var frames = Enumerable.Range(0, 20).Select(i => F(i % 2 == 0 ? 0.0 : 1.0, 0.1)).ToList();
        Assert.True(FlashCheck.IsFlashing(frames));
    }

    [Fact]
    public void SlowPulseIsFine()
    {
        // Dark to light and back once a second: one flash a second.
        var frames = Enumerable.Range(0, 20).Select(i => F(i % 2 == 0 ? 0.05 : 0.7, 0.5)).ToList();
        Assert.False(FlashCheck.IsFlashing(frames));
    }

    [Fact]
    public void GentleMotionIsFine()
    {
        // Rain-like loop: brightness wobbles by a few percent, fast.
        var frames = Enumerable.Range(0, 30).Select(i => F(0.2 + 0.03 * Math.Sin(i), 0.04)).ToList();
        Assert.False(FlashCheck.IsFlashing(frames));
    }

    [Fact]
    public void BrightOnlyChangesDontCount()
    {
        // Both states brighter than the dark limit (white to light grey): not a flash in WCAG's sense.
        var frames = Enumerable.Range(0, 20).Select(i => F(i % 2 == 0 ? 0.95 : 0.83, 0.05)).ToList();
        Assert.False(FlashCheck.IsFlashing(frames));
    }

    [Fact]
    public void RedFlashIsFlashing()
    {
        var frames = Enumerable.Range(0, 20).Select(i => F(0.2, 0.08, i % 2 == 0 ? 0.0 : 0.9)).ToList();
        Assert.True(FlashCheck.IsFlashing(frames));
    }

    [Fact]
    public void StillImagesAndLuminance()
    {
        Assert.False(FlashCheck.IsFlashing([F(0.5, 1)]));
        Assert.Equal(0, FlashCheck.Luminance(0, 0, 0), 6);
        Assert.Equal(1, FlashCheck.Luminance(255, 255, 255), 6);
        Assert.True(FlashCheck.IsSaturatedRed(240, 10, 10));
        Assert.False(FlashCheck.IsSaturatedRed(240, 200, 200));
    }
}
