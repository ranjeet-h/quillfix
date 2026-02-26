#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementStrategy {
    AX,
    Clipboard,
    Unsupported,
}

#[must_use]
pub fn strategy_for_bundle_id(bundle_id: &str) -> ReplacementStrategy {
    match bundle_id {
        "com.apple.TextEdit" | "com.apple.Notes" | "com.apple.mail" => ReplacementStrategy::AX,
        "com.apple.Terminal" => ReplacementStrategy::Unsupported,
        _ => ReplacementStrategy::Clipboard,
    }
}
