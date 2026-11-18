using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Shortwave.Core;
using Shortwave.Integration;
using Shortwave.Model;
using Windows.System;

using Page = Shortwave.Model.Page;

namespace Shortwave.Views;

public sealed partial class MainWindow : Window
{
    private readonly AppModel app;
    private readonly Dictionary<Page, UIElement> pages = [];
    private readonly Queue<Func<ContentDialog>> dialogs = new();
    private bool dialogOpen;
    private ContentDialog? currentDialog;
    private bool syncingNav;
    public nint Hwnd { get; }

    public MainWindow(AppModel app)
    {
        this.app = app;
        InitializeComponent();
        Hwnd = WinRT.Interop.WindowNative.GetWindowHandle(this);
        Title = Diagnostics.AppName;
        TitleText.Text = Diagnostics.AppName;
        string iconPng = Path.Combine(AppContext.BaseDirectory, "Assets", "Shortwave.png");
        TitleIcon.Source = new BitmapImage(new Uri(iconPng));
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Shortwave.ico"));

        double scale = Native.GetDpiForWindow(Hwnd) / 96.0;
        AppWindow.Resize(new Windows.Graphics.SizeInt32((int)(1180 * scale), (int)(760 * scale)));
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter p)
        {
            p.PreferredMinimumWidth = (int)(760 * scale);
            p.PreferredMinimumHeight = (int)(480 * scale);
        }
        AppWindow.Closing += (s, e) =>
        {
            if (App.Current?.MainWindowClosing() == true) return;
            e.Cancel = true;
            s.Hide();
        };

