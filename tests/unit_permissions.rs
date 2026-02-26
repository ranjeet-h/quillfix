use quillfix::permissions::{PermissionState, accessibility_state};

#[test]
fn test_permission_state_enum_variants() {
    let variants = [PermissionState::Granted, PermissionState::Denied, PermissionState::Unknown];
    assert_eq!(variants.len(), 3);
}

#[test]
fn test_accessibility_state_no_panic() {
    let state = accessibility_state();
    assert!(matches!(
        state,
        PermissionState::Granted | PermissionState::Denied | PermissionState::Unknown
    ));
}
