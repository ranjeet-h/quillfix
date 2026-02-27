#![allow(dead_code)]

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use objc2::{MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject, sel};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuItem,
    NSPasteboard, NSPasteboardTypeString, NSStatusBar, NSVariableStatusItemLength,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{MainThreadMarker, NSArray, NSObject, NSObjectProtocol, NSString};
#[cfg(target_os = "macos")]
use std::cell::RefCell;

const DEFAULTS_DOMAIN: &str = "com.quillfix.app";
pub const DEFAULTS_KEY: &str = "quillfix.enabled";
pub const LOGIN_ITEM_NAME: &str = "QuillFix";
const FEEDBACK_URL: &str = "https://github.com/quillfix/quillfix/issues/new";

pub static IS_ENABLED: AtomicBool = AtomicBool::new(false);

// Menu handler that receives menu actions
#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "QuillFixHandler"]
    #[thread_kind = MainThreadOnly]
    pub struct MenuHandler;

    unsafe impl NSObjectProtocol for MenuHandler {}

    impl MenuHandler {
        #[unsafe(method(toggleQuill:))]
        fn toggle(&self, _sender: Option<&AnyObject>) {
            let new_state = toggle_enabled_inner();
            update_toggle_menu_item(new_state);
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, _sender: Option<&AnyObject>) {
            let next = !launch_at_login_enabled();
            set_launch_at_login(next);
            update_launch_at_login_menu_item(next);
        }

        #[unsafe(method(openAccessibilitySettings:))]
        fn open_accessibility_settings(&self, _sender: Option<&AnyObject>) {
            if let Err(e) = crate::permissions::open_accessibility_settings() {
                tracing::error!(phase = "menu", ?e, "failed to open accessibility settings");
            }
        }

        #[unsafe(method(reportIssue:))]
        fn report_issue(&self, _sender: Option<&AnyObject>) {
            if let Err(e) = Command::new("open").arg(FEEDBACK_URL).status() {
                tracing::error!(phase = "menu", ?e, "failed to open feedback URL");
            }
        }

        #[unsafe(method(correctClipboardText:))]
        fn correct_clipboard_text(&self, _sender: Option<&AnyObject>) {
            use crate::llm::prompt::CorrectionResult;

            let pboard = NSPasteboard::generalPasteboard();
            let text = unsafe { pboard.stringForType(NSPasteboardTypeString) };
            let text = if let Some(t) = text {
                t.to_string()
            } else {
                tracing::warn!(phase = "clipboard", "no plain text in clipboard");
                return;
            };
            if text.trim().is_empty() {
                tracing::warn!(phase = "clipboard", "clipboard text is empty");
                return;
            }

            let corrector_arc = crate::corrector::get();
            let result = match corrector_arc.lock() {
                Ok(c) => c.correct(&text),
                Err(e) => {
                    tracing::error!(phase = "clipboard", ?e, "corrector lock poisoned");
                    return;
                }
            };

            match result {
                Ok(CorrectionResult::Changed(corrected)) => {
                    let _ = pboard.clearContents();
                    let _ = pboard.setString_forType(
                        &NSString::from_str(&corrected),
                        unsafe { NSPasteboardTypeString },
                    );
                    tracing::info!(phase = "clipboard", "clipboard text corrected");
                }
                Ok(CorrectionResult::Unchanged) => {
                    tracing::info!(phase = "clipboard", "clipboard text unchanged");
                }
                Ok(CorrectionResult::Error(msg)) => {
                    tracing::error!(phase = "clipboard", msg, "correction failed");
                }
                Err(e) => {
                    tracing::error!(phase = "clipboard", ?e, "correction failed");
                }
            }
        }

        /// `NSServices` handler. macOS calls this when the user invokes
        /// "Correct with QuillFix" from another app's Services menu.
        /// The selector must match `NSMessage` in Info.plist exactly:
        ///   `quillFixCorrectText:userData:error:`
        #[unsafe(method(quillFixCorrectText:userData:error:))]
        fn service_correct_text(
            &self,
            pboard: &NSPasteboard,
            _user_data: Option<&NSString>,
            _error: *mut AnyObject,
        ) {
            use crate::llm::prompt::CorrectionResult;

            let text = unsafe { pboard.stringForType(NSPasteboardTypeString) };
            let text = if let Some(t) = text {
                t.to_string()
            } else {
                tracing::warn!(phase = "service", "no plain-text on pasteboard");
                return;
            };
            if text.trim().is_empty() {
                return;
            }

            let corrector_arc = crate::corrector::get();
            let result = match corrector_arc.lock() {
                Ok(c) => c.correct(&text),
                Err(e) => {
                    tracing::error!(phase = "service", ?e, "corrector lock poisoned");
                    return;
                }
            };

            match result {
                Ok(CorrectionResult::Changed(corrected)) => {
                    let _ = pboard.clearContents();
                    let _ = pboard.setString_forType(
                        &NSString::from_str(&corrected),
                        unsafe { NSPasteboardTypeString },
                    );
                    tracing::info!(phase = "service", "corrected via NSService");
                }
                Ok(CorrectionResult::Unchanged) => {
                    tracing::info!(phase = "service", "unchanged");
                }
                Ok(CorrectionResult::Error(msg)) => {
                    tracing::error!(phase = "service", msg, "model error");
                }
                Err(e) => {
                    tracing::error!(phase = "service", ?e, "correction failed");
                }
            }
        }
    }
);

