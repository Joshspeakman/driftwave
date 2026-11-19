//! A theme's scene as Core Animation layers beneath the window's UI (macOS).
//!
//! The Full layout's scene is two pictures: a backdrop painted once per window size and a half-size
//! motion layer (rain, drops, passing lights) painted 12 times a second. Drawn by the software renderer,
//! the motion layer dirties the whole window every frame: on a Retina screen that's 3.6 million pixels
//! upscaled, blended and then every panel and glyph redrawn over them, about a quarter of a core.
//!
//! Here both pictures are IOSurfaces (tagged sRGB, like softbuffer's frames) set as the contents of two
//! layers under softbuffer's: the GPU stretches and blends them, and the UI above (drawn see-through,
//! `softbuffer::set_see_through`) is redrawn only when it changes. A motion frame costs its painting
//! plus a 0.3-megapixel copy.
//!
//! Create on the main thread once the window exists; drop to take the layers away.
//!
//! Measured on a MacBook Pro (Retina, 2360×1520 px), Night Market, Full, playing, in front: 26.9% of a core
//! drawn by the software renderer, 4.4% with these layers; WindowServer no busier.

#[cfg(target_os = "macos")]
pub use mac::SceneLayers;

/// Elsewhere there are no native scene layers: the app draws the scene itself.
#[cfg(not(target_os = "macos"))]
pub struct SceneLayers;

