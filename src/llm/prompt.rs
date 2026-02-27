#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionResult {
    Changed(String),
    Unchanged,
    Error(String),
}

const META_PREFIXES: [&str; 6] = [
    "here is the corrected text",
    "corrected text:",
    "assistant:",
    "output:",
    "i corrected",
    "i fixed",
];

#[must_use]
pub fn build_prompt(text: &str) -> String {
    // Few-shot ChatML prompt — empirically validated on Qwen2.5-1.5B-Instruct-4bit.
    // The examples anchor the model to copy-editing behaviour and prevent it from
    // rephrasing or responding conversationally.
    format!(
        "<|im_start|>system\n\
You are a spelling and grammar correction engine. Fix every spelling mistake, \
grammar error, and punctuation issue in the user text. Preserve the original \
meaning, tone, and structure exactly — do NOT rephrase, expand, or summarize. \
Output ONLY the corrected text, nothing else.\n\
\n\
Examples:\n\
User: i hav a gret idear for a new prodcut\n\
Assistant: I have a great idea for a new product.\n\
\n\
User: she dont know what she is doing\n\
Assistant: She doesn't know what she is doing.\n\
\n\
User: make shor you harden the system promt so it works corectly\n\
Assistant: make sure you harden the system prompt so it works correctly\n\
\n\
User: the quik brwon fox jumps ovr the lzy dog\n\
Assistant: the quick brown fox jumps over the lazy dog\n\
\n\
User: Wornl words are types of hee.\n\
Assistant: Wrong words are typed here.<|im_end|>\n\
<|im_start|>user\n{text}<|im_end|>\n\
<|im_start|>assistant\n"
    )
}

#[must_use]
pub fn post_process(original: &str, generated: &str) -> CorrectionResult {
    let cleaned = generated.trim();
    if cleaned.is_empty() {
        return CorrectionResult::Error("empty output".to_string());
    }

    let cleaned_lower = cleaned.to_ascii_lowercase();
    if META_PREFIXES.iter().any(|prefix| cleaned_lower.starts_with(prefix)) {
        return CorrectionResult::Error("meta output".to_string());
    }

    if cleaned == original.trim() {
        return CorrectionResult::Unchanged;
    }

    if original.contains('\n') && original.lines().count() != cleaned.lines().count() {
        return CorrectionResult::Error("format changed".to_string());
    }

    if has_protected_token_loss(original, cleaned) {
        return CorrectionResult::Error("protected token changed".to_string());
    }

    if cleaned.len() > original.len().saturating_mul(3) {
        return CorrectionResult::Error("over-generation".to_string());
    }

    CorrectionResult::Changed(cleaned.to_string())
}

fn has_protected_token_loss(original: &str, corrected: &str) -> bool {
    original
        .split_whitespace()
        .filter(|token| is_protected_token(token))
        .any(|token| !corrected.contains(token))
}

fn is_protected_token(token: &str) -> bool {
    token.contains("://")
        || token.contains('@')
        || token.contains("::")
        || token.contains('`')
        || token.chars().any(|ch| {
            matches!(ch, '{' | '}' | '[' | ']' | '(' | ')' | '<' | '>' | '/' | '\\' | '=')
        })
}
