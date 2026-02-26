use quillfix::llm::prompt::{CorrectionResult, build_prompt, post_process};

// ── Prompt structure ────────────────────────────────────────────────────────

#[test]
fn test_build_prompt_contains_system_instruction() {
    let prompt = build_prompt("teh cat");
    assert!(prompt.contains("spelling and grammar correction engine"));
    assert!(prompt.contains("<|im_start|>user"));
    assert!(prompt.contains("teh cat"));
}

#[test]
fn test_build_prompt_restricts_to_spelling_grammar_punctuation() {
    let prompt = build_prompt("anything");
    assert!(prompt.contains("Fix every spelling mistake"));
    assert!(prompt.contains("Output ONLY the corrected text"));
    assert!(prompt.contains("do NOT rephrase"));
    assert!(prompt.contains("nothing else"));
}

#[test]
fn test_build_prompt_uses_chatml_format() {
    let prompt = build_prompt("some text");
    assert!(prompt.starts_with("<|im_start|>system\n"));
    assert!(prompt.contains("<|im_end|>\n<|im_start|>user\n"));
    assert!(prompt.ends_with("<|im_start|>assistant\n"));
}

// ── post_process: basic ─────────────────────────────────────────────────────

#[test]
fn test_post_process_unchanged() {
    assert_eq!(post_process("hello world", "hello world"), CorrectionResult::Unchanged);
}

#[test]
fn test_post_process_whitespace_trim_is_unchanged() {
    assert_eq!(post_process("hello world", "  hello world  "), CorrectionResult::Unchanged);
}

#[test]
fn test_post_process_changed() {
    assert_eq!(
        post_process("teh cat", "the cat"),
        CorrectionResult::Changed("the cat".to_string())
    );
}

#[test]
fn test_post_process_overgeneration() {
    assert!(matches!(post_process("hi", &"x".repeat(200)), CorrectionResult::Error(_)));
}

// ── post_process: guards against unnecessary text ───────────────────────────

#[test]
fn test_post_process_rejects_empty_output() {
    assert!(matches!(post_process("some text", ""), CorrectionResult::Error(_)));
}

#[test]
fn test_post_process_catches_explanation_added() {
    // A model that adds "Here is the corrected text: ..." would over-generate.
    let original = "teh cat";
    let bad_output = "Here is the corrected text: the cat. I fixed the spelling of 'the'.";
    // bad_output.len() = 66, original.len() * 3 = 21 → over-generation caught
    assert!(matches!(post_process(original, bad_output), CorrectionResult::Error(_)));
}

#[test]
fn test_post_process_catches_quoted_output() {
    let original = "I hav a idea";
    let bad_output =
        "\"I have an idea\" - I corrected 'hav' to 'have' and 'a' to 'an' before a vowel.";
    assert!(matches!(post_process(original, bad_output), CorrectionResult::Error(_)));
}

#[test]
fn test_post_process_allows_similar_length_change() {
    // Reasonable correction: similar length output
    let original = "She dont like it";
    let corrected = "She doesn't like it";
    assert_eq!(post_process(original, corrected), CorrectionResult::Changed(corrected.to_string()));
}
