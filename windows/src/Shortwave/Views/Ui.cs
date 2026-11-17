using System.Diagnostics;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage.Pickers;

namespace Shortwave.Views;

/// <summary>Small shell helpers: open links and files, clipboard, file pickers.</summary>
public static class Ui
{
    public static void Open(string target)
    {
        try { Process.Start(new ProcessStartInfo(target) { UseShellExecute = true }); }
        catch (Exception e) { Diagnostics.Log($"open {target}: {e.Message}"); }
    }

    /// <summary>Shows a file selected in Explorer.</summary>
    public static void Reveal(string path)
    {
        try { Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"") { UseShellExecute = true }); }
        catch (Exception e) { Diagnostics.Log($"reveal {path}: {e.Message}"); }
    }

    public static void Copy(string text)
    {
        var p = new DataPackage();
        p.SetText(text);
        Clipboard.SetContent(p);
    }

    public static async Task<string?> PickImage(nint hwnd)
    {
        var picker = new FileOpenPicker { ViewMode = PickerViewMode.Thumbnail, SuggestedStartLocation = PickerLocationId.PicturesLibrary };
        foreach (var ext in new[] { ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tif", ".tiff", ".ico", ".webp", ".heic" }) picker.FileTypeFilter.Add(ext);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        var file = await picker.PickSingleFileAsync();
        return file?.Path;
    }

    public static async Task<string?> PickFolder(nint hwnd)
    {
        var picker = new FolderPicker { SuggestedStartLocation = PickerLocationId.MusicLibrary };
        picker.FileTypeFilter.Add("*");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, hwnd);
        var folder = await picker.PickSingleFolderAsync();
        return folder?.Path;
    }
}
