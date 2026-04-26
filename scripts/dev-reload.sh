#!/usr/bin/env bash
# dev-reload.sh — one-command regen + validate + rebuild + restart
# Usage: bash scripts/dev-reload.sh [--skip-regen] [--skip-validate]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

SKIP_REGEN=false
SKIP_VALIDATE=false
for arg in "$@"; do
    case "$arg" in
        --skip-regen)    SKIP_REGEN=true ;;
        --skip-validate) SKIP_VALIDATE=true ;;
    esac
done

RASTER_DIR="worldgen/generated/terrain-gen/rasters"
WORLDGEN_RASTER_DIR="generated/terrain-gen/rasters"
MANIFEST="$RASTER_DIR/manifest.json"

# plan-tsy-worldgen-v1 §6.1 — TSY 双 manifest 改造
TSY_BLUEPRINT="server/zones.tsy.json"
WORLDGEN_TSY_OUTPUT_DIR="generated/terrain-gen-tsy"
TSY_RASTER_DIR="worldgen/$WORLDGEN_TSY_OUTPUT_DIR/rasters"
WORLDGEN_TSY_RASTER_DIR="$WORLDGEN_TSY_OUTPUT_DIR/rasters"
TSY_MANIFEST="$TSY_RASTER_DIR/manifest.json"

# --- Step 1: Regenerate rasters (overworld + optional TSY) ---
if [ "$SKIP_REGEN" = false ]; then
    if [ -f "$TSY_BLUEPRINT" ]; then
        echo "==> [1/4] Regenerating terrain rasters (overworld + tsy)..."
        (cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster \
             --tsy-blueprint "../$TSY_BLUEPRINT" \
             --tsy-output-dir "$WORLDGEN_TSY_OUTPUT_DIR") || {
            echo "FAIL: terrain generation failed"; exit 1
        }
    else
        echo "==> [1/4] Regenerating terrain rasters (overworld only — no $TSY_BLUEPRINT)..."
        (cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster) || {
            echo "FAIL: terrain generation failed"; exit 1
        }
    fi
    echo "    OK"
else
    echo "==> [1/4] Skipping raster regeneration (--skip-regen)"
fi

# --- Step 2: Validate raster data (overworld + optional TSY) ---
if [ "$SKIP_VALIDATE" = false ]; then
    echo "==> [2/4] Validating raster data..."
    (cd worldgen && .venv/bin/python -c "
from scripts.terrain_gen.harness.raster_check import validate_rasters
import sys
ok, msg = validate_rasters('$WORLDGEN_RASTER_DIR')
print('[overworld]')
print(msg)
ok_all = ok
import os.path
if os.path.isdir('$WORLDGEN_TSY_RASTER_DIR'):
    ok2, msg2 = validate_rasters('$WORLDGEN_TSY_RASTER_DIR')
    print('[tsy]')
    print(msg2)
    ok_all = ok_all and ok2
sys.exit(0 if ok_all else 1)
") || { echo "FAIL: raster validation failed"; exit 1; }
    echo "    OK"
else
    echo "==> [2/4] Skipping validation (--skip-validate)"
fi

# --- Step 3: Rebuild server ---
echo "==> [3/4] Building server..."
(cd server && cargo build 2>&1) || { echo "FAIL: cargo build failed"; exit 1; }
echo "    OK"

# --- Step 4: Restart server ---
echo "==> [4/4] Restarting server..."
pkill -f 'target/debug/bong-server' 2>/dev/null || true
sleep 0.5
MANIFEST_ABS="$(pwd)/$MANIFEST"
TSY_MANIFEST_ABS="$(pwd)/$TSY_MANIFEST"
ENV_ARGS=("BONG_TERRAIN_RASTER_PATH=$MANIFEST_ABS")
if [ -f "$TSY_MANIFEST_ABS" ]; then
    ENV_ARGS+=("BONG_TSY_RASTER_PATH=$TSY_MANIFEST_ABS")
fi
(cd server && env "${ENV_ARGS[@]}" cargo run > /tmp/bong-server.log 2>&1 &)
disown
sleep 2

if grep -q "loaded.*terrain tiles" /tmp/bong-server.log 2>/dev/null; then
    TILES=$(grep -o 'loaded [0-9]* terrain' /tmp/bong-server.log | grep -o '[0-9]*')
    echo "    Server running — $TILES tiles loaded"
else
    echo "    Server starting... check /tmp/bong-server.log"
fi

echo "==> Done. Connect to localhost:25565"
