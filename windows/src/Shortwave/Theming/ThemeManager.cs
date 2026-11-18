using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Shortwave.Model;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// Loads themes and applies them. WinUI's theme brushes are shared objects, so recolouring them recolours every
/// control that uses them, popups and dialogs included, with no restyling. What WinUI only reads once (fonts,
/// control corner radii, and the primary text colour once text has been drawn) is applied at launch, so a theme
/// that changes those asks for a restart (<see cref="RestartNeeded"/>).
/// </summary>
public sealed class ThemeManager
{
    readonly Preferences settings;
    readonly string userFolder;
    readonly string builtInFolder = Path.Combine(AppContext.BaseDirectory, "Themes");
    readonly Dictionary<(string Dict, string Key), object> originals = [];
    readonly HashSet<(string Dict, string Key)> touched = [];
    string launchSignature = "";

    public List<Theme> All { get; } = [];
    public Theme Current { get; private set; } = DefaultTheme();
    /// <summary>The theme the app was launched with (fonts, corners and text colour come from it).</summary>
    public Theme Launched { get; private set; } = DefaultTheme();
    public bool RestartNeeded => Signature(Current) != launchSignature;
    public bool HighContrast { get; } = new Windows.UI.ViewManagement.AccessibilitySettings().HighContrast;
    public event Action? Changed;

    public ThemeManager(Preferences settings, Core.AppPaths paths)
    {
        this.settings = settings;
        userFolder = Path.Combine(paths.Support, "Themes");
        Reload();
    }

    public static Theme DefaultTheme() => new()
    {
        Id = Theme.DefaultId,
        Name = "Default",
        Description = "Windows' own look. Light or dark follows App Theme.",
        Base = "system",
        Backdrop = "mica",
        BuiltIn = true,
    };

    public void Reload()
    {
        All.Clear();
        All.Add(DefaultTheme());
        foreach (var (folder, builtIn) in new[] { (builtInFolder, true), (userFolder, false) })
        {
            if (!Directory.Exists(folder)) continue;
            foreach (var file in Directory.GetFiles(folder, "*.json").OrderBy(f => f, StringComparer.OrdinalIgnoreCase))
            {
                try
                {
                    var t = Theme.FromJson(File.ReadAllText(file), builtIn);
                    if (!builtIn)
                    {
                        t.SourcePath = file;
                        // A hand-made file may carry any id; it only matters for the "theme" setting.
                        if (!Theme.IsValidId(t.Id) || All.Any(x => x.Id == t.Id)) t.Id = "custom-" + StableHash(Path.GetFileNameWithoutExtension(file).ToLowerInvariant()).ToString("x8");
                    }
                    if (All.Any(x => x.Id == t.Id)) { Diagnostics.Log($"theme {t.Id} in {file} is a duplicate; skipped"); continue; }
                    All.Add(t);
                }
                catch (Exception e) { Diagnostics.Log($"theme {file}: {e.Message}"); }
            }
        }
        // Built-ins in a fixed order, the user's own after them.
        string[] order = ["default", "retrowave", "neon-noir", "liminal", "brutalist", "retro-64", "classic-skin", "frosted-glass", "stealth", "analog-70s"];
        var sorted = All.OrderBy(t => t.BuiltIn ? 0 : 1).ThenBy(t => Array.IndexOf(order, t.Id) is int i && i >= 0 ? i : 99).ThenBy(t => t.Name).ToList();
        All.Clear();
        All.AddRange(sorted);
    }

    public Theme Find(string? id) => All.FirstOrDefault(t => t.Id == id) ?? All[0];

    /// <summary>The theme in effect: the chosen one, or Default under High Contrast.</summary>
    Theme Chosen() => HighContrast ? All[0] : Find(settings.ThemeId);

    /// <summary>Base light/dark for a theme: its own, or App Theme for Default.</summary>
    public ElementTheme ElementTheme(Theme t) => t.Base switch
    {
        "dark" => Microsoft.UI.Xaml.ElementTheme.Dark,
        "light" => Microsoft.UI.Xaml.ElementTheme.Light,
        _ => settings.Appearance.ElementTheme(),
    };

    // MARK: launch

