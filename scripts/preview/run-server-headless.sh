#!/usr/bin/env bash
# run-server-headless.sh — 后台启动 Bong server（preview 用），等到端口 ready
#
# 用法:
#   bash scripts/preview/run-server-headless.sh [--release|--debug] [--timeout 60]
#
# 行为:
#   1. 后台启动 cargo run（默认 --release，速度更接近 CI）
#   2. 把进程 PID 写到 /tmp/bong-preview-server.pid
#   3. 轮询 TCP 127.0.0.1:25565，accept 即 ready
#   4. 超时（默认 90s）→ 打印 server log + 杀进程 + exit 1
#
# server 已是 offline mode + mock bridge（无 Redis 依赖），见 server/src/main.rs:64,68
#
# 退出后 server 仍在后台跑；调用方负责 kill `cat /tmp/bong-preview-server.pid` 清理。
# CI 上 job 结束 runner 会自动收回所有进程，无需显式清理。

set -euo pipefail

PROFILE="--release"
TIMEOUT_SECONDS=90
PORT=25565
PID_FILE="/tmp/bong-preview-server.pid"
LOG_FILE="/tmp/bong-preview-server.log"

while [ $# -gt 0 ]; do
  case "$1" in
    --release) PROFILE="--release"; shift ;;
    --debug)   PROFILE=""; shift ;;
    --timeout) TIMEOUT_SECONDS="$2"; shift 2 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT/server"

if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
  echo "❌ Server 已在跑 (PID $(cat "$PID_FILE"))，先 kill 再启动" >&2
  exit 1
fi

echo "[run-server-headless] 启动 server (cwd=$PWD profile=${PROFILE:-debug})..."
# nohup + setsid 防 CI 上父进程退出后子进程被收割
# stdout/stderr 都重定向到 LOG_FILE 方便失败时回看
nohup cargo run $PROFILE >"$LOG_FILE" 2>&1 &
SERVER_PID=$!
echo "$SERVER_PID" > "$PID_FILE"
echo "[run-server-headless] PID=$SERVER_PID log=$LOG_FILE"

# 轮询 25565 ready
elapsed=0
while [ "$elapsed" -lt "$TIMEOUT_SECONDS" ]; do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "❌ Server 进程已死 (PID $SERVER_PID)，最后 30 行 log:" >&2
    tail -n 30 "$LOG_FILE" >&2
    rm -f "$PID_FILE"
    exit 1
  fi
  if (echo > "/dev/tcp/127.0.0.1/$PORT") 2>/dev/null; then
    echo "[run-server-headless] ✅ ready (耗时 ${elapsed}s)，端口 $PORT 接受连接"
    exit 0
  fi
  sleep 1
  elapsed=$((elapsed + 1))
done

echo "❌ Server 在 ${TIMEOUT_SECONDS}s 内未就绪，杀进程并退出" >&2
echo "最后 30 行 log:" >&2
tail -n 30 "$LOG_FILE" >&2
kill -TERM "$SERVER_PID" 2>/dev/null || true
rm -f "$PID_FILE"
exit 1
