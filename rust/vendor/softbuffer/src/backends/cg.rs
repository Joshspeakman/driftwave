//! Softbuffer implementation using CoreGraphics.
use crate::backend_interface::*;
use crate::error::InitError;
use crate::{util, Rect, SoftBufferError};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker, Message};
use objc2_core_foundation::CGPoint;
use objc2_foundation::{
    ns_string, NSDictionary, NSKeyValueChangeKey, NSKeyValueChangeNewKey,
    NSKeyValueObservingOptions, NSNumber, NSObject, NSObjectNSKeyValueObserverRegistration,
    NSString, NSValue,
};
use objc2_quartz_core::{kCAGravityTopLeft, CALayer, CATransaction};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};

use std::collections::VecDeque;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops::Deref;
use std::ptr;

/// Driftwave: presenting through IOSurfaces instead of a new CGImage per frame.
///
/// Upstream softbuffer made a new zeroed buffer every frame (age 0: the renderer redrew the whole
/// window) and wrapped it in a CGImage tagged device RGB, which macOS colour-matched to the display
/// on the CPU (vImage) at full Retina size: most of Driftwave's CPU on a MacBook Pro.
///
/// Here the renderer draws into one persistent buffer (age 1: it redraws only what changed). Only the
/// changed rectangles are copied into an IOSurface tagged sRGB, which becomes the layer's contents;
/// the window server colour-matches IOSurfaces on the GPU, so colours stay exact. A few surfaces
/// rotate so one isn't written while the window server is still showing it.
#[allow(non_upper_case_globals)]
mod iosurface {
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
    /// 'BGRA': softbuffer's 0x00RRGGBB pixels in memory order, with alpha set on copy.
    const PIXEL_FORMAT_BGRA: i32 = 0x4247_5241;

    /// A width × height BGRA surface tagged sRGB, or null.
    pub fn create(width: usize, height: usize) -> IOSurfaceRef {
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

/// One IOSurface and the frame whose content it last received.
#[derive(Debug)]
struct Surface {
    surface: iosurface::IOSurfaceRef,
    width: usize,
    height: usize,
    frame: u64,
}

// SAFETY: IOSurfaces are thread-safe objects; we only lock and write one from the thread presenting.
unsafe impl Send for Surface {}
unsafe impl Sync for Surface {}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: we own one reference from IOSurfaceCreate.
        unsafe { iosurface::CFRelease(self.surface as *const c_void) };
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "SoftbufferObserver"]
    #[ivars = SendCALayer]
    #[derive(Debug)]
    struct Observer;

    /// NSKeyValueObserving
    impl Observer {
        #[unsafe(method(observeValueForKeyPath:ofObject:change:context:))]
        fn observe_value(
            &self,
            key_path: Option<&NSString>,
            _object: Option<&AnyObject>,
            change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
            _context: *mut c_void,
        ) {
            self.update(key_path, change);
        }
    }
);

impl Observer {
    fn new(layer: &CALayer) -> Retained<Self> {
        let this = Self::alloc().set_ivars(SendCALayer(layer.retain()));
        unsafe { msg_send![super(this), init] }
    }

    fn update(
        &self,
        key_path: Option<&NSString>,
        change: Option<&NSDictionary<NSKeyValueChangeKey, AnyObject>>,
    ) {
        let layer = self.ivars();

        let change =
            change.expect("requested a change dictionary in `addObserver`, but none was provided");
        let new = change
            .objectForKey(unsafe { NSKeyValueChangeNewKey })
            .expect("requested change dictionary did not contain `NSKeyValueChangeNewKey`");

        // NOTE: Setting these values usually causes a quarter second animation to occur, which is
        // undesirable.
        //
        // However, since we're setting them inside an observer, there already is a transaction
        // ongoing, and as such we don't need to wrap this in a `CATransaction` ourselves.

        if key_path == Some(ns_string!("contentsScale")) {
            let new = new.downcast::<NSNumber>().unwrap();
            let scale_factor = new.as_cgfloat();

            // Set the scale factor of the layer to match the root layer when it changes (e.g. if
            // moved to a different monitor, or monitor settings changed).
            layer.setContentsScale(scale_factor);
        } else if key_path == Some(ns_string!("bounds")) {
            let new = new.downcast::<NSValue>().unwrap();
            let bounds = new.get_rect().expect("new bounds value was not CGRect");

            // Set `bounds` and `position` so that the new layer is inside the superlayer.
            //
            // This differs from just setting the `bounds`, as it also takes into account any
            // translation that the superlayer may have that we'd want to preserve.
            layer.setFrame(bounds);
        } else {
            panic!("unknown observed keypath {key_path:?}");
        }
    }
}