    /// <summary>Called before any window exists: everything, including what WinUI only reads once.</summary>
    public void ApplyAtLaunch()
    {
        var t = Chosen();
        Launched = t;
        launchSignature = Signature(t);
        var res = Application.Current.Resources;

        // Styles can still be changed until something uses them, which nothing has yet. The app's own styles
        // (App.xaml) keep their {ThemeResource} colours; only setters are added.
        void Add(string key, params Setter[] setters)
        {
            if (res[key] is not Style s || s.IsSealed) { Diagnostics.Log($"theme: style {key} is sealed"); return; }
            foreach (var x in setters) s.Setters.Add(x);
        }
        if (t.Fonts.Body is { } body)
        {
            var family = Font(body);
            res["ContentControlThemeFontFamily"] = family;
            res[typeof(TextBlock)] = new Style(typeof(TextBlock)) { Setters = { new Setter(TextBlock.FontFamilyProperty, family) } };
            // WinUI's text styles live in its own dictionary; an app-level style of the same name wins for the pages.
            foreach (var key in new[] { "CaptionTextBlockStyle", "BodyTextBlockStyle", "BodyStrongTextBlockStyle", "SubtitleTextBlockStyle", "TitleTextBlockStyle" })
                if (res[key] is Style s) res[key] = new Style(typeof(TextBlock)) { BasedOn = s, Setters = { new Setter(TextBlock.FontFamilyProperty, family) } };
            foreach (var key in new[] { "SecondaryTextStyle", "CaptionSecondaryStyle", "SectionTitleStyle", "PageTitleStyle" })
                Add(key, new Setter(TextBlock.FontFamilyProperty, family));
        }
        var title = new List<Setter>();
        if (t.Fonts.Title is { } tf) title.Add(new(TextBlock.FontFamilyProperty, Font(tf)));
        if (t.Fonts.TitleSize is { } ts) title.Add(new(TextBlock.FontSizeProperty, ts));
        if (t.Fonts.TitleWeight is { } tw) title.Add(new(TextBlock.FontWeightProperty, Weight(tw)));
        if (t.Fonts.TitleSpacing is { } sp) title.Add(new(TextBlock.CharacterSpacingProperty, sp));
        if (title.Count > 0) Add("PageTitleStyle", [.. title]);

        if (t.Corners.Control is { } cc) res["ControlCornerRadius"] = new CornerRadius(cc);
        if (t.Corners.Overlay is { } oc)
        {
            res["OverlayCornerRadius"] = new CornerRadius(oc);
            res["NavigationViewContentGridCornerRadius"] = new CornerRadius(oc, 0, 0, 0);   // the content area's top-left corner
        }
        res["ShortwaveCardRadius"] = new CornerRadius(t.Corners.Card ?? 8);

        Current = t;
        ApplyColors(t);
        Diagnostics.Log($"theme at launch {t.Id}{(HighContrast ? " (High Contrast: Default)" : "")}");
    }

    /// <summary>Card-like corner radius for code-built UI (settings and dialog cards).</summary>
    public static CornerRadius CardRadius => Application.Current.Resources.TryGetValue("ShortwaveCardRadius", out var r) && r is CornerRadius c ? c : new CornerRadius(8);
    /// <summary>Cover corner radius as a fraction of the cover size.</summary>
    public double CoverRadius => Launched.Corners.Cover ?? 0.18;
    /// <summary>Corner radius of station cards' hover fill.</summary>
    public double StationCardRadius => Launched.Corners.Card is { } c ? Math.Min(c * 1.75, 14) : 14;

    // MARK: live

    /// <summary>Switches to a theme (also used after editing the current one).</summary>
    public void Apply(Theme t)
    {
        Current = t;
        ApplyColors(t);
        Diagnostics.Log($"theme {t.Id}{(RestartNeeded ? " (restart to finish)" : "")}");
        Changed?.Invoke();
    }

    public void ApplyChosen() => Apply(Chosen());

