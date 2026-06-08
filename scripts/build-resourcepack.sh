#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS_ROOT="${BONG_RESOURCEPACK_ASSETS_ROOT:-$ROOT/client/src/main/resources/assets}"
OUT_DIR="${BONG_RESOURCEPACK_OUT_DIR:-$ROOT/client/resourcepack}"
VERSION="${BONG_RESOURCEPACK_VERSION:-v1}"
PACK_NAME="bong-full-$VERSION.zip"
OUT="$OUT_DIR/$PACK_NAME"
SHA1_OUT="$OUT.sha1"
MANIFEST_OUT="$OUT_DIR/manifest.json"
BUILD_EPOCH="${BONG_RESOURCEPACK_BUILD_EPOCH:-202606080000.00}"

if ! command -v zip >/dev/null 2>&1; then
  echo "zip is required to build the resource pack" >&2
  exit 1
fi

if ! command -v sha1sum >/dev/null 2>&1; then
  echo "sha1sum is required to build the resource pack" >&2
  exit 1
fi

if [[ ! -d "$ASSETS_ROOT" ]]; then
  echo "missing resource asset source directory: $ASSETS_ROOT" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

INCLUDE_PREFIXES=(
  "minecraft/blockstates"
  "minecraft/models"
  "minecraft/textures/block"
  "minecraft/atlases"
  "bong/blockstates"
  "bong/models"
  "bong/geo"
  "bong/animations"
  "bong/textures/block"
  "bong/textures/entity"
  "bong/textures/item"
  "bong/textures/armor"
  "bong/particles"
  "bong/textures/particle"
  "bong-client/textures/hud/effects"
  "bong-client/textures/gui/skill"
  "bong/atmosphere"
  "bong/audio_recipes"
)

should_include() {
  local rel="$1"
  local rel_lc="${rel,,}"
  case "$rel_lc" in
    *.png|*.json|*.ogg|*.obj|*.mtl) ;;
    *) return 1 ;;
  esac
  local prefix
  for prefix in "${INCLUDE_PREFIXES[@]}"; do
    if [[ "$rel_lc" == "$prefix" || "$rel_lc" == "$prefix/"* ]]; then
      return 0
    fi
  done
  return 1
}

mkdir -p "$TMP/assets"
while IFS= read -r -d '' file; do
  if [[ "$file" == "$ASSETS_ROOT/"* ]]; then
    prefix_len=${#ASSETS_ROOT}
    rel="${file:prefix_len+1}"
  else
    rel="$file"
  fi
  should_include "$rel" || continue
  mkdir -p "$TMP/assets/$(dirname "$rel")"
  cp "$file" "$TMP/assets/$rel"
done < <(find "$ASSETS_ROOT" -type f -print0)

cat >"$TMP/pack.mcmeta" <<JSON
{
  "pack": {
    "pack_format": 15,
    "description": "Bong full resource pack $VERSION"
  }
}
JSON

find "$TMP" -exec touch -h -t "$BUILD_EPOCH" {} +
rm -f "$OUT" "$SHA1_OUT" "$MANIFEST_OUT"
(
  cd "$TMP"
  find . -type f | LC_ALL=C sort | sed 's#^./##' | zip -X -q "$OUT" -@
)

SHA1="$(sha1sum "$OUT" | awk '{print $1}')"
printf '%s\n' "$SHA1" >"$SHA1_OUT"
SIZE="$(wc -c <"$OUT" | tr -d '[:space:]')"

python3 - "$ASSETS_ROOT" "$OUT" "$SHA1" "$SIZE" "$VERSION" "$PACK_NAME" "$MANIFEST_OUT" <<'PY'
from __future__ import annotations

import json
import sys
from pathlib import Path

assets_root = Path(sys.argv[1])
out = Path(sys.argv[2])
sha1 = sys.argv[3]
size = int(sys.argv[4])
version = sys.argv[5]
pack_name = sys.argv[6]
manifest_out = Path(sys.argv[7])

subpacks = [
    ("mineral", ["minecraft/blockstates", "minecraft/models", "minecraft/textures/block", "minecraft/atlases", "bong/blockstates", "bong/textures/block"]),
    ("entity-model", ["bong/geo", "bong/animations", "bong/models", "bong/textures/entity", "bong/textures/item", "bong/textures/armor"]),
    ("vfx", ["bong/particles", "bong/textures/particle", "bong-client/textures/hud/effects", "bong-client/textures/gui/skill"]),
    ("audio", ["bong/atmosphere", "bong/audio_recipes"]),
]

def count_files(prefixes: list[str]) -> int:
    total = 0
    for prefix in prefixes:
        base = assets_root / prefix
        if not base.exists():
            continue
        total += sum(1 for p in base.rglob("*") if p.is_file() and p.suffix.lower() in {".png", ".json", ".ogg", ".obj", ".mtl"})
    return total

manifest = {
    "schema": 1,
    "name": "bong-full",
    "version": version,
    "file": pack_name,
    "sha1": sha1,
    "size": size,
    "url_env": "BONG_RESOURCE_PACK_URL",
    "sha1_env": "BONG_RESOURCE_PACK_SHA1",
    "force_accept_default": False,
    "packs": [
        {"id": pack_id, "paths": paths, "file_count": count_files(paths)}
        for pack_id, paths in subpacks
    ],
}
manifest_out.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

if out.stat().st_size != size:
    raise SystemExit("resource pack size changed while writing manifest")
PY

printf 'built %s\nsha1 %s\nmanifest %s\n' "$OUT" "$SHA1" "$MANIFEST_OUT"
