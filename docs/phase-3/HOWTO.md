# Phase 3 HOWTO

## Prerequisites

- Accessibility permission granted to QuillFix in System Settings.

## Build + Run

```bash
bash scripts/bundle.sh
open QuillFix.app
```

## Manual test

- Open TextEdit and select text like `hello world`.
- Verify monitor/debounce pipeline accepts only stable selections.

## Run tests

```bash
cargo test --test unit_debounce --test integration_event_monitor
bash scripts/run_integration_tests.sh phase3
```
