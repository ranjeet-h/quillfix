#!/usr/bin/env bash
set -euo pipefail

PHASE="${1:-all}"

case "$PHASE" in
  phase1)
    cargo test --test unit_permissions -- --nocapture
    ;;
  phase5)
    cargo test --test unit_prompt -- --nocapture
    ;;
  all)
    cargo test -- --nocapture
    ;;
  *)
    echo "Usage: $0 [phase1|phase5|all]" >&2
    exit 1
    ;;
esac
