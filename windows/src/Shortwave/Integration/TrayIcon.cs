using System.Runtime.InteropServices;

namespace Shortwave.Integration;

/// <summary>A menu item for the notification-area menu. <c>Children</c> makes a submenu.</summary>
public sealed record TrayMenuItem(string Text, Action? Action = null, bool Enabled = true, bool Checked = false, List<TrayMenuItem>? Children = null)
{
    public static readonly TrayMenuItem Separator = new("-");
}

/// <summary>
/// The notification-area icon and its menu, the counterpart of the Mac's menu bar extra. Plain Win32
/// (Shell_NotifyIcon on a hidden message window), since WinUI has no tray icon.
/// </summary>
public sealed class TrayIcon : IDisposable
{
    const int WM_APP_TRAY = 0x8001;
    const int WM_LBUTTONUP = 0x0202, WM_RBUTTONUP = 0x0205, WM_CONTEXTMENU = 0x007B, WM_NULL = 0;
    const int NIN_SELECT = 0x0400, NIN_KEYSELECT = 0x0401;
    const uint NIM_ADD = 0, NIM_MODIFY = 1, NIM_DELETE = 2, NIM_SETVERSION = 4;
    const uint NIF_MESSAGE = 1, NIF_ICON = 2, NIF_TIP = 4, NIF_SHOWTIP = 0x80;

    private readonly WndProc proc;
    private readonly nint hwnd;
    private readonly nint icon;
    private readonly Func<List<TrayMenuItem>> menu;
    private readonly Action onClick;
    private readonly uint taskbarCreated;
    private bool added;

    public TrayIcon(string iconPath, string tooltip, Func<List<TrayMenuItem>> menu, Action onClick)
    {
        this.menu = menu;
        this.onClick = onClick;
        proc = WindowProc;
        var wc = new WNDCLASSEX
        {
            cbSize = (uint)Marshal.SizeOf<WNDCLASSEX>(),
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(proc),
            hInstance = GetModuleHandle(null),
            lpszClassName = "DriftwaveTray" + Environment.ProcessId,
        };
        RegisterClassEx(ref wc);
        hwnd = CreateWindowEx(0, wc.lpszClassName, "Driftwave tray", 0, 0, 0, 0, 0, 0, 0, wc.hInstance, 0);
        taskbarCreated = RegisterWindowMessage("TaskbarCreated");
        int size = GetSystemMetrics(49 /* SM_CXSMICON */);
        icon = LoadImage(0, iconPath, 1 /* IMAGE_ICON */, size, size, 0x10 /* LR_LOADFROMFILE */);
        Add(tooltip);
    }

    void Add(string tooltip)
    {
        var data = Data(tooltip);
        added = Shell_NotifyIcon(NIM_ADD, ref data);
        data.uVersion = 4;
        Shell_NotifyIcon(NIM_SETVERSION, ref data);
    }

    NOTIFYICONDATA Data(string tooltip) => new()
    {
        cbSize = (uint)Marshal.SizeOf<NOTIFYICONDATA>(),
        hWnd = hwnd,
        uID = 1,
        uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP,
        uCallbackMessage = WM_APP_TRAY,
        hIcon = icon,
        szTip = tooltip.Length > 127 ? tooltip[..127] : tooltip,
    };

    private string tip = "";
    public void SetTooltip(string tooltip)
    {
        if (tooltip == tip) return;
        tip = tooltip;
        var data = Data(tooltip);
        Shell_NotifyIcon(NIM_MODIFY, ref data);
    }

    nint WindowProc(nint h, uint msg, nint wParam, nint lParam)
    {
        if (msg == WM_APP_TRAY)
        {
            int ev = (int)(lParam & 0xFFFF);
            if (ev is WM_LBUTTONUP or NIN_SELECT or NIN_KEYSELECT) onClick();
            else if (ev is WM_RBUTTONUP or WM_CONTEXTMENU) ShowMenu();
            return 0;
        }
        if (msg == taskbarCreated && taskbarCreated != 0) Add(tip);   // Explorer restarted
        return DefWindowProc(h, msg, wParam, lParam);
    }

