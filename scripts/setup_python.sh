#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="$SCRIPT_DIR/python-inference"

if [ -d "$VENV_DIR" ] && [ -f "$VENV_DIR/bin/activate" ]; then
    echo "Python venv already exists at $VENV_DIR"
else
    echo "Creating Python venv at $VENV_DIR..."
    python3 -m venv "$VENV_DIR"
fi

echo "Installing dependencies..."
"$VENV_DIR/bin/pip" install --no-cache-dir --upgrade pip
"$VENV_DIR/bin/pip" install --no-cache-dir -r "$SCRIPT_DIR/python-inference/requirements.txt"

echo "Setup complete. Run './python-inference/bin/python3 python-inference/infer.py' to test."
