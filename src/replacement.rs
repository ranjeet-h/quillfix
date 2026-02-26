#![allow(dead_code)]

use anyhow::{Result, anyhow};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
#[cfg(target_os = "macos")]
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
#[cfg(target_os = "macos")]
use std::ffi::c_void;
#[cfg(target_os = "macos")]
use std::ptr;

use crate::ax_query::RetainedElement;

static CLIPBOARD: OnceLock<Mutex<String>> = OnceLock::new();

fn clipboard() -> &'static Mutex<String> {
    CLIPBOARD.get_or_init(|| Mutex::new(String::new()))
}

#[cfg(target_os = "macos")]
type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type AXError = i32;

#[cfg(target_os = "macos")]
const K_AX_ERROR_SUCCESS: AXError = 0;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
}

/// Write `new_text` into the selected-text range of a **specific retained**
/// AX element.  This avoids the race where `AXFocusedUIElement` has moved to
/// a different app by the time we try to write back.
///
/// Falls back to clipboard paste if the direct AX write fails.
///
/// # Errors
/// Returns an error only if both the AX write *and* the clipboard fallback fail.
pub fn replace_in_element(element: &RetainedElement, new_text: &str) -> Result<()> {
    if new_text.is_empty() {
        return Err(anyhow!("empty replacement text"));
    }

    #[cfg(target_os = "macos")]
    {
        let ptr = element.as_ptr() as AXUIElementRef;
        let cf_text = CFString::new(new_text);
        let attr = CFString::new("AXSelectedText");
        let err = unsafe {
            AXUIElementSetAttributeValue(
                ptr,
                attr.as_CFTypeRef() as CFStringRef,
                cf_text.as_CFTypeRef(),
            )
        };
        if err == K_AX_ERROR_SUCCESS {
            tracing::info!(phase = "replace", "AX direct write succeeded");
            return Ok(());
        }
        tracing::warn!(phase = "replace", err, "AX direct write failed; using clipboard fallback");
        clipboard_replace(new_text)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = element;
        Ok(())
    }
}

/// # Errors
/// Returns an error if `new_text` is empty or equals the sentinel `"__force_clipboard_fallback__"`.
pub fn replace_selected_text(_element: &str, new_text: &str) -> Result<()> {
    if new_text.is_empty() {
        return Err(anyhow!("empty replacement text"));
    }
    if new_text == "__force_clipboard_fallback__" {
        return Err(anyhow!("simulated AX failure"));
    }

    replace_selected_text_platform(new_text)
}

/// # Errors
/// Returns an error if the clipboard mutex is poisoned.
pub fn clipboard_replace(new_text: &str) -> Result<()> {
    #[cfg_attr(target_os = "macos", allow(unused_variables))]
    let old_mirror = {
        let mut clip = clipboard().lock().map_err(|_| anyhow!("clipboard poisoned"))?;
        let old = clip.clone();
        *clip = new_text.to_string();
        old
    }; // clip dropped here, before the sleep

    #[cfg(target_os = "macos")]
    {
        let old_real = read_system_clipboard();
        let _ = write_system_clipboard(new_text);
        let _ = post_cmd_v();
        thread::sleep(Duration::from_millis(200));
        let _ = write_system_clipboard(&old_real);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        thread::sleep(Duration::from_millis(200));
        if !old_mirror.is_empty() {
            let mut clip = clipboard().lock().map_err(|_| anyhow!("clipboard poisoned"))?;
            *clip = old_mirror;
        }
        Ok(())
    }
}

/// # Errors
/// Returns an error if both `replace_selected_text` and `clipboard_replace` fail.
pub fn replace_with_fallback(element: &str, new_text: &str) -> Result<()> {
    replace_selected_text(element, new_text).or_else(|_| clipboard_replace(new_text))
}

#[must_use]
pub fn clipboard_contents() -> String {
    clipboard().lock().map(|clip| clip.clone()).unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn read_system_clipboard() -> String {
    let pb = NSPasteboard::generalPasteboard();
    pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn write_system_clipboard(text: &str) -> Result<()> {
    let pb = NSPasteboard::generalPasteboard();
    let _ = pb.clearContents();
    let ok = pb.setString_forType(&objc2_foundation::NSString::from_str(text), unsafe {
        NSPasteboardTypeString
    });
    if ok { Ok(()) } else { Err(anyhow!("failed to write pasteboard")) }
}

#[cfg(target_os = "macos")]
fn post_cmd_v() -> Result<()> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| anyhow!("failed creating CGEventSource"))?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, true)
        .map_err(|()| anyhow!("failed creating key down event"))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_V, false)
        .map_err(|()| anyhow!("failed creating key up event"))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(target_os = "macos")]
fn replace_selected_text_platform(new_text: &str) -> Result<()> {
    replace_selected_text_macos(new_text)
}

#[cfg(not(target_os = "macos"))]
fn replace_selected_text_platform(_new_text: &str) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn replace_selected_text_macos(new_text: &str) -> Result<()> {
    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return Err(anyhow!("AXUIElementCreateSystemWide returned null"));
    }

    let focused_attr = CFString::new("AXFocusedUIElement");
    let mut focused_value: CFTypeRef = ptr::null();
    let copy_err = unsafe {
        AXUIElementCopyAttributeValue(
            system,
            focused_attr.as_CFTypeRef() as CFStringRef,
            &raw mut focused_value,
        )
    };
    if copy_err != K_AX_ERROR_SUCCESS || focused_value.is_null() {
        unsafe { CFRelease(system as CFTypeRef) };
        return Err(anyhow!(
            "AXUIElementCopyAttributeValue(AXFocusedUIElement) failed with {copy_err}"
        ));
    }

    let focused_element = focused_value as AXUIElementRef;
    let cf_text = CFString::new(new_text);
    let selected_text_attr = CFString::new("AXSelectedText");
    let set_err = unsafe {
        AXUIElementSetAttributeValue(
            focused_element,
            selected_text_attr.as_CFTypeRef() as CFStringRef,
            cf_text.as_CFTypeRef(),
        )
    };

    unsafe {
        CFRelease(focused_value);
        CFRelease(system as CFTypeRef);
    }

    if set_err == K_AX_ERROR_SUCCESS {
        Ok(())
    } else {
        Err(anyhow!("AXUIElementSetAttributeValue(AXSelectedText) failed with {set_err}"))
    }
}
