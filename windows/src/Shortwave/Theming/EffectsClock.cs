using Microsoft.UI.Xaml.Media;

namespace Shortwave.Theming;

/// <summary>
/// One frame clock for every theme effect. It runs only while something animated is shown, the main window is
/// visible, effects are on and Windows' animation effects are on. In -screenshot runs it never runs: effects draw
/// one fixed frame so renders are repeatable.
/// </summary>
public static class EffectsClock
{
    public interface IAnimated
    {
        /// <summary><paramref name="t"/>: seconds since the effect started; <paramref name="dt"/>: since the last frame.</summary>
        void Tick(double t, double dt);
    }

    /// <summary>The time effects draw at when the clock doesn't run (screenshots, animations off).</summary>
    public const double StillTime = 2.5;

    static readonly List<IAnimated> subscribers = [];
    static bool windowVisible = true;
    static bool running;
    static DateTime start = DateTime.UtcNow, last = DateTime.UtcNow;
    static readonly Windows.UI.ViewManagement.UISettings ui = new();

    public static bool Frozen => Diagnostics.Screenshot != null || !ui.AnimationsEnabled;

    public static void Add(IAnimated a)
    {
        subscribers.Add(a);
        a.Tick(StillTime, 0);
        Update();
    }

    public static void Remove(IAnimated a)
    {
        subscribers.Remove(a);
        Update();
    }

    public static void SetWindowVisible(bool visible)
    {
        windowVisible = visible;
        Update();
    }

    static void Update()
    {
        bool want = subscribers.Count > 0 && windowVisible && !Frozen;
        if (want == running) return;
        running = want;
        Diagnostics.Log(want ? "effects clock running" : "effects clock stopped");
        if (want)
        {
            last = DateTime.UtcNow;
            CompositionTarget.Rendering += OnFrame;
        }
        else CompositionTarget.Rendering -= OnFrame;
    }

    static void OnFrame(object? sender, object e)
    {
        var now = DateTime.UtcNow;
        // 30 frames a second is plenty for these effects and halves their cost.
        if ((now - last).TotalSeconds < 1 / 31.0) return;
        double dt = Math.Min(0.1, (now - last).TotalSeconds);
        last = now;
        double t = StillTime + (now - start).TotalSeconds;
        foreach (var s in subscribers.ToArray())
        {
            try { s.Tick(t, dt); }
            catch (Exception ex) { Diagnostics.Log("effect: " + ex.Message); subscribers.Remove(s); }
        }
    }
}
