//! Small things only the system knows (macOS; elsewhere they return nothing and the app falls back):
//! - [`short_time`]: the time now as the person's own settings write it (12 or 24 hours, their separators);
//! - [`app_prefs`]: another app's saved settings, read only (the first launch brings the Swift Shortwave's across);
//! - [`font_families`]: the installed font families (the theme editor's font pickers).

/// A saved setting's value.
#[derive(Debug, Clone, PartialEq)]
pub enum PrefValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[cfg(target_os = "macos")]
pub use mac::{app_prefs, font_families, short_time};

#[cfg(not(target_os = "macos"))]
pub fn font_families() -> Vec<String> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn short_time() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn app_prefs(_domain: &str, _keys: &[&str]) -> Vec<(String, PrefValue)> {
    Vec::new()
}

#[cfg(target_os = "macos")]
mod mac {
    use super::PrefValue;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    use objc2::msg_send;
    use objc2_foundation::NSString;
    use std::ffi::{c_char, c_void, CStr};

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFBooleanGetTypeID() -> usize;
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, i: isize) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "CoreText", kind = "framework")]
    extern "C" {
        fn CTFontManagerCopyAvailableFontFamilyNames() -> *const c_void;
    }

    /// The installed font families, as Core Text names them, without the system's hidden ones (".SF NS"…).
    pub fn font_families() -> Vec<String> {
        let mut out = Vec::new();
        // SAFETY: a CFArray of CFStrings that we own and release; CFString is toll-free bridged to NSString.
        unsafe {
            let names = CTFontManagerCopyAvailableFontFamilyNames();
            if names.is_null() {
                return out;
            }
            for i in 0..CFArrayGetCount(names) {
                let s = CFArrayGetValueAtIndex(names, i) as *mut AnyObject;
                if let Some(name) = (!s.is_null()).then(|| text(s)).flatten().filter(|n| !n.starts_with('.')) {
                    out.push(name);
                }
            }
            CFRelease(names);
        }
        out
    }

    fn class(name: &CStr) -> Option<&'static AnyClass> {
        AnyClass::get(name)
    }

    /// The text of an NSString.
    ///
    /// # Safety
    /// `s` is a live NSString.
    unsafe fn text(s: *mut AnyObject) -> Option<String> {
        let p: *const c_char = msg_send![s, UTF8String];
        (!p.is_null()).then(|| CStr::from_ptr(p).to_string_lossy().into_owned())
    }

    /// "9:41 PM" or "21:41", as the person's settings write a short time.
    pub fn short_time() -> Option<String> {
        let (formatter, date) = (class(c"NSDateFormatter")?, class(c"NSDate")?);
        // SAFETY: class methods returning autoreleased objects, used at once on this thread.
        unsafe {
            let now: *mut AnyObject = msg_send![date, date];
            // dateStyle NSDateFormatterNoStyle (0), timeStyle NSDateFormatterShortStyle (1).
            let s: *mut AnyObject = msg_send![formatter, localizedStringFromDate: now, dateStyle: 0usize, timeStyle: 1usize];
            if s.is_null() {
                return None;
            }
            text(s).map(|t| t.replace('\u{202f}', " "))
        }
    }

    /// The settings `keys` saved by the app with defaults domain `domain` (its bundle id), where they're text,
    /// numbers or switches. Only read.
    pub fn app_prefs(domain: &str, keys: &[&str]) -> Vec<(String, PrefValue)> {
        let mut out = Vec::new();
        let (Some(defaults), Some(string_class), Some(number_class)) = (class(c"NSUserDefaults"), class(c"NSString"), class(c"NSNumber")) else {
            return out;
        };
        // SAFETY: NSUserDefaults' persistent domain is a dictionary of property-list objects; each is checked for
        // its class before it's read.
        unsafe {
            let ud: *mut AnyObject = msg_send![defaults, standardUserDefaults];
            let name = NSString::from_str(domain);
            let dict: *mut AnyObject = msg_send![ud, persistentDomainForName: &*name];
            if dict.is_null() {
                return out;
            }
            for &k in keys {
                let key = NSString::from_str(k);
                let v: *mut AnyObject = msg_send![dict, objectForKey: &*key];
                if v.is_null() {
                    continue;
                }
                let is_string: Bool = msg_send![v, isKindOfClass: string_class];
                let is_number: Bool = msg_send![v, isKindOfClass: number_class];
                let value = if is_string.as_bool() {
                    text(v).map(PrefValue::Str)
                } else if is_number.as_bool() {
                    if CFGetTypeID(v as *const c_void) == CFBooleanGetTypeID() {
                        let b: Bool = msg_send![v, boolValue];
                        Some(PrefValue::Bool(b.as_bool()))
                    } else {
                        let n: f64 = msg_send![v, doubleValue];
                        Some(PrefValue::Num(n))
                    }
                } else {
                    None
                };
                if let Some(value) = value {
                    out.push((k.to_string(), value));
                }
            }
        }
        out
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn short_time_has_hours_and_minutes() {
        let t = short_time().expect("a time");
        assert!(t.contains(':') || t.contains('.'), "{t}");
        assert!(t.chars().filter(char::is_ascii_digit).count() >= 3, "{t}");
    }

    #[test]
    fn another_apps_text_numbers_and_switches_are_read() {
        // A throwaway domain, written with `defaults` and removed again.
        let domain = "io.github.joshspeakman.Driftwave.PrefsTest";
        let w = |k: &str, kind: &str, v: &str| {
            assert!(std::process::Command::new("defaults").args(["write", domain, k, kind, v]).status().unwrap().success());
        };
        w("recordingMode", "-string", "decide");
        w("pastTracksCount", "-int", "12");
        w("volume", "-float", "0.25");
        w("backgroundPlayback", "-bool", "false");
        let got = app_prefs(domain, &["recordingMode", "pastTracksCount", "volume", "backgroundPlayback", "absent"]);
        let _ = std::process::Command::new("defaults").args(["delete", domain]).status();
        // `defaults delete` leaves an empty file behind.
        if let Some(home) = std::env::var_os("HOME") {
            let _ = std::fs::remove_file(std::path::Path::new(&home).join("Library/Preferences").join(format!("{domain}.plist")));
        }
        assert_eq!(
            got,
            vec![
                ("recordingMode".to_string(), PrefValue::Str("decide".into())),
                ("pastTracksCount".to_string(), PrefValue::Num(12.0)),
                ("volume".to_string(), PrefValue::Num(0.25)),
                ("backgroundPlayback".to_string(), PrefValue::Bool(false)),
            ]
        );
    }

    #[test]
    fn the_installed_fonts_are_listed() {
        let f = font_families();
        assert!(f.iter().any(|n| n == "Helvetica Neue" || n == "Helvetica"), "{} families", f.len());
        assert!(f.iter().all(|n| !n.starts_with('.')));
    }

    #[test]
    fn a_missing_domain_has_no_prefs() {
        assert!(app_prefs("io.github.joshspeakman.NoSuchApp.Test", &["volume"]).is_empty());
    }
}
