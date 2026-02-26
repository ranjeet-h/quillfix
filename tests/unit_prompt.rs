use quillfix::llm::prompt::{CorrectionResult, build_prompt, post_process};

#[test]
fn prompt_contains_system_and_user_text() {
    let prompt = build_prompt("teh cat");
    assert!(prompt.contains("Fix ONLY spelling"));
    assert!(prompt.contains("teh cat"));
}

#[test]
fn unchanged_when_trimmed_equal() {
    assert_eq!(post_process("teh cat", "  teh cat  "), CorrectionResult::Unchanged);
}