#[cfg(target_os = "macos")]
impl MenuHandler {
    #[must_use]
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![Self::alloc(mtm), init] }
    }
}

pub fn run() {
    setup();

    let enabled = load_enabled_state();
    IS_ENABLED.store(enabled, Ordering::SeqCst);

    update_toggle_menu_item(enabled);
    update_launch_at_login_menu_item(launch_at_login_enabled());
}

#[must_use]
pub fn toggle_enabled() -> bool {
    let new_state = toggle_enabled_inner();
    update_toggle_menu_item(new_state);
    new_state
}

#[must_use]
pub fn load_enabled_state() -> bool {
    #[cfg(target_os = "macos")]
    {
        let output =
            Command::new("defaults").args(["read", DEFAULTS_DOMAIN, DEFAULTS_KEY]).output();
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

pub fn save_enabled_state(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        let value = if enabled { "true" } else { "false" };
        let _ = Command::new("defaults")
            .args(["write", DEFAULTS_DOMAIN, DEFAULTS_KEY, "-bool", value])
            .output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}

fn toggle_enabled_inner() -> bool {
    let new_state = !IS_ENABLED.load(Ordering::SeqCst);
    IS_ENABLED.store(new_state, Ordering::SeqCst);
    save_enabled_state(new_state);
    new_state
}

// ==================== Menu Bar UI ====================

#[cfg(target_os = "macos")]
thread_local! {
    static STATUS_ITEM: RefCell<Option<Retained<objc2_app_kit::NSStatusItem>>> = const { RefCell::new(None) };
    static TOGGLE_ITEM: RefCell<Option<Retained<NSMenuItem>>> = const { RefCell::new(None) };
    static LAUNCH_AT_LOGIN_ITEM: RefCell<Option<Retained<NSMenuItem>>> = const { RefCell::new(None) };
    static HANDLER: RefCell<Option<Retained<MenuHandler>>> = const { RefCell::new(None) };
}

#[cfg(target_os = "macos")]
pub fn setup() {
    if STATUS_ITEM.with(|s| s.borrow().is_some()) {
        return;
    }

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let handler = MenuHandler::new(mtm);
    HANDLER.with(|s| *s.borrow_mut() = Some(handler.clone()));

    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    let symbol = NSString::from_str("sparkles");
    if let Some(icon) = NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol, None) {
        icon.setTemplate(true);
        if let Some(btn) = status_item.button(mtm) {
            btn.setImage(Some(&icon));
        }
    }

    let menu = NSMenu::new(mtm);

    // Toggle
    let toggle_title = NSString::from_str("Enable QuillFix");
    let toggle_item = NSMenuItem::new(mtm);
    toggle_item.setTitle(&toggle_title);
    unsafe { toggle_item.setAction(Some(sel!(toggleQuill:))) };
    unsafe { toggle_item.setTarget(Some(&*handler)) };
    menu.addItem(&toggle_item);

    // Launch at Login
    let launch_title = NSString::from_str("Launch at Login");
    let launch_item = NSMenuItem::new(mtm);
    launch_item.setTitle(&launch_title);
    unsafe { launch_item.setAction(Some(sel!(toggleLaunchAtLogin:))) };
    unsafe { launch_item.setTarget(Some(&*handler)) };
    menu.addItem(&launch_item);

    // Clipboard fallback
    let clipboard_item = NSMenuItem::new(mtm);
    clipboard_item.setTitle(&NSString::from_str("Correct Clipboard Text"));
    unsafe { clipboard_item.setAction(Some(sel!(correctClipboardText:))) };
    unsafe { clipboard_item.setTarget(Some(&*handler)) };
    menu.addItem(&clipboard_item);

    // Open Accessibility Settings
    let accessibility_item = NSMenuItem::new(mtm);
    accessibility_item.setTitle(&NSString::from_str("Open Accessibility Settings"));
    unsafe { accessibility_item.setAction(Some(sel!(openAccessibilitySettings:))) };
    unsafe { accessibility_item.setTarget(Some(&*handler)) };
    menu.addItem(&accessibility_item);

    // Feedback
    let feedback_item = NSMenuItem::new(mtm);
    feedback_item.setTitle(&NSString::from_str("Report an Issue"));
    unsafe { feedback_item.setAction(Some(sel!(reportIssue:))) };
    unsafe { feedback_item.setTarget(Some(&*handler)) };
    menu.addItem(&feedback_item);

    // Separator
    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // Quit
    let quit_item = NSMenuItem::new(mtm);
    quit_item.setTitle(&NSString::from_str("Quit QuillFix"));
    unsafe { quit_item.setAction(Some(sel!(terminate:))) };
    unsafe { quit_item.setTarget(None) };
    menu.addItem(&quit_item);

    status_item.setMenu(Some(&menu));

    STATUS_ITEM.with(|s| *s.borrow_mut() = Some(status_item));
    TOGGLE_ITEM.with(|s| *s.borrow_mut() = Some(toggle_item));
    LAUNCH_AT_LOGIN_ITEM.with(|s| *s.borrow_mut() = Some(launch_item));
}

#[cfg(not(target_os = "macos"))]
pub fn setup() {}

#[must_use]
pub fn is_setup() -> bool {
    #[cfg(target_os = "macos")]
    {
        STATUS_ITEM.with(|s| s.borrow().is_some())
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn update_toggle_menu_item(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        TOGGLE_ITEM.with(|slot| {
            if let Some(item) = slot.borrow().as_ref() {
                let title = if enabled { "Disable QuillFix" } else { "Enable QuillFix" };
                item.setTitle(&NSString::from_str(title));
                item.setState(if enabled { NSControlStateValueOn } else { NSControlStateValueOff });
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}

fn update_launch_at_login_menu_item(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        LAUNCH_AT_LOGIN_ITEM.with(|slot| {
            if let Some(item) = slot.borrow().as_ref() {
                item.setState(if enabled { NSControlStateValueOn } else { NSControlStateValueOff });
            }
        });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}

/// Register this app as an `NSServices` provider so that "Correct with QuillFix"
/// appears in the Services menu (and Keyboard > Keyboard Shortcuts > Services)
/// of other applications.
///
/// Must be called after `NSApplication::sharedApplication()` has been obtained.
#[cfg(target_os = "macos")]
pub fn register_services(app: &NSApplication) {
    // Tell AppKit which pasteboard types we send and return.
    let send_type = NSString::from_str("public.utf8-plain-text");
    let return_type = NSString::from_str("public.utf8-plain-text");
    let send_types = NSArray::from_slice(&[send_type.as_ref()]);
    let return_types = NSArray::from_slice(&[return_type.as_ref()]);
    app.registerServicesMenuSendTypes_returnTypes(&send_types, &return_types);

    // Point the app's servicesProvider to our handler object, so macOS
    // routes `quillFixCorrectText:userData:error:` to it.
    HANDLER.with(|h| {
        if let Some(handler) = h.borrow().as_ref() {
            unsafe { app.setServicesProvider(Some(handler.as_ref())) };
        }
    });

    tracing::info!(phase = "service", "NSServices provider registered");
}

#[cfg(not(target_os = "macos"))]
pub fn register_services(_app: &()) {}

// ==================== Login Item ====================

pub fn set_launch_at_login(enabled: bool) {
    #[cfg(target_os = "macos")]
    {
        let script = if enabled {
            "tell application \"System Events\" to make login item at end with properties {path:\"/Applications/QuillFix.app\", hidden:false}"
        } else {
            "tell application \"System Events\" to delete login item \"QuillFix\""
        };
        let _ = Command::new("osascript").args(["-e", script]).output();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
    }
}

#[must_use]
pub fn launch_at_login_enabled() -> bool {
    #[cfg(target_os = "macos")]
    {
        let script = "tell application \"System Events\" to get name of every login item";
        Command::new("osascript")
            .args(["-e", script])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("QuillFix"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
