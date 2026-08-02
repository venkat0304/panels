#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
BINDGEN_BIN=${BINDGEN_BIN:-bindgen}
HEADER="$ROOT_DIR/vendor/libghostty-vt/include/ghostty/vt.h"
OUTPUT="$ROOT_DIR/src/ghostty/bindings.rs"

if ! command -v "$BINDGEN_BIN" >/dev/null 2>&1; then
  echo "error: bindgen CLI 0.72.1 is required (cargo install bindgen-cli --version 0.72.1)" >&2
  exit 1
fi

"$BINDGEN_BIN" "$HEADER" \
  --allowlist-type '^Ghostty.*' \
  --allowlist-function '^ghostty_.*' \
  --allowlist-var '^GHOSTTY_.*' \
  --with-derive-default \
  --output "$OUTPUT" \
  -- "-I$ROOT_DIR/vendor/libghostty-vt/include"

echo "generated $OUTPUT"
