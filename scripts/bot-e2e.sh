#!/usr/bin/env bash
# Bot e2e 编排：起 server（headless offline）→ 跑 scripts/bot/ 协议级黑盒场景 → 收尾。
#
# CI（.github/workflows/e2e.yml「Bot e2e stage」）在 release 二进制已构建、redis 已起的
# job 里调用本脚本，cargo run --release 直接复用缓存。
#
# 本地用法：
#   bash scripts/bot-e2e.sh                          # 自动起 server（release）
#   BOT_E2E_PROFILE=debug bash scripts/bot-e2e.sh    # 用 debug 构建（快）
#   BOT_E2E_REUSE=1 bash scripts/bot-e2e.sh          # 复用已在 25565 跑着的 server
#
# 注意：必须经 cargo run 从**当前 checkout** 构建运行，不要直接跑共享 target 里的旧
# 二进制——CARGO_MANIFEST_DIR 是编译期烙死的，旧二进制可能指向已删 worktree 的资产路径
# 启动即 panic（loot_pools.json not found 实证）。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST="${BOT_E2E_HOST:-127.0.0.1}"
PORT="${BOT_E2E_PORT:-25565}"
PROFILE="${BOT_E2E_PROFILE:-release}"
REUSE="${BOT_E2E_REUSE:-0}"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence/bot-e2e"
SERVER_LOG="$EVIDENCE_DIR/server.log"

mkdir -p "$EVIDENCE_DIR"

SERVER_PID=""
STARTED_REDIS=0

port_open() {
  python3 - "$1" "$2" <<'EOF'
import socket, sys
try:
    socket.create_connection((sys.argv[1], int(sys.argv[2])), timeout=2).close()
except OSError:
    sys.exit(1)
EOF
}

# 递归杀整棵进程树（与 e2e-redis.sh 同模式）：先子孙后父防 reparent 孤儿，
# SIGTERM 后短等 + SIGKILL 兜底，保证 25565 真正释放。
kill_tree() {
  local pid="$1"
  local child
  for child in $(pgrep -P "$pid" 2>/dev/null); do
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
  if [ "$STARTED_REDIS" = "1" ]; then
    docker compose -f "$ROOT/docker-compose.test.yml" down -v --remove-orphans >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# ---- 编解码单测（无需 server；坏了没必要浪费一次 server 启动）----
python3 "$ROOT/scripts/bot/test_protocol.py"

# ---- redis（server 启动依赖）----
if ! port_open 127.0.0.1 6379; then
  echo "[bot-e2e] redis 未起，尝试 docker compose 拉起"
  docker compose -f "$ROOT/docker-compose.test.yml" up -d redis --wait
  STARTED_REDIS=1
fi

# ---- server ----
if port_open "$HOST" "$PORT" && [ "$REUSE" != "1" ] && [ "${BOT_E2E_KILL_STALE:-0}" = "1" ]; then
  # CI 专用兜底：上游 stage 若留下孤儿 server（历史上 e2e-redis.sh 只杀子 shell），
  # 这里按名清掉再自起，保证测的是本 job 构建的二进制。本地默认关闭，绝不误杀 dev server。
  echo "[bot-e2e] BOT_E2E_KILL_STALE=1：清理占用 $PORT 的残留 server 进程"
  pkill -TERM -f 'bong-server' 2>/dev/null || true
  pkill -TERM -f 'cargo run --release' 2>/dev/null || true
  for _ in $(seq 1 15); do
    port_open "$HOST" "$PORT" || break
    sleep 1
  done
fi

if port_open "$HOST" "$PORT"; then
  if [ "$REUSE" = "1" ]; then
    echo "[bot-e2e] 复用已在 $HOST:$PORT 运行的 server（BOT_E2E_REUSE=1）"
  else
    echo "[bot-e2e] $HOST:$PORT 已被占用。要对着现有 server 跑请设 BOT_E2E_REUSE=1；" >&2
    echo "[bot-e2e] 否则先停掉旧 server，避免测到过期二进制。" >&2
    exit 2
  fi
else
  PROFILE_FLAG=""
  if [ "$PROFILE" = "release" ]; then
    PROFILE_FLAG="--release"
  fi
  echo "[bot-e2e] 启动 server（cargo run $PROFILE_FLAG，log: $SERVER_LOG）"
  # 子 shell 继承外层变量无需字符串插值；exec 让 SERVER_PID 直接等于 cargo run
  (
    cd "$ROOT/server"
    export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
    export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
    exec cargo run $PROFILE_FLAG
  ) >"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!

  # 就绪 = 我们自己的 server 日志出现 world bootstrap 锚点 **且** 端口可连。
  # 只看端口会被环境里别人的瞬时 listener（并发 orchestrator 跑集成测试绑 25565）
  # 骗过；只看日志则可能端口还没 bind。
  BOOT_ANCHOR="creating overworld test area"
  echo "[bot-e2e] 等待 $HOST:$PORT 就绪（最长 600s，冷编译会慢）"
  for _ in $(seq 1 300); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      echo "[bot-e2e] server 进程提前退出，log 尾部：" >&2
      tail -n 40 "$SERVER_LOG" >&2
      exit 1
    fi
    if grep -q "$BOOT_ANCHOR" "$SERVER_LOG" && port_open "$HOST" "$PORT"; then
      break
    fi
    sleep 2
  done
  if ! grep -q "$BOOT_ANCHOR" "$SERVER_LOG" || ! port_open "$HOST" "$PORT"; then
    echo "[bot-e2e] 600s 内未同时满足「日志锚点 $BOOT_ANCHOR + 端口 $PORT 可连」，log 尾部：" >&2
    if grep -q "Blocking waiting for file lock" "$SERVER_LOG"; then
      echo "[bot-e2e] 提示：cargo 卡在 build directory 锁——共享 CARGO_TARGET_DIR 正被其他 cargo 进程占用" >&2
    fi
    tail -n 40 "$SERVER_LOG" >&2
    exit 1
  fi
fi

# ---- 场景 ----
EXIT_CODE=0
BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" \
  python3 "$ROOT/scripts/bot/run_scenarios.py" --all 2>&1 | tee "$EVIDENCE_DIR/scenarios.log" || EXIT_CODE=$?

if [ "$EXIT_CODE" != "0" ] && [ -f "$SERVER_LOG" ]; then
  echo "[bot-e2e] 场景失败，server log 尾部（完整见 $SERVER_LOG）："
  tail -n 60 "$SERVER_LOG"
fi

exit "$EXIT_CODE"
