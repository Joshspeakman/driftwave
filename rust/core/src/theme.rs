//! A look for the whole app: colours, fonts, corner shapes, backdrop and decorative effects, stored as a
//! `shortwave-theme` JSON file (format in docs/THEMES.md). Port of the model in `Shortwave/Theming/Theme.cs`;
//! applying a theme to the UI belongs to the app.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "shortwave-theme";
pub const DEFAULT_ID: &str = "default";

/// An sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { a: 255, r, g, b }
    }

    /// `#RGB`, `#RRGGBB` or `#AARRGGBB` (the `#` is optional).
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim().trim_start_matches('#');
        let hex: String = match s.len() {
            3 => s.chars().flat_map(|c| [c, c]).collect::<String>().chars().collect::<String>(),
            _ => s.to_owned(),
        };
        let hex = if hex.len() == 6 { format!("FF{hex}") } else { hex };
        if hex.len() != 8 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(&hex, 16).ok()?;
        Some(Color { a: (v >> 24) as u8, r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 })
    }

    /// `#RRGGBB` when opaque, otherwise `#AARRGGBB`.
    pub fn text(self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.a, self.r, self.g, self.b)
        }
    }

    pub fn with_alpha(self, alpha: f64) -> Color {
        Color { a: (alpha * 255.0).round().clamp(0.0, 255.0) as u8, ..self }
    }

    pub fn mix(self, other: Color, t: f64) -> Color {
        let m = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
        Color { a: m(self.a, other.a), r: m(self.r, other.r), g: m(self.g, other.g), b: m(self.b, other.b) }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeFonts {
    /// Font for all text and controls; `None` keeps the platform font.
    pub body: Option<String>,
    /// Font for page titles; `None` uses the body font.
    pub title: Option<String>,
    pub title_size: Option<f64>,
    pub title_weight: Option<String>,
    pub title_uppercase: bool,
    /// Letter spacing in thousandths of an em.
    pub title_spacing: Option<i32>,
    /// Font for the LCD track display effect.
    pub lcd: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThemeCorners {
    pub control: Option<f64>,
    pub overlay: Option<f64>,
    /// Station cards' hover shape, settings cards and the recording card.
    pub card: Option<f64>,
    /// Cover corner radius as a fraction of the cover size (0.18 by default).
    pub cover: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub description: String,
    /// dark, light, or system (the Default theme: follows the app's appearance setting).
    pub base: String,
    /// mica, acrylic or none (a solid background colour).
    pub backdrop: String,
    pub built_in: bool,
    /// Colour tokens by name (see [`TOKENS`]).
    pub colors: BTreeMap<String, Color>,
    pub fonts: ThemeFonts,
    pub corners: ThemeCorners,
    /// Effect name to its options (see [`EFFECT_KINDS`]).
    pub effects: BTreeMap<String, Map<String, Value>>,
    /// The file a user theme was loaded from (save and delete use it; never written to the JSON).
    pub source_path: Option<PathBuf>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            id: DEFAULT_ID.into(),
            name: "Default".into(),
            description: String::new(),
            base: "system".into(),
            backdrop: "mica".into(),
            built_in: false,
            colors: BTreeMap::new(),
            fonts: ThemeFonts::default(),
            corners: ThemeCorners::default(),
            effects: BTreeMap::new(),
            source_path: None,
        }
    }
}

