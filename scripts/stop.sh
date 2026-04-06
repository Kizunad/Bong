#!/bin/bash
# stop.sh — 停止所有 Bong 进程
tmux kill-session -t bong 2>/dev/null && echo "Killed tmux session 'bong'" || echo "No session 'bong' found"
pkill -f "bong-server" 2>/dev/null && echo "Killed bong-server" || true
pkill -f "tiandao/src/main.ts" 2>/dev/null && echo "Killed tiandao agent" || true
redis-cli shutdown nosave 2>/dev/null && echo "Stopped Redis" || true
echo "Done"
