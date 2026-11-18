using System.Globalization;
using System.Text.Json;
using System.Text.Json.Nodes;
using Windows.UI;

namespace Shortwave.Theming;

/// <summary>
/// A look for the whole app: colours, fonts, corner shapes, backdrop and decorative effects. Stored as a
/// <c>shortwave-theme</c> JSON file (format in docs/THEMES.md). Built-in themes ship in the app's Themes
/// folder; the user's own live in the data folder's Themes folder.
/// </summary>
public sealed class Theme
{
    public const string Format = "shortwave-theme";
    public const string DefaultId = "default";

    public string Id { get; set; } = DefaultId;
    public string Name { get; set; } = "Default";
    public string Description { get; set; } = "";
    /// <summary>dark, light, or system (the Default theme: follows App Theme).</summary>
    public string Base { get; set; } = "system";
    /// <summary>mica, acrylic or none (a solid background colour).</summary>
    public string Backdrop { get; set; } = "mica";
    public bool BuiltIn { get; set; }
    /// <summary>Colour tokens by name (see <see cref="Tokens"/>); missing ones keep WinUI's value.</summary>
    public Dictionary<string, Color> Colors { get; set; } = [];
    public ThemeFonts Fonts { get; set; } = new();
    public ThemeCorners Corners { get; set; } = new();
    /// <summary>Effect name to its options (see <see cref="Effects"/>).</summary>
    public Dictionary<string, JsonObject> Effects { get; set; } = [];

    public bool IsDefault => Id == DefaultId;
    /// <summary>The file a user theme was loaded from (save and delete use it; never written to the JSON).</summary>
    public string? SourcePath { get; set; }

    /// <summary>Ids are lower-case letters, digits and hyphens: they become file names.</summary>
    public static bool IsValidId(string? id) =>
        !string.IsNullOrEmpty(id) && id.Length <= 64 && id.All(c => c is >= 'a' and <= 'z' or >= '0' and <= '9' or '-');
    public static string NewCustomId() => "custom-" + Guid.NewGuid().ToString("N")[..8];
    public bool IsDark => Base == "dark";

    /// <summary>The colour tokens a theme can set, with the label the editor shows.</summary>
    public static readonly (string Key, string Label)[] Tokens =
    [
        ("background", "Window background"),
        ("surface", "Content area"),
        ("card", "Cards"),
        ("cardStroke", "Card borders"),
        ("control", "Buttons and fields"),
        ("controlStroke", "Button borders"),
        ("subtle", "Hover and selection"),
        ("divider", "Dividers"),
        ("text", "Text"),
        ("textSecondary", "Secondary text"),
        ("textTertiary", "Faint text and icons"),
        ("accent", "Accent"),
        ("accentText", "Accent text and links"),
        ("onAccent", "Text on accent"),
        ("dialog", "Dialogs and menus"),
    ];

    /// <summary>The decorative effects, with the label the editor shows.</summary>
    public static readonly (string Key, string Label)[] EffectKinds =
    [
        ("synthwave", "Synthwave sun and grid"),
        ("rain", "Neon rain"),
        ("aurora", "Drifting colour behind glass"),
        ("wood", "Walnut wood grain"),
        ("concrete", "Concrete texture"),
        ("grain", "Film grain"),
        ("vignette", "Vignette"),
        ("scanlines", "Scanlines"),
        ("frame", "Screen border"),
        ("titleShadow", "Title glow or shadow"),
        ("cursor", "Block cursor after titles"),
        ("spotlight", "Spotlight that follows the pointer"),
        ("lcd", "LCD track display"),
        ("analyzer", "Spectrum bars (decorative)"),
        ("dial", "Tuning dial"),
        ("vu", "VU meter (decorative)"),
    ];

    public Color? Get(string token) => Colors.TryGetValue(token, out var c) ? c : null;
    public Color Get(string token, Color fallback) => Colors.TryGetValue(token, out var c) ? c : fallback;
    public bool Has(string effect) => Effects.ContainsKey(effect);
    public JsonObject Options(string effect) => Effects.TryGetValue(effect, out var o) ? o : [];

    public Theme Clone()
    {
        var t = (Theme)MemberwiseClone();
        t.Colors = new(Colors);
        t.Fonts = Fonts with { };
        t.Corners = Corners with { };
        t.Effects = Effects.ToDictionary(e => e.Key, e => (JsonObject)e.Value.DeepClone());
        return t;
    }

    // MARK: JSON

    public static Theme FromJson(string json, bool builtIn = false)
    {
        var root = JsonNode.Parse(json) as JsonObject ?? throw new FormatException("This file isn’t a Shortwave theme.");
        if ((string?)root["format"] is { } f && f != Format) throw new FormatException("This file isn’t a Shortwave theme.");
        var t = new Theme
        {
            Id = Str(root, "id") ?? throw new FormatException("The theme has no id."),
            Name = Str(root, "name") ?? "Untitled",
            Description = Str(root, "description") ?? "",
            Base = Str(root, "base") is "light" ? "light" : Str(root, "base") is "system" ? "system" : "dark",
            Backdrop = Str(root, "backdrop") is "acrylic" or "none" or "mica" ? Str(root, "backdrop")! : "mica",
            BuiltIn = builtIn,
        };
        if (root["colors"] is JsonObject colors)
            foreach (var (k, v) in colors)
                if (v is JsonValue jv && jv.TryGetValue(out string? s) && ParseColor(s) is { } c) t.Colors[k] = c;
        if (root["fonts"] is JsonObject fo)
        {
            t.Fonts = new ThemeFonts
            {
                Body = Str(fo, "body"),
                Title = Str(fo, "title"),
                TitleSize = Num(fo, "titleSize"),
                TitleWeight = Str(fo, "titleWeight"),
                TitleUppercase = fo["titleUppercase"] is JsonValue u && u.TryGetValue(out bool b) && b,
                TitleSpacing = (int?)Num(fo, "titleSpacing"),
                Lcd = Str(fo, "lcd"),
            };
        }
        if (root["corners"] is JsonObject co)
            t.Corners = new ThemeCorners { Control = Num(co, "control"), Overlay = Num(co, "overlay"), Card = Num(co, "card"), Cover = Num(co, "cover") };
        if (root["effects"] is JsonObject ef)
            foreach (var (k, v) in ef)
                t.Effects[k] = v as JsonObject ?? [];
        return t;
    }