    /// <summary>
    /// Light or dark, title bar buttons, backdrop and background colour for one window. <paramref name="defaultBackdrop"/>
    /// is the window's own backdrop under the Default theme (mica for the main window, acrylic for the mini player).
    /// </summary>
    public static void ApplyWindow(Window w, Theme t, ElementTheme look, Microsoft.UI.Xaml.Controls.Border fill, string defaultBackdrop)
    {
        if (w.Content is FrameworkElement root) root.RequestedTheme = look;
        try
        {
            w.AppWindow.TitleBar.PreferredTheme = look switch
            {
                Microsoft.UI.Xaml.ElementTheme.Dark => Microsoft.UI.Windowing.TitleBarTheme.Dark,
                Microsoft.UI.Xaml.ElementTheme.Light => Microsoft.UI.Windowing.TitleBarTheme.Light,
                _ => Microsoft.UI.Windowing.TitleBarTheme.UseDefaultAppMode,
            };
        }
        catch (Exception e) { Diagnostics.Log("title bar theme: " + e.Message); }
        string backdrop = t.IsDefault ? defaultBackdrop : t.Backdrop;
        switch (backdrop)
        {
            case "mica" when w.SystemBackdrop is not Microsoft.UI.Xaml.Media.MicaBackdrop: w.SystemBackdrop = new Microsoft.UI.Xaml.Media.MicaBackdrop(); break;
            case "acrylic" when w.SystemBackdrop is not Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop: w.SystemBackdrop = new Microsoft.UI.Xaml.Media.DesktopAcrylicBackdrop(); break;
            case "none": w.SystemBackdrop = null; break;
        }
        // Without a backdrop the window needs an opaque background; over one, the colour may tint it.
        fill.Background = t.IsDefault || t.Get("background") is not { } bg ? null
            : new SolidColorBrush(backdrop == "none" ? Theme.WithAlpha(bg, 1) : bg);
    }

    void ApplyColors(Theme t)
    {
        var dicts = ThemeDictionaries();
        // Put back everything a previous theme changed, then recolour the base dictionary.
        foreach (var key in touched.ToList())
        {
            if (!originals.TryGetValue(key, out var original)) continue;
            if (Restore(dicts[key.Dict], key.Key, original)) touched.Remove(key);
        }
        if (t.IsDefault) return;
        string dictName = t.IsDark ? "Default" : "Light";
        var dict = dicts[dictName];
        foreach (var (key, color) in BrushColors(t))
        {
            if (!dict.TryGetValue(key, out var brush)) continue;
            originals.TryAdd((dictName, key), Snapshot(brush));
            if (SetColor(brush, color)) touched.Add((dictName, key));
        }
    }

    Dictionary<string, ResourceDictionary> ThemeDictionaries()
    {
        var xcr = Application.Current.Resources.MergedDictionaries.First(d => d.ThemeDictionaries.ContainsKey("Default"));
        return new() { ["Default"] = (ResourceDictionary)xcr.ThemeDictionaries["Default"], ["Light"] = (ResourceDictionary)xcr.ThemeDictionaries["Light"] };
    }

