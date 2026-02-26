use quillfix::ax_query::CGRect;
use quillfix::popup::{NSPoint, PopupController};
use std::thread;
use std::time::Duration;

#[test]
fn test_position_near_basic() {
    let mut popup = PopupController::default();
    popup.set_screen_bounds(1440.0, 900.0);

    let point = popup.position_near(CGRect { x: 100.0, y: 200.0, width: 300.0, height: 20.0 });

    assert!((point.x - 408.0).abs() <= 1.0);
    assert!((point.y - 228.0).abs() <= 1.0);
}

#[test]
fn test_position_near_clamps_to_screen() {
    let mut popup = PopupController::default();
    popup.set_screen_bounds(1440.0, 900.0);

    let point = popup.position_near(CGRect { x: 1420.0, y: 500.0, width: 100.0, height: 20.0 });

    assert!(point.x + 32.0 <= 1440.0);
}

#[test]
fn test_auto_hide_timer_fires() {
    let mut popup = PopupController::default();
    popup.show(NSPoint { x: 100.0, y: 100.0 });
    assert!(popup.is_visible());

    thread::sleep(Duration::from_millis(4001));
    popup.tick();
    assert!(!popup.is_visible());
}
