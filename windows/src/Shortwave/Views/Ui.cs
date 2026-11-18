using System.Diagnostics;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Shortwave.Model;
using Windows.ApplicationModel.DataTransfer;
using Microsoft.Windows.Storage.Pickers;

namespace Shortwave.Views;

/// <summary>Small shell helpers: theme brushes, open links and files, clipboard, file pickers.</summary>
public static class Ui
{
    /// <summary>
    /// A theme brush for code that builds UI. <c>Application.Current.Resources[key]</c> gives Windows' theme,
    /// so when App Theme forces light or dark, look the brush up in that theme's dictionary instead.
    /// </summary>
    public static Brush ThemeBrush(string key)
    {
        var theme = App.Current?.Model?.Settings.Appearance ?? AppTheme.System;
        if (theme != AppTheme.System)
        {
            string[] names = theme == AppTheme.Dark ? ["Dark", "Default"] : ["Light"];
            if (Find(Application.Current.Resources, names, key) is Brush b) return b;
        }
        return (Brush)Application.Current.Resources[key];
    }

    static object? Find(ResourceDictionary d, string[] themes, string key)
    {
        foreach (var t in themes)
            if (d.ThemeDictionaries.TryGetValue(t, out var td) && td is ResourceDictionary r && r.TryGetValue(key, out var v)) return v;
        foreach (var m in d.MergedDictionaries)
            if (Find(m, themes, key) is { } v) return v;
        return null;
    }

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

    // File pickers use the Windows App SDK's own pickers (Microsoft.Windows.Storage.Pickers). The older
    // Windows.Storage.Pickers fail with E_FAIL when the app runs elevated; these work elevated and unpackaged.
    static Microsoft.UI.WindowId Id(nint hwnd) => Microsoft.UI.Win32Interop.GetWindowIdFromWindow(hwnd);

    public static async Task<string?> PickImage(nint hwnd)
    {
        var picker = new FileOpenPicker(Id(hwnd)) { ViewMode = PickerViewMode.Thumbnail, SuggestedStartLocation = PickerLocationId.PicturesLibrary };
        foreach (var ext in new[] { ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tif", ".tiff", ".ico", ".webp", ".heic" }) picker.FileTypeFilter.Add(ext);
        var file = await picker.PickSingleFileAsync();
        return file?.Path;
    }

    public static async Task<string?> PickStationFileToOpen(nint hwnd)
    {
        var picker = new FileOpenPicker(Id(hwnd)) { SuggestedStartLocation = PickerLocationId.DocumentsLibrary, CommitButtonText = "Import" };
        picker.FileTypeFilter.Add(".json");
        var file = await picker.PickSingleFileAsync();
        return file?.Path;
    }

    public static async Task<string?> PickStationFileToSave(nint hwnd, string suggestedName)
    {
        var picker = new FileSavePicker(Id(hwnd))
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            SuggestedFileName = Path.GetFileNameWithoutExtension(suggestedName),
            CommitButtonText = "Export",
            DefaultFileExtension = ".json",
        };
        picker.FileTypeChoices.Add("Shortwave station file", [".json"]);
        var file = await picker.PickSaveFileAsync();
        return file?.Path;
    }

    public static async Task<string?> PickFolder(nint hwnd)
    {
        var picker = new FolderPicker(Id(hwnd)) { SuggestedStartLocation = PickerLocationId.MusicLibrary };
        var folder = await picker.PickSingleFolderAsync();
        return folder?.Path;
    }
}
