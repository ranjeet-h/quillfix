#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_ID="mlx-community/Qwen3.5-0.8B-MLX-8bit"
TARGET_DIR="$ROOT_DIR/resources/model"

mkdir -p "$TARGET_DIR"

if command -v hf >/dev/null 2>&1; then
  hf download "$MODEL_ID" --local-dir "$TARGET_DIR"
elif command -v huggingface-cli >/dev/null 2>&1; then
  huggingface-cli download "$MODEL_ID" --local-dir "$TARGET_DIR"
else
  echo "Install Hugging Face CLI first: pipx install huggingface_hub" >&2
  exit 1
fi

echo "Model downloaded to $TARGET_DIR"
