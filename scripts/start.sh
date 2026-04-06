#!/bin/bash
# start.sh — 一键启动 Redis + Server + Agent（tmux 三面板）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Rust 工具链
export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"

SESSION="bong"

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
tmux send-keys -t "$SESSION:main" "redis-server --loglevel warning" Enter

# Pane 1: Server
tmux split-window -h -t "$SESSION:main"
tmux send-keys -t "$SESSION:main.1" \
  "export PATH='/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:\$PATH' && \
   export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && \
   cd '$ROOT/server' && \
   echo '[bong] starting server...' && \
   cargo run --release 2>&1" Enter

# Pane 2: Agent (延迟启动，等 server + redis 就绪)
tmux split-window -v -t "$SESSION:main.1"
tmux send-keys -t "$SESSION:main.2" \
  "sleep 8 && \
   cd '$ROOT/agent/packages/tiandao' && \
   echo '[bong] starting tiandao agent...' && \
   npx tsx src/main.ts 2>&1" Enter

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
echo "  2: Tiandao agent (3 agents)"
