#!/bin/bash
# start.sh — 一键启动 Redis + Server + Agent（tmux 三面板）
# 用法: start.sh [--mock]    # --mock 走 npm run start:mock，不调真实 LLM
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"

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

stop_managed_server_before_start() {
  local status

  if bong_server_stop_managed_for_replacement "tmux teardown and stack startup"; then
    return 0
  else
    status=$?
  fi
  echo "FAIL: refusing to destroy tmux session or start a replacement server" >&2
  return "$status"
}

start_bong_stack() {
# 先让已记录的精确 server PID 完整处理 SIGTERM/AppExit/Last；不能先杀 tmux，
# 否则其 HUP 会跳过服务器的优雅关服路径。
stop_managed_server_before_start || return $?
if bong_server_tmux_has_unmanaged_server "$SESSION"; then
  echo "FAIL: tmux session '$SESSION' still owns an unrecorded bong-server; refusing HUP shutdown" >&2
  exit 1
else
  tmux_scan_status=$?
  if [ "$tmux_scan_status" -eq 2 ]; then
    echo "FAIL: could not verify tmux session '$SESSION'; refusing teardown" >&2
    exit 1
  fi
fi

# 杀掉旧会话
tmux kill-session -t "$SESSION" 2>/dev/null || true

(
  cd "$ROOT/server"
  cargo build --release
)
server_executable="$(bong_server_resolve_executable "$ROOT/server" "$CARGO_TARGET_DIR/release/bong-server")"
runtime_dir="$(bong_server_runtime_dir)"
server_ready_path="$runtime_dir/bong-server-ready-$$"
if [ -e "$server_ready_path" ] || [ -L "$server_ready_path" ]; then
  echo "FAIL: refusing to overwrite existing server readiness path $server_ready_path" >&2
  exit 1
fi

# 创建 tmux session，3 个 pane
#   pane 0: Redis
#   pane 1: Rust server
#   pane 2: Tiandao agent

tmux new-session -d -s "$SESSION" -n main

# Pane 0: Redis
tmux send-keys -t "$SESSION:main" "if redis-cli ping >/dev/null 2>&1; then printf '[bong] redis already running on :6379\n'; else redis-server --loglevel warning; fi" Enter

# Pane 1: Server
# 先 build，再由 shell 直接 exec 最终二进制；记录的 PID 始终是 server 本身，不是 cargo 或 pane shell。
tmux split-window -h -t "$SESSION:main"
tmux send-keys -t "$SESSION:main.1" \
  "export PATH='${RUNTIME_PATH}' && \
   export CARGO_TARGET_DIR='${CARGO_TARGET_DIR}' && \
   export BONG_TERRAIN_RASTER_PATH='${BONG_TERRAIN_RASTER_PATH}' && \
   export BONG_ROGUE_SEED_COUNT='${BONG_ROGUE_SEED_COUNT:-20}' && \
   export BONG_DORMANT_ROGUE_SEED_COUNT='${BONG_DORMANT_ROGUE_SEED_COUNT:-1000}' && \
   export BONG_NPC_NO_DORMANT='${BONG_NPC_NO_DORMANT:-0}' && \
   export BONG_SERVER_READY_PATH='$server_ready_path' && \
   cd '$ROOT/server' && \
   exec '${CARGO_TARGET_DIR}/release/bong-server'" Enter

server_pid=""
server_starttime=""
server_executable_identity=""
server_identity_pinned=0
cleanup_pinned_server_or_preserve_tmux() {
  local reason="$1" cleanup_status

  if [ "$server_identity_pinned" -ne 1 ]; then
    rm -f -- "$server_ready_path"
    echo "FAIL: $reason; server identity is not pinned, preserving tmux for diagnosis" >&2
    return 1
  fi
  if bong_server_stop_pinned_process \
    "$server_pid" "$server_starttime" "$server_executable_identity" 10 2; then
    cleanup_status=0
  else
    cleanup_status=$?
  fi
  rm -f -- "$server_ready_path"
  if [ "$cleanup_status" -ne 0 ]; then
    echo "FAIL: $reason; could not safely stop pinned server, preserving tmux for diagnosis" >&2
    return 1
  fi
  tmux kill-session -t "$SESSION" 2>/dev/null || true
  return 0
}
for _ in $(seq 1 500); do
  pane_pid="$(tmux display-message -p -t "$SESSION:main.1" '#{pane_pid}' 2>/dev/null || true)"
  if bong_server_wait_for_executable "$pane_pid" "$server_executable" 1; then
    server_pid="$pane_pid"
    break
  fi
  sleep 0.01
done
if [ -z "$server_pid" ]; then
  cleanup_pinned_server_or_preserve_tmux \
    "server pane did not exec $server_executable" || true
  exit 1
fi
server_starttime="$(bong_server_process_starttime "$server_pid")" || {
  rm -f -- "$server_ready_path"
  echo "FAIL: could not pin server starttime before readiness; preserving tmux for diagnosis" >&2
  exit 1
}
server_executable_identity="$(bong_server_process_executable_identity "$server_pid")" || {
  rm -f -- "$server_ready_path"
  echo "FAIL: could not pin server executable identity before readiness; preserving tmux for diagnosis" >&2
  exit 1
}
server_identity_pinned=1

ready_pid=""
for _ in $(seq 1 1000); do
  if ready_pid="$(bong_server_read_ready_pid "$server_ready_path")"; then
    break
  else
    ready_status=$?
  fi
  if [ "$ready_status" -eq 2 ]; then
    cleanup_pinned_server_or_preserve_tmux \
      "server published invalid readiness evidence at $server_ready_path" || true
    exit 1
  fi
  if ! bong_server_process_is_running "$server_pid" \
    || [ "$(bong_server_process_starttime "$server_pid" 2>/dev/null || true)" != "$server_starttime" ] \
    || [ "$(bong_server_process_executable_identity "$server_pid" 2>/dev/null || true)" != "$server_executable_identity" ]; then
    cleanup_pinned_server_or_preserve_tmux \
      "server identity changed or exited before publishing readiness" || true
    exit 1
  fi
  sleep 0.01
done
if [ "$ready_pid" != "$server_pid" ] \
  || [ "$(bong_server_process_starttime "$server_pid" 2>/dev/null || true)" != "$server_starttime" ] \
  || [ "$(bong_server_process_executable_identity "$server_pid" 2>/dev/null || true)" != "$server_executable_identity" ]; then
  cleanup_pinned_server_or_preserve_tmux \
    "readiness evidence did not match the pinned server identity" || true
  exit 1
fi
rm -f -- "$server_ready_path"

# Valence starts its async TCP bind from PostStartup. Readiness proves all
# application Startup systems succeeded; publishing PID authority additionally
# requires the exact pinned server to own the IPv4 listener and accept a probe.
port_ready=0
listener_inspection_failed=0
for _ in $(seq 1 500); do
  if bong_server_pinned_process_owns_ipv4_listener \
      "$server_pid" "$server_starttime" "$server_executable_identity" 25565; then
    listener_status=0
  else
    listener_status=$?
  fi
  if [ "$listener_status" -ne 0 ] && [ "$listener_status" -ne 1 ]; then
    listener_inspection_failed=1
    break
  fi
  if [ "$listener_status" -eq 0 ] && bong_server_port_is_open 25565; then
    if bong_server_pinned_process_owns_ipv4_listener \
        "$server_pid" "$server_starttime" "$server_executable_identity" 25565; then
      port_ready=1
      break
    else
      listener_status=$?
    fi
    if [ "$listener_status" -ne 1 ]; then
      listener_inspection_failed=1
      break
    fi
  fi
  if ! bong_server_pinned_process_status \
    "$server_pid" "$server_starttime" "$server_executable_identity"; then
    cleanup_pinned_server_or_preserve_tmux \
      "server identity changed or became uninspectable before binding port 25565" || true
    exit 1
  fi
  sleep 0.01
done
if [ "$port_ready" -ne 1 ]; then
  if [ "$listener_inspection_failed" -eq 1 ]; then
    bind_failure_reason="production server listener ownership became uninspectable before authority publication"
  else
    bind_failure_reason="production server completed Startup but did not bind port 25565"
  fi
  if ! cleanup_pinned_server_or_preserve_tmux "$bind_failure_reason"; then
    exit 1
  fi
  echo "FAIL: $bind_failure_reason" >&2
  exit 1
fi

if ! bong_server_write_record "$server_pid" "$server_executable"; then
  if ! cleanup_pinned_server_or_preserve_tmux \
      "could not record server pid $server_pid"; then
    exit 1
  fi
  echo "FAIL: could not record server pid $server_pid" >&2
  exit 1
fi

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
}

bong_server_with_lock start_bong_stack