#[derive(Debug)]
pub struct CGImpl<D, W> {
    /// Our layer.
    layer: SendCALayer,
    /// The layer that our layer was created from.
    ///
    /// Can also be retrieved from `layer.superlayer()`.
    root_layer: SendCALayer,
    observer: Retained<Observer>,
    /// The width of the underlying buffer.
    width: usize,
    /// The height of the underlying buffer.
    height: usize,
    /// Driftwave (see `iosurface`): the persistent drawing buffer and whether it holds the last frame.
    canvas: Vec<u32>,
    canvas_valid: bool,
    /// The surfaces in rotation, which one is the layer's contents now, frames presented so far, and
    /// the damage of the last few frames (to bring an older surface up to date).
    surfaces: Vec<Surface>,
    current: Option<usize>,
    frame: u64,
    damage: VecDeque<(u64, Vec<Rect>)>,
    window_handle: W,
    _display: PhantomData<D>,
}

impl<D, W> Drop for CGImpl<D, W> {
    fn drop(&mut self) {
        // SAFETY: Registered in `new`, must be removed before the observer is deallocated.
        unsafe {
            self.root_layer
                .removeObserver_forKeyPath(&self.observer, ns_string!("contentsScale"));
            self.root_layer
                .removeObserver_forKeyPath(&self.observer, ns_string!("bounds"));
        }
    }
}

impl<D: HasDisplayHandle, W: HasWindowHandle> SurfaceInterface<D, W> for CGImpl<D, W> {
    type Context = D;
    type Buffer<'a>
        = BufferImpl<'a, D, W>
    where
        Self: 'a;

    fn new(window_src: W, _display: &D) -> Result<Self, InitError<W>> {
        // `NSView`/`UIView` can only be accessed from the main thread.
        let _mtm = MainThreadMarker::new().ok_or(SoftBufferError::PlatformError(
            Some("can only access Core Graphics handles from the main thread".to_string()),
            None,
        ))?;

        let root_layer = match window_src.window_handle()?.as_raw() {
            RawWindowHandle::AppKit(handle) => {
                // SAFETY: The pointer came from `WindowHandle`, which ensures that the
                // `AppKitWindowHandle` contains a valid pointer to an `NSView`.
                //
                // We use `NSObject` here to avoid importing `objc2-app-kit`.
                let view: &NSObject = unsafe { handle.ns_view.cast().as_ref() };

                // Force the view to become layer backed
                let _: () = unsafe { msg_send![view, setWantsLayer: Bool::YES] };

                // SAFETY: `-[NSView layer]` returns an optional `CALayer`
                let layer: Option<Retained<CALayer>> = unsafe { msg_send![view, layer] };
                layer.expect("failed making the view layer-backed")
            }
            RawWindowHandle::UiKit(handle) => {
                // SAFETY: The pointer came from `WindowHandle`, which ensures that the
                // `UiKitWindowHandle` contains a valid pointer to an `UIView`.
                //
                // We use `NSObject` here to avoid importing `objc2-ui-kit`.
                let view: &NSObject = unsafe { handle.ui_view.cast().as_ref() };

                // SAFETY: `-[UIView layer]` returns `CALayer`
                let layer: Retained<CALayer> = unsafe { msg_send![view, layer] };
                layer
            }
            _ => return Err(InitError::Unsupported(window_src)),
        };

        // Add a sublayer, to avoid interfering with the root layer, since setting the contents of
        // e.g. a view-controlled layer is brittle.
        let layer = CALayer::new();
        root_layer.addSublayer(&layer);

        // Set the anchor point and geometry. Softbuffer's uses a coordinate system with the origin
        // in the top-left corner.
        //
        // NOTE: This doesn't really matter unless we start modifying the `position` of our layer
        // ourselves, but it's nice to have in place.
        layer.setAnchorPoint(CGPoint::new(0.0, 0.0));
        layer.setGeometryFlipped(true);

        // Do not use auto-resizing mask.
        //
        // This is done to work around a bug in macOS 14 and above, where views using auto layout
        // may end up setting fractional values as the bounds, and that in turn doesn't propagate
        // properly through the auto-resizing mask and with contents gravity.
        //
        // Instead, we keep the bounds of the layer in sync with the root layer using an observer,
        // see below.
        //
        // layer.setAutoresizingMask(kCALayerHeightSizable | kCALayerWidthSizable);

        let observer = Observer::new(&layer);
        // Observe changes to the root layer's bounds and scale factor, and apply them to our layer.
        //
        // The previous implementation updated the scale factor inside `resize`, but this works
        // poorly with transactions, and is generally inefficient. Instead, we update the scale
        // factor only when needed because the super layer's scale factor changed.
        //
        // Note that inherent in this is an explicit design decision: We control the `bounds` and
        // `contentsScale` of the layer directly, and instead let the `resize` call that the user
        // controls only be the size of the underlying buffer.
        //
        // SAFETY: Observer deregistered in `Drop` before the observer object is deallocated.
        unsafe {
            root_layer.addObserver_forKeyPath_options_context(
                &observer,
                ns_string!("contentsScale"),
                NSKeyValueObservingOptions::New | NSKeyValueObservingOptions::Initial,
                ptr::null_mut(),
            );
            root_layer.addObserver_forKeyPath_options_context(
                &observer,
                ns_string!("bounds"),
                NSKeyValueObservingOptions::New | NSKeyValueObservingOptions::Initial,
                ptr::null_mut(),
            );
        }

        // Set the content so that it is placed in the top-left corner if it does not have the same
        // size as the surface itself.
        //
        // TODO(madsmtm): Consider changing this to `kCAGravityResize` to stretch the content if
        // resized to something that doesn't fit, see #177.
        layer.setContentsGravity(unsafe { kCAGravityTopLeft });


        // Grab initial width and height from the layer (whose properties have just been initialized
        // by the observer using `NSKeyValueObservingOptionInitial`).
        let size = layer.bounds().size;
        let scale_factor = layer.contentsScale();
        let width = (size.width * scale_factor) as usize;
        let height = (size.height * scale_factor) as usize;

        Ok(Self {
            layer: SendCALayer(layer),
            root_layer: SendCALayer(root_layer),
            observer,
            canvas: Vec::new(),
            canvas_valid: false,
            surfaces: Vec::new(),
            current: None,
            frame: 0,
            damage: VecDeque::new(),
            width,
            height,
            _display: PhantomData,
            window_handle: window_src,
        })
    }

