use quillfix::menu_bar::{DEFAULTS_KEY, load_enabled_state, save_enabled_state};

#[test]
fn test_defaults_key_constant() {
    assert_eq!(DEFAULTS_KEY, "quillfix.enabled");
}

#[test]
fn test_load_save_enabled_state_roundtrip() {
    save_enabled_state(true);
    assert!(load_enabled_state());

    save_enabled_state(false);
    assert!(!load_enabled_state());
}
