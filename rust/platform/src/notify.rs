//! "Now playing" notifications on a new track (off by default; a setting turns them on).
//!
//! macOS: UNUserNotificationCenter, which only works from a bundled app (`Driftwave.app`, made by
//! `scripts/package-mac.sh`); from a bare binary, such as `cargo run` or a test run, it does nothing
//! rather than crash. Windows: toast notifications under Driftwave's app id, which only an installed copy has (its
//! Start-menu shortcut carries the id; see `winapp`). Linux: not yet.

pub struct Notifier {
    #[cfg(target_os = "macos")]
    bundled: bool,
}

impl Default for Notifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier {
    pub fn new() -> Notifier {
        #[cfg(target_os = "macos")]
        {
            use objc2_foundation::NSBundle;
            // UNUserNotificationCenter raises an exception without a bundle identifier.
            let bundled = NSBundle::mainBundle().bundleIdentifier().is_some();
            Notifier { bundled }
        }
        #[cfg(not(target_os = "macos"))]
        Notifier {}
    }

    /// Whether notifications can be shown at all from this copy.
    pub fn available(&self) -> bool {
        #[cfg(target_os = "macos")]
        return self.bundled;
        #[cfg(windows)]
        return crate::winapp::installed() && crate::winapp::shortcut_path().is_some_and(|p| p.exists());
        #[cfg(not(any(target_os = "macos", windows)))]
        false
    }

    /// Asks once for permission (when the setting is turned on). `done` gets the answer.
    pub fn request_permission(&self, done: impl Fn(bool) + Send + Sync + 'static) {
        #[cfg(target_os = "macos")]
        {
            if !self.bundled {
                return done(false);
            }
            use block2::RcBlock;
            use objc2::runtime::Bool;
            use objc2_foundation::NSError;
            use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
            let center = UNUserNotificationCenter::currentNotificationCenter();
            let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| done(granted.as_bool()));
            center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &block);
        }
        // Windows asks nothing first (the person can turn them off in Settings > System > Notifications).
        #[cfg(not(target_os = "macos"))]
        done(self.available());
    }

    /// Shows a new track: the song as the title, the artist under it, and the station.
    /// Each replaces the last, so they don't pile up.
    pub fn track(&self, song: &str, artist: Option<&str>, station: &str) {
        #[cfg(target_os = "macos")]
        {
            if !self.bundled {
                return;
            }
            use objc2_foundation::NSString;
            use objc2_user_notifications::{UNMutableNotificationContent, UNNotificationRequest, UNUserNotificationCenter};
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(song));
            if let Some(a) = artist {
                content.setSubtitle(&NSString::from_str(a));
            }
            content.setBody(&NSString::from_str(station));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str("track"), &content, None);
            UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
        }
        #[cfg(windows)]
        {
            if self.available() {
                let _ = crate::winapp::toast(song, &[artist.unwrap_or(""), station], "track");
            }
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = (song, artist, station);
    }
}
