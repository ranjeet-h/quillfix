#![allow(dead_code)]

pub mod backend;
pub mod prompt;

use anyhow::Result;
use backend::LlmBackend;
use prompt::CorrectionResult;
use std::time::Instant;

pub struct Corrector {
    backend: LlmBackend,
}

impl Default for Corrector {
    fn default() -> Self {
        Self::new()
    }
}

impl Corrector {
    #[must_use]
    pub fn new() -> Self {
        Self { backend: LlmBackend::new() }
    }

    /// Correct `text` using the loaded LLM.
    ///
    /// # Errors
    /// Returns an error if `ensure_loaded` fails or inference returns an error.
    pub fn correct(&self, text: &str) -> Result<CorrectionResult> {
        let started = Instant::now();
        self.backend.ensure_loaded()?;
        let prompt = prompt::build_prompt(text);
        let raw = self.backend.infer(&prompt)?;
        let result = prompt::post_process(text, &raw);
        tracing::info!(
            phase = "correct",
            text_len = text.len(),
            latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            changed = matches!(result, CorrectionResult::Changed(_)),
            "correction completed"
        );
        Ok(result)
    }

    /// Pre-warm the backend (load model / spawn Python subprocess).
    /// Safe to call from any thread. No-op after first successful call.
    ///
    /// # Errors
    /// Returns an error if the backend cannot be loaded.
    pub fn ensure_loaded(&self) -> anyhow::Result<()> {
        self.backend.ensure_loaded()
    }

    #[must_use]
    pub fn backend_loaded(&self) -> bool {
        self.backend.is_loaded()
    }
}
