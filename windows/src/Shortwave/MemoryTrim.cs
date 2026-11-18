using System.Runtime.InteropServices;

namespace Shortwave;

/// <summary>
/// When the main window is minimised or closed to the notification area, Driftwave gives back the memory it was
/// only using to draw: a few seconds later it runs a full garbage collection and lets Windows page out what isn't
/// in use. Playback keeps running; the window's memory comes back on its own when it's shown again.
/// </summary>
public static class MemoryTrim
{
    static Timer? pending;
    static bool shown = true;

    public static void WindowShown(bool isShown)
    {
        if (isShown == shown) return;
        shown = isShown;
        pending?.Dispose();
        pending = null;
        if (!isShown) pending = new Timer(_ => Trim(), null, TimeSpan.FromSeconds(4), Timeout.InfiniteTimeSpan);
    }

    static void Trim()
    {
        if (shown) return;
        try
        {
            System.Runtime.GCSettings.LargeObjectHeapCompactionMode = System.Runtime.GCLargeObjectHeapCompactionMode.CompactOnce;
            GC.Collect(2, GCCollectionMode.Aggressive, blocking: true, compacting: true);
            GC.WaitForPendingFinalizers();
            using var me = System.Diagnostics.Process.GetCurrentProcess();
            SetProcessWorkingSetSizeEx(me.Handle, -1, -1, 0);
            Diagnostics.Log($"memory trimmed: working set {me.WorkingSet64 / 1024 / 1024} MB before");
        }
        catch (Exception e) { Diagnostics.Log("memory trim: " + e.Message); }
    }

    [DllImport("kernel32.dll")]
    static extern bool SetProcessWorkingSetSizeEx(IntPtr process, nint min, nint max, uint flags);
}