    #[inline]
    fn window(&self) -> &W {
        &self.window_handle
    }

    fn resize(&mut self, width: NonZeroU32, height: NonZeroU32) -> Result<(), SoftBufferError> {
        if (self.width, self.height) != (width.get() as usize, height.get() as usize) {
            self.canvas_valid = false;
        }
        self.width = width.get() as usize;
        self.height = height.get() as usize;
        Ok(())
    }

    fn buffer_mut(&mut self) -> Result<BufferImpl<'_, D, W>, SoftBufferError> {
        let len = self.width * self.height;
        let (buffer, age) = if self.canvas_valid && self.canvas.len() == len {
            (std::mem::take(&mut self.canvas), 1)
        } else {
            (vec![0; len], 0)
        };
        Ok(BufferImpl { buffer: Some(util::PixelBuffer(buffer)), age, imp: self })
    }
}

#[derive(Debug)]
pub struct BufferImpl<'a, D, W> {
    imp: &'a mut CGImpl<D, W>,
    /// Taken on present; given back to the surface if the buffer is dropped without presenting.
    buffer: Option<util::PixelBuffer>,
    age: u8,
}

impl<D, W> Drop for BufferImpl<'_, D, W> {
    fn drop(&mut self) {
        if let Some(b) = self.buffer.take() {
            // Not presented: what's in it is still the last frame plus nothing shown, so keep it only
            // if it was the last frame to begin with.
            if self.age == 1 {
                self.imp.canvas = b.0;
            } else {
                self.imp.canvas_valid = false;
            }
        }
    }
}

