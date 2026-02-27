# Cross-Platform Plan: Windows and Linux Support

## Current State
QuillFix is currently a **macOS-only** application with heavy platform-specific dependencies:
- **UI Framework**: AppKit (NSStatusBar, NSMenu, NSApplication)
- **System Integration**: Core Foundation, Objective-C bindings
- **Permissions**: Accessibility APIs, NSServices
- **Storage**: macOS defaults system
- **ML**: Metal-accelerated Candle/MLX

## Goal
Add native Windows and Linux support while maintaining the same system tray/menubar experience - no GUI window needed.

## Architecture Strategy

### Platform Abstraction Layer
Create trait-based system for platform-specific operations:

```rust
// src/platform/mod.rs
pub trait PlatformUI {
    fn create_system_tray() -> Result<Box<dyn SystemTray>>;
    fn show_notification(title: &str, message: &str);
    fn run_event_loop();
}

pub trait PlatformStorage {
    fn get_bool(key: &str) -> Option<bool>;
    fn set_bool(key: &str, value: bool);
}

pub trait PlatformPermissions {
    fn check_accessibility() -> PermissionState;
    fn request_accessibility() -> bool;
}
```

### Directory Structure
```
src/
├── core/
│   ├── corrector.rs
│   ├── llm/
│   └── models.rs
├── platform/
│   ├── mod.rs
│   ├── macos/        # Current implementation
│   ├── windows/      # New implementation
│   └── linux/        # New implementation
└── ui/
    └── tray/          # Cross-platform tray logic
```

## Windows Implementation

### Dependencies
```toml
# Cargo.toml
[target.'cfg(windows)'.dependencies]
tray-icon = "0.14"
windows = { version = "0.58", features = [
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Com",
]}
winit = "0.29"
winreg = "0.52"  # For auto-start
clipboard = "0.5"
```

### Key Features
- **System Tray**: Native Windows notification area
- **Context Menu**: Right-click menu with options
- **Auto-start**: Registry entries for startup
- **Clipboard**: Win32 clipboard API
- **Notifications**: Windows toast notifications

### Implementation Example
```rust
// src/platform/windows.rs
use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem}};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct WindowsTray {
    tray: TrayIcon,
}

impl WindowsTray {
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let toggle_item = MenuItem::new("Enable QuillFix", true, None);
        let clipboard_item = MenuItem::new("Correct Clipboard", true, None);
        let quit_item = MenuItem::new("Quit", true, None);
        
        menu.append(&toggle_item);
        menu.append(&clipboard_item);
        menu.append_separator();
        menu.append(&quit_item);
        
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("QuillFix")
            .build()?;
            
        Ok(Self { tray })
    }
    
    pub fn set_autostart(enabled: bool) -> Result<()> {
        // Use winreg to add/remove from HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    }
}
```

## Linux Implementation

### Dependencies
```toml
# Cargo.toml
[target.'cfg(target_os = "linux")'.dependencies]
tray-icon = "0.14"
libappindicator = "0.9"
gtk4 = "0.8"
libayatana-appindicator = "0.9"
xdg = "2.5"  # For config directories
```

### Key Features
- **System Tray**: libappindicator for cross-desktop support
- **Context Menu**: GTK4 menus
- **Auto-start**: .desktop files in ~/.config/autostart/
- **Clipboard**: GTK4 clipboard API
- **Notifications**: libnotify

### Implementation Example
```rust
// src/platform/linux.rs
use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem}};
use libappindicator::{AppIndicator, AppIndicatorStatus};

pub struct LinuxTray {
    indicator: AppIndicator,
    tray: TrayIcon,
}

impl LinuxTray {
    pub fn new() -> Result<Self> {
        let indicator = AppIndicator::new("quillfix", "quillfix-icon");
        indicator.set_status(AppIndicatorStatus::Active);
        
        let menu = Menu::new();
        let toggle_item = MenuItem::new("Enable QuillFix", true, None);
        let clipboard_item = MenuItem::new("Correct Clipboard", true, None);
        let quit_item = MenuItem::new("Quit", true, None);
        
        menu.append(&toggle_item);
        menu.append(&clipboard_item);
        menu.append_separator();
        menu.append(&quit_item);
        
        indicator.set_menu(&mut menu);
        
        Ok(Self { indicator, tray })
    }
    
    pub fn set_autostart(enabled: bool) -> Result<()> {
        // Create/remove .desktop file in ~/.config/autostart/
    }
}
```

## Cross-Platform System Tray

### Recommended Approach: `tray-icon` crate
- Single API for Windows, Linux, and macOS
- Native system tray integration
- Event handling for menu clicks
- Icon management

