#![allow(dead_code)]

use crate::ax_query::{CGRect, query_selection};
use crate::debounce::Debouncer;
use crate::popup;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// ── macOS-only imports ──────────────────────────────────────────────────────
#[cfg(target_os = "macos")]
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::runloop::{CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes};
#[cfg(target_os = "macos")]
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CallbackResult,
};
#[cfg(target_os = "macos")]
use std::ffi::c_void;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionCandidate {
    pub pid: i32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorMode {
    Hybrid,
    ObserverOnly,
}

static IS_RUNNING: AtomicBool = AtomicBool::new(false);
static CALLBACKS_THIS_SECOND: AtomicUsize = AtomicUsize::new(0);
static LAST_SECOND_TICK: OnceLock<Mutex<Instant>> = OnceLock::new();
static DEBOUNCE_MS: AtomicUsize = AtomicUsize::new(300);
static DEBOUNCER: OnceLock<Mutex<Debouncer>> = OnceLock::new();
/// Guards the event-tap background thread so it is spawned at most once per
/// start/stop cycle.  Reset to `false` by `stop()` so that `start()` can
/// re-launch the thread in subsequent calls (important for tests).
#[cfg(target_os = "macos")]
static THREAD_STARTED: AtomicBool = AtomicBool::new(false);

// ── AXObserver FFI (macOS only) ────────────────────────────────────────────
#[cfg(target_os = "macos")]
type AXObserverRef = *mut c_void;
#[cfg(target_os = "macos")]
type AXUIElementRef = *const c_void;
#[cfg(target_os = "macos")]
type AXError = i32;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXObserverCreate(
        application: i32,
        callback: unsafe extern "C" fn(
            observer: AXObserverRef,
            element: AXUIElementRef,
            notification: CFTypeRef,
            user_info: *mut c_void,
        ),
        out_observer: *mut AXObserverRef,
    ) -> AXError;

    fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFTypeRef,
        user_info: *mut c_void,
    ) -> AXError;

    fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFTypeRef;

    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
}

/// Start the global event monitor:
/// - On macOS: spawns a background thread that runs `CFRunLoop` with a
///   `CGEventTap` watching mouse-up events.  The callback queries AX selection
///   and routes it to the popup.
/// - On other platforms: only sets `IS_RUNNING`.
pub fn start() {
    IS_RUNNING.store(true, Ordering::SeqCst);

    #[cfg(target_os = "macos")]
    {
        spawn_event_tap_thread();
    }
}

/// Stop the global event monitor:
/// - On macOS: the background thread self-terminates because `IS_RUNNING` is
///   `false`; the `CFRunLoop` stops at its next iteration.
/// - Resets rate-limit and debounce state.
pub fn stop() {
    IS_RUNNING.store(false, Ordering::SeqCst);
    CALLBACKS_THIS_SECOND.store(0, Ordering::SeqCst);
    #[cfg(target_os = "macos")]
    THREAD_STARTED.store(false, Ordering::SeqCst);
    if let Some(debouncer) = DEBOUNCER.get()
        && let Ok(mut inner) = debouncer.lock()
    {
        *inner = Debouncer::new(current_debounce_ms());
    }
}

#[must_use]
pub fn is_running() -> bool {
    IS_RUNNING.load(Ordering::SeqCst)
}

/// Register an `AXObserver` for `kAXSelectedTextChangedNotification` on the
/// application with the given `pid`.
///
/// Returns `MonitorMode::Hybrid` when the real observer was installed (or when
/// already installed), `MonitorMode::ObserverOnly` on failure.
///
/// **Must be called from the thread that owns the `CFRunLoop`** (i.e., from
/// inside the event-tap background thread).
#[must_use]
pub fn register_ax_observer(pid: i32) -> MonitorMode {
    #[cfg(target_os = "macos")]
    {
        register_ax_observer_macos(pid)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        MonitorMode::Hybrid
    }
}

// ── AXObserver callback ────────────────────────────────────────────────────

/// Callback fired by the `AXObserver` when `AXSelectedTextChanged` arrives.
/// Queries the current AX selection and routes it to the popup.
#[cfg(target_os = "macos")]
unsafe extern "C" fn ax_observer_callback(
    _observer: AXObserverRef,
    _element: AXUIElementRef,
    _notification: CFTypeRef,
    _user_info: *mut c_void,
) {
    if !is_running() {
        return;
    }
    route_ax_selection();
}

/// Query the focused element's selection and display popup.
#[cfg(target_os = "macos")]
fn route_ax_selection() {
    tracing::debug!(phase = "route", "starting");
    let result = crate::ax_query::query_focused_selection();
    tracing::debug!(phase = "route", ?result, "query result");
    if let Some((selection, bounds)) = result {
        tracing::debug!(phase = "route", text = %selection.text, "showing popup");
        let point = popup::position_near(bounds);
        let store = popup::last_selection_store();
        if let Ok(mut slot) = store.lock() {
            *slot = Some(selection.clone());
        }
        popup::show(point);
    }
}