#[cfg(not(target_os = "macos"))]
impl SceneLayers {
    pub fn new(_ns_view: *mut std::ffi::c_void) -> Option<SceneLayers> {
        None
    }
    pub fn set_backdrop(&mut self, _rgba: &[u8], _w: usize, _h: usize) {}
    pub fn set_motion(&mut self, _frame: Option<(&[u8], usize, usize)>) {}
    pub fn set_visible(&mut self, _on: bool) {}
    pub fn ready(&self) -> bool {
        false
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::msg_send;
    use objc2_quartz_core::{CAAutoresizingMask, CALayer, CATransaction};
    use std::ffi::c_void;
    use std::ptr;

    pub struct SceneLayers {
        root: Retained<CALayer>,
        back: Retained<CALayer>,
        motion: Retained<CALayer>,
        back_surface: Option<Surface>,
        /// Motion frames rotate through a few surfaces, so one isn't written while the window server shows it.
        motion_surfaces: Vec<Surface>,
        motion_current: Option<usize>,
        visible: bool,
    }

    impl SceneLayers {
        /// `ns_view`: the window's content view (winit's `AppKitWindowHandle::ns_view`).
        pub fn new(ns_view: *mut c_void) -> Option<SceneLayers> {
            if ns_view.is_null() {
                return None;
            }
            // SAFETY: a live NSView from the window handle, used on the main thread. It's made
            // layer-backed (as softbuffer does on its first frame, which may not have come yet), and
            // `-layer` returns that backing layer.
            let root: Option<Retained<CALayer>> = unsafe {
                let view = &*(ns_view as *const NSObject);
                let _: () = msg_send![view, setWantsLayer: objc2::runtime::Bool::YES];
                msg_send![view, layer]
            };
            let root = root?;
            let back = CALayer::new();
            let motion = CALayer::new();
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            for (i, l) in [&back, &motion].into_iter().enumerate() {
                l.setFrame(root.bounds());
                // Follows the window's size; `contentsGravity` is "resize" by default, so a picture made
                // for one size stretches smoothly until the next one arrives.
                l.setAutoresizingMask(CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable);
                l.setHidden(true);
                // Beneath softbuffer's layer (and everything else).
                root.insertSublayer_atIndex(l, i as u32);
            }
            back.setOpaque(true);
            CATransaction::commit();
            Some(SceneLayers { root, back, motion, back_surface: None, motion_surfaces: Vec::new(), motion_current: None, visible: false })
        }

        /// The backdrop has arrived (until then the UI should stay opaque).
        pub fn ready(&self) -> bool {
            self.back_surface.is_some()
        }

        /// The still picture, premultiplied RGBA, `w`×`h` pixels (the window's size in pixels).
        pub fn set_backdrop(&mut self, rgba: &[u8], w: usize, h: usize) {
            // The window server may still be showing the old surface: a new one each time (once per size).
            let Some(surface) = Surface::new(w, h) else { return };
            surface.write(rgba, w, h);
            self.show(&self.back, &surface);
            self.back_surface = Some(surface);
        }

        /// The moving layer, premultiplied RGBA at its own (smaller) size, or `None` to hide it.
        pub fn set_motion(&mut self, frame: Option<(&[u8], usize, usize)>) {
            let Some((rgba, w, h)) = frame else {
                if !self.motion.isHidden() {
                    CATransaction::begin();
                    CATransaction::setDisableActions(true);
                    self.motion.setHidden(true);
                    CATransaction::commit();
                }
                return;
            };
            self.motion_surfaces.retain(|s| s.w == w && s.h == h);
            if self.motion_current.is_some_and(|c| c >= self.motion_surfaces.len()) {
                self.motion_current = None;
            }
            let current = self.motion_current;
            let free = self.motion_surfaces.iter().position(|s| !s.in_use()).filter(|&i| Some(i) != current);
            let index = match free {
                Some(i) => i,
                None if self.motion_surfaces.len() < 3 => {
                    let Some(s) = Surface::new(w, h) else { return };
                    self.motion_surfaces.push(s);
                    self.motion_surfaces.len() - 1
                }
                // All busy: skip this frame rather than tear one on screen.
                None => return,
            };
            self.motion_surfaces[index].write(rgba, w, h);
            self.show(&self.motion, &self.motion_surfaces[index]);
            self.motion_current = Some(index);
            if self.visible && self.motion.isHidden() {
                CATransaction::begin();
                CATransaction::setDisableActions(true);
                self.motion.setHidden(false);
                CATransaction::commit();
            }
        }

        /// Shows or hides the scene (the Full layout with a scene theme, or not).
        pub fn set_visible(&mut self, on: bool) {
            if on == self.visible {
                return;
            }
            self.visible = on;
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            self.back.setHidden(!on);
            self.motion.setHidden(!on || self.motion_current.is_none());
            CATransaction::commit();
        }

        fn show(&self, layer: &CALayer, surface: &Surface) {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            // Keep up with the window (autoresizing can lag a frame behind a live resize).
            let bounds = self.root.bounds();
            if layer.frame() != bounds {
                layer.setFrame(bounds);
            }
            // SAFETY: an IOSurfaceRef is a valid object for `contents` (toll-free bridged to IOSurface).
            unsafe {
                let object = &*(surface.surface as *const AnyObject);
                layer.setContents(Some(object));
                let _: () = msg_send![layer, setContentsChanged];
            }
            if layer == &*self.back {
                layer.setHidden(!self.visible);
            }
            CATransaction::commit();
        }
    }

    impl Drop for SceneLayers {
        fn drop(&mut self) {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            self.back.removeFromSuperlayer();
            self.motion.removeFromSuperlayer();
            CATransaction::commit();
        }
    }

    /// One BGRA IOSurface tagged sRGB.
    struct Surface {
        surface: ffi::IOSurfaceRef,
        w: usize,
        h: usize,
    }

    impl Surface {
        fn new(w: usize, h: usize) -> Option<Surface> {
            let surface = ffi::create(w, h);
            (!surface.is_null()).then_some(Surface { surface, w, h })
        }

        fn in_use(&self) -> bool {
            // SAFETY: a live surface we own.
            unsafe { ffi::IOSurfaceIsInUse(self.surface) != 0 }
        }

        /// Premultiplied RGBA in, BGRA out.
        fn write(&self, rgba: &[u8], w: usize, h: usize) {
            debug_assert!(rgba.len() >= w * h * 4);
            // SAFETY: the surface is ours, `w`×`h` 4-byte pixels, and locked while written within its rows.
            unsafe {
                ffi::IOSurfaceLock(self.surface, 0, ptr::null_mut());
                let base = ffi::IOSurfaceGetBaseAddress(self.surface) as *mut u8;
                let stride = ffi::IOSurfaceGetBytesPerRow(self.surface);
                for y in 0..h.min(self.h) {
                    let src = &rgba[y * w * 4..(y + 1) * w * 4];
                    let dst = std::slice::from_raw_parts_mut(base.add(y * stride), w.min(self.w) * 4);
                    for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                        d[0] = s[2];
                        d[1] = s[1];
                        d[2] = s[0];
                        d[3] = s[3];
                    }
                }
                ffi::IOSurfaceUnlock(self.surface, 0, ptr::null_mut());
            }
        }
    }

