# Phase 1 HOWTO

## Prerequisites

- macOS 14+
- Rust stable with Edition 2024 support
- Xcode Command Line Tools (`xcode-select --install`)
- Hugging Face CLI (`pipx install huggingface_hub` or `pip install huggingface_hub`)

## Download model

```bash
bash scripts/download_model.sh
```

## Build

Debug:

```bash
cargo build
```

Release app bundle:

```bash
bash scripts/bundle.sh
```

## Run

```bash
open QuillFix.app
```

Check Console.app for subsystem `com.quillfix.app` entries and permission logs.

## Run tests

```bash
cargo test --test unit_permissions --test integration_bundle
```

Or run the project quality gate:

```bash
make check-all
```

## Expected result

- `unit_permissions` and `integration_bundle` pass
- `QuillFix.app` is created
- `codesign --verify --deep QuillFix.app` exits successfully
