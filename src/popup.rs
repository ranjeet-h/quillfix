#![allow(dead_code)]

use crate::ax_query::{CGRect, SelectionInfo};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(target_os = "macos"))]
fn get_clipboard_text() -> Option<String> {
    None
}

// ── macOS-only imports ──────────────────────────────────────────────────────
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSBackingStoreType, NSEvent, NSEventMask, NSFloatingWindowLevel, NSImage, NSImageView, NSPanel,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowStyleMask,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{MainThreadMarker, NSPoint as FNSPoint, NSRect, NSSize, NSString};
#[cfg(target_os = "macos")]
use objc2_quartz_core::{CABasicAnimation, CAMediaTiming};

// ── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NSPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct PopupController {
    visible: bool,
    icon_name: String,
    auto_hide_at: Option<Instant>,
    screen_width: f64,
    screen_height: f64,
}

impl Default for PopupController {
    fn default() -> Self {
        Self {
            visible: false,
            icon_name: "sparkles".to_string(),
            auto_hide_at: None,
            screen_width: 1440.0,
            screen_height: 900.0,
        }
    }
}

impl PopupController {
    pub const fn set_screen_bounds(&mut self, width: f64, height: f64) {
        self.screen_width = width;
        self.screen_height = height;
    }

    #[must_use]
    pub fn position_near(&self, bounds: CGRect) -> NSPoint {
        position_near_with_screen(bounds, self.screen_width, self.screen_height)
    }

    pub fn show(&mut self, at: NSPoint) {
        self.visible = true;
        self.icon_name = "sparkles".to_string();
        self.auto_hide_at = Some(Instant::now() + Duration::from_millis(4000));
        show_panel(at, "sparkles");
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.auto_hide_at = None;
        hide_panel();
    }

    pub fn show_success(&mut self) {
        self.icon_name = "checkmark.circle.fill".to_string();
        update_panel_icon("checkmark.circle.fill");
        thread::sleep(Duration::from_millis(180));
        self.hide();
        self.icon_name = "sparkles".to_string();
    }

    pub fn tick(&mut self) {
        if self.auto_hide_at.is_some_and(|deadline| Instant::now() >= deadline) {
            self.hide();
        }
    }

    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    #[must_use]
    pub fn icon_name(&self) -> &str {
        &self.icon_name
    }
}

// ── Statics ─────────────────────────────────────────────────────────────────

static POPUP: OnceLock<Arc<Mutex<PopupController>>> = OnceLock::new();

/// Shared store of the most recent selection, updated by the event monitor.
static LAST_SELECTION: OnceLock<Arc<Mutex<Option<SelectionInfo>>>> = OnceLock::new();

/// Stored corrector function for the real `NSPanel` click handler.
type CorrectorFn =
    dyn Fn(&str) -> anyhow::Result<crate::llm::prompt::CorrectionResult> + Send + Sync;
static CLICK_CORRECTOR: OnceLock<Arc<CorrectorFn>> = OnceLock::new();

/// Register the corrector function that the `NSPanel` click handler invokes.
///
/// Must be called once from `main()` before the panel is shown.
pub fn set_click_corrector<F>(f: F)
where
    F: Fn(&str) -> anyhow::Result<crate::llm::prompt::CorrectionResult> + Send + Sync + 'static,
{
    let _ = CLICK_CORRECTOR.set(Arc::new(f));
}

/// Dispatch a correction via the registered click corrector.
fn handle_panel_click() {
    let Some(corrector) = CLICK_CORRECTOR.get().cloned() else {
        return;
    };
    on_click(move |text| corrector(text));
}

#[must_use]
pub fn controller() -> Arc<Mutex<PopupController>> {
    POPUP.get_or_init(|| Arc::new(Mutex::new(PopupController::default()))).clone()
}

