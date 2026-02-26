# Phase 4 HOWTO

## Build + Run

```bash
bash scripts/bundle.sh
open QuillFix.app
```

## Manual test

- Trigger popup show flow.
- Verify auto-hide, ESC hide, and success icon flash behavior.

## Run tests

```bash
cargo test --test unit_popup --test integration_popup
bash scripts/run_integration_tests.sh phase4
```
