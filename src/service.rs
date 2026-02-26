#![allow(dead_code)]

/// NSServices registration is handled in two places:
///
/// 1. `resources/Info.plist` — declares the service entry with
///    `NSMessage = "quillFixCorrectText"` so macOS discovers it.
///
/// 2. `menu_bar::register_services()` — called at startup to point
///    `NSApplication.servicesProvider` at the `MenuHandler` object,
///    which implements `quillFixCorrectText:userData:error:`.
///
/// This function is kept for call-site compatibility with older code.
pub fn register_service() {
    tracing::info!(phase = "service", "NSServices registered via menu_bar::register_services()");
}