#[must_use]
pub fn last_selection_store() -> Arc<Mutex<Option<SelectionInfo>>> {
    LAST_SELECTION.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

#[must_use]
pub fn create() -> Arc<Mutex<PopupController>> {
    controller()
}

#[must_use]
pub fn position_near(bounds: CGRect) -> NSPoint {
    position_near_with_screen(bounds, 1440.0, 900.0)
}

pub fn show(at: NSPoint) {
    if let Ok(mut inner) = controller().lock() {
        inner.show(at);
    }
}

pub fn hide() {
    if let Ok(mut inner) = controller().lock() {
        inner.hide();
    }
}

#[must_use]
pub fn is_visible() -> bool {
    controller().lock().is_ok_and(|inner| inner.is_visible())
}

/// Call this when the user clicks the popup icon.
///
/// Reads the last `SelectionInfo`, dispatches correction on a background thread,
/// then shows the success checkmark or hides on error.
///
/// The `corrector_fn` parameter is injectable for testing.
pub fn on_click<F>(corrector_fn: F)
where
    F: FnOnce(&str) -> anyhow::Result<crate::llm::prompt::CorrectionResult> + Send + 'static,
{
    use crate::llm::prompt::CorrectionResult;
    use crate::replacement::{clipboard_replace, replace_with_fallback};

    // Try to get selection from store, or fallback to clipboard
    let selection = last_selection_store()
        .lock()
        .ok()
        .and_then(|guard: std::sync::MutexGuard<Option<SelectionInfo>>| guard.clone());

    let sel = match selection {
        Some(s) => s,
        None => {
            // No stored selection - hide popup
            tracing::warn!(phase = "popup", "no stored selection");
            hide();
            return;
        }
    };

    let popup = controller();
    let sel_text = sel.text.clone();
    let sel_role = sel.element_role.clone();
    thread::spawn(move || {
        let result = corrector_fn(&sel_text);
        match result {
            Ok(CorrectionResult::Changed(corrected)) => {
                // Try AX replacement first, then clipboard fallback
                if replace_with_fallback(&sel_role, &corrected).is_err() {
                    // Try pure clipboard method as last resort
                    if clipboard_replace(&corrected).is_err() {
                        tracing::error!(
                            phase = "replacement",
                            text_len = sel_text.len(),
                            "both AX and clipboard replacement failed"
                        );
                        hide();
                        return;
                    }
                }
                if let Ok(mut inner) = popup.lock() {
                    inner.show_success();
                }
            }
            Ok(CorrectionResult::Unchanged) => {
                if let Ok(mut inner) = popup.lock() {
                    inner.show_success();
                }
            }
            Ok(CorrectionResult::Error(msg)) => {
                tracing::error!(
                    phase = "correction",
                    error = msg,
                    text_len = sel.text.len(),
                    "correction error"
                );
                if let Ok(mut inner) = popup.lock() {
                    inner.hide();
                }
            }
            Err(err) => {
                tracing::error!(
                    phase = "correction",
                    ?err,
                    text_len = sel.text.len(),
                    "correction returned Err"
                );
                if let Ok(mut inner) = popup.lock() {
                    inner.hide();
                }
            }
        }
    });
}

fn position_near_with_screen(bounds: CGRect, screen_width: f64, screen_height: f64) -> NSPoint {
    let x = (bounds.x + bounds.width + 8.0).clamp(0.0, (screen_width - 32.0).max(0.0));
    let y = (bounds.y + bounds.height + 8.0).clamp(0.0, (screen_height - 32.0).max(0.0));
    NSPoint { x, y }
}

// ── macOS NSPanel implementation ────────────────────────────────────────────

/// Sends a closure to the main queue via GCD.  Safe to call from any thread.
#[cfg(target_os = "macos")]
fn dispatch_async_main<F: FnOnce() + Send + 'static>(f: F) {
    dispatch2::DispatchQueue::main().exec_async(f);
}

/// Wrapper around `Retained<AnyObject>` that is `Send + Sync`.
///
/// # Safety
/// The wrapped object is an `NSEvent` monitor token that is only created and
/// destroyed on the main thread.  We manually assert the required traits because
/// Objective-C objects that internally hold raw pointers are not automatically
/// `Send`/`Sync` in objc2.  The token is never accessed concurrently; it is
/// kept alive solely to prevent the monitor from being deregistered.
#[cfg(target_os = "macos")]
struct MonitorToken(objc2::rc::Retained<objc2::runtime::AnyObject>);

