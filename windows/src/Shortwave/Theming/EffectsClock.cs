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
    static bool windowVisible = true, windowActive = true;
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

    /// <summary>Effects rest while another window has the focus: Driftwave then costs next to nothing.</summary>
    public static void SetWindowActive(bool active)
    {
        windowActive = active;
        Update();
    }

    /// <summary>
    /// A moving element driven by this clock: <paramref name="apply"/> sets its composition visual's properties
    /// (translation, opacity, scale) for a time in seconds. Setting composition properties costs no layout, and
    /// the window only redraws at the clock's rate (24 a second) instead of the compositor's 60 that a running
    /// composition animation would cause. It stops with the clock (window hidden, inactive or minimised) and
    /// removes itself when the element leaves the window.
    /// </summary>
    public static void AddTrack(Microsoft.UI.Xaml.UIElement element, Action<Microsoft.UI.Composition.Visual, double> apply)
    {
        Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.SetIsTranslationEnabled(element, true);
        var visual = Microsoft.UI.Xaml.Hosting.ElementCompositionPreview.GetElementVisual(element);
        var track = new Track(visual, apply);
        if (Frozen) { apply(visual, StillTime); return; }
        Add(track);
        if (element is Microsoft.UI.Xaml.FrameworkElement fe) fe.Unloaded += (_, _) => Remove(track);
    }

    sealed class Track(Microsoft.UI.Composition.Visual visual, Action<Microsoft.UI.Composition.Visual, double> apply) : IAnimated
    {
        public void Tick(double t, double dt) => apply(visual, t);
    }

    /// <summary>Sets one axis of a visual's Translation (the others keep their values).</summary>
    public static void SetTranslation(Microsoft.UI.Composition.Visual v, float? x = null, float? y = null)
    {
        v.Properties.TryGetVector3("Translation", out var cur);
        v.Properties.InsertVector3("Translation", new System.Numerics.Vector3(x ?? cur.X, y ?? cur.Y, cur.Z));
    }

    static bool Moving => windowVisible && windowActive && !Frozen;

    static void Update()
    {
        bool want = subscribers.Count > 0 && Moving;
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
        // 24 frames a second is plenty for slow ambient effects, and the window redraws only that often.
        if ((now - last).TotalSeconds < 1 / 25.0) return;
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
