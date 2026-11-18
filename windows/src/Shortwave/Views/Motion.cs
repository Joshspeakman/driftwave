using System.Numerics;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;

namespace Shortwave.Views;

/// <summary>
/// The app's motion: items easing in, cards gliding to new places, panes sliding, small presses. All of it is
/// smooth movement and fading (never flashing), on the composition thread where possible, and it follows Windows'
/// "Animation effects" setting. Screenshot test runs get no motion, so renders are never caught mid-animation.
/// </summary>
public static class Motion
{
    static readonly Windows.UI.ViewManagement.UISettings uiSettings = new();
    static ImplicitAnimationCollection? reposition;
    static readonly System.Runtime.CompilerServices.ConditionalWeakTable<FrameworkElement, Storyboard> slides = new();
    static CompositionEasingFunction? ease;

    /// <summary>Fluent's "decelerate" curve: quick start, gentle landing.</summary>
    /// <remarks>A new one each time: a KeySpline can belong to only one key frame.</remarks>
    public static KeySpline DecelerateSpline => new() { ControlPoint1 = new(0.1, 0.9), ControlPoint2 = new(0.2, 1) };
    public static readonly TimeSpan Fast = TimeSpan.FromMilliseconds(180), Normal = TimeSpan.FromMilliseconds(300), Slow = TimeSpan.FromMilliseconds(420);

    public static bool Enabled
    {
        get
        {
            if (Diagnostics.Screenshot != null) return false;
            try { return uiSettings.AnimationsEnabled; } catch { return true; }
        }
    }

    static Visual VisualOf(UIElement e) => ElementCompositionPreview.GetElementVisual(e);

    static CompositionEasingFunction Ease(Compositor c) =>
        ease ??= c.CreateCubicBezierEasingFunction(new Vector2(0.1f, 0.9f), new Vector2(0.2f, 1f));

    /// <summary>Fades and rises an element into place. <paramref name="order"/> staggers a group (capped).</summary>
    public static void Entrance(UIElement e, int order = 0, float rise = 16, float fromScale = 0.97f)
    {
        if (!Enabled) { Reset(e); return; }
        ElementCompositionPreview.SetIsTranslationEnabled(e, true);
        var v = VisualOf(e);
        var c = v.Compositor;
        CenterOnSelf(v);
        var delay = TimeSpan.FromMilliseconds(Math.Min(order, 14) * 22);

        var fade = c.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(0, 0);
        fade.InsertKeyFrame(1, 1, Ease(c));
        fade.Duration = Normal;
        fade.DelayTime = delay;
        fade.DelayBehavior = AnimationDelayBehavior.SetInitialValueBeforeDelay;

        var move = c.CreateVector3KeyFrameAnimation();
        move.InsertKeyFrame(0, new Vector3(0, rise, 0));
        move.InsertKeyFrame(1, Vector3.Zero, Ease(c));
        move.Duration = Slow;
        move.DelayTime = delay;
        move.DelayBehavior = AnimationDelayBehavior.SetInitialValueBeforeDelay;

        var grow = c.CreateVector3KeyFrameAnimation();
        grow.InsertKeyFrame(0, new Vector3(fromScale, fromScale, 1));
        grow.InsertKeyFrame(1, Vector3.One, Ease(c));
        grow.Duration = Slow;
        grow.DelayTime = delay;
        grow.DelayBehavior = AnimationDelayBehavior.SetInitialValueBeforeDelay;

        v.StartAnimation("Opacity", fade);
        v.StartAnimation("Translation", move);
        v.StartAnimation("Scale", grow);
    }

    /// <summary>Puts an element back at rest (a recycled element may have been caught mid-animation).</summary>
    public static void Reset(UIElement e)
    {
        var v = VisualOf(e);
        v.StopAnimation("Opacity");
        v.StopAnimation("Scale");
        v.Opacity = 1;
        v.Scale = Vector3.One;
        ElementCompositionPreview.SetIsTranslationEnabled(e, true);
        v.StopAnimation("Translation");
        v.Properties.InsertVector3("Translation", Vector3.Zero);
    }

    /// <summary>While on, the element glides to wherever layout moves it instead of jumping.</summary>
    public static void Glide(UIElement e, bool on)
    {
        var v = VisualOf(e);
        if (!on || !Enabled) { v.ImplicitAnimations = null; return; }
        var c = v.Compositor;
        if (reposition is null)
        {
            var offset = c.CreateVector3KeyFrameAnimation();
            offset.Target = "Offset";
            offset.InsertExpressionKeyFrame(1, "this.FinalValue", Ease(c));
            offset.Duration = Slow;
            reposition = c.CreateImplicitAnimationCollection();
            reposition["Offset"] = offset;
        }
        v.ImplicitAnimations = reposition;
    }