        app.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(AppModel.Page)) ShowPage();
            if (e.PropertyName is nameof(AppModel.ShowPlayer)) UpdatePlayerPane();
        };
        app.Player.PropertyChanged += (_, e) => { if (e.PropertyName is nameof(Audio.Player.Station) or "") UpdatePlayerPane(); };
        app.ShowStationRequested += s => Enqueue(() => new StationDetailDialog(app, s, () => Hwnd));
        app.ShowTrackRequested += t => Enqueue(() => new TrackDetailDialog(app, t));
        app.EditStationRequested += s => Enqueue(() => new LocalStationDialog(app, s, () => Hwnd));
        app.NoticeRequested += (title, message) => Enqueue(() => new ContentDialog
        {
            Title = title,
            Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap },
            CloseButtonText = "OK",
            DefaultButton = ContentDialogButton.Close,
        });
        app.ImportFileRequested += () => _ = ChooseImportFile();
        app.ExportFileRequested += () => _ = ChooseExportFile();
        if (Diagnostics.OpenAddStation) app.AddLocalStation();

        AddShortcuts();
        ShowPage();
        UpdatePlayerPane();
        Nav.Loaded += (_, _) =>
        {
            ShowPage();   // SettingsItem exists only once the template is applied
            // Start with focus in the pane without a keyboard focus rectangle on the menu button.
            if (Nav.SelectedItem is Control c) c.Focus(FocusState.Pointer);
        };
    }

    UIElement PageView(Page p)
    {
        if (!pages.TryGetValue(p, out var v))
        {
            v = p switch
            {
                Page.Discover => new DiscoverPage(app),
                Page.Search => new SearchPage(app),
                Page.Settings => new SettingsPage(app, () => Hwnd),
                _ => new LibraryPage(app),
            };
            pages[p] = v;
        }
        return v;
    }

    void ShowPage()
    {
        PageHost.Content = PageView(app.Page);
        syncingNav = true;
        if (app.Page == Page.Settings) Nav.SelectedItem = Nav.SettingsItem;
        else Nav.SelectedItem = Nav.MenuItems.OfType<NavigationViewItem>().First(i => (string)i.Tag == app.Page.ToString());
        syncingNav = false;
        if (app.Page == Page.Search && PageHost.Content is SearchPage sp) DispatcherQueue.TryEnqueue(sp.FocusSearch);
    }

    void Nav_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (syncingNav) return;
        if (args.IsSettingsSelected) { app.Page = Page.Settings; return; }
        if (args.SelectedItem is NavigationViewItem { Tag: string tag } && Enum.TryParse<Page>(tag, out var page)) app.Page = page;
    }

    void UpdatePlayerPane()
    {
        PlayerPane.Visibility = app.ShowPlayer ? Visibility.Visible : Visibility.Collapsed;
        BottomBar.Visibility = !app.ShowPlayer && app.Player.Station != null ? Visibility.Visible : Visibility.Collapsed;
        PlayerToggleItem.Content = app.ShowPlayer ? "Hide Player" : "Show Player";
    }

    // Footer commands (they don't select a page). ItemInvoked also covers keyboard and UI Automation.
    void Nav_ItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (args.InvokedItemContainer == AddStationItem) app.AddLocalStation();
        else if (args.InvokedItemContainer == PlayerToggleItem) app.ShowPlayer = !app.ShowPlayer;
        else if (args.InvokedItemContainer == MiniPlayerItem) App.Current?.ShowMiniPlayer();
    }

    /// <summary>The Mac's menu shortcuts with Ctrl in place of Command. Space is deliberately not play/stop.</summary>
    void AddShortcuts()
    {
        void Add(VirtualKey key, VirtualKeyModifiers mods, Action action)
        {
            var a = new KeyboardAccelerator { Key = key, Modifiers = mods };
            a.Invoked += (_, e) =>
            {
                if (dialogOpen && key is not VirtualKey.Enter) { e.Handled = false; return; }
                action();
                e.Handled = true;
            };
            Root.KeyboardAccelerators.Add(a);
        }
        var ctrl = VirtualKeyModifiers.Control;
        var ctrlAlt = VirtualKeyModifiers.Control | VirtualKeyModifiers.Menu;
        var ctrlShift = VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift;
        Add(VirtualKey.Enter, ctrl, () => { if (app.Player.HasStation) app.Player.TogglePlayback(); });
        Add(VirtualKey.Up, ctrl, app.Player.IncreaseVolume);
        Add(VirtualKey.Down, ctrl, app.Player.DecreaseVolume);
        Add(VirtualKey.M, ctrlAlt, app.Player.ToggleMute);
        Add(VirtualKey.Right, ctrl, () => app.Player.Skip(1));
        Add(VirtualKey.Left, ctrl, () => app.Player.Skip(-1));
        Add(VirtualKey.Number1, ctrl, () => app.Page = Page.Library);
        Add(VirtualKey.Number2, ctrl, () => app.Page = Page.Discover);
        Add(VirtualKey.F, ctrl, () => { if (app.Page == Page.Search && PageHost.Content is SearchPage sp) sp.FocusSearch(); else app.Page = Page.Search; });
        Add(VirtualKey.P, ctrlAlt, () => app.ShowPlayer = !app.ShowPlayer);
        Add(VirtualKey.N, ctrl, app.AddLocalStation);
        Add(VirtualKey.M, ctrlShift, () => App.Current?.ShowMiniPlayer());
        Add(VirtualKey.I, ctrlShift, app.ChooseImportFile);
        Add(VirtualKey.E, ctrlShift, app.ChooseExportFile);
        Add(VirtualKey.Q, ctrl, () => App.Current?.Quit());
        Root.KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden;
    }

    /// <summary>Import Stations… (Ctrl+Shift+I, the Library's More menu, the empty library).</summary>
    async Task ChooseImportFile()
    {
        if (await Ui.PickStationFileToOpen(Hwnd) is { } path) app.ImportReporting(path);
    }

    /// <summary>Export Stations… (Ctrl+Shift+E, the Library's More menu).</summary>
    async Task ChooseExportFile()
    {
        if (app.Library.Count == 0) { app.Notice("Nothing to Export", "Your library is empty."); return; }
        if (await Ui.PickStationFileToSave(Hwnd, StationExchange.DefaultFileName) is { } path) app.ExportReporting(path);
    }

    /// <summary>App Theme changed (the window's root is already set): restyle an open dialog too.</summary>
    public void ThemeChanged(ElementTheme theme)
    {
        if (currentDialog != null) currentDialog.RequestedTheme = theme;
    }

    // ContentDialogs can't overlap, so requests queue up and show one at a time.
    void Enqueue(Func<ContentDialog> make)
    {
        dialogs.Enqueue(make);
        if (!dialogOpen) _ = ShowNext();
    }

    async Task ShowNext()
    {
        while (dialogs.TryDequeue(out var make))
        {
            if (Root.XamlRoot is null) { await Task.Delay(100); dialogs.Enqueue(make); continue; }
            dialogOpen = true;
            var d = make();
            d.XamlRoot = Root.XamlRoot;
            d.RequestedTheme = Root.RequestedTheme;   // Default: follows Windows, like the window
            currentDialog = d;
            try { await d.ShowAsync(); }
            catch (Exception e) { Diagnostics.Log("dialog: " + e.Message); }
            dialogOpen = false;
            currentDialog = null;
        }
    }
}
