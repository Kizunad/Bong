#!/usr/bin/env bash
# pipeline.sh — terrain_gen 主流程
# 用法:
#   cd worldgen && bash pipeline.sh
#   cd worldgen && bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-smoke raster
#   cd worldgen && bash pipeline.sh ../server/zones.worldview.example.json generated/snapshot anvil
#   cd worldgen && bash pipeline.sh ../server/zones.worldview.example.json generated/snapshot anvil 128
#
# BACKEND:
#   raster (默认): 写 raster .bin layers + zone PNG previews
#   worldpainter: 写 wp 项目（已有）
#   anvil: 先跑 raster（保证 PNG 预览完整），再用 P1 anvil_world_export 写
#          <OUTPUT>/world/region/r.X.Z.mca（plan-worldgen-anvil-export-v1 §2）
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BLUEPRINT_REL="${1:-../server/zones.worldview.example.json}"
OUTPUT_REL="${2:-generated/terrain-gen-smoke}"
BACKEND="${3:-raster}"
TILE_SIZE="${4:-512}"

echo "=== 末法残土 terrain_gen Pipeline ==="
echo "蓝图: ${BLUEPRINT_REL}"
echo "输出目录: ${OUTPUT_REL}"
echo "Bake backend: ${BACKEND}"
echo "Tile size: ${TILE_SIZE}"
echo ""

# Anvil backend: 先跑 raster（PR #78 worldgen-preview workflow 仍消费 PNG previews），
# 再叠 anvil 文件树。raster 自身的 manifest/PNG 仍正确产出。
TERRAIN_BACKEND="$BACKEND"
if [ "$BACKEND" = "anvil" ]; then
  TERRAIN_BACKEND="raster"
fi

python3 -m scripts.terrain_gen \
  --blueprint "$BLUEPRINT_REL" \
  --output-dir "$OUTPUT_REL" \
  --tile-size "$TILE_SIZE" \
  --backend "$TERRAIN_BACKEND"

if [ "$BACKEND" = "anvil" ]; then
  echo ""
  echo "=== Anvil world export (plan-worldgen-anvil-export-v1 §2) ==="
  WORLD_OUT="$OUTPUT_REL/world"
  # 默认 chunk 范围 ±25 = ±400 blocks，覆盖 plan-worldgen-snapshot-v1 iso ±400 tp
  # 使用 rolling_hills synthetic height_fn（P2 / 后续 plan 接入真 raster reader 时
  # 只换 fn body 不改 export_anvil_world 签名）
  python3 - <<PYEOF
from pathlib import Path
import sys, time

sys.path.insert(0, str(Path("scripts/terrain_gen")))
from anvil_world_export import export_anvil_world, rolling_hills_height_fn

t0 = time.perf_counter()
height_fn = rolling_hills_height_fn(base=64, amplitude=12, period_blocks=128)
result = export_anvil_world(
    Path("$WORLD_OUT"),
    chunk_x_min=-25, chunk_x_max=25,
    chunk_z_min=-25, chunk_z_max=25,
    height_fn=height_fn,
)
dt = time.perf_counter() - t0
print(f"chunks_written={result['chunks_written']} regions_written={result['regions_written']} took={dt:.1f}s")
print(f"region_dir={result['region_dir']}")
PYEOF
fi

echo ""
echo "主要产物:"
echo "  plan: ${OUTPUT_REL}/terrain-plan.json"
echo "  summary: ${OUTPUT_REL}/terrain-fields-summary.json"
echo "  focus layout: ${OUTPUT_REL}/focus-layout-preview.png"
echo "  focus surface: ${OUTPUT_REL}/focus-surface-preview.png"
echo "  focus height: ${OUTPUT_REL}/focus-height-preview.png"
if [ "$BACKEND" = "worldpainter" ]; then
  echo "  worldpainter dir: ${OUTPUT_REL}/worldpainter"
fi
if [ "$BACKEND" = "raster" ] || [ "$BACKEND" = "anvil" ]; then
  echo "  raster dir: ${OUTPUT_REL}/rasters"
  echo "  raster manifest: ${OUTPUT_REL}/rasters/manifest.json"
fi
if [ "$BACKEND" = "anvil" ]; then
  echo "  anvil world: ${OUTPUT_REL}/world (BONG_WORLD_PATH=…/world)"
  echo "  anvil regions: ${OUTPUT_REL}/world/region/r.*.mca"
fi
echo ""
echo "分区预览示例:"
echo "  ${OUTPUT_REL}/zone-blood_valley-surface-preview.png"
echo "  ${OUTPUT_REL}/zone-qingyun_peaks-height-preview.png"
echo "  ${OUTPUT_REL}/zone-north_wastes-layout-preview.png"
echo ""
echo "完成。"