#[cfg(target_os = "macos")]
#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: Monitor tokens are only created/dropped on the main thread.
unsafe impl Send for MonitorToken {}
#[cfg(target_os = "macos")]
// SAFETY: same as above.
unsafe impl Sync for MonitorToken {}

/// Internal panel state — only ever accessed on the main thread.
#[cfg(target_os = "macos")]
struct PanelState {
    panel: objc2::rc::Retained<NSPanel>,
    image_view: objc2::rc::Retained<NSImageView>,
    /// Opaque token returned by `addGlobalMonitorForEventsMatchingMask:handler:`.
    /// Kept alive to prevent the monitor from being de-registered.
    _mouse_monitor: MonitorToken,
    _key_monitor: MonitorToken,
    _click_monitor: MonitorToken,
}

/// `PanelState` is only ever created and mutated on the main thread; the
/// `OnceLock<Mutex<…>>` wrapper provides the cross-thread lock.
#[cfg(target_os = "macos")]
#[allow(clippy::non_send_fields_in_send_ty)]
// SAFETY: all access is behind a Mutex and happens on the main thread.
unsafe impl Send for PanelState {}
#[cfg(target_os = "macos")]
// SAFETY: same as above.
unsafe impl Sync for PanelState {}

/// Global panel state.  `None` until `ensure_panel` is called on the main thread.
#[cfg(target_os = "macos")]
static PANEL: OnceLock<Mutex<Option<PanelState>>> = OnceLock::new();

#[cfg(target_os = "macos")]
fn panel_store() -> &'static Mutex<Option<PanelState>> {
    PANEL.get_or_init(|| Mutex::new(None))
}

/// Create (or return cached) the floating `NSPanel`.  **Must run on the main thread.**
#[cfg(target_os = "macos")]
fn ensure_panel(mtm: MainThreadMarker) {
    let Ok(mut guard) = panel_store().lock() else { return };
    if guard.is_some() {
        return;
    }

    // ── Build NSPanel ──────────────────────────────────────────────────────
    let panel_size = 40.0_f64;
    let content_rect = NSRect {
        origin: FNSPoint { x: 0.0, y: 0.0 },
        size: NSSize { width: panel_size, height: panel_size },
    };

    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;

    let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
        mtm.alloc::<NSPanel>(),
        content_rect,
        style,
        NSBackingStoreType::Buffered,
        false,
    );

    // Transparent, floating, non-activating
    panel.setOpaque(false);
    panel.setAlphaValue(0.0);
    panel.setLevel(NSFloatingWindowLevel + 1);
    panel.setBecomesKeyOnlyIfNeeded(true);

    // Clear background (vibrancy handles rendering)
    let clear = objc2_app_kit::NSColor::clearColor();
    panel.setBackgroundColor(Some(&clear));

    // ── NSVisualEffectView ────────────────────────────────────────────────
    let effect_view = NSVisualEffectView::new(mtm);
    effect_view.setMaterial(NSVisualEffectMaterial::HUDWindow);
    effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect_view.setState(NSVisualEffectState::Active);
    effect_view.setWantsLayer(true);
    effect_view.setFrame(NSRect {
        origin: FNSPoint { x: 0.0, y: 0.0 },
        size: NSSize { width: panel_size, height: panel_size },
    });

    // Round corners on the layer
    if let Some(layer) = effect_view.layer() {
        layer.setCornerRadius(8.0);
        layer.setMasksToBounds(true);
    }

    panel.setContentView(Some(effect_view.as_ref()));

    // ── NSImageView with SF Symbol ────────────────────────────────────────
    let icon_margin = 8.0_f64;
    let icon_size = icon_margin.mul_add(-2.0, panel_size);
    let image_view = NSImageView::new(mtm);
    image_view.setFrame(NSRect {
        origin: FNSPoint { x: icon_margin, y: icon_margin },
        size: NSSize { width: icon_size, height: icon_size },
    });
    set_image_view_symbol(&image_view, "sparkles");
    effect_view.addSubview(image_view.as_ref());

    // ── NSEvent global monitors (mouse-down and ESC) ───────────────────────
    let mouse_monitor = install_global_mouse_monitor(mtm);
    let key_monitor = install_global_key_monitor(mtm);

    // ── Local click monitor → on_click ────────────────────────────────────
    let click_monitor = install_local_click_monitor(mtm);

    *guard = Some(PanelState {
        panel,
        image_view,
        _mouse_monitor: MonitorToken(mouse_monitor),
        _key_monitor: MonitorToken(key_monitor),
        _click_monitor: MonitorToken(click_monitor),
    });
}

