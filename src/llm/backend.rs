use anyhow::Result;

#[derive(Default)]
pub struct LlmBackend;

impl LlmBackend {
    pub async fn infer(&self, prompt: &str) -> Result<String> {
        // Phase 5 will use mlx-rs here. This keeps the scaffold compilable.
        Ok(prompt.to_string())
    }
}
