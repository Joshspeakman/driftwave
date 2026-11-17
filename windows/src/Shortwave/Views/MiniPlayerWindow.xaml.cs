using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Shortwave.Integration;
using Shortwave.Model;

namespace Shortwave.Views;

/// <summary>A small always-on-top player window (the Mac's floating Mini Player).</summary>
public sealed partial class MiniPlayerWindow : Window
{
    public nint Hwnd { get; }

    public MiniPlayerWindow(AppModel model)
    {
        InitializeComponent();
        Hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        Title = "Mini Player";
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(DragArea);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Shortwave.ico"));
        if (AppWindow.Presenter is OverlappedPresenter p)
        {
            p.IsAlwaysOnTop = true;
            p.IsResizable = false;
            p.IsMaximizable = false;
            p.IsMinimizable = true;
        }
        double scale = Native.GetDpiForWindow(Hwnd) / 96.0;
        int w = (int)(400 * scale), h = (int)(104 * scale);
        // Top right of the primary work area, like the Mac's .topTrailing.
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        AppWindow.MoveAndResize(new Windows.Graphics.RectInt32(area.X + area.Width - w - (int)(24 * scale), area.Y + (int)(24 * scale), w, h));
        // Closing hides it so it opens again where it was.
        AppWindow.Closing += (s, e) =>
        {
            if (closingForReal) return;
            e.Cancel = true;
            s.Hide();
        };
    }

    private bool closingForReal;
    public new void Close() { closingForReal = true; base.Close(); }
}
