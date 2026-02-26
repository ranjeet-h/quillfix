use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

static BUILD_ONCE: Once = Once::new();

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn app_path() -> PathBuf {
    repo_root().join("QuillFix.app")
}

fn ensure_bundle_built() {
    BUILD_ONCE.call_once(|| {
        let status = Command::new("bash")
            .arg("scripts/bundle.sh")
            .current_dir(repo_root())
            .status()
            .expect("failed to run bundle script");
        assert!(status.success(), "bundle script failed: {status}");
    });
}

fn has_safetensors(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };

    entries.flatten().any(|entry| entry.path().extension().is_some_and(|ext| ext == "safetensors"))
}

#[test]
#[cfg(target_os = "macos")]
fn test_app_bundle_structure() {
    ensure_bundle_built();

    let app = app_path();
    let binary = app.join("Contents/MacOS/quillfix");
    let info_plist_path = app.join("Contents/Info.plist");
    let model_dir = app.join("Contents/Resources/model");

    assert!(binary.exists(), "bundle binary missing: {}", binary.display());
    assert!(info_plist_path.exists(), "Info.plist missing: {}", info_plist_path.display());

    let plist = plist::Value::from_file(&info_plist_path).expect("invalid plist format");
    let dict = plist.as_dictionary().expect("plist root must be dictionary");

    assert_eq!(dict.get("LSUIElement"), Some(&plist::Value::Boolean(true)));
    assert_eq!(
        dict.get("CFBundleIdentifier"),
        Some(&plist::Value::String("com.quillfix.app".to_string()))
    );

    let usage = dict
        .get("NSAccessibilityUsageDescription")
        .and_then(plist::Value::as_string)
        .unwrap_or_default();
    assert!(!usage.trim().is_empty(), "NSAccessibilityUsageDescription empty");

    assert!(
        has_safetensors(&model_dir),
        "model dir missing .safetensors file at {}",
        model_dir.display()
    );
}

#[test]
#[cfg(target_os = "macos")]
fn test_codesign_verify() {
    ensure_bundle_built();

    let status = Command::new("codesign")
        .arg("--verify")
        .arg("--deep")
        .arg(app_path())
        .status()
        .expect("failed to run codesign verify");

    assert!(status.success(), "codesign verify failed: {status}");
}
