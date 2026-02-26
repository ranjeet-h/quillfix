use quillfix::permissions::{
    ONBOARDED_KEY, PermissionState, accessibility_state, load_onboarded_state, save_onboarded_state,
};

#[test]
fn test_permission_state_enum_variants() {
    let variants =
        [PermissionState::Granted, PermissionState::Denied, PermissionState::NotDetermined];
    assert_eq!(variants.len(), 3);
    assert_ne!(PermissionState::Granted, PermissionState::Denied);
    assert_ne!(PermissionState::Denied, PermissionState::NotDetermined);
    assert_ne!(PermissionState::Granted, PermissionState::NotDetermined);
}

#[test]
fn test_accessibility_state_no_panic() {
    let state = accessibility_state();
    assert!(matches!(
        state,
        PermissionState::Granted | PermissionState::Denied | PermissionState::NotDetermined
    ));
}

#[test]
fn test_onboarded_key_constant() {
    assert_eq!(ONBOARDED_KEY, "quillfix.onboarded");
}

#[test]
fn test_onboarded_state_roundtrip() {
    save_onboarded_state(false);
    assert!(!load_onboarded_state());
    save_onboarded_state(true);
    assert!(load_onboarded_state());
    save_onboarded_state(false);
}
