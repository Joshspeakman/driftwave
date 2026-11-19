//! "Now playing" notifications on a new track (off by default; a setting turns them on).
//!
//! macOS: UNUserNotificationCenter, which only works from a bundled app (`Driftwave.app`, made by
//! `scripts/package-mac.sh`); from a bare binary, such as `cargo run` or a test run, it does nothing
//! rather than crash. Windows and Linux: not yet (the C# app used Windows app notifications).

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
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(not(target_os = "macos"))]
        done(false);
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
        #[cfg(not(target_os = "macos"))]
        let _ = (song, artist, station);
    }
}
