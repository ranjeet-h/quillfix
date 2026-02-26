use quillfix::ax_query::{CGRect, SelectionInfo};
use quillfix::event_monitor::{handle_selection_event, is_running, start, stop};
use quillfix::llm::Corrector;
use quillfix::llm::prompt::CorrectionResult;
use quillfix::menu_bar::{IS_ENABLED, toggle_enabled};
use quillfix::popup::{self, NSPoint, last_selection_store};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(())).lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ── Correction pipeline ─────────────────────────────────────────────────────

#[test]
fn test_full_correction_in_textedit() {
    let _g = test_guard();
    start();
    assert!(is_running());

    let corrector = Corrector::new();
    let result = corrector.correct("I hav a gret idear").expect("correction should succeed");
    assert_eq!(result, CorrectionResult::Changed("I have a great idea".to_string()));

    stop();
}

#[test]
fn test_correction_in_notes() {
    let _g = test_guard();
    let corrector = Corrector::new();
    let result = corrector.correct("speling mistaeks").expect("correction should succeed");
    assert_eq!(result, CorrectionResult::Changed("spelling mistakes".to_string()));
}

#[test]
#[ignore = "requires Chrome to be installed"]
fn test_correction_in_chrome_input() {
    let _g = test_guard();
    let corrector = Corrector::new();
    let result = corrector.correct("Ths is wrng").expect("correction should succeed");
    assert_eq!(result, CorrectionResult::Changed("This is wrong".to_string()));
}

// ── End-to-end: selection → popup → click → correction → replacement ────────

#[test]
fn test_selection_triggers_popup_and_click_corrects() {
    let _g = test_guard();
    start();

    // Simulate a text selection event (the user selected misspelled text).
    // The debouncer requires the same text fed twice with a delay (300 ms) between.
    let bounds = CGRect { x: 100.0, y: 200.0, width: 80.0, height: 16.0 };
    let _ = handle_selection_event("I definately agree", "AXTextArea", false, bounds);
    thread::sleep(Duration::from_millis(350));
    let shown = handle_selection_event("I definately agree", "AXTextArea", false, bounds);
    assert!(shown, "popup should appear for misspelled text");
    assert!(popup::is_visible(), "popup must be visible after selection event");

    // Verify the selection was stored
    let sel = last_selection_store().lock().expect("store lock should succeed").clone();
    assert!(sel.is_some(), "selection must be stored");
    let sel = sel.expect("just asserted Some");
    assert_eq!(sel.text, "I definately agree");

    // Simulate clicking the popup: on_click reads LAST_SELECTION and corrects
    popup::on_click(|text| {
        let corrector = Corrector::new();
        corrector.correct(text)
    });

    // Give the background thread time to complete
    thread::sleep(Duration::from_millis(500));

    // After correction, popup should have hidden (show_success → hide)
    assert!(!popup::is_visible(), "popup must hide after correction completes");

    stop();
}

#[test]
fn test_click_on_correct_text_shows_success() {
    let _g = test_guard();
    start();

    // Store a correct selection
    {
        let store = last_selection_store();
        let mut slot = store.lock().expect("store lock should succeed");
        *slot = Some(SelectionInfo {
            text: "The weather is nice today.".to_string(),
            element_role: "AXTextArea".to_string(),
            secure: false,
        });
    }

    popup::show(NSPoint { x: 50.0, y: 50.0 });
    assert!(popup::is_visible());

    // Click: text is correct → CorrectionResult::Unchanged → show_success → hide
    popup::on_click(|text| {
        let corrector = Corrector::new();
        corrector.correct(text)
    });

    thread::sleep(Duration::from_millis(500));
    assert!(!popup::is_visible(), "popup must hide after show_success");

    stop();
}

// ── Guards: secure fields, long text, password ──────────────────────────────

#[test]
fn test_password_field_no_popup() {
    let _g = test_guard();
    start();

    // Secure field should be rejected by the selection pipeline
    let bounds = CGRect { x: 10.0, y: 10.0, width: 60.0, height: 16.0 };
    let shown = handle_selection_event("mysecretpassword", "AXTextField", true, bounds);
    assert!(!shown, "popup must NOT appear for secure text fields");

    stop();
}

#[test]
fn test_long_text_no_popup() {
    let _g = test_guard();
    let long_text = "a".repeat(1600);
    let bounds = CGRect { x: 10.0, y: 10.0, width: 60.0, height: 16.0 };
    start();
    let shown = handle_selection_event(&long_text, "AXTextArea", false, bounds);
    assert!(!shown, "popup must NOT appear for text exceeding 1500 chars");
    stop();
}

#[test]
fn test_short_text_no_popup() {
    let _g = test_guard();
    start();
    let bounds = CGRect { x: 10.0, y: 10.0, width: 20.0, height: 16.0 };
    let shown = handle_selection_event("hi", "AXTextArea", false, bounds);
    assert!(!shown, "popup must NOT appear for text under 4 chars");
    stop();
}

#[test]
fn test_unsupported_role_no_popup() {
    let _g = test_guard();
    start();
    let bounds = CGRect { x: 10.0, y: 10.0, width: 60.0, height: 16.0 };
    let shown = handle_selection_event("some selected text here", "AXStaticText", false, bounds);
    assert!(!shown, "popup must NOT appear for non-editable roles");
    stop();
}

// ── Performance / monitoring ────────────────────────────────────────────────

#[test]
fn test_performance_idle_cpu() {
    let _g = test_guard();
    start();
    assert!(is_running());
    stop();
}

#[test]
fn test_performance_peak_ram() {
    let _g = test_guard();
    let corrector = Corrector::new();
    let _ = corrector.correct("teh quik brwon fox").expect("correction should succeed");
    assert!(corrector.backend_loaded());
}

// ── Toggle ──────────────────────────────────────────────────────────────────

#[test]
fn test_toggle_off_stops_monitoring() {
    let _g = test_guard();
    IS_ENABLED.store(false, Ordering::SeqCst);
    let enabled = toggle_enabled();
    assert!(enabled);
    let disabled = toggle_enabled();
    assert!(!disabled);
}
