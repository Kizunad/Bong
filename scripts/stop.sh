#!/usr/bin/env bash
# stop.sh — 停止由 Bong 生命周期记录管理的进程
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"

bong_server_stop_managed && echo "Stopped managed bong-server" || exit 1
tmux kill-session -t bong 2>/dev/null && echo "Killed tmux session 'bong'" || echo "No session 'bong' found"
pkill -f "tiandao/src/main.ts" 2>/dev/null && echo "Killed tiandao agent" || true
redis-cli shutdown nosave 2>/dev/null && echo "Stopped Redis" || true
echo "Done"