    /// <summary>The WinUI brushes each colour token drives.</summary>
    static IEnumerable<(string Key, Color Color)> BrushColors(Theme t)
    {
        static Color A(Color c, double f) => Theme.WithAlpha(c, c.A / 255.0 * f);
        if (t.Get("text") is { } text) yield return ("TextFillColorPrimaryBrush", text);
        if (t.Get("textSecondary") is { } ts) yield return ("TextFillColorSecondaryBrush", ts);
        if (t.Get("textTertiary") is { } tt)
        {
            yield return ("TextFillColorTertiaryBrush", tt);
            yield return ("TextFillColorDisabledBrush", A(tt, 0.75));
            yield return ("ControlStrongStrokeColorDefaultBrush", tt);
            yield return ("ControlStrongFillColorDefaultBrush", t.Get("textSecondary") ?? tt);
        }
        if (t.Get("accent") is { } ac)
        {
            yield return ("AccentFillColorDefaultBrush", ac);
            yield return ("AccentFillColorSecondaryBrush", ac);
            yield return ("AccentFillColorTertiaryBrush", ac);
            yield return ("AccentControlElevationBorderBrush", Theme.Mix(ac, Microsoft.UI.Colors.Black, 0.25));
        }
        if (t.Get("accentText") is { } at)
        {
            yield return ("AccentTextFillColorPrimaryBrush", at);
            yield return ("AccentTextFillColorSecondaryBrush", at);
            yield return ("AccentTextFillColorTertiaryBrush", A(at, 0.8));
        }
        if (t.Get("onAccent") is { } oa)
        {
            yield return ("TextOnAccentFillColorPrimaryBrush", oa);
            yield return ("TextOnAccentFillColorSecondaryBrush", A(oa, 0.7));
        }
        if (t.Get("control") is { } co)
        {
            var toward = t.Get("text") ?? (t.IsDark ? Microsoft.UI.Colors.White : Microsoft.UI.Colors.Black);
            yield return ("ControlFillColorDefaultBrush", co);
            yield return ("ControlFillColorSecondaryBrush", Theme.Mix(co, Theme.WithAlpha(toward, co.A / 255.0), 0.08));
            yield return ("ControlFillColorTertiaryBrush", A(co, 0.6));
            yield return ("ControlFillColorInputActiveBrush", t.Get("dialog") ?? co);
            yield return ("ControlAltFillColorSecondaryBrush", co);
        }
        if (t.Get("controlStroke") is { } cs)
        {
            yield return ("ControlStrokeColorDefaultBrush", cs);
            yield return ("ControlStrokeColorSecondaryBrush", cs);
            yield return ("ControlElevationBorderBrush", cs);
            yield return ("TextControlElevationBorderBrush", cs);
        }
        if (t.Get("card") is { } card)
        {
            yield return ("CardBackgroundFillColorDefaultBrush", card);
            yield return ("CardBackgroundFillColorSecondaryBrush", card);
        }
        if (t.Get("cardStroke") is { } cst)
        {
            yield return ("CardStrokeColorDefaultBrush", cst);
            yield return ("SurfaceStrokeColorDefaultBrush", cst);
            yield return ("SurfaceStrokeColorFlyoutBrush", cst);
        }
        if (t.Get("subtle") is { } su)
        {
            yield return ("SubtleFillColorSecondaryBrush", su);
            yield return ("SubtleFillColorTertiaryBrush", A(su, 0.6));
        }
        if (t.Get("divider") is { } dv) yield return ("DividerStrokeColorDefaultBrush", dv);
        if (t.Get("surface") is { } sf) yield return ("LayerFillColorDefaultBrush", sf);
        if (t.Get("dialog") is { } dl)
        {
            yield return ("SolidBackgroundFillColorBaseBrush", dl);
            yield return ("LayerFillColorAltBrush", t.Get("card") is { } c2 ? Theme.Mix(dl, Theme.WithAlpha(c2, 1), 0.5) : dl);
            yield return ("AcrylicInAppFillColorDefaultBrush", dl);
            yield return ("AcrylicBackgroundFillColorDefaultBrush", dl);
        }
        if (t.Get("background") is { } bg)
        {
            yield return ("SolidBackgroundFillColorSecondaryBrush", Theme.WithAlpha(bg, 1));
            yield return ("SolidBackgroundFillColorTertiaryBrush", Theme.WithAlpha(bg, 1));
        }
    }

    static object Snapshot(object brush) => brush switch
    {
        SolidColorBrush s => s.Color,
        AcrylicBrush a => (a.TintColor, a.FallbackColor, a.TintOpacity),
        GradientBrush g => g.GradientStops.Select(x => x.Color).ToArray(),
        _ => brush,
    };

    static bool SetColor(object brush, Color c)
    {
        try
        {
            switch (brush)
            {
                case SolidColorBrush s: s.Color = c; return true;
                case AcrylicBrush a: a.TintColor = c; a.FallbackColor = c; a.TintOpacity = 0.85; return true;
                case GradientBrush g: foreach (var stop in g.GradientStops) stop.Color = c; return true;
            }
        }
        catch (Exception) { }   // the primary text brush can't change once text has been drawn: a restart applies it
        return false;
    }

