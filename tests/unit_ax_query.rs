use quillfix::ax_query::{CGRect, quartz_to_appkit, query_focused_selection, query_selection};

#[test]
fn test_query_selection_filters_roles_and_lengths() {
    assert!(query_selection("abc", "AXTextArea", false).is_none());
    assert!(query_selection("valid text", "AXUnknownRole", false).is_none());
    assert!(query_selection("valid text", "AXTextArea", true).is_none());
    assert!(query_selection("valid text", "AXTextArea", false).is_some());
}

#[test]
fn test_query_focused_selection_no_panic() {
    let _ = query_focused_selection();
}

/// Quartz uses top-left origin (y increases downward).
/// AppKit uses bottom-left origin (y increases upward).
///
/// For a 900-pixel-tall display, a rect at Quartz(y=100, height=20) should
/// map to AppKit y = 900 - 100 - 20 = 780.
#[test]
fn test_quartz_to_appkit_basic() {
    // On non-macOS, quartz_to_appkit returns the rect unchanged.
    // On macOS, it converts using the real display height; we can only test
    // the formula itself by crafting a portable assertion.
    let rect = CGRect { x: 50.0, y: 100.0, width: 200.0, height: 20.0 };
    let converted = quartz_to_appkit(rect);
    // x and width/height must be unchanged
    assert!((converted.x - 50.0).abs() < 1e-6);
    assert!((converted.width - 200.0).abs() < 1e-6);
    assert!((converted.height - 20.0).abs() < 1e-6);
    // On macOS the y should be > 0 for a normal display
    #[cfg(target_os = "macos")]
    {
        assert!(converted.y >= 0.0, "AppKit y should be non-negative");
    }
    // On non-macOS it should be unchanged
    #[cfg(not(target_os = "macos"))]
    {
        assert!((converted.y - 100.0).abs() < 1e-6);
    }
}

/// Verify that a rect near the top of the screen (small Quartz y) maps to a
/// large AppKit y value (near the top of a bottom-origin coordinate system).
#[test]
fn test_quartz_to_appkit_top_of_screen() {
    let rect = CGRect { x: 0.0, y: 5.0, width: 100.0, height: 20.0 };
    let converted = quartz_to_appkit(rect);
    // In AppKit coordinates the top of the screen has the *largest* y value.
    // On macOS, with any normal display height >= 600, the result should be >= 575.
    #[cfg(target_os = "macos")]
    {
        assert!(converted.y >= 575.0, "top-of-screen rect should have large AppKit y");
    }
    #[cfg(not(target_os = "macos"))]
    {
        // passthrough
        assert!((converted.y - 5.0).abs() < 1e-6);
    }
}
