using System.Text.Json.Nodes;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Shortwave.Model;
using Shortwave.Theming;
using Windows.UI;

namespace Shortwave.Views;

/// <summary>
/// Settings > Appearance: the theme gallery, the restart notice, effects, and the editor for the user's own
/// themes (colours, fonts, corners, backdrop, effects; import, export, delete).
/// </summary>
public sealed class ThemeSettings : StackPanel
{
    readonly AppModel app;
    readonly ThemeManager themes;
    readonly Func<nint> hwnd;
    readonly Func<Border> card;
    readonly VariableSizedWrapGrid gallery = new() { Orientation = Orientation.Horizontal, ItemWidth = 148, ItemHeight = 132, Margin = new Thickness(0, 8, 0, 0) };
    readonly InfoBar restart = new() { Severity = InfoBarSeverity.Informational, IsClosable = false, Title = "Restart to finish" };
    readonly ToggleSwitch effects = new() { OnContent = "", OffContent = "", MinWidth = 0 };
    readonly Slider intensity = new() { Minimum = 0, Maximum = 100, StepFrequency = 5, Width = 200 };
    readonly StackPanel editor = new() { Spacing = 10 };
    readonly Expander customize = new() { HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch };
    readonly DispatcherQueueTimer saveTimer;
    Theme? editing;
    bool updating;

