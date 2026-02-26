use quillfix::llm::Corrector;
use quillfix::llm::prompt::CorrectionResult;
use quillfix::replacement::replace_with_fallback;

fn assert_contains(output: &CorrectionResult, needle: &str) {
    match output {
        CorrectionResult::Changed(s) => {
            assert!(
                s.to_lowercase().contains(&needle.to_lowercase()),
                "expected '{needle}' in result, got: {s}"
            );
        }
        _ => panic!("expected Changed, got: {output:?}"),
    }
}

fn assert_unchanged(output: &CorrectionResult) {
    assert!(matches!(output, CorrectionResult::Unchanged), "expected Unchanged, got: {output:?}");
}

// ── Spelling corrections ────────────────────────────────────────────────────

#[test]
fn test_corrector_fixes_known_misspelling() {
    let corrector = Corrector::new();
    let output = corrector.correct("teh quik brwon fox").expect("correction should succeed");
    assert_contains(&output, "quick");
    assert_contains(&output, "brown");
}

#[test]
fn test_corrector_fixes_common_typos() {
    let corrector = Corrector::new();
    let output =
        corrector.correct("I recieve a seperate package").expect("correction should succeed");
    assert_contains(&output, "receive");
    assert_contains(&output, "separate");
}

#[test]
fn test_corrector_fixes_definately() {
    let corrector = Corrector::new();
    let output =
        corrector.correct("I definately agree with you").expect("correction should succeed");
    assert_contains(&output, "definitely");
}

#[test]
fn test_corrector_fixes_occured() {
    let corrector = Corrector::new();
    let output =
        corrector.correct("An error occured during processing").expect("correction should succeed");
    assert_contains(&output, "occurred");
}

// ── Grammar corrections ────────────────────────────────────────────────────

#[test]
fn test_corrector_fixes_subject_verb_agreement() {
    let corrector = Corrector::new();
    let output = corrector.correct("She dont like coffee").expect("correction should succeed");
    assert_contains(&output, "doesn't");
}

#[test]
fn test_corrector_fixes_pronoun_case() {
    let corrector = Corrector::new();
    let output =
        corrector.correct("him and me went to the park").expect("correction should succeed");
    assert_contains(&output, "he");
}

#[test]
fn test_corrector_fixes_their_theyre() {
    let corrector = Corrector::new();
    let output = corrector.correct("their going to the store").expect("correction should succeed");
    assert_contains(&output, "they");
}

// ── Preserves correct text (no unnecessary changes) ─────────────────────────

#[test]
fn test_corrector_preserves_correct_text() {
    let corrector = Corrector::new();
    let output = corrector
        .correct("The quick brown fox jumps over the lazy dog.")
        .expect("correction should succeed");
    assert_unchanged(&output);
}

#[test]
fn test_corrector_preserves_already_correct_sentence() {
    let corrector = Corrector::new();
    let output = corrector.correct("I am going to the store.").expect("correction should succeed");
    assert_unchanged(&output);
}

#[test]
fn test_corrector_preserves_technical_text() {
    let corrector = Corrector::new();
    let output = corrector
        .correct("The API returns a JSON payload with status 200.")
        .expect("correction should succeed");
    assert_unchanged(&output);
}

#[test]
fn test_corrector_does_not_add_extra_text() {
    let corrector = Corrector::new();
    let output = corrector.correct("teh quik brwon fox").expect("correction should succeed");
    if let CorrectionResult::Changed(corrected) = &output {
        assert!(!corrected.contains("Here is"), "model must not add explanations: {corrected}");
        assert!(
            !corrected.contains("corrected"),
            "model must not add meta-commentary: {corrected}"
        );
        assert!(
            corrected.len() <= "teh quik brwon fox".len() * 2,
            "output should not be much longer than input: {corrected}"
        );
    }
}

// ── Replacement ─────────────────────────────────────────────────────────────

#[test]
fn test_ax_replacement_in_textedit() {
    replace_with_fallback("AXTextArea", "the quick").expect("replacement should succeed");
}

// ── Model identity ──────────────────────────────────────────────────────────

#[test]
fn test_model_is_qwen2_architecture() {
    let config_path = std::path::Path::new("resources/model/config.json");
    if !config_path.exists() {
        eprintln!("skipping: model not downloaded (run scripts/download_model.sh)");
        return;
    }
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(config_path).expect("config.json must be readable"),
    )
    .expect("config.json must be valid JSON");
    assert_eq!(config["model_type"], "qwen2", "model must be Qwen2");
    assert_eq!(
        config["architectures"][0], "Qwen2ForCausalLM",
        "architecture must be Qwen2ForCausalLM"
    );
    assert_eq!(config["hidden_size"], 896, "hidden_size must be 896 (0.5B variant)");
    assert_eq!(config["num_hidden_layers"], 24, "num_hidden_layers must be 24 (0.5B variant)");
}

#[test]
fn test_download_script_references_correct_model() {
    let script =
        std::fs::read_to_string("scripts/download_model.sh").expect("download script must exist");
    assert!(
        script.contains("mlx-community/Qwen2.5-0.5B-Instruct-4bit"),
        "download script must reference the correct model ID"
    );
}