impl<D: HasDisplayHandle, W: HasWindowHandle> BufferInterface for BufferImpl<'_, D, W> {
    fn width(&self) -> NonZeroU32 {
        NonZeroU32::new(self.imp.width as u32).unwrap()
    }

    fn height(&self) -> NonZeroU32 {
        NonZeroU32::new(self.imp.height as u32).unwrap()
    }

    #[inline]
    fn pixels(&self) -> &[u32] {
        self.buffer.as_ref().expect("buffer")
    }

    #[inline]
    fn pixels_mut(&mut self) -> &mut [u32] {
        self.buffer.as_mut().expect("buffer")
    }

    fn age(&self) -> u8 {
        self.age
    }

    fn present(self) -> Result<(), SoftBufferError> {
        let full = Rect { x: 0, y: 0, width: self.width(), height: self.height() };
        self.present_with_damage(&[full])
    }

    fn present_with_damage(mut self, damage: &[Rect]) -> Result<(), SoftBufferError> {
        let (w, h) = (self.imp.width, self.imp.height);
        let full = Rect { x: 0, y: 0, width: self.width(), height: self.height() };
        let damage: Vec<Rect> = if self.age == 0 || damage.is_empty() { vec![full] } else { damage.to_vec() };
        let imp = &mut *self.imp;
        imp.canvas = self.buffer.take().expect("buffer").0;
        imp.canvas_valid = true;
        imp.frame += 1;
        imp.damage.push_back((imp.frame, damage));
        while imp.damage.len() > 6 {
            imp.damage.pop_front();
        }

        // A surface of the right size that the window server isn't using, the most up to date first.
        imp.surfaces.retain(|s| s.width == w && s.height == h);
        if imp.current.is_some_and(|c| c >= imp.surfaces.len()) {
            imp.current = None;
        }
        let current = imp.current;
        let free = imp
            .surfaces
            .iter()
            .enumerate()
            // SAFETY: a live surface we own.
            .filter(|(i, s)| Some(*i) != current && unsafe { iosurface::IOSurfaceIsInUse(s.surface) } == 0)
            .max_by_key(|(_, s)| s.frame)
            .map(|(i, _)| i);
        let index = match free {
            Some(i) => i,
            None if imp.surfaces.len() < 4 => {
                let surface = iosurface::create(w, h);
                if surface.is_null() {
                    return Err(SoftBufferError::PlatformError(Some("IOSurfaceCreate failed".into()), None));
                }
                imp.surfaces.push(Surface { surface, width: w, height: h, frame: 0 });
                imp.surfaces.len() - 1
            }
            // All busy: take the oldest that isn't on screen.
            None => imp
                .surfaces
                .iter()
                .enumerate()
                .filter(|(i, _)| Some(*i) != current)
                .min_by_key(|(_, s)| s.frame)
                .map(|(i, _)| i)
                .unwrap_or(0),
        };

        // What this surface is missing: the damage of every frame since it was last updated, or all of
        // it if that's further back than we remember.
        let since = imp.surfaces[index].frame;
        let remembered = imp.damage.front().is_some_and(|(f, _)| *f <= since + 1);
        let rects: Vec<Rect> = if since == 0 || !remembered {
            vec![full]
        } else {
            imp.damage.iter().filter(|(f, _)| *f > since).flat_map(|(_, r)| r.iter().copied()).collect()
        };
        let surface = imp.surfaces[index].surface;
        // SAFETY: the surface is ours and locked while we write inside its bounds.
        unsafe {
            iosurface::IOSurfaceLock(surface, 0, ptr::null_mut());
            let base = iosurface::IOSurfaceGetBaseAddress(surface) as *mut u8;
            let stride = iosurface::IOSurfaceGetBytesPerRow(surface);
            for r in &rects {
                let x0 = (r.x as usize).min(w);
                let x1 = (r.x as usize + r.width.get() as usize).min(w);
                for y in (r.y as usize).min(h)..(r.y as usize + r.height.get() as usize).min(h) {
                    let src = &imp.canvas[y * w + x0..y * w + x1];
                    let dst = std::slice::from_raw_parts_mut(base.add(y * stride).cast::<u32>().add(x0), x1 - x0);
                    for (d, s) in dst.iter_mut().zip(src) {
                        *d = *s | 0xFF00_0000; // opaque
                    }
                }
            }
            iosurface::IOSurfaceUnlock(surface, 0, ptr::null_mut());
        }
        imp.surfaces[index].frame = imp.frame;

        // The CALayer has a default action associated with a change in the layer contents, causing
        // a quarter second fade transition to happen every time a new buffer is applied. This can
        // be avoided by wrapping the operation in a transaction and disabling all actions.
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        // SAFETY: an IOSurfaceRef is a valid object for `contents` (toll-free bridged to IOSurface).
        unsafe {
            let object = &*(surface as *const AnyObject);
            imp.layer.setContents(Some(object));
            if current == Some(index) {
                let _: () = msg_send![&**imp.layer, setContentsChanged];
            }
        }
        CATransaction::commit();
        imp.current = Some(index);
        Ok(())
    }
}

#[derive(Debug)]
struct SendCALayer(Retained<CALayer>);

// SAFETY: CALayer is dubiously thread safe, like most things in Core Animation.
// But since we make sure to do our changes within a CATransaction, it is
// _probably_ fine for us to use CALayer from different threads.
//
// See also:
// https://developer.apple.com/documentation/quartzcore/catransaction/1448267-lock?language=objc
// https://stackoverflow.com/questions/76250226/how-to-render-content-of-calayer-on-a-background-thread
unsafe impl Send for SendCALayer {}
// SAFETY: Same as above.
unsafe impl Sync for SendCALayer {}

impl Deref for SendCALayer {
    type Target = CALayer;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