    public ThemeSettings(AppModel app, Func<nint> hwnd, Func<Border> card)
    {
        this.app = app;
        themes = app.Themes;
        this.hwnd = hwnd;
        this.card = card;
        Spacing = 4;
        saveTimer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        saveTimer.Interval = TimeSpan.FromMilliseconds(300);
        saveTimer.IsRepeating = false;
        saveTimer.Tick += (_, _) => SaveEdits();

        // Theme gallery
        var galleryCard = card();
        var gp = new StackPanel();
        gp.Children.Add(new TextBlock { Text = "Theme" });
        gp.Children.Add(Caption("Choose a look for the whole app. Default follows Windows; the others bring their own colours, type and effects."));
        gp.Children.Add(gallery);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 8, 0, 0) };
        buttons.Children.Add(Btn("Import Theme…", async () => await ImportTheme()));
        buttons.Children.Add(Btn("Open Themes Folder", () =>
        {
            Directory.CreateDirectory(themes.UserFolder);
            Ui.Open(themes.UserFolder);
        }));
        gp.Children.Add(buttons);
        galleryCard.Child = gp;
        Children.Add(galleryCard);

        var restartButton = Btn("Restart Now", () => App.Current?.Restart());
        restartButton.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        restart.ActionButton = restartButton;
        Children.Add(restart);

        // Effects
        var fxCard = card();
        var fx = new Grid { ColumnSpacing = 12, RowSpacing = 8 };
        fx.ColumnDefinitions.Add(new ColumnDefinition());
        fx.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        fx.RowDefinitions.Add(new RowDefinition());
        fx.RowDefinitions.Add(new RowDefinition());
        var fxText = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        fxText.Children.Add(new TextBlock { Text = "Theme Effects" });
        fxText.Children.Add(Caption("Animations and decorations such as the synthwave grid, rain, scanlines, the tuning dial and the meters. They pause while Windows' animation effects are off."));
        fx.Children.Add(fxText);
        AutomationProperties.SetName(effects, "Theme Effects");
        Grid.SetColumn(effects, 1);
        fx.Children.Add(effects);
        var intensityRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        intensityRow.Children.Add(new TextBlock { Text = "Intensity", VerticalAlignment = VerticalAlignment.Center, Style = (Style)Application.Current.Resources["SecondaryTextStyle"] });
        AutomationProperties.SetName(intensity, "Effect intensity");
        intensityRow.Children.Add(intensity);
        Grid.SetRow(intensityRow, 1);
        Grid.SetColumnSpan(intensityRow, 2);
        fx.Children.Add(intensityRow);
        fxCard.Child = fx;
        Children.Add(fxCard);
        effects.Toggled += (_, _) => { if (!updating) app.Settings.EffectsEnabled = effects.IsOn; };
        intensity.ValueChanged += (_, e) => { if (!updating) app.Settings.EffectIntensity = e.NewValue / 100; };

        // Editor
        var header = new StackPanel { Padding = new Thickness(0, 6, 0, 6) };
        header.Children.Add(new TextBlock { Text = "Customize Theme" });
        header.Children.Add(Caption("Change colours, type, corners and effects. Built-in themes stay as they are: customizing makes a copy."));
        customize.Header = header;
        customize.Content = editor;
        Children.Add(customize);

        themes.Changed += Refresh;
        app.Settings.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName is nameof(Preferences.EffectsEnabled) or nameof(Preferences.EffectIntensity) or nameof(Preferences.Appearance)) Refresh();
        };
        Refresh();
    }

    static TextBlock Caption(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap, Style = (Style)Application.Current.Resources["CaptionSecondaryStyle"] };

    static Button Btn(string text, Action click)
    {
        var b = new Button { Content = text };
        b.Click += (_, _) => click();
        return b;
    }

    void Refresh()
    {
        updating = true;
        BuildGallery();
        var t = themes.Current;
        restart.IsOpen = themes.RestartNeeded;
        restart.Message = $"{(t.IsDefault ? "Going back to Default" : t.Name)} changes fonts, corners or the text colour, which Shortwave sets when it starts. Playback resumes after the restart.";
        effects.IsOn = app.Settings.EffectsEnabled;
        intensity.Value = Math.Round(app.Settings.EffectIntensity * 100);
        intensity.IsEnabled = effects.IsOn;
        if (!ReferenceEquals(editing, t)) BuildEditor();
        updating = false;
    }

    // MARK: gallery

    void BuildGallery()
    {
        gallery.Children.Clear();
        foreach (var t in themes.All)
        {
            bool selected = t.Id == themes.Current.Id;
            var b = new Button
            {
                Padding = new Thickness(6), Width = 140, Height = 124, HorizontalContentAlignment = HorizontalAlignment.Stretch,
                BorderThickness = new Thickness(selected ? 2 : 1),
                Content = Tile(t),
            };
            if (selected) b.BorderBrush = Ui.ThemeBrush("AccentFillColorDefaultBrush");
            AutomationProperties.SetName(b, t.Name + (selected ? " (current)" : ""));
            ToolTipService.SetToolTip(b, string.IsNullOrEmpty(t.Description) ? t.Name : $"{t.Name}: {t.Description}");
            var id = t.Id;
            b.Click += (_, _) => app.Settings.ThemeId = id;
            gallery.Children.Add(b);
        }
    }

    /// <summary>A miniature of the theme: its background, a content panel, an accent pill and its title font.</summary>
    FrameworkElement Tile(Theme t)
    {
        bool dark = t.IsDefault ? themes.ElementTheme(t) switch
        {
            ElementTheme.Dark => true,
            ElementTheme.Light => false,
            _ => Application.Current.RequestedTheme == ApplicationTheme.Dark,
        } : t.IsDark;
        Color bg = t.Get("background") is { } c ? Theme.WithAlpha(c, 1) : dark ? Color.FromArgb(255, 32, 32, 32) : Color.FromArgb(255, 243, 243, 243);
        Color surface = t.Get("surface") is { } s ? Theme.Mix(bg, Theme.WithAlpha(s, 1), s.A / 255.0) : dark ? Color.FromArgb(255, 40, 40, 40) : Color.FromArgb(255, 250, 250, 250);
        Color text = t.Get("text") ?? (dark ? Microsoft.UI.Colors.White : Color.FromArgb(255, 26, 26, 26));
        Color secondary = t.Get("textSecondary") ?? Theme.WithAlpha(text, 0.6);
        Color accent = t.Get("accent") ?? (Application.Current.Resources["SystemAccentColor"] is Color a ? a : Microsoft.UI.Colors.SteelBlue);
        var preview = new Grid { Height = 78, Background = new SolidColorBrush(bg), CornerRadius = new CornerRadius(Math.Min(t.Corners.Overlay ?? 6, 10)) };
        if (t.Has("synthwave")) preview.Background = new LinearGradientBrush
        {
            StartPoint = new Windows.Foundation.Point(0, 0), EndPoint = new Windows.Foundation.Point(0, 1),
            GradientStops = { new GradientStop { Color = bg, Offset = 0 }, new GradientStop { Color = Color.FromArgb(255, 255, 46, 151), Offset = 1 } },
        };
        if (t.Has("frame")) { preview.BorderBrush = new SolidColorBrush(WindowEffects.Opt(t.Options("frame"), "color", accent)); preview.BorderThickness = new Thickness(5); }
        var panel = new Border
        {
            Background = new SolidColorBrush(surface), Margin = new Thickness(34, 8, 8, 8), CornerRadius = new CornerRadius(Math.Min(t.Corners.Card ?? 4, 6)),
            BorderBrush = t.Get("cardStroke") is { } cs ? new SolidColorBrush(cs) : null, BorderThickness = new Thickness(t.Get("cardStroke") is null ? 0 : 1),
        };
        var inner = new StackPanel { Margin = new Thickness(8, 6, 6, 6), Spacing = 4 };
        inner.Children.Add(new TextBlock
        {
            Text = t.Fonts.TitleUppercase ? "AA" : "Aa", FontSize = Math.Min(20, (t.Fonts.TitleSize ?? 28) * 0.62), Foreground = new SolidColorBrush(text),
            FontFamily = ThemeManager.Font(t.Fonts.Title ?? t.Fonts.Body ?? "Segoe UI Variable Display"), FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        });
        inner.Children.Add(new Rectangle { Height = 3, Width = 46, HorizontalAlignment = HorizontalAlignment.Left, Fill = new SolidColorBrush(secondary), RadiusX = 1.5, RadiusY = 1.5 });
        inner.Children.Add(new Rectangle { Height = 7, Width = 26, HorizontalAlignment = HorizontalAlignment.Left, Fill = new SolidColorBrush(accent), RadiusX = 3.5, RadiusY = 3.5 });
        panel.Child = inner;
        preview.Children.Add(panel);
        // A sliver of the navigation pane.
        var nav = new StackPanel { Margin = new Thickness(8, 12, 0, 0), Spacing = 6, HorizontalAlignment = HorizontalAlignment.Left };
        for (int i = 0; i < 3; i++) nav.Children.Add(new Rectangle { Width = 18, Height = 3, Fill = new SolidColorBrush(Theme.WithAlpha(text, i == 0 ? 0.9 : 0.45)), RadiusX = 1.5, RadiusY = 1.5 });
        preview.Children.Add(nav);
        var stack = new StackPanel { Spacing = 6 };
        stack.Children.Add(preview);
        stack.Children.Add(new TextBlock { Text = t.Name, TextTrimming = TextTrimming.CharacterEllipsis, HorizontalAlignment = HorizontalAlignment.Center, Style = (Style)Application.Current.Resources["CaptionTextBlockStyle"] });
        return stack;
    }

    // MARK: editor

    void BuildEditor()
    {
        editing = themes.Current;
        var t = editing;
        editor.Children.Clear();
        if (t.BuiltIn)
        {
            editor.Children.Add(Caption(t.IsDefault
                ? "Default is Windows' own look. Make a copy to start your own theme from it."
                : $"{t.Name} is built in. Make a copy to change it; the copy is saved as one of your themes."));
            var copy = Btn($"Customize a Copy of {t.Name}", () =>
            {
                var c = themes.Duplicate(themes.Current);
                app.Settings.ThemeId = c.Id;
            });
            copy.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
            editor.Children.Add(copy);
            return;
        }

        // Name, base, backdrop
        var name = new TextBox { Text = t.Name, Width = 240, Header = "Name" };
        name.TextChanged += (_, _) => { t.Name = name.Text.Trim().Length > 0 ? name.Text.Trim() : "Untitled"; Changed(); };
        var baseBox = Combo("Light or dark", ["Dark", "Light"], t.IsDark ? 0 : 1, i => { t.Base = i == 0 ? "dark" : "light"; Changed(); });
        var backdropBox = Combo("Backdrop", ["Solid colour", "Mica", "Acrylic (see-through)"], t.Backdrop switch { "mica" => 1, "acrylic" => 2, _ => 0 },
            i => { t.Backdrop = i switch { 1 => "mica", 2 => "acrylic", _ => "none" }; Changed(); });
        editor.Children.Add(Row(name, baseBox, backdropBox));

        // Colours
        editor.Children.Add(Section("Colours"));
        var colors = new VariableSizedWrapGrid { Orientation = Orientation.Horizontal, ItemWidth = 250, ItemHeight = 40 };
        foreach (var (key, label) in Theme.Tokens) colors.Children.Add(ColorRow(t, key, label));
        editor.Children.Add(colors);

        // Type
        editor.Children.Add(Section("Type"));
        var bodyFont = FontBox("Text font", t.Fonts.Body, f => { t.Fonts = t.Fonts with { Body = f }; Changed(); });
        var titleFont = FontBox("Title font", t.Fonts.Title, f => { t.Fonts = t.Fonts with { Title = f }; Changed(); });
        var size = new NumberBox { Header = "Title size", Value = t.Fonts.TitleSize ?? 28, Minimum = 14, Maximum = 72, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact, Width = 120 };
        size.ValueChanged += (_, e) => { if (!double.IsNaN(e.NewValue)) { t.Fonts = t.Fonts with { TitleSize = e.NewValue }; Changed(); } };
        var spacing = new NumberBox { Header = "Letter spacing", Value = t.Fonts.TitleSpacing ?? 0, Minimum = -100, Maximum = 800, SmallChange = 20, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact, Width = 140 };
        spacing.ValueChanged += (_, e) => { if (!double.IsNaN(e.NewValue)) { t.Fonts = t.Fonts with { TitleSpacing = (int)e.NewValue }; Changed(); } };
        var weight = Combo("Title weight", ["Light", "Normal", "SemiBold", "Bold", "Black"],
            Math.Max(0, Array.IndexOf(new[] { "Light", "Normal", "SemiBold", "Bold", "Black" }, t.Fonts.TitleWeight ?? "SemiBold")),
            i => { t.Fonts = t.Fonts with { TitleWeight = new[] { "Light", "Normal", "SemiBold", "Bold", "Black" }[i] }; Changed(); });
        var upper = new ToggleSwitch { Header = "Upper-case titles", IsOn = t.Fonts.TitleUppercase };
        upper.Toggled += (_, _) => { t.Fonts = t.Fonts with { TitleUppercase = upper.IsOn }; Changed(); };
        editor.Children.Add(Row(bodyFont, titleFont));
        editor.Children.Add(Row(size, weight, spacing, upper));

        // Shape
        editor.Children.Add(Section("Shape"));
        var corners = new Slider { Header = "Corner radius", Minimum = 0, Maximum = 20, Value = t.Corners.Overlay ?? 8, Width = 220, StepFrequency = 1 };
        corners.ValueChanged += (_, e) => { t.Corners = t.Corners with { Control = Math.Round(e.NewValue / 2), Overlay = e.NewValue, Card = e.NewValue }; Changed(); };
        var cover = new Slider { Header = "Cover roundness", Minimum = 0, Maximum = 50, Value = Math.Round((t.Corners.Cover ?? 0.18) * 100), Width = 220, StepFrequency = 1 };
        cover.ValueChanged += (_, e) => { t.Corners = t.Corners with { Cover = e.NewValue / 100 }; Changed(); };
        editor.Children.Add(Row(corners, cover));

        // Effects
        editor.Children.Add(Section("Effects"));
        var fxGrid = new VariableSizedWrapGrid { Orientation = Orientation.Horizontal, ItemWidth = 250, ItemHeight = 36 };
        foreach (var (key, label) in Theme.EffectKinds)
        {
            var box = new CheckBox { Content = label, IsChecked = t.Has(key) };
            box.Checked += (_, _) => { t.Effects.TryAdd(key, []); Changed(); };
            box.Unchecked += (_, _) => { t.Effects.Remove(key); Changed(); };
            fxGrid.Children.Add(box);
        }
        editor.Children.Add(fxGrid);
        editor.Children.Add(Caption("Effects use the theme's colours. Each effect's own settings (colours, sizes) can be changed in the theme file: Export Theme, edit it, then Import Theme."));

        // Export, delete
        var delete = new Button { Content = "Delete Theme" };
        var confirm = new Flyout();
        var confirmPanel = new StackPanel { Spacing = 10, MaxWidth = 280 };
        confirmPanel.Children.Add(new TextBlock { Text = $"Delete “{t.Name}”? This can't be undone.", TextWrapping = TextWrapping.Wrap });
        var yes = Btn("Delete", () =>
        {
            confirm.Hide();
            themes.Delete(t);
            editing = null;
            app.Settings.ThemeId = Theme.DefaultId;
            Refresh();
        });
        confirmPanel.Children.Add(yes);
        confirm.Content = confirmPanel;
        delete.Flyout = confirm;
        editor.Children.Add(Row(Btn("Export Theme…", async () => await ExportTheme(t)), delete));
    }

    void Changed()
    {
        if (updating) return;
        saveTimer.Stop();
        saveTimer.Start();
    }

    void SaveEdits()
    {
        if (editing is not { BuiltIn: false } t) return;
        try { themes.Save(t); } catch (Exception e) { Diagnostics.Log("theme save: " + e.Message); }
        themes.Apply(t);
    }

    static TextBlock Section(string text) => new() { Text = text, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Margin = new Thickness(0, 8, 0, 0) };

    static StackPanel Row(params FrameworkElement[] items)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 16 };
        foreach (var i in items) { i.VerticalAlignment = VerticalAlignment.Bottom; row.Children.Add(i); }
        return row;
    }

    ComboBox Combo(string header, string[] items, int selected, Action<int> changed)
    {
        var c = new ComboBox { Header = header, ItemsSource = items, SelectedIndex = selected, MinWidth = 150 };
        c.SelectionChanged += (_, _) => { if (c.SelectedIndex >= 0) changed(c.SelectedIndex); };
        return c;
    }

    ComboBox FontBox(string header, string? value, Action<string?> changed)
    {
        const string defaultLabel = "Default (Segoe UI Variable)";
        var items = new List<string> { defaultLabel };
        items.AddRange(ThemeManager.SuggestedFonts);
        var c = new ComboBox { Header = header, ItemsSource = items, IsEditable = true, Width = 230, Text = value ?? defaultLabel, SelectedItem = value ?? defaultLabel };
        ToolTipService.SetToolTip(c, "Pick a font or type the name of any installed font.");
        void Commit(string? text)
        {
            text = text?.Trim();
            changed(string.IsNullOrEmpty(text) || text == defaultLabel ? null : text);
        }
        c.SelectionChanged += (_, _) => { if (c.SelectedItem is string s) Commit(s); };
        c.TextSubmitted += (_, e) => Commit(e.Text);
        return c;
    }

    FrameworkElement ColorRow(Theme t, string key, string label)
    {
        var grid = new Grid { ColumnSpacing = 8, Margin = new Thickness(0, 0, 12, 0) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition());
        var current = t.Get(key) ?? Microsoft.UI.Colors.Gray;
        var swatch = new Rectangle { Width = 28, Height = 20, RadiusX = 3, RadiusY = 3, Fill = new SolidColorBrush(current), Stroke = Ui.ThemeBrush("ControlStrokeColorDefaultBrush") };
        var hex = new TextBox { Text = Theme.ColorText(current), Width = 100, FontFamily = new FontFamily("Consolas") };
        AutomationProperties.SetName(hex, label + " colour");
        var picker = new ColorPicker { IsAlphaEnabled = true, IsMoreButtonVisible = false, IsColorChannelTextInputVisible = false, Color = current };
        var button = new Button { Padding = new Thickness(4), Content = swatch, Flyout = new Flyout { Content = picker } };
        AutomationProperties.SetName(button, "Choose " + label.ToLowerInvariant());
        ToolTipService.SetToolTip(button, label);
        void Set(Color c, bool fromText)
        {
            t.Colors[key] = c;
            ((SolidColorBrush)swatch.Fill).Color = c;
            if (!fromText) hex.Text = Theme.ColorText(c);
            Changed();
        }
        picker.ColorChanged += (_, e) => Set(e.NewColor, false);
        hex.LostFocus += (_, _) => { if (Theme.ParseColor(hex.Text) is { } c && c != t.Get(key)) { picker.Color = c; Set(c, true); } };
        var text = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
        grid.Children.Add(button);
        Grid.SetColumn(hex, 1);
        grid.Children.Add(hex);
        Grid.SetColumn(text, 2);
        grid.Children.Add(text);
        return grid;
    }

    // MARK: files

    async Task ImportTheme()
    {
        string? path;
        try { path = await Ui.PickStationFileToOpen(hwnd()); }
        catch (Exception e) { app.Notice("Couldn’t Import Theme", e.Message); return; }
        if (path is null) return;
        try
        {
            var t = themes.Import(path);
            app.Settings.ThemeId = t.Id;
            Refresh();
        }
        catch (Exception e) { app.Notice("Couldn’t Import Theme", e is FormatException or System.Text.Json.JsonException ? "This file isn’t a Shortwave theme." : e.Message); }
    }

    async Task ExportTheme(Theme t)
    {
        string? path;
        try { path = await Ui.PickJsonToSave(hwnd(), t.Name + ".json", "Shortwave theme"); }
        catch (Exception e) { app.Notice("Couldn’t Export Theme", e.Message); return; }
        if (path is null) return;
        try { ThemeManager.Export(t, path); }
        catch (Exception e) { app.Notice("Couldn’t Export Theme", e.Message); }
    }
}
