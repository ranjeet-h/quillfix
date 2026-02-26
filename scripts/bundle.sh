#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="QuillFix"
APP_DIR="$ROOT_DIR/${APP_NAME}.app"
BIN_PATH="$ROOT_DIR/target/release/quillfix"
RES_DIR="$ROOT_DIR/resources"
PYTHON_DIR="$ROOT_DIR/python-inference"

cargo build --release

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/quillfix"
cp "$RES_DIR/Info.plist" "$APP_DIR/Contents/Info.plist"
cp "$RES_DIR/entitlements.plist" "$APP_DIR/Contents/Resources/entitlements.plist"
if [[ -d "$RES_DIR/model" ]]; then
  cp -R "$RES_DIR/model" "$APP_DIR/Contents/Resources/model"
fi

if [[ -d "$PYTHON_DIR" ]]; then
  cp -R "$PYTHON_DIR" "$APP_DIR/Contents/Resources/python-inference"
fi

IDENTITY="${CODE_SIGN_IDENTITY:--}"
codesign --force --deep --options runtime --entitlements "$RES_DIR/entitlements.plist" --sign "$IDENTITY" "$APP_DIR"

echo "Bundled: $APP_DIR"
