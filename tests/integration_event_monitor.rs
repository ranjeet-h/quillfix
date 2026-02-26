use quillfix::ax_query::{CGRect, query_selection};
use quillfix::event_monitor::{
    handle_selection_event, is_running, register_ax_observer, start, stop,
};
use quillfix::popup::{is_visible, last_selection_store};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().expect("test guard lock should succeed")
}

#[test]
fn test_ax_query_fires_on_selection() {
    let _guard = test_guard();
    start();
    assert!(is_running());

    let result = query_selection("QuillFix test selection", "AXTextArea", false);
    assert!(result.is_some());

    let mode = register_ax_observer(123);
    let mode_text = format!("{mode:?}");
    assert!(!mode_text.is_empty());

    stop();
}

#[test]
fn test_password_field_skipped() {
    let _guard = test_guard();
    let result = query_selection("secret value", "AXTextField", true);
    assert!(result.is_none());
}

#[test]
fn test_callback_wiring_updates_popup_and_selection() {
    let _guard = test_guard();
    start();

    {
        let store = last_selection_store();
        let mut slot = store.lock().expect("selection lock should succeed");
        *slot = None;
    }

    let bounds = CGRect { x: 100.0, y: 120.0, width: 160.0, height: 20.0 };

    let first = handle_selection_event("QuillFix callback", "AXTextArea", false, bounds);
    assert!(!first);

    thread::sleep(Duration::from_millis(550));

    let second = handle_selection_event("QuillFix callback", "AXTextArea", false, bounds);
    assert!(second);
    assert!(is_visible());

    let store = last_selection_store();
    let selected = store.lock().expect("selection lock should succeed").clone();
    assert!(selected.is_some());
    assert_eq!(selected.expect("selection should be set").text, "QuillFix callback");

    stop();
}
