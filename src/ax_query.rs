#![allow(dead_code)]

#[cfg(target_os = "macos")]
use core_foundation::base::{CFGetTypeID, CFRelease, CFRetain, CFTypeID, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBooleanGetTypeID;
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ptr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionInfo {
    pub text: String,
    pub element_role: String,
    pub secure: bool,
}

/// A retained reference to a macOS AX UI element.
///
/// - `CFRetain`ed on construction so it remains valid after the originating
///   thread moves on (e.g. after the menu closes and focus shifts).
/// - `CFRelease`d automatically when dropped.
/// - Marked `Send` because CF objects are reference-counted and thread-safe
///   for retain/release purposes, and `AXUIElementSetAttributeValue` may be
///   called from any thread.
#[cfg(target_os = "macos")]
pub struct RetainedElement {
    ptr: *const c_void,
}

#[cfg(target_os = "macos")]
impl RetainedElement {
    /// Retain `raw` and wrap it.  `raw` must be a valid, non-null
    /// `AXUIElementRef`.
    ///
    /// # Safety
    /// Caller must pass a valid `AXUIElementRef`.
    unsafe fn new(raw: *const c_void) -> Self {
        unsafe { CFRetain(raw as CFTypeRef) };
        Self { ptr: raw }
    }

    pub(crate) fn as_ptr(&self) -> *const c_void {
        self.ptr
    }
}

#[cfg(target_os = "macos")]
impl Drop for RetainedElement {
    fn drop(&mut self) {
        unsafe { CFRelease(self.ptr as CFTypeRef) };
    }
}

// SAFETY: AXUIElementRef is a Core Foundation ref-counted object; retain/release
// and AXUIElementSetAttributeValue are safe to call from any thread.
#[cfg(target_os = "macos")]
unsafe impl Send for RetainedElement {}
#[cfg(target_os = "macos")]
unsafe impl Sync for RetainedElement {}

/// A non-owning placeholder used on non-macOS builds so the type exists.
#[cfg(not(target_os = "macos"))]
pub struct RetainedElement;

#[cfg(target_os = "macos")]
type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type AXValueRef = *const c_void;
#[cfg(target_os = "macos")]
type Boolean = u8;
#[cfg(target_os = "macos")]
type AXError = i32;

#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: AXError = 0;
#[cfg(target_os = "macos")]
const K_AX_VALUE_CG_RECT_TYPE: i32 = 3;

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGPointFFI {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGSizeFFI {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct CGRectFFI {
    origin: CGPointFFI,
    size: CGSizeFFI,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    fn AXValueGetType(value: AXValueRef) -> i32;
    fn AXValueGetValue(value: AXValueRef, value_type: i32, value_ptr: *mut c_void) -> Boolean;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFBooleanGetValue(boolean: *const c_void) -> Boolean;
}

// NSScreen.mainScreen.frame.size.height via CoreGraphics display API
#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayBounds(display_id: u32) -> CGRectFFI;
    fn CGMainDisplayID() -> u32;
}

/// Convert a Quartz (top-left-origin) `CGRect` to AppKit coordinates
/// (bottom-left-origin) using the main display height.
#[must_use]
pub fn quartz_to_appkit(rect: CGRect) -> CGRect {
    #[cfg(target_os = "macos")]
    {
        let screen_height = main_display_height_macos();
        CGRect {
            x: rect.x,
            y: screen_height - rect.y - rect.height,
            width: rect.width,
            height: rect.height,
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        rect
    }
}

#[cfg(target_os = "macos")]
fn main_display_height_macos() -> f64 {
    let bounds = unsafe { CGDisplayBounds(CGMainDisplayID()) };
    bounds.size.height
}

#[must_use]
pub fn query_selection(text: &str, role: &str, secure: bool) -> Option<SelectionInfo> {
    let trimmed = text.trim();
    if secure {
        return None;
    }

    if !(4..=1500).contains(&trimmed.len()) {
        return None;
    }

    // Allow common text input roles including web views (AXWebArea) and combos
    let allowed_roles = ["AXTextArea", "AXTextField", "AXWebArea", "AXComboBox", "AXSearchField"];
    if !allowed_roles.contains(&role) {
        return None;
    }

    Some(SelectionInfo { text: trimmed.to_string(), element_role: role.to_string(), secure })
}

/// Best-effort real macOS AX selection query for the currently focused element.
///
/// Returns `None` if AX is unavailable, permission is missing, no selected
/// text exists, or bounds cannot be resolved.
///
/// The returned `CGRect` is in **AppKit coordinates** (bottom-left origin).
/// The returned [`RetainedElement`] holds a `CFRetain`-ed reference to the
/// focused UI element so write-back can target it even after focus shifts.
#[must_use]
pub fn query_focused_selection() -> Option<(SelectionInfo, CGRect, RetainedElement)> {
    #[cfg(target_os = "macos")]
    {
        query_focused_selection_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn query_focused_selection_macos() -> Option<(SelectionInfo, CGRect, RetainedElement)> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return None;
    }

    let focused = copy_attribute(system, "AXFocusedUIElement")?;
    let focused_element = focused as AXUIElementRef;

    let text = read_cfstring_attribute(focused_element, "AXSelectedText")?;
    let role = read_cfstring_attribute(focused_element, "AXRole")?;
    let secure = read_cfbool_attribute(focused_element, "AXSecureTextEntry").unwrap_or(false);
    let selection = query_selection(&text, &role, secure)?;

    let range = copy_attribute(focused_element, "AXSelectedTextRange")?;
    let bounds_value =
        copy_parameterized_attribute(focused_element, "AXBoundsForRange", range as CFTypeRef)?;
    let quartz_bounds = decode_ax_cgrect(bounds_value as AXValueRef)?;
    let bounds = quartz_to_appkit(quartz_bounds);

    // Retain the focused element before we release the system-wide ref.
    // The caller is responsible for the element's lifetime via RetainedElement.
    let retained = unsafe { RetainedElement::new(focused_element) };

    unsafe {
        CFRelease(bounds_value);
        CFRelease(range);
        CFRelease(focused);
        CFRelease(system as CFTypeRef);
    }

    Some((selection, bounds, retained))
}

#[cfg(target_os = "macos")]
fn copy_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
    let attr = CFString::new(attribute);
    let mut value: CFTypeRef = ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &raw mut value)
    };
    if err == K_AX_ERROR_SUCCESS && !value.is_null() { Some(value) } else { None }
}