// ── AXObserver registration ────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn register_ax_observer_macos(pid: i32) -> MonitorMode {
    use core_foundation::string::CFString;

    // Safety: all pointers are checked; the observer's run-loop source is
    // added to the *current* thread's run loop (the event-tap thread).
    unsafe {
        let mut observer: AXObserverRef = std::ptr::null_mut();
        let err = AXObserverCreate(pid, ax_observer_callback, &raw mut observer);
        if err != 0 || observer.is_null() {
            tracing::warn!(phase = "event_monitor", pid, err, "AXObserverCreate failed");
            return MonitorMode::ObserverOnly;
        }

        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            CFRelease(observer.cast_const().cast::<c_void>());
            tracing::warn!(
                phase = "event_monitor",
                pid,
                "AXUIElementCreateApplication returned null"
            );
            return MonitorMode::ObserverOnly;
        }

        let notification = CFString::new("AXSelectedTextChanged");
        let add_err = AXObserverAddNotification(
            observer,
            app_element,
            notification.as_CFTypeRef(),
            std::ptr::null_mut(),
        );
        CFRelease(app_element);

        if add_err != 0 {
            tracing::warn!(
                phase = "event_monitor",
                pid,
                add_err,
                "AXObserverAddNotification failed"
            );
            CFRelease(observer.cast_const().cast::<c_void>());
            return MonitorMode::ObserverOnly;
        }

        // Add the observer's run-loop source to the current (event-tap) thread's
        // run loop so both the tap and the observer share the same loop.
        let source_ref = AXObserverGetRunLoopSource(observer);
        if !source_ref.is_null() {
            let rl_source = CFRunLoopSource::wrap_under_create_rule(source_ref as *mut _);
            CFRunLoop::get_current().add_source(&rl_source, kCFRunLoopCommonModes);
        }

        // `observer` is intentionally retained; its lifetime is tied to the
        // run-loop source which stays alive until the thread exits.
        tracing::info!(phase = "event_monitor", pid, "AXObserver registered");
        MonitorMode::Hybrid
    }
}

// ── Background thread: CGEventTap + CFRunLoop ──────────────────────────────

/// Spawn (at most once) a background thread that:
/// 1. Creates a `CGEventTap` listening for `LeftMouseUp` events.
/// 2. Adds the tap's mach-port source to a `CFRunLoop`.
/// 3. Runs the loop until `IS_RUNNING` becomes `false`.
#[cfg(target_os = "macos")]
fn spawn_event_tap_thread() {
    if THREAD_STARTED.swap(true, Ordering::SeqCst) {
        return; // already started
    }

    std::thread::Builder::new()
        .name("quillfix-event-tap".to_string())
        .spawn(event_tap_thread_body)
        .unwrap_or_else(|e| {
            tracing::error!(phase = "event_monitor", ?e, "failed to spawn event-tap thread");
            THREAD_STARTED.store(false, Ordering::SeqCst);
            // Return a dummy JoinHandle — the closure below panics in that case.
            panic!("failed to spawn event-tap thread: {e}");
        });
}

#[cfg(target_os = "macos")]
fn event_tap_thread_body() {
    // Build the CGEventTap.  The `Send` bound on the callback is satisfied
    // because all shared state we touch is `Send` (atomics, OnceLock<Mutex>).
    let tap_result = CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::LeftMouseUp],
        |_proxy, _event_type, event| {
            use core_graphics::event::EventField;
            if !is_running() {
                return CallbackResult::Keep;
            }
            on_callback();

            // Attempt to get the source PID and register an AX observer.
            #[allow(clippy::cast_possible_truncation)]
            let pid =
                event.get_integer_value_field(EventField::EVENT_SOURCE_UNIX_PROCESS_ID) as i32;
            if pid > 0 {
                let _ = register_ax_observer(pid);
            }

            // Show popup on every click when app is enabled
            // It will check for text selection when user clicks the popup
            if is_running() {
                use crate::ax_query::CGRect;
                let bounds = CGRect { x: 100.0, y: 100.0, width: 100.0, height: 20.0 };
                popup::show(popup::position_near(bounds));
            }

            CallbackResult::Keep
        },
    );

    let tap = match tap_result {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(
                phase = "event_monitor",
                ?e,
                "CGEventTap::new failed (Input Monitoring permission may be missing)"
            );
            return;
        }
    };

    // Add the tap's mach-port as a run-loop source on *this* thread's loop.
    let source: CFRunLoopSource = match tap.mach_port().create_runloop_source(0) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(phase = "event_monitor", ?e, "failed to create run-loop source");
            return;
        }
    };

    let run_loop = CFRunLoop::get_current();
    run_loop.add_source(&source, unsafe { kCFRunLoopCommonModes });
    tap.enable();
    tracing::info!(phase = "event_monitor", "CGEventTap enabled; running CFRunLoop");

    // Pump the run loop in short intervals so we can detect `stop()`.
    while is_running() {
        // Run loop for up to 250 ms, then re-check the flag.
        CFRunLoop::run_in_mode(
            unsafe { core_foundation::runloop::kCFRunLoopDefaultMode },
            Duration::from_millis(250),
            false,
        );
    }

    // `tap` drops here, which invalidates the mach port and disables the tap.
    drop(tap);
    tracing::info!(phase = "event_monitor", "CGEventTap disabled; event-tap thread exiting");
}

