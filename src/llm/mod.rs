pub mod backend;
pub mod prompt;

use anyhow::Result;

pub struct Corrector {
    backend: backend::LlmBackend,
}

impl Corrector {
    pub fn new() -> Self {
        Self { backend: backend::LlmBackend::default() }
    }

    pub async fn correct(&self, text: &str) -> Result<prompt::CorrectionResult> {
        let generated = self.backend.infer(&prompt::build_prompt(text)).await?;
        Ok(prompt::post_process(text, &generated))
    }
}