/// The colour tokens a theme can set, with the label the editor shows.
pub const TOKENS: [(&str, &str); 15] = [
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

/// The decorative effects, with the label the editor shows. None of them may flash (docs/THEMES.md).
pub const EFFECT_KINDS: [(&str, &str); 16] = [
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
    ("analyzer", "Spectrum bars"),
    ("dial", "Tuning dial"),
    ("vu", "VU meter"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

impl std::fmt::Display for ThemeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ThemeError {}

/// Ids are lower-case letters, digits and hyphens: they become file names.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `custom-` and eight hex digits.
pub fn new_custom_id() -> String {
    format!("custom-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
}

/// A non-blank string value.
fn str_val(o: &Map<String, Value>, k: &str) -> Option<String> {
    o.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_owned)
}

fn num_val(o: &Map<String, Value>, k: &str) -> Option<f64> {
    o.get(k).and_then(Value::as_f64)
}

fn num_json(d: f64) -> Value {
    if d.fract() == 0.0 && d.abs() < 9.0e15 {
        Value::from(d as i64)
    } else {
        serde_json::Number::from_f64(d).map(Value::Number).unwrap_or(Value::Null)
    }
}

impl Theme {
    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_ID
    }

    pub fn is_dark(&self) -> bool {
        self.base == "dark"
    }

    pub fn color(&self, token: &str) -> Option<Color> {
        self.colors.get(token).copied()
    }

    pub fn has(&self, effect: &str) -> bool {
        self.effects.contains_key(effect)
    }

    pub fn options(&self, effect: &str) -> Map<String, Value> {
        self.effects.get(effect).cloned().unwrap_or_default()
    }

    pub fn from_json(json: &str, built_in: bool) -> Result<Theme, ThemeError> {
        let not_theme = || ThemeError("This file isn’t a Driftwave theme.".into());
        let root = match serde_json::from_str::<Value>(json) {
            Ok(Value::Object(o)) => o,
            _ => return Err(not_theme()),
        };
        if let Some(f) = root.get("format").and_then(Value::as_str) {
            if f != FORMAT {
                return Err(not_theme());
            }
        }
        let base = match str_val(&root, "base").as_deref() {
            Some("light") => "light",
            Some("system") => "system",
            _ => "dark",
        };
        let backdrop = match str_val(&root, "backdrop").as_deref() {
            Some(b @ ("acrylic" | "none" | "mica")) => b.to_owned(),
            _ => "mica".into(),
        };
        let mut t = Theme {
            id: str_val(&root, "id").ok_or_else(|| ThemeError("The theme has no id.".into()))?,
            name: str_val(&root, "name").unwrap_or_else(|| "Untitled".into()),
            description: str_val(&root, "description").unwrap_or_default(),
            base: base.into(),
            backdrop,
            built_in,
            ..Theme::default()
        };
        if let Some(Value::Object(colors)) = root.get("colors") {
            for (k, v) in colors {
                if let Some(c) = v.as_str().and_then(Color::parse) {
                    t.colors.insert(k.clone(), c);
                }
            }
        }
        if let Some(Value::Object(fo)) = root.get("fonts") {
            t.fonts = ThemeFonts {
                body: str_val(fo, "body"),
                title: str_val(fo, "title"),
                title_size: num_val(fo, "titleSize"),
                title_weight: str_val(fo, "titleWeight"),
                title_uppercase: fo.get("titleUppercase").and_then(Value::as_bool).unwrap_or(false),
                title_spacing: num_val(fo, "titleSpacing").map(|d| d as i32),
                lcd: str_val(fo, "lcd"),
            };
        }
        if let Some(Value::Object(co)) = root.get("corners") {
            t.corners = ThemeCorners {
                control: num_val(co, "control"),
                overlay: num_val(co, "overlay"),
                card: num_val(co, "card"),
                cover: num_val(co, "cover"),
            };
        }
        if let Some(Value::Object(ef)) = root.get("effects") {
            for (k, v) in ef {
                t.effects.insert(k.clone(), v.as_object().cloned().unwrap_or_default());
            }
        }
        Ok(t)
    }

    /// The theme file, keys in the documented order; known tokens and effects only.
    pub fn to_json(&self) -> String {
        let mut root = Map::new();
        root.insert("format".into(), FORMAT.into());
        root.insert("version".into(), 1.into());
        root.insert("id".into(), self.id.clone().into());
        root.insert("name".into(), self.name.clone().into());
        root.insert("description".into(), self.description.clone().into());
        root.insert("base".into(), self.base.clone().into());
        root.insert("backdrop".into(), self.backdrop.clone().into());
        let mut colors = Map::new();
        for (key, _) in TOKENS {
            if let Some(c) = self.colors.get(key) {
                colors.insert(key.into(), c.text().into());
            }
        }
        root.insert("colors".into(), Value::Object(colors));
        let f = &self.fonts;
        let mut fo = Map::new();
        if let Some(v) = &f.body {
            fo.insert("body".into(), v.clone().into());
        }
        if let Some(v) = &f.title {
            fo.insert("title".into(), v.clone().into());
        }
        if let Some(v) = f.title_size {
            fo.insert("titleSize".into(), num_json(v));
        }
        if let Some(v) = &f.title_weight {
            fo.insert("titleWeight".into(), v.clone().into());
        }
        if f.title_uppercase {
            fo.insert("titleUppercase".into(), true.into());
        }
        if let Some(v) = f.title_spacing {
            fo.insert("titleSpacing".into(), v.into());
        }
        if let Some(v) = &f.lcd {
            fo.insert("lcd".into(), v.clone().into());
        }
        root.insert("fonts".into(), Value::Object(fo));
        let mut co = Map::new();
        for (k, v) in [("control", self.corners.control), ("overlay", self.corners.overlay), ("card", self.corners.card), ("cover", self.corners.cover)] {
            if let Some(v) = v {
                co.insert(k.into(), num_json(v));
            }
        }
        root.insert("corners".into(), Value::Object(co));
        let mut ef = Map::new();
        for (key, _) in EFFECT_KINDS {
            if let Some(o) = self.effects.get(key) {
                ef.insert(key.into(), Value::Object(o.clone()));
            }
        }
        root.insert("effects".into(), Value::Object(ef));
        serde_json::to_string_pretty(&Value::Object(root)).expect("JSON values always serialize")
    }
}

/// Loads every `*.json` theme in `dir` (built-in or the user's), skipping unreadable files and invalid ids, sorted
/// by file name. User themes remember their file.
pub fn load_folder(dir: &Path, built_in: bool) -> Vec<Theme> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> =
        entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            let mut t = Theme::from_json(&text, built_in).ok().filter(|t| is_valid_id(&t.id))?;
            if !built_in {
                t.source_path = Some(p);
            }
            Some(t)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes")
    }

    #[test]
    fn every_built_in_theme_loads() {
        let themes = load_folder(&built_in_dir(), true);
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(themes.len(), 9, "{ids:?}");
        let token_keys: Vec<&str> = TOKENS.iter().map(|t| t.0).collect();
        let effect_keys: Vec<&str> = EFFECT_KINDS.iter().map(|t| t.0).collect();
        for t in &themes {
            assert!(is_valid_id(&t.id) && t.built_in && t.source_path.is_none());
            assert!(t.base == "dark" || t.base == "light", "{}", t.id);
            for k in t.colors.keys() {
                assert!(token_keys.contains(&k.as_str()), "{}: unknown token {k}", t.id);
            }
            for k in t.effects.keys() {
                assert!(effect_keys.contains(&k.as_str()), "{}: unknown effect {k}", t.id);
            }
            // Round trip.
            assert_eq!(Theme::from_json(&t.to_json(), true).unwrap(), *t, "{}", t.id);
        }
        assert!(ids.contains(&"retrowave") && ids.contains(&"stealth"));
    }

    #[test]
    fn no_built_in_theme_flashes() {
        for t in load_folder(&built_in_dir(), true) {
            let text = t.to_json().to_lowercase();
            for word in ["flicker", "blink", "strobe", "flash"] {
                assert!(!text.contains(word), "{} mentions {word}", t.id);
            }
        }
    }

    #[test]
    fn colors() {
        assert_eq!(Color::parse("#abc"), Some(Color::rgb(0xAA, 0xBB, 0xCC)));
        assert_eq!(Color::parse("ff2a6d"), Some(Color::rgb(0xFF, 0x2A, 0x6D)));
        assert_eq!(Color::parse("#80FF0000").unwrap().a, 0x80);
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::parse("#GGGGGG"), None);
        assert_eq!(Color::rgb(1, 2, 3).text(), "#010203");
        assert_eq!(Color::rgb(1, 2, 3).with_alpha(0.5).text(), "#80010203");
        assert_eq!(Color::rgb(0, 0, 0).mix(Color::rgb(255, 255, 255), 0.5), Color::rgb(128, 128, 128));
    }

    #[test]
    fn ids_and_validation() {
        assert!(is_valid_id("neon-noir") && !is_valid_id("..\\library") && !is_valid_id("") && !is_valid_id("Caps"));
        let id = new_custom_id();
        assert!(id.starts_with("custom-") && id.len() == 15 && is_valid_id(&id));
        assert!(Theme::from_json("[]", false).is_err());
        assert!(Theme::from_json(r#"{"format":"other","id":"x"}"#, false).is_err());
        assert!(Theme::from_json(r#"{"name":"no id"}"#, false).is_err());
        let t = Theme::from_json(r##"{"id":"x","base":"weird","backdrop":"glass","colors":{"text":"nope","accent":"#f00"}}"##, false).unwrap();
        assert_eq!((t.base.as_str(), t.backdrop.as_str(), t.name.as_str()), ("dark", "mica", "Untitled"));
        assert_eq!(t.colors.len(), 1);
    }

    #[test]
    fn writes_documented_key_order() {
        let t = Theme { id: "custom-1".into(), name: "Mine".into(), base: "dark".into(), ..Theme::default() };
        let json = t.to_json();
        let pos = |k: &str| json.find(&format!("\"{k}\"")).unwrap();
        assert!(pos("format") < pos("version") && pos("version") < pos("id") && pos("backdrop") < pos("colors"));
        assert!(pos("colors") < pos("fonts") && pos("fonts") < pos("corners") && pos("corners") < pos("effects"));
    }
}
