#!/usr/bin/env bash
# pipeline.sh — 末法残土世界生成总流程
# 用法:
#   cd worldgen && bash pipeline.sh
#   cd worldgen && bash pipeline.sh 768 ../server/zones.worldview.example.json
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORLD_DIR="$SCRIPT_DIR/server/mofa-world"
RADIUS="${1:-512}"
BLUEPRINT_REL="${2:-../server/zones.worldview.example.json}"

echo "=== 末法残土世界生成 Pipeline ==="
echo "半径: ${RADIUS} 格"
echo "蓝图: ${BLUEPRINT_REL}"
echo "世界目录: ${WORLD_DIR}"
echo ""

echo "[1/3] Datapack 预生成"
bash "$SCRIPT_DIR/worldgen.sh" "$RADIUS"
echo ""

echo "[2/3] Python 后处理"
if [ ! -d "$SCRIPT_DIR/.venv" ]; then
    echo "[!] 未找到 .venv，先执行: bash setup.sh"
    exit 1
fi

source "$SCRIPT_DIR/.venv/bin/activate"
python3 "$SCRIPT_DIR/scripts/postprocess.py" "server/mofa-world" --blueprint "$BLUEPRINT_REL"
echo ""

echo "[3/3] Valence 接入提示"
echo "导出环境变量后启动 server:"
echo "  BONG_WORLD_PATH=$WORLD_DIR cargo run"
echo ""
echo "完成。"
