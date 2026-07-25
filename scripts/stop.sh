#!/usr/bin/env bash
# stop.sh — 停止由 Bong 生命周期记录管理的进程
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"

stop_bong_stack() {
    if ! bong_server_stop_managed; then
        echo "FAIL: refusing to destroy tmux session while managed server ownership is unverified" >&2
        exit 1
    fi
    if bong_server_tmux_has_unmanaged_server bong; then
        echo "FAIL: tmux session 'bong' still owns an unrecorded bong-server; refusing HUP shutdown" >&2
        exit 1
    fi

    echo "Stopped managed bong-server"
    tmux kill-session -t bong 2>/dev/null && echo "Killed tmux session 'bong'" || echo "No session 'bong' found"
    pkill -f "tiandao/src/main.ts" 2>/dev/null && echo "Killed tiandao agent" || true
    redis-cli shutdown nosave 2>/dev/null && echo "Stopped Redis" || true
}

bong_server_with_lock stop_bong_stack
echo "Done"
