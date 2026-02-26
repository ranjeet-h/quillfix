#!/usr/bin/env bash
set -euo pipefail

PHASE=${1:-all}
case "$PHASE" in
  phase1) cargo test --test integration_bundle ;;
  phase2) cargo test --test integration_menu_bar ;;
  phase3) cargo test --test integration_event_monitor ;;
  phase4) cargo test --test integration_popup ;;
  phase5) cargo test --test integration_llm ;;
  phase6) cargo test --test integration_e2e ;;
  all)    cargo test ;;
  *)
    echo "Usage: bash scripts/run_integration_tests.sh [phase1|phase2|phase3|phase4|phase5|phase6|all]" >&2
    exit 1
    ;;
esac
