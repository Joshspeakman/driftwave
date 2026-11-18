using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Shortwave.Core;
using Shortwave.Integration;
using Shortwave.Model;
using Shortwave.Theming;
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
    private readonly WindowEffects effects;
    private readonly TitleDecor titleDecor = new();
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
        // Theme animations run only while the window can be seen.
        effects = new WindowEffects(EffectsBack, EffectsFront, Root);
        AppWindow.Changed += (s, e) =>
        {
            if (e.DidVisibilityChange || e.DidPresenterChange)
                EffectsClock.SetWindowVisible(s.IsVisible && s.Presenter is not Microsoft.UI.Windowing.OverlappedPresenter { State: Microsoft.UI.Windowing.OverlappedPresenterState.Minimized });
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
        var view = PageView(app.Page);
        PageHost.Content = view;
        if (view is FrameworkElement fe)
        {
            if (fe.IsLoaded) DecorateTitles(fe);
            else
            {
                void Once(object s, RoutedEventArgs e) { fe.Loaded -= Once; DecorateTitles(fe); }
                fe.Loaded += Once;
            }
        }
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
        // The navigation pane (where focus starts) and lists use Ctrl+Up/Down and Enter themselves, so those
        // accelerators never fired from there. Take these three before the focused control sees them.
        Root.PreviewKeyDown += (_, e) =>
        {
            if (dialogOpen || !IsDown(VirtualKey.Control) || IsDown(VirtualKey.Menu) || IsDown(VirtualKey.Shift)) return;
            switch (e.Key)
            {
                case VirtualKey.Up: app.Player.IncreaseVolume(); break;
                case VirtualKey.Down: app.Player.DecreaseVolume(); break;
                case VirtualKey.Enter: if (app.Player.HasStation) app.Player.TogglePlayback(); break;
                default: return;
            }
            e.Handled = true;
        };
    }

    static bool IsDown(VirtualKey key) =>
        InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down);

    /// <summary>Import Stations… (Ctrl+Shift+I, the Library's More menu, the empty library).</summary>
    async Task ChooseImportFile()
    {
        Diagnostics.Log("picker import");
        string? path;
        try { path = await Ui.PickStationFileToOpen(Hwnd); }
        catch (Exception e) { Diagnostics.Log("picker import failed: " + e); app.Notice("Couldn’t Import Stations", e.Message); return; }
        if (path != null) app.ImportReporting(path);
    }

    /// <summary>Export Stations… (Ctrl+Shift+E, the Library's More menu).</summary>
    async Task ChooseExportFile()
    {
        if (app.Library.Count == 0) { app.Notice("Nothing to Export", "Your library is empty."); return; }
        Diagnostics.Log("picker export");
        string? path;
        try { path = await Ui.PickStationFileToSave(Hwnd, StationExchange.DefaultFileName); }
        catch (Exception e) { Diagnostics.Log("picker export failed: " + e); app.Notice("Couldn’t Export Stations", e.Message); return; }
        if (path != null) app.ExportReporting(path);
    }

    void DecorateTitles(FrameworkElement page)
    {
        foreach (var title in TitleDecor.FindTitles(page).ToList()) titleDecor.Attach(title);
    }

    /// <summary>
    /// Applies a theme to this window: light or dark, title bar buttons, backdrop and background, the screen
    /// border, the window effects, page title treatments and the player pane's decorations.
    /// </summary>
    public void ApplyLook(Theme theme, ElementTheme look)
    {
        bool fx = app.Settings.EffectsEnabled;
        double intensity = app.Settings.EffectIntensity;
        ThemeManager.ApplyWindow(this, theme, look, BackdropFill, "mica");
        if (currentDialog != null) currentDialog.RequestedTheme = look;
        if (fx && intensity > 0 && !theme.IsDefault && theme.Has("frame"))
        {
            var o = theme.Options("frame");
            Root.Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(WindowEffects.Opt(o, "color", Microsoft.UI.Colors.Gray));
            Root.Padding = new Thickness(WindowEffects.Opt(o, "thickness", 12));
        }
        else
        {
            Root.Background = null;
            Root.Padding = new Thickness(0);
        }
        effects.Apply(theme, fx, intensity);
        titleDecor.Apply(theme, fx, intensity, PageHost.Content is DependencyObject page ? TitleDecor.FindTitles(page) : []);
        if (PlayerPane.Child is PlayerPanel panel) panel.SetDecor(PlayerDecor.For(theme, app.Player, fx && intensity > 0));
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
            Diagnostics.Log($"dialog {d.GetType().Name} {d.Title}");
            try { await d.ShowAsync(); }
            catch (Exception e) { Diagnostics.Log("dialog: " + e.Message); }
            dialogOpen = false;
            currentDialog = null;
        }
    }
}
