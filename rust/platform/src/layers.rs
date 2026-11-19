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
//! Two more for the UI's moving strips, which otherwise redraw a full-width band of the window every tick:
//! - **rows** (under the UI): synthwave's grid rows. Each row is one colour across the window, so the picture
//!   is one pixel wide and the floor's height, stretched sideways by the GPU.
//! - **band** (over the UI): the scanlines' slow bright band, a fixed gradient that only moves.
//!
//! And the still **front** picture over everything (grain, vignette, scanlines): as a layer, the UI's own
//! redraws (meters, marquees) don't blend a full-window picture over themselves each time.
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
    pub fn set_rows(&mut self, _frame: Option<(&[u8], usize, usize)>, _rect: (f64, f64, f64, f64)) {}
    pub fn set_band(&mut self, _rect: Option<(f64, f64, f64, f64)>) {}
    pub fn set_front(&mut self, _frame: Option<(&[u8], usize, usize)>) {}
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
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_quartz_core::{kCAFilterNearest, CAAutoresizingMask, CALayer, CATransaction};
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
        /// Synthwave's rows (under the UI) and the scanline band (over it), and whether each is wanted.
        rows: Sheet,
        rows_on: bool,
        band: Sheet,
        band_on: bool,
        front: Sheet,
        front_on: bool,
        visible: bool,
    }

    /// A layer whose picture changes: a few IOSurfaces in rotation, so one isn't written while shown.
    struct Sheet {
        layer: Retained<CALayer>,
        surfaces: Vec<Surface>,
        current: Option<usize>,
    }

    impl Sheet {
        fn new() -> Sheet {
            let layer = CALayer::new();
            layer.setHidden(true);
            Sheet { layer, surfaces: Vec::new(), current: None }
        }

        /// Writes the picture into a free surface and shows it. False if every surface is still on screen.
        fn put(&mut self, rgba: &[u8], w: usize, h: usize) -> bool {
            self.surfaces.retain(|s| s.w == w && s.h == h);
            if self.current.is_some_and(|c| c >= self.surfaces.len()) {
                self.current = None;
            }
            let current = self.current;
            let free = self.surfaces.iter().enumerate().position(|(i, s)| Some(i) != current && !s.in_use());
            let index = match free {
                Some(i) => i,
                None if self.surfaces.len() < 3 => {
                    let Some(s) = Surface::new(w, h) else { return false };
                    self.surfaces.push(s);
                    self.surfaces.len() - 1
                }
                None => return false,
            };
            self.surfaces[index].write(rgba, w, h);
            // SAFETY: an IOSurfaceRef is a valid object for `contents` (toll-free bridged to IOSurface).
            unsafe {
                let object = &*(self.surfaces[index].surface as *const AnyObject);
                self.layer.setContents(Some(object));
                let _: () = msg_send![&*self.layer, setContentsChanged];
            }
            self.current = Some(index);
            true
        }
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
            // The rows: right above the motion. Exact pixels (they're drawn at the screen's resolution).
            let rows = Sheet::new();
            rows.layer.setMagnificationFilter(unsafe { kCAFilterNearest });
            rows.layer.setMinificationFilter(unsafe { kCAFilterNearest });
            root.insertSublayer_atIndex(&rows.layer, 2);
            // The front picture and the band: over everything (`raise` keeps them there if softbuffer's layer
            // arrives later), the band over the front as in the UI.
            let front = Sheet::new();
            front.layer.setFrame(root.bounds());
            front.layer.setAutoresizingMask(CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable);
            root.addSublayer(&front.layer);
            let mut band = Sheet::new();
            root.addSublayer(&band.layer);
            // White, see-through to 9/255 in the middle and clear at both ends, as the Slint band was.
            let px: Vec<u8> = (0..140)
                .flat_map(|y| {
                    let t = (y as f32 + 0.5) / 140.0;
                    let a = (9.0 * (1.0 - (t - 0.5).abs() * 2.0)).round() as u8;
                    [a, a, a, a]
                })
                .collect();
            band.put(&px, 1, 140);
            CATransaction::commit();
            Some(SceneLayers {
                root,
                back,
                motion,
                back_surface: None,
                motion_surfaces: Vec::new(),
                motion_current: None,
                rows,
                rows_on: false,
                band,
                band_on: false,
                front,
                front_on: false,
                visible: false,
            })
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

        /// Synthwave's rows: premultiplied RGBA `w`×`h` px for the rectangle `rect` (x, y, width, height in
        /// points from the window's top left), or `None` to hide them.
        pub fn set_rows(&mut self, frame: Option<(&[u8], usize, usize)>, rect: (f64, f64, f64, f64)) {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            self.rows_on = frame.is_some();
            if let Some((rgba, w, h)) = frame {
                self.place(&self.rows.layer, rect);
                self.rows.put(rgba, w, h);
            }
            self.rows.layer.setHidden(!(self.visible && self.rows_on));
            CATransaction::commit();
        }

        /// The scanline band at `rect` (points from the window's top left), or `None` to hide it.
        pub fn set_band(&mut self, rect: Option<(f64, f64, f64, f64)>) {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            self.band_on = rect.is_some();
            if let Some(r) = rect {
                self.place(&self.band.layer, r);
                self.raise();
            }
            self.band.layer.setHidden(!(self.visible && self.band_on));
            CATransaction::commit();
        }

        /// The still picture over everything, premultiplied RGBA at the window's size in pixels, or `None`.
        pub fn set_front(&mut self, frame: Option<(&[u8], usize, usize)>) {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            self.front_on = frame.is_some();
            if let Some((rgba, w, h)) = frame {
                let bounds = self.root.bounds();
                if self.front.layer.frame() != bounds {
                    self.front.layer.setFrame(bounds);
                }
                self.front.put(rgba, w, h);
                self.raise();
            }
            self.front.layer.setHidden(!(self.visible && self.front_on));
            CATransaction::commit();
        }

        /// Keeps the front picture and then the band on top, over softbuffer's layer even if that arrived
        /// after ours.
        fn raise(&self) {
            let subs = unsafe { self.root.sublayers() };
            let n = subs.as_ref().map_or(0, |s| s.count());
            let at = |i: usize| subs.as_ref().filter(|_| n > i).map(|s| s.objectAtIndex(n - 1 - i));
            let ok = at(0).as_deref() == Some(&*self.band.layer) && at(1).as_deref() == Some(&*self.front.layer);
            if !ok {
                self.front.layer.removeFromSuperlayer();
                self.band.layer.removeFromSuperlayer();
                self.root.addSublayer(&self.front.layer);
                self.root.addSublayer(&self.band.layer);
            }
        }

        /// A rectangle in points from the window's top left, in the root layer's own geometry.
        fn place(&self, layer: &CALayer, (x, y, w, h): (f64, f64, f64, f64)) {
            let bounds = self.root.bounds();
            let y = if self.root.isGeometryFlipped() { y } else { bounds.size.height - y - h };
            let frame = CGRect::new(CGPoint::new(x, y), CGSize::new(w, h));
            if layer.frame() != frame {
                layer.setFrame(frame);
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
            self.rows.layer.setHidden(!(on && self.rows_on));
            self.band.layer.setHidden(!(on && self.band_on));
            self.front.layer.setHidden(!(on && self.front_on));
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
            self.rows.layer.removeFromSuperlayer();
            self.band.layer.removeFromSuperlayer();
            self.front.layer.removeFromSuperlayer();
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
