#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="QuillFix"
APP_DIR="$ROOT_DIR/${APP_NAME}.app"
DIST_DIR="$ROOT_DIR/dist"

VERSION="$(awk -F ' *= *' '/^version *=/ { gsub(/"/, "", $2); print $2; exit }' "$ROOT_DIR/Cargo.toml")"
DMG_PATH="$DIST_DIR/${APP_NAME}-${VERSION}.dmg"

bash "$ROOT_DIR/scripts/bundle.sh"

mkdir -p "$DIST_DIR"
rm -f "$DMG_PATH"

hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$APP_DIR" \
  -ov \
  -format UDZO \
  "$DMG_PATH"

echo "Created DMG: $DMG_PATH"
