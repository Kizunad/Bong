#!/usr/bin/env bash
# 真实生产链：Python protocol Bot → MC 1.20.1 C2S chat → Rust server
# → 独立 Redis bong:player_chat → Tiandao ChatSignal 五分钟窗口。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence/chat-signal-window-$RUN_ID"
REDIS_LOG="$EVIDENCE_DIR/redis.log"
SERVER_LOG="$EVIDENCE_DIR/server.log"
BOT_LOG="$EVIDENCE_DIR/bot.log"
TIANDAO_LOG="$EVIDENCE_DIR/tiandao.log"
PROFILE="${CHAT_WINDOW_E2E_PROFILE:-debug}"
HOST="127.0.0.1"
MC_PORT=25565
MARKER="chat-window-$RUN_ID"

mkdir -p "$EVIDENCE_DIR"

REDIS_PID=""
SERVER_PID=""
REDIS_CONTAINER=""

port_open() {
  python3 - "$1" "$2" <<'PY'
import socket
import sys

try:
    socket.create_connection((sys.argv[1], int(sys.argv[2])), timeout=1).close()
except OSError:
    raise SystemExit(1)
PY
}

pid_belongs_to_tree() {
  local candidate="$1"
  local root_pid="$2"
  while [[ "$candidate" =~ ^[0-9]+$ ]] && [ "$candidate" -gt 1 ]; do
    [ "$candidate" = "$root_pid" ] && return 0
    candidate="$(
      awk '/^PPid:/ { print $2; exit }' "/proc/$candidate/status" 2>/dev/null || true
    )"
  done
  return 1
}

port_owned_by_tree() {
  local root_pid="$1"
  local port="$2"
  local listener_pid
  while IFS= read -r listener_pid; do
    if [ -n "$listener_pid" ] && pid_belongs_to_tree "$listener_pid" "$root_pid"; then
      return 0
    fi
  done < <(
    ss -4 -H -ltnp "sport = :$port" 2>/dev/null \
      | grep -oE 'pid=[0-9]+' \
      | cut -d= -f2 \
      | sort -u \
      || true
  )
  return 1
}

kill_tree() {
  local pid="$1"
  local child
  for child in $(pgrep -P "$pid" 2>/dev/null || true); do
    kill_tree "$child"
  done
  kill "$pid" 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$pid" 2>/dev/null || return 0
    sleep 0.2
  done
  kill -9 "$pid" 2>/dev/null || true
}

cleanup() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill_tree "$SERVER_PID"
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
    kill_tree "$REDIS_PID"
    wait "$REDIS_PID" 2>/dev/null || true
  fi
  if [ -n "$REDIS_CONTAINER" ]; then
    docker rm -f "$REDIS_CONTAINER" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if port_open "$HOST" "$MC_PORT"; then
  echo "[chat-window-e2e] $HOST:$MC_PORT 已被占用；拒绝误连旧 server" >&2
  exit 2
fi

REDIS_PORT="$(python3 - <<'PY'
import socket

with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
REDIS_URL="redis://$HOST:$REDIS_PORT"

if command -v redis-server >/dev/null 2>&1; then
  redis-server \
    --bind "$HOST" \
    --port "$REDIS_PORT" \
    --save "" \
    --appendonly no \
    --loglevel warning >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"
elif command -v docker >/dev/null 2>&1; then
  REDIS_CONTAINER="bong-chat-window-$RUN_ID"
  docker run --rm --name "$REDIS_CONTAINER" \
    -p "$HOST:$REDIS_PORT:6379" redis:7-alpine >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"
else
  echo "[chat-window-e2e] 需要 redis-server 或 docker 提供独立 Redis" >&2
  exit 2
fi

for _ in $(seq 1 30); do
  if port_open "$HOST" "$REDIS_PORT"; then
    break
  fi
  sleep 1
done
if ! port_open "$HOST" "$REDIS_PORT"; then
  echo "[chat-window-e2e] 独立 Redis 未就绪，log: $REDIS_LOG" >&2
  exit 1
fi

PROFILE_FLAG=()
if [ "$PROFILE" = "release" ]; then
  PROFILE_FLAG=(--release)
elif [ "$PROFILE" != "debug" ]; then
  echo "[chat-window-e2e] CHAT_WINDOW_E2E_PROFILE 只支持 debug/release，实际 $PROFILE" >&2
  exit 2
fi

(
  cd "$ROOT/server"
  export REDIS_URL
  export BONG_SKIP_SKIN_PREFETCH=1
  export BONG_ROGUE_SEED_COUNT=0
  unset BONG_TERRAIN_RASTER_PATH
  exec cargo run "${PROFILE_FLAG[@]}"
) >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

BOOT_ANCHOR="spawned tsy dimension layer (empty, awaits worldgen)"
for _ in $(seq 1 300); do
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "[chat-window-e2e] server 提前退出，log 尾部：" >&2
    tail -n 80 "$SERVER_LOG" >&2
    exit 1
  fi
  if grep -Fq "failed to start TCP listener" "$SERVER_LOG"; then
    echo "[chat-window-e2e] 当前 server 未持有 TCP listener，拒绝误连外部进程：" >&2
    tail -n 80 "$SERVER_LOG" >&2
    exit 1
  fi
  if grep -Fq "$BOOT_ANCHOR" "$SERVER_LOG" \
    && port_open "$HOST" "$MC_PORT" \
    && port_owned_by_tree "$SERVER_PID" "$MC_PORT"; then
    break
  fi
  sleep 2
done
if ! grep -Fq "$BOOT_ANCHOR" "$SERVER_LOG" \
  || ! port_open "$HOST" "$MC_PORT" \
  || ! port_owned_by_tree "$SERVER_PID" "$MC_PORT"; then
  echo "[chat-window-e2e] server 未同时满足启动锚点与当前进程树端口归属，log 尾部：" >&2
  tail -n 80 "$SERVER_LOG" >&2
  exit 1
fi

PYTHONPATH="$ROOT/scripts" python3 - "$HOST" "$MC_PORT" "$MARKER" >"$BOT_LOG" 2>&1 <<'PY'
import sys

from bot.bot import Bot

host, port, marker = sys.argv[1], int(sys.argv[2]), sys.argv[3]
with Bot("ChatWindowBot", host=host, port=port) as bot:
    bot.expect_event("game_join", timeout=15.0)
    bot.expect_event("pos_look", timeout=15.0)
    bot.chat(marker)
    echo = bot.expect_chat(marker, timeout=10.0)
    print(f"[chat-window-e2e] bot echo={echo.data['text']!r}")
    bot.assert_alive("聊天写入 Redis 后")
PY

(
  cd "$ROOT"
  export REDIS_URL CHAT_WINDOW_E2E_MARKER="$MARKER"
  "$ROOT/agent/node_modules/.bin/tsx" "$ROOT/scripts/e2e/chat-signal-window.ts"
) >"$TIANDAO_LOG" 2>&1

grep -Fq "[chat-window-e2e] PASS" "$TIANDAO_LOG"
echo "[chat-window-e2e] PASS"
echo "  marker: $MARKER"
echo "  evidence: $EVIDENCE_DIR"
echo "  bot: $BOT_LOG"
echo "  tiandao: $TIANDAO_LOG"
