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

# worldgen-v4 P1 §8.1 #7 — dev-only 3D 预览控制台依赖（FastAPI + uvicorn）。
# 用 --console 显式安装；默认不装，保证 CI / 生产 raster 流水线只依赖 numpy。
if [[ " $* " == *" --console "* ]]; then
    pip install --quiet -r requirements-dev.txt
    echo "[✓] 安装 console dev 依赖（fastapi + uvicorn + httpx）"
fi

echo ""
echo "完成。后续用法："
echo "  source .venv/bin/activate"
echo "  python3 -m scripts.terrain_gen --backend raster"
echo "  bash setup.sh --console     # 额外装 3D 控制台后端依赖"
echo "  bash pipeline.sh --console  # 跑 raster 后启动控制台 (http://127.0.0.1:8765)"