    impl Drop for Surface {
        fn drop(&mut self) {
            // SAFETY: created by IOSurfaceCreate; a layer showing it holds its own reference.
            unsafe { ffi::CFRelease(self.surface) };
        }
    }

    #[allow(non_upper_case_globals)]
    mod ffi {
        use std::ffi::c_void;
        use std::ptr;

        pub type IOSurfaceRef = *mut c_void;
        type CFTypeRef = *const c_void;
        type CFStringRef = *const c_void;

        #[repr(C)]
        pub struct CallBacks {
            _private: [u8; 0],
        }

        #[link(name = "IOSurface", kind = "framework")]
        extern "C" {
            static kIOSurfaceWidth: CFStringRef;
            static kIOSurfaceHeight: CFStringRef;
            static kIOSurfaceBytesPerElement: CFStringRef;
            static kIOSurfacePixelFormat: CFStringRef;
            static kIOSurfaceColorSpace: CFStringRef;
            fn IOSurfaceCreate(properties: CFTypeRef) -> IOSurfaceRef;
            pub fn IOSurfaceLock(surface: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
            pub fn IOSurfaceUnlock(surface: IOSurfaceRef, options: u32, seed: *mut u32) -> i32;
            pub fn IOSurfaceGetBaseAddress(surface: IOSurfaceRef) -> *mut c_void;
            pub fn IOSurfaceGetBytesPerRow(surface: IOSurfaceRef) -> usize;
            pub fn IOSurfaceIsInUse(surface: IOSurfaceRef) -> u8;
            fn IOSurfaceSetValue(surface: IOSurfaceRef, key: CFStringRef, value: CFTypeRef);
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            static kCFTypeDictionaryKeyCallBacks: CallBacks;
            static kCFTypeDictionaryValueCallBacks: CallBacks;
            fn CFDictionaryCreate(
                allocator: *const c_void,
                keys: *const CFTypeRef,
                values: *const CFTypeRef,
                count: isize,
                key_callbacks: *const CallBacks,
                value_callbacks: *const CallBacks,
            ) -> CFTypeRef;
            fn CFNumberCreate(allocator: *const c_void, number_type: isize, value: *const c_void) -> CFTypeRef;
            pub fn CFRelease(cf: CFTypeRef);
        }

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            static kCGColorSpaceSRGB: CFStringRef;
            fn CGColorSpaceCreateWithName(name: CFStringRef) -> *mut c_void;
            fn CGColorSpaceCopyPropertyList(space: *mut c_void) -> CFTypeRef;
            fn CGColorSpaceRelease(space: *mut c_void);
        }

        const CF_NUMBER_SINT32: isize = 3;
        const PIXEL_FORMAT_BGRA: i32 = 0x4247_5241;

        /// A width × height BGRA surface tagged sRGB (as softbuffer's frames are), or null.
        pub fn create(width: usize, height: usize) -> IOSurfaceRef {
            if width == 0 || height == 0 {
                return ptr::null_mut();
            }
            // SAFETY: plain CoreFoundation / IOSurface calls; everything created here is released here,
            // except the surface itself (released by `Surface`'s Drop).
            unsafe {
                let values = [width as i32, height as i32, 4, PIXEL_FORMAT_BGRA];
                let numbers = values.map(|v| CFNumberCreate(ptr::null(), CF_NUMBER_SINT32, &v as *const i32 as *const c_void));
                let keys = [kIOSurfaceWidth, kIOSurfaceHeight, kIOSurfaceBytesPerElement, kIOSurfacePixelFormat];
                let dict = CFDictionaryCreate(
                    ptr::null(),
                    keys.as_ptr(),
                    numbers.as_ptr(),
                    4,
                    &kCFTypeDictionaryKeyCallBacks,
                    &kCFTypeDictionaryValueCallBacks,
                );
                let surface = IOSurfaceCreate(dict);
                CFRelease(dict);
                for n in numbers {
                    CFRelease(n);
                }
                if !surface.is_null() {
                    let space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
                    if !space.is_null() {
                        let plist = CGColorSpaceCopyPropertyList(space);
                        if !plist.is_null() {
                            IOSurfaceSetValue(surface, kIOSurfaceColorSpace, plist);
                            CFRelease(plist);
                        }
                        CGColorSpaceRelease(space);
                    }
                }
                surface
            }
        }
    }
}
