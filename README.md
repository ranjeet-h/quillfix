# QuillFix

QuillFix is a macOS menu bar app for one-click grammar/spelling correction of selected text using a local MLX model.

## Stack

- Rust 2024 edition (`rustc 1.93.1` currently installed)
- macOS 14+
- `hf-hub` (Hugging Face model downloads)
- `mlx-rs` (Apple MLX inference; behind `local-llm` feature)

## Project Layout

- `src/`: app source modules
- `tests/`: unit/integration tests
- `resources/`: `Info.plist`, entitlements, model directory
- `scripts/`: bundle/model/test helper scripts
- `docs/phase-*`: phase HOWTOs

## Prerequisites

- Rust toolchain (stable with Edition 2024 support)
- macOS command line tools
- Full Xcode install required for MLX compilation (`xcrun metal`)
- Hugging Face CLI (`hf` or `huggingface-cli`) for model download

## Build

Default scaffold build (without MLX compile):

```bash
cargo check
cargo test
```

Or run the one-shot local quality gate:

```bash
make check-all
```

Create a distributable DMG installer:

```bash
make dmg
```

Full local-LLM build (requires full Xcode Metal toolchain):

```bash
cargo check --features local-llm
cargo build --release --features local-llm
```

## Scripts

- `scripts/download_model.sh`: downloads `mlx-community/Qwen3.5-0.8B-MLX-8bit` into `resources/model/`
- `scripts/bundle.sh`: builds and bundles `QuillFix.app`, then signs it
- `scripts/create_dmg.sh`: builds `QuillFix.app` and packages `dist/QuillFix-<version>.dmg`
- `scripts/run_integration_tests.sh`: phase-scoped test entrypoint

## Notes

- `resources/model/` is gitignored.
- `build.rs` links required macOS frameworks for AppKit/CoreGraphics/AX APIs.
- `make check-all-llm` runs lint/test/build with `--features local-llm` (requires full Xcode + Metal tools).

pkill -x quillfix; sleep 1 && bash scripts/bundle.sh && open QuillFix.app
