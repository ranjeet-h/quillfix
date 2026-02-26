# Phase 2 HOWTO

## Goal

Implement QuillFix enable/disable state wiring with persistent defaults storage.

## Run tests

```bash
cargo test --test unit_menu_bar --test integration_menu_bar
```

## Notes

- `DEFAULTS_KEY` is `quillfix.enabled`.
- Menu state is persisted using macOS `defaults` under domain `com.quillfix.app`.
- Event monitor start/stop hooks are called when state flips.
