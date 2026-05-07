#!/bin/bash
# start.sh — 一键启动 Redis + Server + Agent（tmux 三面板）
# 用法: start.sh [--mock]    # --mock 走 npm run start:mock，不调真实 LLM
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

AGENT_CMD="npx tsx src/main.ts"
for arg in "$@"; do
  case "$arg" in
    --mock) AGENT_CMD="npm run start:mock" ;;
    *) echo "unknown arg: $arg" >&2; exit 1 ;;
  esac
done

# Rust 工具链
export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
RUNTIME_PATH="$PATH"

SESSION="bong"

# 地形 raster manifest（server 读 BONG_TERRAIN_RASTER_PATH 决定是否加载真实地图）
RASTER_MANIFEST="$ROOT/worldgen/generated/terrain-gen/rasters/manifest.json"
if [ -f "$RASTER_MANIFEST" ]; then
  BONG_TERRAIN_RASTER_PATH="$RASTER_MANIFEST"
  echo "[bong] terrain raster: $RASTER_MANIFEST"
else
  BONG_TERRAIN_RASTER_PATH=""
  echo "[bong] WARN: raster manifest not found at $RASTER_MANIFEST — 将 fallback 扁平世界"
  echo "       先跑: bash scripts/dev-reload.sh  (或 cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster)"
fi

# 检查 Redis
if ! command -v redis-server &>/dev/null; then
  echo "Redis not installed. Run: sudo apt install -y redis-server"
  exit 1
fi

# 杀掉旧会话
tmux kill-session -t "$SESSION" 2>/dev/null || true

# 创建 tmux session，3 个 pane
#   pane 0: Redis
#   pane 1: Rust server
#   pane 2: Tiandao agent

tmux new-session -d -s "$SESSION" -n main

# Pane 0: Redis
tmux send-keys -t "$SESSION:main" "if redis-cli ping >/dev/null 2>&1; then printf '[bong] redis already running on :6379\n'; else redis-server --loglevel warning; fi" Enter

# Pane 1: Server
# BONG_ROGUE_SEED_COUNT 默认 0：100 NPC 在单核 WSL2 跑不动（37 个 per-NPC system
# × 100 entity → TPS 实测 0.7，所有玩家 packet 卡几秒）。需要 LOD 优化才能恢复
# 100 seed。可手动覆盖：BONG_ROGUE_SEED_COUNT=10 bash start.sh
tmux split-window -h -t "$SESSION:main"
tmux send-keys -t "$SESSION:main.1" \
  "export PATH='${RUNTIME_PATH}' && \
   export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && \
   export BONG_TERRAIN_RASTER_PATH='${BONG_TERRAIN_RASTER_PATH}' && \
   export BONG_ROGUE_SEED_COUNT='${BONG_ROGUE_SEED_COUNT:-0}' && \
   cd '$ROOT/server' && \
   echo '[bong] starting server (rogue seed='\$BONG_ROGUE_SEED_COUNT')...' && \
   cargo run --release 2>&1" Enter

# Pane 2: Agent (延迟启动，等 server + redis 就绪)
tmux split-window -v -t "$SESSION:main.1"
tmux send-keys -t "$SESSION:main.2" \
  "sleep 8 && \
   cd '$ROOT/agent/packages/tiandao' && \
   echo '[bong] starting tiandao agent ($AGENT_CMD)...' && \
   $AGENT_CMD 2>&1" Enter

# 布局均匀
tmux select-layout -t "$SESSION:main" main-vertical

echo "=== Bong started in tmux session '$SESSION' ==="
echo ""
echo "  tmux attach -t $SESSION    # 查看"
echo "  tmux kill-session -t $SESSION  # 停止全部"
echo ""
echo "Panes:"
echo "  0: Redis"
echo "  1: Rust server (:25565)"
echo "  2: Tiandao agent ($AGENT_CMD)"
