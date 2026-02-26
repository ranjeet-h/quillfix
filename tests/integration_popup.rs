use quillfix::popup::{NSPoint, controller};
use std::thread;
use std::time::Duration;

#[test]
fn test_popup_appears_on_selection_event() {
    let popup = controller();
    {
        let mut inner = popup.lock().expect("popup lock should succeed");
        inner.show(NSPoint { x: 50.0, y: 50.0 });
    }
    assert!(popup.lock().expect("popup lock should succeed").is_visible());
}

#[test]
fn test_popup_hides_on_esc() {
    let popup = controller();
    {
        let mut inner = popup.lock().expect("popup lock should succeed");
        inner.show(NSPoint { x: 60.0, y: 60.0 });
        inner.hide();
    }
    assert!(!popup.lock().expect("popup lock should succeed").is_visible());
}

#[test]
fn test_popup_hides_on_timeout() {
    let popup = controller();
    {
        let mut inner = popup.lock().expect("popup lock should succeed");
        inner.show(NSPoint { x: 70.0, y: 70.0 });
    }
    thread::sleep(Duration::from_millis(4200));
    {
        let mut inner = popup.lock().expect("popup lock should succeed");
        inner.tick();
        let visible = inner.is_visible();
        drop(inner);
        assert!(!visible);
    }
}

#[test]
fn test_popup_shows_success_icon_on_click() {
    let popup = controller();
    {
        let mut inner = popup.lock().expect("popup lock should succeed");
        inner.show(NSPoint { x: 80.0, y: 80.0 });
        inner.show_success();
        let icon = inner.icon_name() == "sparkles";
        let not_visible = !inner.is_visible();
        drop(inner);
        assert!(icon);
        assert!(not_visible);
    }
}