    /// <summary>A soft lift (or settle) of an element, e.g. a card under the pointer.</summary>
    public static void Lift(UIElement e, bool up, float scale = 1.035f)
    {
        var v = VisualOf(e);
        CenterOnSelf(v);
        var target = up ? new Vector3(scale, scale, 1) : Vector3.One;
        if (!Enabled) { v.Scale = target; return; }
        var c = v.Compositor;
        var a = c.CreateVector3KeyFrameAnimation();
        a.InsertKeyFrame(1, target, Ease(c));
        a.Duration = up ? Normal : Fast;
        v.StartAnimation("Scale", a);
    }

    /// <summary>A small press-and-release bounce, to confirm an action (added to library, saved).</summary>
    public static void Pop(UIElement e, float peak = 1.14f)
    {
        if (!Enabled) return;
        var v = VisualOf(e);
        CenterOnSelf(v);
        var c = v.Compositor;
        var a = c.CreateVector3KeyFrameAnimation();
        a.InsertKeyFrame(0.0f, Vector3.One);
        a.InsertKeyFrame(0.35f, new Vector3(peak, peak, 1), Ease(c));
        a.InsertKeyFrame(1.0f, Vector3.One, Ease(c));
        a.Duration = TimeSpan.FromMilliseconds(420);
        v.StartAnimation("Scale", a);
    }

    /// <summary>A crossfade-in for content that just changed (a new cover, a new title).</summary>
    public static void FadeIn(UIElement e, float fromScale = 0.96f)
    {
        if (!Enabled) return;
        var v = VisualOf(e);
        CenterOnSelf(v);
        var c = v.Compositor;
        var fade = c.CreateScalarKeyFrameAnimation();
        fade.InsertKeyFrame(0, 0.25f);
        fade.InsertKeyFrame(1, 1, Ease(c));
        fade.Duration = Slow;
        var grow = c.CreateVector3KeyFrameAnimation();
        grow.InsertKeyFrame(0, new Vector3(fromScale, fromScale, 1));
        grow.InsertKeyFrame(1, Vector3.One, Ease(c));
        grow.Duration = Slow;
        v.StartAnimation("Opacity", fade);
        v.StartAnimation("Scale", grow);
    }

    /// <summary>Scale around the element's centre, whatever its size.</summary>
    static void CenterOnSelf(Visual v)
    {
        var center = v.Compositor.CreateExpressionAnimation("Vector3(this.Target.Size.X / 2, this.Target.Size.Y / 2, 0)");
        v.StartAnimation("CenterPoint", center);
    }

    /// <summary>
    /// Shows or hides a side pane by sliding its width (and fading it), so the content beside it reflows smoothly.
    /// </summary>
    public static void SlidePane(FrameworkElement pane, bool show, double width)
    {
        if (!Enabled)
        {
            pane.Width = width;
            pane.Opacity = 1;
            pane.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
            return;
        }
        // A new slide replaces one still running (stopping doesn't run its completion).
        if (slides.TryGetValue(pane, out var running)) { running.Pause(); double wNow = pane.ActualWidth, oNow = pane.Opacity; running.Stop(); slides.Remove(pane); pane.Width = wNow; pane.Opacity = oNow; }
        if (show && pane.Visibility == Visibility.Collapsed) { pane.Width = 0; pane.Opacity = 0; }
        pane.Visibility = Visibility.Visible;
        var sb = new Storyboard();
        var w = new DoubleAnimationUsingKeyFrames { EnableDependentAnimation = true };
        w.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = KeyTime.FromTimeSpan(Slow), Value = show ? width : 0, KeySpline = DecelerateSpline });
        Storyboard.SetTarget(w, pane);
        Storyboard.SetTargetProperty(w, "Width");
        var o = new DoubleAnimationUsingKeyFrames();
        o.KeyFrames.Add(new SplineDoubleKeyFrame { KeyTime = KeyTime.FromTimeSpan(show ? Slow : Fast), Value = show ? 1 : 0, KeySpline = DecelerateSpline });
        Storyboard.SetTarget(o, pane);
        Storyboard.SetTargetProperty(o, "Opacity");
        sb.Children.Add(w);
        sb.Children.Add(o);
        slides.AddOrUpdate(pane, sb);
        sb.Completed += (_, _) =>
        {
            slides.Remove(pane);
            if (!show) pane.Visibility = Visibility.Collapsed;
            pane.Width = width;
            pane.Opacity = 1;
            sb.Stop();
        };
        sb.Begin();
    }

    /// <summary>Lets the existing children of a repeater glide while its items change, then turns that off again.</summary>
    public static void GlideDuring(FrameworkElement repeater)
    {
        if (!Enabled) return;
        var children = new List<UIElement>();
        for (int i = 0; i < Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChildrenCount(repeater); i++)
            if (Microsoft.UI.Xaml.Media.VisualTreeHelper.GetChild(repeater, i) is UIElement child) children.Add(child);
        foreach (var child in children) Glide(child, true);
        var timer = repeater.DispatcherQueue.CreateTimer();
        timer.Interval = Slow + TimeSpan.FromMilliseconds(80);
        timer.IsRepeating = false;
        timer.Tick += (_, _) => { foreach (var child in children) Glide(child, false); };
        timer.Start();
    }
}
