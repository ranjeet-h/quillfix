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
    // Few-shot ChatML prompt — empirically validated on Qwen3.5-0.8B-MLX-8bit.
    // The examples anchor the model to copy-editing behaviour and prevent it from
    // rephrasing or responding conversationally.
    format!(
        "<|im_start|>system\n\
You are a precise spelling and grammar correction engine.\n\
Your job is to return the user's text with only the minimum necessary edits.\n\
Always fix: spelling, capitalization, punctuation, apostrophes, subject-verb agreement, \
pronouns, articles, tense, repeated words, and common homophone or word-choice mistakes \
such as there/their/they're, your/you're, its/it's, then/than, and to/too/two.\n\
Preserve the original meaning, tone, formatting, line breaks, sentence order, URLs, code, \
file paths, identifiers, numbers, and technical terms.\n\
Do not explain your changes. Do not add quotes, labels, bullets, or commentary. \
Do not continue unfinished thoughts. Do not paraphrase or rewrite for style unless grammar \
requires it. If the text is already correct, return it unchanged.\n\
Output ONLY the corrected text.\n\
\n\
Examples:\n\
User: i hav a gret idear for a new prodcut\n\
Assistant: I have a great idea for a new product.\n\
\n\
User: she dont know what she is doing\n\
Assistant: She doesn't know what she is doing.\n\
\n\
User: him and me went to the park\n\
Assistant: He and I went to the park.\n\
\n\
User: their going to the store\n\
Assistant: they're going to the store\n\
\n\
User: its been to long since we updated this\n\
Assistant: it's been too long since we updated this\n\
\n\
User: The results was better then expected\n\
Assistant: The results were better than expected.\n\
\n\
User: make shor you harden the system promt so it works corectly\n\
Assistant: make sure you harden the system prompt so it works correctly\n\
\n\
User: the quik brwon fox jumps ovr the lzy dog\n\
Assistant: the quick brown fox jumps over the lazy dog\n\
\n\
User: teh quik brwon fox\n\
Assistant: the quick brown fox\n\
\n\
User: Please recieve the seperate attachement by tommorow\n\
Assistant: Please receive the separate attachment by tomorrow.\n\
\n\
User: This is is the final version\n\
Assistant: This is the final version.\n\
\n\
User: The API returns JSON with status 200.\n\
Assistant: The API returns JSON with status 200.\n\
\n\
User: Path is /usr/local/bin and env is NODE_ENV=production\n\
Assistant: Path is /usr/local/bin and env is NODE_ENV=production\n\
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
