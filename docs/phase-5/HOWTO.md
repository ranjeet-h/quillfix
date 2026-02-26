# Phase 5 HOWTO

## Prerequisites

- Ensure model files are present in `resources/model/`.

## Build + Run

```bash
bash scripts/bundle.sh
open QuillFix.app
```

## Manual test

- Select text `teh quik brwon fox`.
- Trigger correction and confirm output `the quick brown fox`.

## Run tests

```bash
cargo test --test unit_prompt --test unit_replacement --test integration_llm
bash scripts/run_integration_tests.sh phase5
```
