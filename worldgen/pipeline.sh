#!/usr/bin/env bash
# pipeline.sh — terrain_gen 主流程
# 用法:
#   cd worldgen && bash pipeline.sh
#   cd worldgen && bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-smoke raster
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BLUEPRINT_REL="${1:-../server/zones.worldview.example.json}"
OUTPUT_REL="${2:-generated/terrain-gen-smoke}"
BACKEND="${3:-raster}"

echo "=== 末法残土 terrain_gen Pipeline ==="
echo "蓝图: ${BLUEPRINT_REL}"
echo "输出目录: ${OUTPUT_REL}"
echo "Bake backend: ${BACKEND}"
echo ""

python3 -m scripts.terrain_gen \
  --blueprint "$BLUEPRINT_REL" \
  --output-dir "$OUTPUT_REL" \
  --backend "$BACKEND"

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
if [ "$BACKEND" = "raster" ]; then
  echo "  raster dir: ${OUTPUT_REL}/rasters"
  echo "  raster manifest: ${OUTPUT_REL}/rasters/manifest.json"
fi
echo ""
echo "分区预览示例:"
echo "  ${OUTPUT_REL}/zone-blood_valley-surface-preview.png"
echo "  ${OUTPUT_REL}/zone-qingyun_peaks-height-preview.png"
echo "  ${OUTPUT_REL}/zone-north_wastes-layout-preview.png"
echo ""
echo "完成。"
