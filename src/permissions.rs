#[cfg(target_os = "macos")]
use core_foundation::base::CFTypeRef;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Granted,
    Denied,
    Unknown,
}

pub fn accessibility_state() -> PermissionState {
    #[cfg(target_os = "macos")]
    {
        // Passing null uses default prompt behavior (no automatic prompt).
        let trusted = unsafe { AXIsProcessTrustedWithOptions(std::ptr::null_mut()) };
        return if trusted { PermissionState::Granted } else { PermissionState::Denied };
    }

    #[cfg(not(target_os = "macos"))]
    {
        PermissionState::Unknown
    }
}

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
