#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="QuillFix"
APP_DIR="$ROOT_DIR/${APP_NAME}.app"
BIN_PATH="$ROOT_DIR/target/release/quillfix"
RES_DIR="$ROOT_DIR/resources"
PYTHON_DIR="$ROOT_DIR/python-inference"

remove_tree() {
  local target="$1"
  python3 - "$target" <<'PY'
import shutil
import sys
from pathlib import Path

path = Path(sys.argv[1])
if path.exists():
    shutil.rmtree(path)
PY
}

prune_python_runtime() {
  local runtime_dir="$1"
  local site_packages
  site_packages="$(find "$runtime_dir/lib" -type d -path '*/site-packages' | head -n 1)"

  remove_tree "$runtime_dir/include"
  remove_tree "$runtime_dir/share"
  remove_tree "$runtime_dir/__pycache__"

  while IFS= read -r dir; do
    remove_tree "$dir"
  done < <(find "$runtime_dir" -type d \( -name '__pycache__' -o -name 'tests' -o -name 'test' \) -prune -print)
  find "$runtime_dir" -type f \( -name '*.pyc' -o -name '*.pyo' \) -delete

  if [[ -n "$site_packages" ]]; then
    rm -rf \
      "$site_packages"/pip* \
      "$site_packages"/setuptools* \
      "$site_packages"/wheel* \
      "$site_packages"/torch \
      "$site_packages"/torch-*.dist-info \
      "$site_packages"/functorch \
      "$site_packages"/torchgen \
      "$site_packages"/cv2 \
      "$site_packages"/opencv_python-*.dist-info \
      "$site_packages"/pandas \
      "$site_packages"/pandas-*.dist-info \
      "$site_packages"/pyarrow \
      "$site_packages"/pyarrow-*.dist-info \
      "$site_packages"/datasets \
      "$site_packages"/fastapi \
      "$site_packages"/uvicorn \
      "$site_packages"/aiohttp \
      "$site_packages"/aiohappyeyeballs* \
      "$site_packages"/aiosignal* \
      "$site_packages"/frozenlist \
      "$site_packages"/multidict \
      "$site_packages"/propcache \
      "$site_packages"/yarl \
      "$site_packages"/starlette \
      "$site_packages"/sympy \
      "$site_packages"/sympy-*.dist-info \
      "$site_packages"/networkx \
      "$site_packages"/networkx-*.dist-info \
      "$site_packages"/google \
      "$site_packages"/protobuf-*.dist-info \
      "$site_packages"/hf_xet \
      "$site_packages"/hf_xet-*.dist-info \
      "$site_packages"/hf_transfer \
      "$site_packages"/hf_transfer-*.dist-info \
      "$site_packages"/rich \
      "$site_packages"/Pygments \
      "$site_packages"/markdown_it* \
      "$site_packages"/typer* \
      "$site_packages"/shellingham* \
      "$site_packages"/annotated_doc* \
      "$site_packages"/annotated_types* \
      "$site_packages"/multiprocess \
      "$site_packages"/dill* \
      "$site_packages"/xxhash* \
      "$site_packages"/attr \
      "$site_packages"/attrs*
  fi
}

cargo build --release

remove_tree "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/quillfix"
cp "$RES_DIR/Info.plist" "$APP_DIR/Contents/Info.plist"
cp "$RES_DIR/entitlements.plist" "$APP_DIR/Contents/Resources/entitlements.plist"
if [[ -d "$RES_DIR/model" ]]; then
  cp -R "$RES_DIR/model" "$APP_DIR/Contents/Resources/model"
fi

if [[ -d "$PYTHON_DIR" ]]; then
  mkdir -p "$APP_DIR/Contents/Resources/python-inference"
  cp -R "$PYTHON_DIR"/. "$APP_DIR/Contents/Resources/python-inference"
  prune_python_runtime "$APP_DIR/Contents/Resources/python-inference"
fi

IDENTITY="${CODE_SIGN_IDENTITY:--}"
codesign --force --deep --options runtime --entitlements "$RES_DIR/entitlements.plist" --sign "$IDENTITY" "$APP_DIR"

echo "Bundled: $APP_DIR"
