#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/client/src/main/resources/assets/minecraft/textures/block"
OUT_DIR="$ROOT/client/resourcepack"
OUT="$OUT_DIR/bong-mineral-v1.zip"
SHA1_OUT="$OUT.sha1"

if ! command -v zip >/dev/null 2>&1; then
  echo "zip is required to build the resource pack" >&2
  exit 1
fi

if ! command -v sha1sum >/dev/null 2>&1; then
  echo "sha1sum is required to build the resource pack" >&2
  exit 1
fi

if [[ ! -d "$SRC" ]]; then
  echo "missing block texture source directory: $SRC" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

mkdir -p "$TMP/assets/minecraft/textures/block"
cp "$SRC"/*.png "$TMP/assets/minecraft/textures/block/"

cat >"$TMP/pack.mcmeta" <<'JSON'
{
  "pack": {
    "pack_format": 15,
    "description": "Bong mineral v1 ore texture overrides"
  }
}
JSON

find "$TMP" -exec touch -h -t 202604290000.00 {} +
rm -f "$OUT" "$SHA1_OUT"
(
  cd "$TMP"
  find . -type f | LC_ALL=C sort | sed 's#^./##' | zip -X -q "$OUT" -@
)

sha1sum "$OUT" | awk '{print $1}' >"$SHA1_OUT"
printf 'built %s\nsha1 %s\n' "$OUT" "$(cat "$SHA1_OUT")"