    void ShowMenu()
    {
        var actions = new Dictionary<int, Action>();
        int nextId = 1;
        nint Build(List<TrayMenuItem> items)
        {
            nint m = CreatePopupMenu();
            foreach (var item in items)
            {
                if (item.Text == "-") { AppendMenu(m, 0x800 /* MF_SEPARATOR */, 0, null); continue; }
                uint flags = (item.Enabled ? 0u : 0x3u /* MF_GRAYED|MF_DISABLED */) | (item.Checked ? 0x8u /* MF_CHECKED */ : 0);
                string text = item.Text.Replace("&", "&&");
                if (item.Children is { } children)
                {
                    AppendMenu(m, flags | 0x10 /* MF_POPUP */, (nuint)Build(children), text);
                }
                else
                {
                    int id = nextId++;
                    if (item.Action != null) actions[id] = item.Action;
                    AppendMenu(m, flags, (nuint)id, text);
                }
            }
            return m;
        }
        nint root = Build(menu());
        GetCursorPos(out var pt);
        SetForegroundWindow(hwnd);
        int chosen = TrackPopupMenuEx(root, 0x100 /* TPM_RETURNCMD */ | 0x20 /* TPM_BOTTOMALIGN */ | 0x8 /* TPM_RIGHTALIGN */, pt.X, pt.Y, hwnd, 0);
        PostMessage(hwnd, WM_NULL, 0, 0);
        DestroyMenu(root);
        if (chosen > 0 && actions.TryGetValue(chosen, out var a)) a();
    }

    public void Dispose()
    {
        if (added) { var data = Data(""); Shell_NotifyIcon(NIM_DELETE, ref data); added = false; }
        if (icon != 0) DestroyIcon(icon);
        DestroyWindow(hwnd);
    }

    delegate nint WndProc(nint hwnd, uint msg, nint wParam, nint lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct WNDCLASSEX
    {
        public uint cbSize; public uint style; public nint lpfnWndProc; public int cbClsExtra; public int cbWndExtra;
        public nint hInstance; public nint hIcon; public nint hCursor; public nint hbrBackground;
        public string? lpszMenuName; public string lpszClassName; public nint hIconSm;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct NOTIFYICONDATA
    {
        public uint cbSize; public nint hWnd; public uint uID; public uint uFlags; public uint uCallbackMessage; public nint hIcon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string szTip;
        public uint dwState; public uint dwStateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 256)] public string szInfo;
        public uint uVersion;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 64)] public string szInfoTitle;
        public uint dwInfoFlags; public Guid guidItem; public nint hBalloonIcon;
    }

    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern ushort RegisterClassEx(ref WNDCLASSEX wc);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern nint CreateWindowEx(uint exStyle, string cls, string name, uint style, int x, int y, int w, int h, nint parent, nint menu, nint inst, nint param);
    [DllImport("user32.dll")] static extern nint DefWindowProc(nint h, uint msg, nint w, nint l);
    [DllImport("user32.dll")] static extern bool DestroyWindow(nint h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern uint RegisterWindowMessage(string s);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern nint LoadImage(nint inst, string name, uint type, int cx, int cy, uint load);
    [DllImport("user32.dll")] static extern bool DestroyIcon(nint h);
    [DllImport("user32.dll")] static extern nint CreatePopupMenu();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool AppendMenu(nint menu, uint flags, nuint id, string? text);
    [DllImport("user32.dll")] static extern int TrackPopupMenuEx(nint menu, uint flags, int x, int y, nint hwnd, nint tpm);
    [DllImport("user32.dll")] static extern bool DestroyMenu(nint menu);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(nint h);
    [DllImport("user32.dll")] static extern bool PostMessage(nint h, uint msg, nint w, nint l);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern nint GetModuleHandle(string? name);
    [DllImport("shell32.dll", CharSet = CharSet.Unicode)] static extern bool Shell_NotifyIcon(uint msg, ref NOTIFYICONDATA data);
}