// ── Throttle / debounce helpers ────────────────────────────────────────────

pub fn on_callback() {
    let now = Instant::now();
    let tick = LAST_SECOND_TICK.get_or_init(|| Mutex::new(now));

    if let Ok(mut locked) = tick.lock()
        && now.duration_since(*locked) >= Duration::from_secs(1)
    {
        *locked = now;
        CALLBACKS_THIS_SECOND.store(0, Ordering::SeqCst);
    }

    let count = CALLBACKS_THIS_SECOND.fetch_add(1, Ordering::SeqCst) + 1;
    if count > 20 {
        DEBOUNCE_MS.store(500, Ordering::SeqCst);
    }
}

#[must_use]
pub fn current_debounce_ms() -> u64 {
    DEBOUNCE_MS.load(Ordering::SeqCst) as u64
}

pub fn reset_debounce_ms() {
    DEBOUNCE_MS.store(300, Ordering::SeqCst);
}

/// Process a text-selection callback end-to-end:
/// debounce -> query filter -> popup state update.
///
/// Returns `true` when a popup was shown, `false` otherwise.
#[must_use]
pub fn handle_selection_event(text: &str, role: &str, secure: bool, bounds: CGRect) -> bool {
    if !is_running() {
        return false;
    }

    on_callback();

    let debouncer = DEBOUNCER.get_or_init(|| Mutex::new(Debouncer::new(current_debounce_ms())));
    let stable_text = debouncer.lock().map_or(None, |mut inner| inner.feed(text));

    let Some(stable_text) = stable_text else {
        return false;
    };

    let Some(selection) = query_selection(&stable_text, role, secure) else {
        return false;
    };

    let store = popup::last_selection_store();
    if store.lock().map_or(true, |mut slot| {
        *slot = Some(selection);
        false
    }) {
        return false;
    }

    popup::show(popup::position_near(bounds));
    true
}

// ── RSS memory check via sysctl ─────────────────────────────────────────────

/// Returns the resident set size (RSS) of the current process in bytes.
///
/// Uses `proc_info` via `sysctl` on macOS; returns `None` on unsupported
/// platforms or when the syscall fails.
#[must_use]
pub fn rss_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        rss_bytes_macos()
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn rss_bytes_macos() -> Option<u64> {
    use std::mem;

    // `proc_taskinfo` layout (from <sys/proc_info.h>):
    //  u64 pti_virtual_size
    //  u64 pti_resident_size   ← bytes 8..16
    //  ... (many more fields)
    // We fetch it via `proc_pidinfo(pid, PROC_PIDTASKINFO, 0, buf, size)`.
    const PROC_PIDTASKINFO: i32 = 4;
    const PROC_TASKINFO_SIZE: usize = 232; // sizeof(struct proc_taskinfo)

    #[link(name = "proc", kind = "dylib")]
    unsafe extern "C" {
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut u8, buffersize: i32) -> i32;
    }

    let pid = unsafe { libc::getpid() };
    let mut buf = [0u8; PROC_TASKINFO_SIZE];
    let ret = unsafe {
        proc_pidinfo(
            pid,
            PROC_PIDTASKINFO,
            0,
            buf.as_mut_ptr(),
            i32::try_from(PROC_TASKINFO_SIZE).ok()?,
        )
    };
    if ret <= 0 {
        return None;
    }
    // pti_resident_size is the second u64 in the struct (offset 8).
    let rss = u64::from_ne_bytes(buf[8..16].try_into().ok()?);
    // Suppress unused-import warning in edge cases
    let _ = mem::size_of::<u64>();
    Some(rss)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `rss_bytes()` returns a plausible value for the current
    /// process (> 0 bytes, < 2 `GiB` sanity cap).
    #[test]
    #[cfg(target_os = "macos")]
    fn test_rss_bytes_returns_plausible_value() {
        let rss = rss_bytes().expect("rss_bytes() should succeed on macOS");
        assert!(rss > 0, "RSS should be greater than 0 bytes");
        assert!(rss < 2 * 1024 * 1024 * 1024, "RSS {rss} bytes exceeds 2 GiB sanity cap");
    }

    /// On non-macOS platforms `rss_bytes()` must return `None` (not panic).
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_rss_bytes_returns_none_on_non_macos() {
        assert_eq!(rss_bytes(), None);
    }
}
