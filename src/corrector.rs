//! Shared global LLM corrector instance.
//!
//! Initialised once at startup and pre-warmed on a background thread.
//! Both the menu-bar action and the `NSServices` handler access it here.

use std::sync::{Arc, Mutex, OnceLock};

use crate::llm::Corrector;

static CORRECTOR: OnceLock<Arc<Mutex<Corrector>>> = OnceLock::new();

/// Return (or lazily create) the shared [`Corrector`] handle.
///
/// The returned `Arc` can be cloned and sent to background threads.
pub fn get() -> Arc<Mutex<Corrector>> {
    CORRECTOR.get_or_init(|| Arc::new(Mutex::new(Corrector::new()))).clone()
}
