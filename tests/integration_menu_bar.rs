#[cfg(target_os = "macos")]
use objc2_foundation::MainThreadMarker;
use quillfix::menu_bar::{
    IS_ENABLED, is_setup, load_enabled_state, run, save_enabled_state, setup, toggle_enabled,
};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn test_toggle_persists_state() {
    let _guard = test_guard();
    save_enabled_state(false);
    IS_ENABLED.store(false, Ordering::SeqCst);

    run();
    let first = toggle_enabled();
    assert!(first);
    assert!(load_enabled_state());

    let second = toggle_enabled();
    assert!(!second);
    assert!(!load_enabled_state());
}

#[test]
fn test_status_item_exists_after_setup() {
    let _guard = test_guard();
    #[cfg(target_os = "macos")]
    if MainThreadMarker::new().is_none() {
        return;
    }
    setup();
    assert!(is_setup());
}

#[test]
fn test_toggle_persists_after_restart() {
    let _guard = test_guard();
    save_enabled_state(false);
    IS_ENABLED.store(false, Ordering::SeqCst);
    run();

    let enabled = toggle_enabled();
    assert!(enabled);
    assert!(load_enabled_state());

    // Simulate restart: clear in-memory state and run boot path again.
    IS_ENABLED.store(false, Ordering::SeqCst);
    run();
    assert!(IS_ENABLED.load(Ordering::SeqCst));

    // Cleanup for other tests.
    save_enabled_state(false);
    IS_ENABLED.store(false, Ordering::SeqCst);
    run();
}