### Shared Features
```rust
// src/ui/tray/mod.rs
pub struct TrayApp {
    #[cfg(target_os = "macos")]
    macos_tray: crate::platform::macos::MacOSTray,
    #[cfg(windows)]
    windows_tray: crate::platform::windows::WindowsTray,
    #[cfg(target_os = "linux")]
    linux_tray: crate::platform::linux::LinuxTray,
}

impl TrayApp {
    pub fn new() -> Result<Self> {
        match std::env::consts::OS {
            "macos" => Ok(Self { macos_tray: MacOSTray::new()? }),
            "windows" => Ok(Self { windows_tray: WindowsTray::new()? }),
            "linux" => Ok(Self { linux_tray: LinuxTray::new()? }),
            _ => Err("Unsupported platform".into()),
        }
    }
    
    pub fn toggle_enabled(&mut self) {
        // Platform-specific toggle logic
    }
    
    pub fn correct_clipboard(&mut self) {
        // Shared clipboard correction logic
    }
}
```

## ML Backend Changes

### Current Dependencies
```toml
candle-core = { version = "0.9.2", features = ["metal"] }  # macOS only
```

### Cross-Platform Dependencies
```toml
candle-core = { version = "0.9.2", features = ["cuda"] }     # Windows
candle-core = { version = "0.9.2", features = ["accelerate"] }  # Linux (optional)
candle-core = { version = "0.9.2" }                        # CPU fallback
```

### Platform-Specific Features
```toml
[target.'cfg(target_os = "macos")'.dependencies]
candle-core = { version = "0.9.2", features = ["metal"] }

[target.'cfg(windows)'.dependencies]
candle-core = { version = "0.9.2", features = ["cuda"] }

[target.'cfg(target_os = "linux")'.dependencies]
candle-core = { version = "0.9.2", optional-features = ["accelerate"] }
```

## Build Configuration

### Updated build.rs
```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        for framework in ["AppKit", "CoreGraphics", "CoreFoundation", "ApplicationServices", "Accessibility"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    
    #[cfg(windows)]
    {
        // Windows-specific build configuration
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=shell32");
    }
    
    #[cfg(target_os = "linux")]
    {
        // Linux-specific build configuration
        println!("cargo:rustc-link-lib=appindicator");
        println!("cargo:rustc-link-lib=gtk4");
    }
}
```

### Updated Cargo.toml
```toml
[package]
name = "quillfix"
version = "0.1.0"
edition = "2024"
description = "QuillFix cross-platform text correction system tray app"
keywords = ["text", "correction", "system-tray", "nlp", "cross-platform"]
categories = ["text-processing", "os::macos-apis", "os::windows-apis", "os::linux-apis"]

[dependencies]
# Core dependencies (shared)
anyhow = "1.0.102"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
tokio = { version = "1.49.0", features = ["rt-multi-thread", "macros", "sync", "time"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.22", features = ["env-filter", "fmt"] }

# Cross-platform UI
tray-icon = "0.14"
winit = "0.29"

# Platform-specific dependencies
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10.1"
core-graphics = "0.25.0"
objc2 = "0.6.3"
objc2-app-kit = { version = "0.3.2", features = ["NSApplication", "NSMenu", "NSStatusBar"] }
objc2-foundation = { version = "0.3.2", features = ["NSArray", "NSString"] }
tracing-oslog = "0.3.0"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_Com",
]}
winreg = "0.52"
clipboard = "0.5"

[target.'cfg(target_os = "linux")'.dependencies]
libappindicator = "0.9"
gtk4 = "0.8"
libayatana-appindicator = "0.9"
xdg = "2.5"

# ML dependencies (platform-specific)
[target.'cfg(target_os = "macos")'.dependencies]
candle-core = { version = "0.9.2", optional = true, features = ["metal"] }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

[target.'cfg(windows)'.dependencies]
candle-core = { version = "0.9.2", optional = true, features = ["cuda"] }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
candle-core = { version = "0.9.2", optional = true }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

[features]
default = []
local-llm = [
    "dep:hf-hub",
    "candle-core",
    "candle-nn", 
    "candle-transformers",
    "tokenizers",
    "safetensors",
]
```

## Implementation Steps

1. **Create platform abstraction layer** (`src/platform/mod.rs`)
2. **Implement Windows system tray** (`src/platform/windows.rs`)
3. **Implement Linux system tray** (`src/platform/linux.rs`)
4. **Update main.rs for cross-platform support**
5. **Update build configuration**
6. **Test on Windows and Linux**
7. **Package for distribution** (MSI for Windows, AppImage/Flatpak for Linux)

## Distribution

### Windows
- **MSI installer** with `wix`
- **Auto-start** registry entries
- **Uninstaller** with clean removal

### Linux
- **AppImage** for universal distribution
- **Flatpak** for sandboxed distribution
- **.deb** for Debian/Ubuntu
- **.rpm** for Fedora/openSUSE

## Testing Strategy

### Unit Tests
- Platform abstraction layer
- Core correction logic
- Configuration management

### Integration Tests
- System tray functionality
- Clipboard operations
- Auto-start behavior

### Platform Tests
- Windows 10/11 compatibility
- Linux desktop environment testing (GNOME, KDE, XFCE)
- Accessibility features

## Timeline

1. **Week 1**: Platform abstraction layer + Windows implementation
2. **Week 2**: Linux implementation + cross-platform testing
3. **Week 3**: Build configuration + packaging
4. **Week 4**: Final testing + documentation

This approach maintains the exact same user experience across all platforms - just a system tray icon with menu options, no GUI window needed.
