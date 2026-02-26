#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionResult {
    Changed(String),
    Unchanged,
    Error(String),
}

pub fn build_prompt(text: &str) -> String {
    format!(
        "<|im_start|>system\nFix ONLY spelling, grammar, and punctuation. Return ONLY corrected text.\n<|im_end|>\n<|im_start|>user\n{text}\n<|im_end|>\n<|im_start|>assistant\n"
    )
}

pub fn post_process(original: &str, generated: &str) -> CorrectionResult {
    let cleaned = generated.trim();
    if cleaned.is_empty() {
        return CorrectionResult::Error("empty model output".to_string());
    }

    if cleaned == original.trim() {
        return CorrectionResult::Unchanged;
    }

    if cleaned.len() > original.len().saturating_mul(5).max(200) {
        return CorrectionResult::Error("output too long".to_string());
    }

    CorrectionResult::Changed(cleaned.to_string())
}
