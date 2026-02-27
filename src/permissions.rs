#![allow(dead_code)]

#[cfg(target_os = "macos")]
use core_foundation::base::CFTypeRef;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertStyle};
#[cfg(target_os = "macos")]
use objc2_foundation::{MainThreadMarker, NSString};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied,
    NotDetermined,
}

const DEFAULTS_DOMAIN: &str = "com.quillfix.app";
pub const ONBOARDED_KEY: &str = "quillfix.onboarded";

#[must_use]
pub fn accessibility_state() -> PermissionState {
    #[cfg(target_os = "macos")]
    {
        // Passing null uses default prompt behavior (no automatic prompt).
        let trusted = unsafe { AXIsProcessTrustedWithOptions(std::ptr::null_mut()) };
        if trusted { PermissionState::Granted } else { PermissionState::Denied }
    }

    #[cfg(not(target_os = "macos"))]
    {
        PermissionState::NotDetermined
    }
}

/// # Errors
/// Returns an error if the system command to open accessibility settings fails.
pub fn open_accessibility_settings() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .status()
            .map(|_| ())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[must_use]
pub fn load_onboarded_state() -> bool {
    #[cfg(target_os = "macos")]
    {
        let output =
            Command::new("defaults").args(["read", DEFAULTS_DOMAIN, ONBOARDED_KEY]).output();
        match output {
            Ok(out) if out.status.success() => {
                let value = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
                value == "1" || value == "true" || value == "yes"
            }
            _ => false,
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn save_onboarded_state(onboarded: bool) {
    #[cfg(target_os = "macos")]
    {
        let value = if onboarded { "true" } else { "false" };
        let _ = Command::new("defaults")
            .args(["write", DEFAULTS_DOMAIN, ONBOARDED_KEY, "-bool", value])
            .status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = onboarded;
    }
}

/// First-launch onboarding flow:
/// 1) Opens Accessibility settings if needed.
/// 2) Polls trust state until granted or timeout.
/// 3) Persists `quillfix.onboarded = true` once granted.
#[must_use]
pub fn run_first_launch_onboarding(timeout: Duration, poll_interval: Duration) -> bool {
    if load_onboarded_state() {
        return true;
    }

    if accessibility_state() == PermissionState::Granted {
        save_onboarded_state(true);
        show_services_hint_alert();
        return true;
    }

    if !show_onboarding_alert() {
        return false;
    }

    let _ = open_accessibility_settings();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if accessibility_state() == PermissionState::Granted {
            save_onboarded_state(true);
            show_services_hint_alert();
            return true;
        }
        std::thread::sleep(poll_interval);
    }

    false
}

#[cfg(target_os = "macos")]
fn show_onboarding_alert() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return true;
    };

    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Informational);
    alert.setMessageText(&NSString::from_str("Enable Accessibility for QuillFix"));
    alert.setInformativeText(&NSString::from_str(
        "QuillFix needs Accessibility access to read selected text and apply corrections.",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Open Settings"));
    alert.addButtonWithTitle(&NSString::from_str("Later"));

    alert.runModal() == NSAlertFirstButtonReturn
}

#[cfg(not(target_os = "macos"))]
fn show_onboarding_alert() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn show_services_hint_alert() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let alert = NSAlert::new(mtm);
    alert.setAlertStyle(NSAlertStyle::Informational);
    alert.setMessageText(&NSString::from_str("QuillFix Setup Complete"));
    alert.setInformativeText(&NSString::from_str(
        "You can now use \"Correct with QuillFix\" from right-click > Services in supported apps. \
If Services is unavailable, use the QuillFix menu bar item \"Correct Clipboard Text\" as fallback.",
    ));
    alert.addButtonWithTitle(&NSString::from_str("Got it"));
    let _ = alert.runModal();
}

#[cfg(not(target_os = "macos"))]
fn show_services_hint_alert() {}