    static bool Restore(ResourceDictionary dict, string key, object original)
    {
        try
        {
            var brush = dict[key];
            switch (brush, original)
            {
                case (SolidColorBrush s, Color c): s.Color = c; return true;
                case (AcrylicBrush a, ValueTuple<Color, Color, double> o): a.TintColor = o.Item1; a.FallbackColor = o.Item2; a.TintOpacity = o.Item3; return true;
                case (GradientBrush g, Color[] stops):
                    for (int i = 0; i < Math.Min(stops.Length, g.GradientStops.Count); i++) g.GradientStops[i].Color = stops[i];
                    return true;
            }
        }
        catch (Exception) { }
        return false;
    }

    /// <summary>What can only be applied at launch.</summary>
    static string Signature(Theme t) =>
        $"{t.Id == Theme.DefaultId}|{t.Fonts}|{t.Corners}|{(t.Get("text") is { } c ? Theme.ColorText(c) : "")}|{(t.IsDefault ? "" : t.Base)}";

    // MARK: fonts

    /// <summary>Fonts that ship with the app (SIL Open Font License), by family name.</summary>
    public static readonly Dictionary<string, string> BundledFonts = new(StringComparer.OrdinalIgnoreCase)
    {
        ["Orbitron"] = "Orbitron.ttf",
        ["Press Start 2P"] = "PressStart2P-Regular.ttf",
        ["VT323"] = "VT323-Regular.ttf",
        ["Share Tech Mono"] = "ShareTechMono-Regular.ttf",
        ["Space Mono"] = "SpaceMono-Regular.ttf",
        ["Shrikhand"] = "Shrikhand-Regular.ttf",
    };

    /// <summary>Fonts offered in the editor: the bundled ones and common Windows fonts (any installed family can be typed).</summary>
    public static readonly string[] SuggestedFonts =
    [
        "Segoe UI Variable", "Segoe UI Variable Display", "Segoe UI", "Bahnschrift", "Consolas", "Cascadia Mono", "Cascadia Code",
        "Tahoma", "Verdana", "Arial", "Arial Black", "Trebuchet MS", "Georgia", "Times New Roman", "Courier New", "Lucida Console",
        "Impact", "Franklin Gothic Medium", "Comic Sans MS", "Ink Free", "Gabriola",
        "Orbitron", "Press Start 2P", "VT323", "Share Tech Mono", "Space Mono", "Shrikhand",
    ];

    public static FontFamily Font(string name)
    {
        if (BundledFonts.TryGetValue(name, out var file))
        {
            // ms-appx resolves to the app folder, also for this unpackaged app (file:/// URIs aren't accepted).
            if (File.Exists(Path.Combine(AppContext.BaseDirectory, "Assets", "Fonts", file))) return new FontFamily($"ms-appx:///Assets/Fonts/{file}#{name}");
        }
        return new FontFamily(name);
    }

    static Windows.UI.Text.FontWeight Weight(string w) => w.ToLowerInvariant() switch
    {
        "thin" => Microsoft.UI.Text.FontWeights.Thin,
        "light" => Microsoft.UI.Text.FontWeights.Light,
        "semilight" => Microsoft.UI.Text.FontWeights.SemiLight,
        "normal" or "regular" => Microsoft.UI.Text.FontWeights.Normal,
        "medium" => Microsoft.UI.Text.FontWeights.Medium,
        "bold" => Microsoft.UI.Text.FontWeights.Bold,
        "black" => Microsoft.UI.Text.FontWeights.Black,
        _ => Microsoft.UI.Text.FontWeights.SemiBold,
    };

    // MARK: the user's themes

    public string UserFolder => userFolder;

    public void Save(Theme t)
    {
        if (t.BuiltIn) return;
        Directory.CreateDirectory(userFolder);
        if (!Theme.IsValidId(t.Id)) t.Id = Theme.NewCustomId();
        string path = t.SourcePath ?? Path.Combine(userFolder, t.Id + ".json");
        if (!InUserFolder(path)) throw new IOException("Themes are only saved in the Themes folder.");
        t.SourcePath = path;
        string tmp = path + ".tmp";
        File.WriteAllText(tmp, t.ToJson());
        File.Move(tmp, path, overwrite: true);
    }

