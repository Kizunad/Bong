#!/usr/bin/env bash
# setup.sh — 初始化 worldgen terrain_gen 环境
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== worldgen 环境初始化 ==="

# Python venv
if [ ! -d ".venv" ]; then
    python3 -m venv .venv
    echo "[✓] 创建 .venv"
else
    echo "[·] .venv 已存在，跳过"
fi

source .venv/bin/activate
pip install --quiet --upgrade pip
pip install --quiet numpy
echo "[✓] 安装 numpy（terrain_gen.fields 依赖）"

echo ""
echo "完成。后续用法："
echo "  source .venv/bin/activate"
echo "  python3 -m scripts.terrain_gen --backend raster"
