use quillfix::replacement::{clipboard_contents, clipboard_replace};

#[test]
fn test_clipboard_replace_writes_to_pasteboard() {
    clipboard_replace("corrected text").expect("clipboard replace should succeed");
    assert_eq!(clipboard_contents(), "corrected text");
}