/// Update the SF Symbol shown in the image view.  **Main thread only.**
#[cfg(target_os = "macos")]
fn set_image_view_symbol(image_view: &NSImageView, symbol_name: &str) {
    let name = NSString::from_str(symbol_name);
    if let Some(img) =
        NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None::<&NSString>)
    {
        image_view.setImage(Some(&img));
    }
}

/// Position the panel and animate it in.  **Main thread only.**
#[cfg(target_os = "macos")]
fn show_panel_on_main(at: NSPoint, icon: &str) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    ensure_panel(mtm);

    let Ok(guard) = panel_store().lock() else { return };
    let Some(state) = guard.as_ref() else { return };

    // Move window origin
    state.panel.setFrameOrigin(FNSPoint { x: at.x, y: at.y });
    // Update icon
    set_image_view_symbol(&state.image_view, icon);

    // Make visible with CABasicAnimation (opacity + scale)
    state.panel.setAlphaValue(0.0);
    state.panel.orderFront(None);

    animate_show(&state.panel);
}

/// Animate the panel out and order it off screen.  **Main thread only.**
#[cfg(target_os = "macos")]
fn hide_panel_on_main() {
    let Ok(guard) = panel_store().lock() else { return };
    let Some(state) = guard.as_ref() else { return };
    animate_hide_and_order_out(&state.panel);
}

/// Update the icon symbol on the existing panel.  **Main thread only.**
#[cfg(target_os = "macos")]
fn update_icon_on_main(icon: &str) {
    let Ok(guard) = panel_store().lock() else { return };
    let Some(state) = guard.as_ref() else { return };
    set_image_view_symbol(&state.image_view, icon);
}

// ── Animation helpers ───────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn make_number_float(val: f32) -> objc2::rc::Retained<objc2_foundation::NSNumber> {
    objc2_foundation::NSNumber::numberWithFloat(val)
}

#[cfg(target_os = "macos")]
fn animate_show(panel: &NSPanel) {
    if let Some(content_view) = panel.contentView()
        && let Some(layer) = content_view.layer()
    {
        // Opacity: 0 → 1
        let opacity_anim =
            CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
        opacity_anim.setDuration(0.15);
        let from_val = make_number_float(0.0_f32);
        let to_val = make_number_float(1.0_f32);
        unsafe {
            opacity_anim.setFromValue(Some(from_val.as_ref()));
            opacity_anim.setToValue(Some(to_val.as_ref()));
        }
        opacity_anim.setRemovedOnCompletion(false);
        layer.addAnimation_forKey(opacity_anim.as_ref(), Some(&NSString::from_str("show-opacity")));

        // Scale: 0.8 → 1.0 (bounce-in feel)
        let scale_anim =
            CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("transform.scale")));
        scale_anim.setDuration(0.15);
        let from_val = make_number_float(0.8_f32);
        let to_val = make_number_float(1.0_f32);
        unsafe {
            scale_anim.setFromValue(Some(from_val.as_ref()));
            scale_anim.setToValue(Some(to_val.as_ref()));
        }
        scale_anim.setRemovedOnCompletion(false);
        layer.addAnimation_forKey(scale_anim.as_ref(), Some(&NSString::from_str("show-scale")));
    }
    panel.setAlphaValue(1.0);
}

#[cfg(target_os = "macos")]
fn animate_hide_and_order_out(panel: &NSPanel) {
    if let Some(content_view) = panel.contentView()
        && let Some(layer) = content_view.layer()
    {
        // Opacity: 1 → 0
        let opacity_anim =
            CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
        opacity_anim.setDuration(0.12);
        let from_val = make_number_float(1.0_f32);
        let to_val = make_number_float(0.0_f32);
        unsafe {
            opacity_anim.setFromValue(Some(from_val.as_ref()));
            opacity_anim.setToValue(Some(to_val.as_ref()));
        }
        opacity_anim.setRemovedOnCompletion(false);
        layer.addAnimation_forKey(opacity_anim.as_ref(), Some(&NSString::from_str("hide-opacity")));
    }
    panel.setAlphaValue(0.0);
    panel.orderOut(None);
}

