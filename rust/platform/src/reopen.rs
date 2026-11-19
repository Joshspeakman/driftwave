//! A click on the Dock icon while the window is hidden (closed to the menu bar): macOS asks the app delegate
//! `applicationShouldHandleReopen:hasVisibleWindows:`. winit registers no delegate (it leaves that to the app,
//! see `winit::platform::macos`), so without this the click does nothing.
//!
//! [`install`] once the event loop runs (main thread); [`take`] in the app's tick, and show the window when it
//! says so. Elsewhere both do nothing.

#[cfg(target_os = "macos")]
pub use mac::{install, take};

#[cfg(not(target_os = "macos"))]
pub fn install() {}

#[cfg(not(target_os = "macos"))]
pub fn take() -> bool {
    false
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::ffi::class_addMethod;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject, Sel};
    use objc2::{define_class, msg_send, sel, ClassType};
    use std::sync::atomic::{AtomicBool, Ordering};

    static REOPENED: AtomicBool = AtomicBool::new(false);
    static INSTALLED: AtomicBool = AtomicBool::new(false);

    extern "C-unwind" fn should_reopen(_this: *mut AnyObject, _cmd: Sel, _app: *mut AnyObject, _visible: Bool) -> Bool {
        REOPENED.store(true, Ordering::Relaxed);
        Bool::YES
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "DriftwaveAppDelegate"]
        struct Delegate;

        impl Delegate {
            #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
            fn should_handle_reopen(&self, _app: &AnyObject, _visible: Bool) -> Bool {
                REOPENED.store(true, Ordering::Relaxed);
                Bool::YES
            }
        }
    );

    /// Hooks the Dock-click reopen. Idempotent; call on the main thread once the event loop runs.
    pub fn install() {
        if INSTALLED.swap(true, Ordering::Relaxed) {
            return;
        }
        let Some(app_class) = AnyClass::get(c"NSApplication") else { return };
        // SAFETY: NSApplication's shared instance and its delegate, on the main thread; `setDelegate:` takes any
        // object (AppKit asks it what it responds to). The delegate property is weak, so ours is kept forever.
        unsafe {
            let app: *mut AnyObject = msg_send![app_class, sharedApplication];
            if app.is_null() {
                return;
            }
            let current: *mut AnyObject = msg_send![app, delegate];
            if current.is_null() {
                let d: Retained<Delegate> = msg_send![Delegate::class(), new];
                let _: () = msg_send![app, setDelegate: &*d];
                std::mem::forget(d);
            } else {
                // Someone else's delegate: add the method to its class (no effect if it already has one).
                let cls: *const AnyClass = msg_send![current, class];
                let imp: unsafe extern "C-unwind" fn() = std::mem::transmute(should_reopen as extern "C-unwind" fn(_, _, _, _) -> _);
                class_addMethod(cls as *mut AnyClass, sel!(applicationShouldHandleReopen:hasVisibleWindows:), imp, c"c@:@c".as_ptr());
            }
        }
    }

    /// Whether the Dock icon was clicked since the last call.
    pub fn take() -> bool {
        REOPENED.swap(false, Ordering::Relaxed)
    }
}
