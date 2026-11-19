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
        string iconPng = Path.Combine(AppContext.BaseDirectory, "Assets", "Driftwave.png");
        TitleIcon.Source = new BitmapImage(new Uri(iconPng));
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        // Tall caption buttons (48 px) centred on the title row, as in other Windows 11 apps with a navigation pane.
        AppWindow.TitleBar.PreferredHeightOption = Microsoft.UI.Windowing.TitleBarHeightOption.Tall;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Driftwave.ico"));

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
        // Theme scenes place things in the pane, content and player areas: keep them told how wide the pane is.
        void PaneChanged()
        {
            double width = Nav.IsPaneOpen && Nav.DisplayMode == NavigationViewDisplayMode.Expanded ? Nav.OpenPaneLength : Nav.CompactPaneLength;
            if (Math.Abs(Theming.SceneEffect.PaneWidth - width) > 0.5) { Theming.SceneEffect.PaneWidth = width; effects.Relayout(); }
        }
        Nav.DisplayModeChanged += (_, _) => PaneChanged();
        Nav.PaneOpened += (_, _) => PaneChanged();
        Nav.PaneClosed += (_, _) => PaneChanged();
        // Any change (restoring from minimised reports only a size or position change); SetWindowVisible is cheap.
        AppWindow.Changed += (s, e) =>
        {
            bool shown = s.IsVisible && s.Presenter is not Microsoft.UI.Windowing.OverlappedPresenter { State: Microsoft.UI.Windowing.OverlappedPresenterState.Minimized };
            EffectsClock.SetWindowVisible(shown);
            MemoryTrim.WindowShown(shown);
        };
        Activated += (_, e) => EffectsClock.SetWindowActive(e.WindowActivationState != WindowActivationState.Deactivated);

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
        // The check at launch found a new version: say so once, without a dialog.
        var updateNow = new Button { Content = "Update Now", Style = (Style)Application.Current.Resources["AccentButtonStyle"] };
        updateNow.Click += async (_, _) => { UpdateBar.IsOpen = false; await app.Updates.DownloadAndInstall(); };
        UpdateBar.ActionButton = updateNow;
        app.Updates.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName != nameof(Updater.State)) return;
            DispatcherQueue.TryEnqueue(() =>
            {
                if (app.Updates.State == UpdateState.Available && app.Updates.Latest is { } r && Updater.CanInstall)
                {
                    UpdateBar.Message = $"{(r.Title.StartsWith("Driftwave", StringComparison.Ordinal) ? r.Title : $"{Diagnostics.AppName} {r.Version}")} is ready to install. Your station carries on after the restart.";
                    UpdateBar.IsOpen = true;
                }
                else if (app.Updates.State is UpdateState.Failed && UpdateBar.IsOpen) UpdateBar.Message = app.Updates.Message;
            });
        };

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

    Page? shownPage;

    void ShowPage()
    {
        var view = PageView(app.Page);
        if (!ReferenceEquals(PageHost.Content, view))
        {
            // Pages further down the list come in from the right, earlier ones from the left.
            PageHost.ContentTransitions = Motion.Enabled && shownPage is { } before
                ? [new Microsoft.UI.Xaml.Media.Animation.ContentThemeTransition { HorizontalOffset = app.Page > before ? 48 : -48, VerticalOffset = 0 }]
                : null;
            PageHost.Content = view;
        }
        shownPage = app.Page;
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

    bool playerPaneShown;

    void UpdatePlayerPane()
    {
        bool show = app.ShowPlayer;
        Diagnostics.Log($"player pane {(show ? "shown" : "hidden")}");
        if (!playerPaneShown) PlayerPane.Visibility = show ? Visibility.Visible : Visibility.Collapsed;   // first time: no slide
        else if ((PlayerPane.Visibility == Visibility.Visible) != show || PlayerPane.Opacity < 1)
            try { Motion.SlidePane(PlayerPane, show, 312); } catch (Exception ex) { Diagnostics.Log("slide failed: " + ex); PlayerPane.Visibility = show ? Visibility.Visible : Visibility.Collapsed; }
        playerPaneShown = true;
        bool bar = !show && app.Player.Station != null;
        if (bar && BottomBar.Visibility == Visibility.Collapsed) { BottomBar.Visibility = Visibility.Visible; Motion.Entrance(BottomBar, rise: 24, fromScale: 1f); }
        else if (!bar) BottomBar.Visibility = Visibility.Collapsed;
        PlayerToggleItem.Content = app.ShowPlayer ? "Hide Player" : "Show Player";
        if (Theming.SceneEffect.PlayerShown != show) { Theming.SceneEffect.PlayerShown = show; effects.Relayout(); }
    }

    // Footer commands (they don't select a page). ItemInvoked also covers keyboard and UI Automation.
    void Nav_ItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (args.InvokedItemContainer == AddStationItem) app.AddLocalStation();
        else if (args.InvokedItemContainer == PlayerToggleItem) app.ShowPlayer = !app.ShowPlayer;
        else if (args.InvokedItemContainer == MiniPlayerItem) App.Current?.ShowMiniPlayer();
    }

    readonly Dictionary<(VirtualKey, VirtualKeyModifiers), long> lastShortcut = [];

    /// <summary>The Mac's menu shortcuts with Ctrl in place of Command. Space is deliberately not play/stop.</summary>
    void AddShortcuts()
    {
        void Add(VirtualKey key, VirtualKeyModifiers mods, Action action)
        {
            var a = new KeyboardAccelerator { Key = key, Modifiers = mods };
            a.Invoked += (_, e) =>
            {
                if (dialogOpen && key is not VirtualKey.Enter) { e.Handled = false; return; }
                // WinUI can deliver one key press to an accelerator twice (seen with Ctrl+Alt+P), which would undo a
                // toggle. Nobody presses the same shortcut twice within 150 ms, so a second call that soon is that.
                long now = Environment.TickCount64;
                var id = (key, mods);
                bool repeat = lastShortcut.TryGetValue(id, out long last) && now - last < 150;
                lastShortcut[id] = now;
                Diagnostics.Log($"shortcut {mods}+{key}{(repeat ? " (repeat ignored)" : "")}");
                if (!repeat) action();
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
    /// The navigation pane, the player card and the bottom bar. Default: clear pane over Mica and Windows' card fill.
    /// Themes: the pane takes the theme's background (nearly opaque, so window effects don't run under the menu)
    /// and the cards a solid blend of the theme's card colour over its background, so effects stay behind them too.
    /// </summary>
    /// <summary>The window title in the theme's style (<c>windowTitle</c>: font, color, size, weight, uppercase).</summary>
    void ApplyWindowTitle(Theme theme)
    {
        var o = theme.IsDefault ? null : theme.Options("windowTitle");
        bool styled = o is { Count: > 0 };
        TitleText.FontFamily = styled && o!["font"] is System.Text.Json.Nodes.JsonValue f && f.TryGetValue(out string? font)
            ? ThemeManager.Font(font) : (Microsoft.UI.Xaml.Media.FontFamily)Application.Current.Resources["ContentControlThemeFontFamily"];
        TitleText.FontSize = styled ? WindowEffects.Opt(o!, "size", 12) : 12;
        TitleText.FontWeight = styled && o!["weight"] is System.Text.Json.Nodes.JsonValue wv && wv.TryGetValue(out string? weight) && weight == "bold"
            ? Microsoft.UI.Text.FontWeights.Bold : Microsoft.UI.Text.FontWeights.Normal;
        bool upper = styled && o!["uppercase"] is System.Text.Json.Nodes.JsonValue u && u.TryGetValue(out bool up) && up;
        TitleText.Text = upper ? Diagnostics.AppName.ToUpperInvariant() : Diagnostics.AppName;
        if (styled && o!["color"] is not null) TitleText.Foreground = new Microsoft.UI.Xaml.Media.SolidColorBrush(WindowEffects.Opt(o!, "color", Microsoft.UI.Colors.White));
        else TitleText.ClearValue(TextBlock.ForegroundProperty);
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, MeasureTitle);
    }

    /// <summary>Tells theme scenes where the title ends, so nothing is drawn over it.</summary>
    void MeasureTitle()
    {
        if (TitleText.XamlRoot is null) return;
        double end = TitleText.TransformToVisual(Root).TransformPoint(new Windows.Foundation.Point(TitleText.ActualWidth, 0)).X;
        if (Math.Abs(end - Theming.SceneEffect.TitleEnd) > 0.5) { Theming.SceneEffect.TitleEnd = end; effects.Relayout(); }
    }

    void ApplySurfaces(Theme theme)
    {
        var paneColor = Microsoft.UI.Colors.Transparent;
        Microsoft.UI.Xaml.Media.Brush cardBrush = Ui.ThemeBrush("CardBackgroundFillColorDefaultBrush");
        // Glass themes (a system backdrop behind the theme) keep their see-through panes and cards: the colour
        // drifting behind the glass is the point. Solid themes get solid surfaces.
        // Photo themes ("scene": {"surfaces": "translucent"}): smoky glass over the picture, in the theme's own colours.
        bool translucent = !theme.IsDefault && theme.Options("scene")["surfaces"] is System.Text.Json.Nodes.JsonValue sv
            && sv.TryGetValue(out string? surf) && surf == "translucent";
        if (translucent)
        {
            paneColor = theme.Get("pane") ?? Theme.WithAlpha(theme.Get("background") ?? Microsoft.UI.Colors.Black, 0.6);
            if (theme.Get("card") is { } glass) cardBrush = new Microsoft.UI.Xaml.Media.SolidColorBrush(glass);
        }
        else if (!theme.IsDefault && theme.Backdrop == "none" && theme.Get("background") is { } bg)
        {
            var solid = Theme.WithAlpha(bg, 1);
            paneColor = Theme.WithAlpha(theme.Get("pane") ?? bg, 0.94);
            var card = theme.Get("card") is { } c ? Theme.Mix(solid, Theme.WithAlpha(c, 1), c.A / 255.0) : solid;
            cardBrush = new Microsoft.UI.Xaml.Media.SolidColorBrush(Theme.WithAlpha(card, 0.97));
        }
        foreach (var key in new[] { "NavigationViewExpandedPaneBackground", "NavigationViewDefaultPaneBackground" })
            if (Nav.Resources[key] is Microsoft.UI.Xaml.Media.SolidColorBrush b) b.Color = paneColor;
        PlayerCard.Background = cardBrush;
        BottomBar.Background = cardBrush;
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
        ApplySurfaces(theme);
        ApplyWindowTitle(theme);
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
        if (PlayerCard.Child is PlayerPanel panel) panel.SetDecor(PlayerDecor.For(theme, app.Player, fx && intensity > 0));
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