    public string ToJson()
    {
        var root = new JsonObject
        {
            ["format"] = Format,
            ["version"] = 1,
            ["id"] = Id,
            ["name"] = Name,
            ["description"] = Description,
            ["base"] = Base,
            ["backdrop"] = Backdrop,
        };
        var colors = new JsonObject();
        foreach (var (key, _) in Tokens) if (Colors.TryGetValue(key, out var c)) colors[key] = ColorText(c);
        root["colors"] = colors;
        var fo = new JsonObject();
        if (Fonts.Body != null) fo["body"] = Fonts.Body;
        if (Fonts.Title != null) fo["title"] = Fonts.Title;
        if (Fonts.TitleSize != null) fo["titleSize"] = Fonts.TitleSize;
        if (Fonts.TitleWeight != null) fo["titleWeight"] = Fonts.TitleWeight;
        if (Fonts.TitleUppercase) fo["titleUppercase"] = true;
        if (Fonts.TitleSpacing != null) fo["titleSpacing"] = Fonts.TitleSpacing;
        if (Fonts.Lcd != null) fo["lcd"] = Fonts.Lcd;
        root["fonts"] = fo;
        var co = new JsonObject();
        if (Corners.Control != null) co["control"] = Corners.Control;
        if (Corners.Overlay != null) co["overlay"] = Corners.Overlay;
        if (Corners.Card != null) co["card"] = Corners.Card;
        if (Corners.Cover != null) co["cover"] = Corners.Cover;
        root["corners"] = co;
        var ef = new JsonObject();
        foreach (var (key, _) in EffectKinds) if (Effects.TryGetValue(key, out var o)) ef[key] = o.DeepClone();
        root["effects"] = ef;
        return root.ToJsonString(new JsonSerializerOptions { WriteIndented = true, Encoder = System.Text.Encodings.Web.JavaScriptEncoder.UnsafeRelaxedJsonEscaping });
    }

    static string? Str(JsonObject o, string k) => o[k] is JsonValue v && v.TryGetValue(out string? s) && !string.IsNullOrWhiteSpace(s) ? s : null;
    static double? Num(JsonObject o, string k) => o[k] is JsonValue v && v.TryGetValue(out double d) ? d : null;

    /// <summary>#RGB, #RRGGBB or #AARRGGBB.</summary>
    public static Color? ParseColor(string? s)
    {
        if (string.IsNullOrWhiteSpace(s)) return null;
        s = s.Trim().TrimStart('#');
        if (s.Length == 3) s = string.Concat(s.Select(ch => $"{ch}{ch}"));
        if (s.Length == 6) s = "FF" + s;
        if (s.Length != 8 || !uint.TryParse(s, NumberStyles.HexNumber, CultureInfo.InvariantCulture, out uint v)) return null;
        return Color.FromArgb((byte)(v >> 24), (byte)(v >> 16), (byte)(v >> 8), (byte)v);
    }

    public static string ColorText(Color c) => c.A == 255 ? $"#{c.R:X2}{c.G:X2}{c.B:X2}" : $"#{c.A:X2}{c.R:X2}{c.G:X2}{c.B:X2}";

    // MARK: colour helpers

    public static Color WithAlpha(Color c, double alpha) => Color.FromArgb((byte)Math.Clamp(Math.Round(alpha * 255), 0, 255), c.R, c.G, c.B);
    public static Color Mix(Color a, Color b, double t) => Color.FromArgb(
        (byte)Math.Round(a.A + (b.A - a.A) * t), (byte)Math.Round(a.R + (b.R - a.R) * t),
        (byte)Math.Round(a.G + (b.G - a.G) * t), (byte)Math.Round(a.B + (b.B - a.B) * t));
}

public sealed record ThemeFonts
{
    /// <summary>Font for all text and controls; null keeps Segoe UI Variable.</summary>
    public string? Body { get; init; }
    /// <summary>Font for page titles; null uses the body font.</summary>
    public string? Title { get; init; }
    public double? TitleSize { get; init; }
    public string? TitleWeight { get; init; }
    public bool TitleUppercase { get; init; }
    /// <summary>Letter spacing in thousandths of an em (XAML CharacterSpacing).</summary>
    public int? TitleSpacing { get; init; }
    /// <summary>Font for the LCD track display effect.</summary>
    public string? Lcd { get; init; }
}

public sealed record ThemeCorners
{
    public double? Control { get; init; }
    public double? Overlay { get; init; }
    /// <summary>Station cards' hover shape, settings cards and the recording card.</summary>
    public double? Card { get; init; }
    /// <summary>Cover corner radius as a fraction of the cover size (0.18 by default).</summary>
    public double? Cover { get; init; }
}
