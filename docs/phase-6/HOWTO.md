# Phase 6 HOWTO

## Build release

```bash
bash scripts/bundle.sh
```

## Run

```bash
open QuillFix.app
```

## Full test suite

```bash
cargo test
bash scripts/run_integration_tests.sh all
```

## Notes

- Includes integration/e2e test coverage for correction flow and guards.
