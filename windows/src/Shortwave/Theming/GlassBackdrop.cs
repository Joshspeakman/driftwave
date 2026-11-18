using Microsoft.UI.Composition;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// See-through frosted glass for themes with <c>"backdrop": "acrylic"</c>: Windows' thin acrylic (the desktop
/// behind the window shows through, blurred) with a light tint of the theme's background, instead of the standard
/// acrylic's heavy tint. The Default theme keeps Windows' own acrylic (the mini player).
/// </summary>
public sealed class GlassBackdrop(Color tint, float tintOpacity, float luminosityOpacity) : SystemBackdrop
{
    DesktopAcrylicController? controller;

    public bool Matches(Color t, float to, float lo) => t == tint && to == tintOpacity && lo == luminosityOpacity;

    protected override void OnTargetConnected(ICompositionSupportsSystemBackdrop connectedTarget, XamlRoot xamlRoot)
    {
        base.OnTargetConnected(connectedTarget, xamlRoot);
        controller = new DesktopAcrylicController
        {
            Kind = DesktopAcrylicKind.Thin,
            TintColor = tint,
            TintOpacity = tintOpacity,
            LuminosityOpacity = luminosityOpacity,
            FallbackColor = tint,
        };
        controller.SetSystemBackdropConfiguration(GetDefaultSystemBackdropConfiguration(connectedTarget, xamlRoot));
        controller.AddSystemBackdropTarget(connectedTarget);
    }

    protected override void OnTargetDisconnected(ICompositionSupportsSystemBackdrop disconnectedTarget)
    {
        base.OnTargetDisconnected(disconnectedTarget);
        controller?.RemoveSystemBackdropTarget(disconnectedTarget);
        controller?.Dispose();
        controller = null;
    }
}