// ── Local click monitor (panel click → on_click) ──────────────────────────

/// Install an `NSEvent` local monitor for `LeftMouseDown` that fires
/// `handle_panel_click()` when the click lands on our panel's window.
///
/// Returns an opaque monitor token; dropping it deregisters the monitor.
#[cfg(target_os = "macos")]
fn install_local_click_monitor(
    _mtm: MainThreadMarker,
) -> objc2::rc::Retained<objc2::runtime::AnyObject> {
    use block2::RcBlock;

    let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        let click_on_panel = PANEL.get().is_some_and(|panel_guard| {
            panel_guard.lock().is_ok_and(|g| {
                g.as_ref().is_some_and(|state| {
                    let our_wn: isize = unsafe { msg_send![&*state.panel, windowNumber] };
                    let click_wn: isize = unsafe { msg_send![event_ref, windowNumber] };
                    click_wn == our_wn
                })
            })
        });

        if click_on_panel {
            handle_panel_click();
            std::ptr::null_mut() // consume the event
        } else {
            event.as_ptr() // pass through
        }
    });

    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, &block)
    };

    monitor.unwrap_or_else(|| unsafe { objc2::msg_send![objc2::class!(NSObject), new] })
}

// ── NSEvent global monitors ─────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn install_global_mouse_monitor(
    _mtm: MainThreadMarker,
) -> objc2::rc::Retained<objc2::runtime::AnyObject> {
    use block2::RcBlock;

    let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
        // Dismiss if the click is outside our panel
        let event_ref = unsafe { event.as_ref() };
        let hide_it = PANEL.get().is_some_and(|panel_guard| {
            panel_guard.lock().is_ok_and(|g| {
                g.as_ref().is_some_and(|state| {
                    let our_window_num: isize = unsafe { msg_send![&*state.panel, windowNumber] };
                    let click_window_num: isize = unsafe { msg_send![event_ref, windowNumber] };
                    click_window_num != our_window_num
                })
            })
        });

        if hide_it {
            hide();
        }
    });

    let monitor =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::LeftMouseDown, &block);

    // addGlobalMonitor returns an id; if nil just return a dummy NSObject.
    monitor.unwrap_or_else(|| unsafe { objc2::msg_send![objc2::class!(NSObject), new] })
}

#[cfg(target_os = "macos")]
fn install_global_key_monitor(
    _mtm: MainThreadMarker,
) -> objc2::rc::Retained<objc2::runtime::AnyObject> {
    use block2::RcBlock;

    let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| {
        let event_ref = unsafe { event.as_ref() };
        // keyCode 53 = Escape
        let key_code: u16 = unsafe { msg_send![event_ref, keyCode] };
        if key_code == 53 {
            hide();
        }
    });

    let monitor =
        NSEvent::addGlobalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block);

    monitor.unwrap_or_else(|| unsafe { objc2::msg_send![objc2::class!(NSObject), new] })
}

// ── Public platform-dispatch functions ─────────────────────────────────────

/// Show the panel at `at` with the given SF Symbol icon.
/// Dispatches to the main thread via GCD if not already on it.
fn show_panel(at: NSPoint, icon: &str) {
    #[cfg(target_os = "macos")]
    {
        let icon_owned = icon.to_string();
        dispatch_async_main(move || {
            show_panel_on_main(at, &icon_owned);
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (at, icon);
    }
}

/// Hide the panel.  Dispatches to main thread via GCD.
fn hide_panel() {
    #[cfg(target_os = "macos")]
    {
        dispatch_async_main(hide_panel_on_main);
    }
}

/// Update the panel icon symbol.  Dispatches to main thread via GCD.
fn update_panel_icon(icon: &str) {
    #[cfg(target_os = "macos")]
    {
        let icon_owned = icon.to_string();
        dispatch_async_main(move || {
            update_icon_on_main(&icon_owned);
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = icon;
    }
}