#[cfg(target_os = "macos")]
fn copy_parameterized_attribute(
    element: AXUIElementRef,
    attribute: &str,
    parameter: CFTypeRef,
) -> Option<CFTypeRef> {
    let attr = CFString::new(attribute);
    let mut value: CFTypeRef = ptr::null();
    let err = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element,
            attr.as_concrete_TypeRef(),
            parameter,
            &raw mut value,
        )
    };
    if err == K_AX_ERROR_SUCCESS && !value.is_null() { Some(value) } else { None }
}

#[cfg(target_os = "macos")]
fn read_cfstring_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let value = copy_attribute(element, attribute)?;
    let type_id = unsafe { CFGetTypeID(value) };
    let is_string = type_id == CFString::type_id();
    if !is_string {
        unsafe { CFRelease(value) };
        return None;
    }

    let cf = unsafe { CFString::wrap_under_create_rule(value as CFStringRef) };
    Some(cf.to_string())
}

#[cfg(target_os = "macos")]
fn read_cfbool_attribute(element: AXUIElementRef, attribute: &str) -> Option<bool> {
    let value = copy_attribute(element, attribute)?;
    let type_id: CFTypeID = unsafe { CFGetTypeID(value) };
    let is_bool = type_id == unsafe { CFBooleanGetTypeID() };
    if !is_bool {
        unsafe { CFRelease(value) };
        return None;
    }

    let b = unsafe { CFBooleanGetValue(value) } != 0;
    unsafe { CFRelease(value) };
    Some(b)
}

#[cfg(target_os = "macos")]
fn decode_ax_cgrect(value: AXValueRef) -> Option<CGRect> {
    let ty = unsafe { AXValueGetType(value) };
    if ty != K_AX_VALUE_CG_RECT_TYPE {
        return None;
    }

    let mut raw = CGRectFFI {
        origin: CGPointFFI { x: 0.0, y: 0.0 },
        size: CGSizeFFI { width: 0.0, height: 0.0 },
    };
    let ok =
        unsafe { AXValueGetValue(value, K_AX_VALUE_CG_RECT_TYPE, (&raw mut raw).cast::<c_void>()) }
            != 0;
    if !ok {
        return None;
    }

    Some(CGRect {
        x: raw.origin.x,
        y: raw.origin.y,
        width: raw.size.width,
        height: raw.size.height,
    })
}