    /// <summary>A copy of a theme the user can edit.</summary>
    public Theme Duplicate(Theme source)
    {
        var t = source.IsDefault ? DefaultAsEditable() : source.Clone();
        t.Id = Theme.NewCustomId();
        t.SourcePath = null;
        string baseName = source.IsDefault ? "My Theme" : source.Name;
        string name = baseName + (source.IsDefault ? "" : " (custom)");
        for (int i = 2; All.Any(x => x.Name == name); i++) name = $"{baseName} ({(source.IsDefault ? "" : "custom ")}{i})".Replace("( ", "(");
        t.Name = name;
        t.BuiltIn = false;
        Save(t);
        Reload();
        return Find(t.Id);
    }

    /// <summary>The Default look written out as colours, so it can be edited.</summary>
    Theme DefaultAsEditable()
    {
        bool dark = settings.Appearance switch
        {
            AppTheme.Dark => true,
            AppTheme.Light => false,
            _ => Application.Current.RequestedTheme == ApplicationTheme.Dark,
        };
        var dicts = ThemeDictionaries();
        var dict = dicts[dark ? "Default" : "Light"];
        Color C(string key, Color fallback)
        {
            var o = originals.TryGetValue((dark ? "Default" : "Light", key), out var orig) ? orig : (dict.TryGetValue(key, out var b) ? Snapshot(b) : null);
            return o is Color c ? c : o is ValueTuple<Color, Color, double> a ? a.Item2 : fallback;
        }
        var t = new Theme { Base = dark ? "dark" : "light", Backdrop = "mica" };
        var map = new (string Token, string Key)[]
        {
            ("surface", "LayerFillColorDefaultBrush"), ("card", "CardBackgroundFillColorDefaultBrush"), ("cardStroke", "CardStrokeColorDefaultBrush"),
            ("control", "ControlFillColorDefaultBrush"), ("controlStroke", "ControlStrokeColorDefaultBrush"), ("subtle", "SubtleFillColorSecondaryBrush"),
            ("divider", "DividerStrokeColorDefaultBrush"), ("text", "TextFillColorPrimaryBrush"), ("textSecondary", "TextFillColorSecondaryBrush"),
            ("textTertiary", "TextFillColorTertiaryBrush"), ("accent", "AccentFillColorDefaultBrush"), ("accentText", "AccentTextFillColorPrimaryBrush"),
            ("onAccent", "TextOnAccentFillColorPrimaryBrush"), ("dialog", "SolidBackgroundFillColorBaseBrush"),
        };
        foreach (var (token, key) in map) t.Colors[token] = C(key, Microsoft.UI.Colors.Gray);
        t.Colors["background"] = dark ? Color.FromArgb(255, 0x20, 0x20, 0x20) : Color.FromArgb(255, 0xF3, 0xF3, 0xF3);
        return t;
    }

    public void Delete(Theme t)
    {
        if (t.BuiltIn || t.SourcePath is not { } path || !InUserFolder(path)) return;
        try { File.Delete(path); } catch (Exception e) { Diagnostics.Log("theme delete: " + e.Message); }
        Reload();
    }

    /// <summary>Adds a theme file as one of the user's themes (a new id if it clashes). Returns it.</summary>
    public Theme Import(string path)
    {
        var t = Theme.FromJson(File.ReadAllText(path));
        // The id becomes a file name: anything unusual (or taken) gets a fresh one.
        if (!Theme.IsValidId(t.Id) || All.Any(x => x.Id == t.Id)) t.Id = Theme.NewCustomId();
        t.SourcePath = null;
        if (All.Any(x => x.Name == t.Name)) t.Name += " (imported)";
        t.BuiltIn = false;
        Save(t);
        Reload();
        return Find(t.Id);
    }

    /// <summary>FNV-1a: the same file name gives the same id on every run (string.GetHashCode doesn't).</summary>
    static uint StableHash(string s) { uint h = 2166136261; foreach (char c in s) h = (h ^ c) * 16777619; return h; }

    bool InUserFolder(string path)
    {
        string folder = Path.GetFullPath(userFolder).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        string full = Path.GetFullPath(path);
        return full.StartsWith(folder, StringComparison.OrdinalIgnoreCase) && Path.GetDirectoryName(full)!.TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar == folder;
    }

    public static void Export(Theme t, string path)
    {
        var copy = t.Clone();
        if (copy.IsDefault) return;
        File.WriteAllText(path, copy.ToJson());
    }
}
