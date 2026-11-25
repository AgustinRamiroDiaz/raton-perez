//! macOS: asks the Accessibility API where the caret is. Coordinates are
//! global points from the top-left of the primary screen.

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

use super::{plausible, Anchor};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type AXUIElementRef = CFTypeRef;
type AXError = i32;
const AX_SUCCESS: AXError = 0;
const AX_VALUE_CGPOINT: u32 = 1;
const AX_VALUE_CGSIZE: u32 = 2;
const AX_VALUE_CGRECT: u32 = 3;
/// Seconds to wait on an app that doesn't answer, so a hung app can't stall us.
const AX_TIMEOUT: f32 = 0.25;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: f32) -> AXError;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, out: *mut c_void) -> u8;
}

/// A +1 CoreFoundation reference, released on drop.
struct Owned(CFTypeRef);

impl Owned {
    fn new(r: CFTypeRef) -> Option<Self> {
        (!r.is_null()).then_some(Self(r))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn attr(element: &Owned, name: &str) -> Option<Owned> {
    let name = CFString::new(name);
    let mut out: CFTypeRef = std::ptr::null();
    let err =
        unsafe { AXUIElementCopyAttributeValue(element.0, name.as_concrete_TypeRef(), &mut out) };
    if err == AX_SUCCESS {
        Owned::new(out)
    } else {
        None
    }
}

fn param_attr(element: &Owned, name: &str, param: &Owned) -> Option<Owned> {
    let name = CFString::new(name);
    let mut out: CFTypeRef = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element.0,
            name.as_concrete_TypeRef(),
            param.0,
            &mut out,
        )
    };
    if err == AX_SUCCESS {
        Owned::new(out)
    } else {
        None
    }
}

fn value<T: Default>(v: &Owned, value_type: u32) -> Option<T> {
    let mut out = T::default();
    let ok = unsafe { AXValueGetValue(v.0, value_type, &mut out as *mut T as *mut c_void) };
    (ok != 0).then_some(out)
}

/// Returns the caret's rectangle in the focused text field of app `pid`, or
/// the field's own frame when the app won't say where the caret is.
pub fn locate(pid: Option<i32>) -> Option<Anchor> {
    let app = pid.and_then(|pid| Owned::new(unsafe { AXUIElementCreateApplication(pid) }));
    if let Some(app) = &app {
        unsafe {
            AXUIElementSetMessagingTimeout(app.0, AX_TIMEOUT);
            // Electron apps (Slack, Discord, VS Code) and Chromium only build
            // their accessibility tree once a client asks for it.
            let key = CFString::new("AXManualAccessibility");
            AXUIElementSetAttributeValue(
                app.0,
                key.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            );
        }
    }

    let focused = app
        .as_ref()
        .and_then(|app| attr(app, "AXFocusedUIElement"))
        .or_else(|| {
            let system = Owned::new(unsafe { AXUIElementCreateSystemWide() })?;
            unsafe { AXUIElementSetMessagingTimeout(system.0, AX_TIMEOUT) };
            attr(&system, "AXFocusedUIElement")
        })?;

    if let Some(caret) = attr(&focused, "AXSelectedTextRange")
        .and_then(|range| param_attr(&focused, "AXBoundsForRange", &range))
        .and_then(|bounds| value::<CGRect>(&bounds, AX_VALUE_CGRECT))
        .map(|r| Anchor {
            x: r.origin.x,
            y: r.origin.y,
            width: r.size.width,
            height: r.size.height,
        })
        .filter(plausible)
    {
        return Some(caret);
    }

    let pos = attr(&focused, "AXPosition").and_then(|v| value::<CGPoint>(&v, AX_VALUE_CGPOINT))?;
    let size = attr(&focused, "AXSize").and_then(|v| value::<CGSize>(&v, AX_VALUE_CGSIZE))?;
    Some(Anchor {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
    .filter(plausible)
}
