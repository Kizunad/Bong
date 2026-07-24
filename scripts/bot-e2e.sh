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
BOT_NOVICE_RASTER_DIR=""
BOT_RASTER_READY_PAYLOAD=""

# ownership 只能由本轮 self-start server 的 exact ready marker 授予；拒绝继承调用方
# 或上一轮 shell 留下的声明。REUSE 也没有修改外部 server 启动环境的权限。
unset BOT_E2E_AMBIENT_FIXTURE_OWNED

# 自起 server 固定由当前 checkout 监听本机 IPv4；若要连接远端或 IPv6 server，
# 必须显式 REUSE，避免 ownership 校验命中 IPv4 子进程、Bot 却连到另一地址族旧服。
if [ "$REUSE" != "1" ] && [ "$HOST" != "127.0.0.1" ]; then
  echo "[bot-e2e] 自起模式仅支持 BOT_E2E_HOST=127.0.0.1；远端/IPv6 请同时设置 BOT_E2E_REUSE=1" >&2
  exit 2
fi

mkdir -p "$EVIDENCE_DIR"

# 测试诚实性约束：Bot 必须能黑盒证明真实 manifest → Startup loader →
# PoiNoviceRegistry，而不是只看到 register() 无条件创建的空 resource。默认生成
# 一个 stdlib-only 的 256×256 平地 v2 raster fixture（六类 novice POI 各 1）。
# 自起模式强制使用并 pin 本轮 token；只有这样 ambient 协议场景才能证明 server 实际加载的
# manifest 与它核验的 support/feet/head 二进制属于同一次运行。REUSE 模式无法拥有 server
# 启动环境，故不伪造 ownership；调用方若显式只跑其它场景仍可复用外部 raster。
if [ "$REUSE" != "1" ]; then
  if [ -z "${BONG_TERRAIN_RASTER_PATH:-}" ]; then
    BOT_NOVICE_RASTER_DIR="$(mktemp -d "$EVIDENCE_DIR/novice-raster.XXXXXX")"
    BOT_E2E_AMBIENT_FIXTURE_TOKEN="$(
      python3 -c 'import secrets; print(secrets.token_hex(16))'
    )"
    export BONG_TERRAIN_RASTER_PATH
    BONG_TERRAIN_RASTER_PATH="$(
      python3 "$ROOT/scripts/bot/make_novice_raster_fixture.py" \
        "$BOT_NOVICE_RASTER_DIR" \
        --fixture-token "$BOT_E2E_AMBIENT_FIXTURE_TOKEN"
    )"
    echo "[bot-e2e] novice raster fixture: $BONG_TERRAIN_RASTER_PATH"
  else
    echo "[bot-e2e] 自起 server 不接受外部 BONG_TERRAIN_RASTER_PATH；严格 fixture 场景必须由本轮 harness 独占生成" >&2
    exit 2
  fi
  BONG_TERRAIN_RASTER_PATH="$(python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).resolve(strict=True))' "$BONG_TERRAIN_RASTER_PATH")"
  export BONG_TERRAIN_RASTER_PATH
  export BOT_E2E_AMBIENT_FIXTURE_TOKEN
  export BOT_E2E_AMBIENT_FIXTURE_MANIFEST="$BONG_TERRAIN_RASTER_PATH"
  BOT_RASTER_READY_PAYLOAD="[bong][world] BOT_RASTER_FIXTURE_READY manifest=$BONG_TERRAIN_RASTER_PATH token=$BOT_E2E_AMBIENT_FIXTURE_TOKEN"
else
  unset BOT_E2E_AMBIENT_FIXTURE_OWNED
  unset BOT_E2E_AMBIENT_FIXTURE_MANIFEST
  unset BOT_E2E_AMBIENT_FIXTURE_TOKEN
fi

SERVER_PID=""
STARTED_REDIS=0
SPIRITWOOD_STATE_DIR=""

# 真实灵木场景会按生产契约持久化已采伐位置。自起 server 时给每轮 e2e 独立状态，
# 否则第二次运行会正确 hydrate 上一轮的树干并把可重复测试误判成不可采。
# REUSE 模式无法改变既有 server 环境，仍由调用方负责其世界状态。
if [ "$REUSE" != "1" ] && [ -z "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then
  SPIRITWOOD_STATE_DIR="$(mktemp -d "$EVIDENCE_DIR/spiritwood-state.XXXXXX")"
  export BONG_SPIRITWOOD_HARVESTED_PATH="$SPIRITWOOD_STATE_DIR/harvested.json"
fi

port_open() {
  python3 - "$1" "$2" <<'EOF'
import socket, sys
try:
    socket.create_connection((sys.argv[1], int(sys.argv[2])), timeout=2).close()
except OSError:
    sys.exit(1)
EOF
}

# 端口可连还不够：cargo 编译窗口内可能有旧 server 抢先监听同一端口。只有 listener
# PID 属于本次 `cargo run` 进程树，Bot 才能确信连到当前 checkout 的二进制。
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

# Readiness is not a permanent capability: the owned server may exit after the first marker/port
# check and another process may take 25565. Re-evaluate the whole binding before, throughout and
# after the scenario runner so a replacement listener can never turn the local fixture files into
# false evidence about the server Bot actually exercised.
self_started_fixture_runtime_is_current() {
  [ "$REUSE" != "1" ] \
    && [ -n "$SERVER_PID" ] \
    && kill -0 "$SERVER_PID" 2>/dev/null \
    && grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" \
    && port_open "$HOST" "$PORT" \
    && port_owned_by_tree "$SERVER_PID" "$PORT"
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
  if [ -n "${WATCH_PID:-}" ] && kill -0 "$WATCH_PID" 2>/dev/null; then
    kill "$WATCH_PID" 2>/dev/null || true
    wait "$WATCH_PID" 2>/dev/null || true
  fi
  if [ -n "${RUNTIME_WATCH_LOG:-}" ]; then
    rm -f "$RUNTIME_WATCH_LOG.stop" "$RUNTIME_WATCH_LOG" 2>/dev/null || true
  fi
  if [ -n "${RUNTIME_WATCH_DIR:-}" ]; then
    rmdir "$RUNTIME_WATCH_DIR" 2>/dev/null || true
  fi
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill_tree "$SERVER_PID"
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [ "$STARTED_REDIS" = "1" ]; then
    docker compose -f "$ROOT/docker-compose.test.yml" down -v --remove-orphans >/dev/null 2>&1 || true
  fi
  if [ -n "$SPIRITWOOD_STATE_DIR" ]; then
    rm -f "$SPIRITWOOD_STATE_DIR/harvested.json" "$SPIRITWOOD_STATE_DIR/harvested.tmp"
    rmdir "$SPIRITWOOD_STATE_DIR" 2>/dev/null || true
  fi
  if [ -n "$BOT_NOVICE_RASTER_DIR" ] && [ -d "$BOT_NOVICE_RASTER_DIR" ]; then
    rm -rf "$BOT_NOVICE_RASTER_DIR"
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
    # 协议 Bot 覆盖大量 dev-only 命令；生产默认不暴露这些确定性测试接缝。
    export BONG_DEV_MODE="${BONG_DEV_MODE:-1}"
    exec cargo run $PROFILE_FLAG
  ) >"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!

  # 就绪 = 本轮 server 成功加载本轮独占 raster（canonical path + high-entropy token）
  # **且** 本轮 cargo 进程树实际持有可连接端口。通用 world bootstrap 日志不足以证明
  # server 加载的是哪个 manifest；fixture marker 只能在完整 provider load 成功后由 Rust 打印。
  echo "[bot-e2e] 等待 $HOST:$PORT 与本轮 raster fixture 就绪（最长 600s，冷编译会慢）"
  for _ in $(seq 1 300); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      echo "[bot-e2e] server 进程提前退出，log 尾部：" >&2
      tail -n 40 "$SERVER_LOG" >&2
      exit 1
    fi
    if grep -Fq "failed to start TCP listener" "$SERVER_LOG"; then
      echo "[bot-e2e] 当前 server TCP listener 启动失败，拒绝连接同端口的外部进程：" >&2
      tail -n 40 "$SERVER_LOG" >&2
      exit 1
    fi
    if grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" \
      && port_open "$HOST" "$PORT" \
      && port_owned_by_tree "$SERVER_PID" "$PORT"; then
      export BOT_E2E_AMBIENT_FIXTURE_OWNED=1
      break
    fi
    sleep 2
  done
  if [ "${BOT_E2E_AMBIENT_FIXTURE_OWNED:-0}" != "1" ] \
    || ! grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" \
    || ! port_open "$HOST" "$PORT" \
    || ! port_owned_by_tree "$SERVER_PID" "$PORT"; then
    echo "[bot-e2e] 600s 内未同时满足「本轮 fixture marker + 当前 server 进程树持有端口 $PORT」，log 尾部：" >&2
    if grep -q "Blocking waiting for file lock" "$SERVER_LOG"; then
      echo "[bot-e2e] 提示：cargo 卡在 build directory 锁——共享 CARGO_TARGET_DIR 正被其他 cargo 进程占用" >&2
    fi
    tail -n 40 "$SERVER_LOG" >&2
    exit 1
  fi
fi

# ---- 场景 ----
EXIT_CODE=0
SCENARIOS_LOG="$EVIDENCE_DIR/scenarios.log"
RUNTIME_WATCH_DIR=""
RUNTIME_WATCH_LOG=""

if [ "$REUSE" != "1" ] && ! self_started_fixture_runtime_is_current; then
  echo "[bot-e2e] 场景启动前本轮 server/fixture ownership 已失效，拒绝运行 Bot" >&2
  exit 1
fi

# Watch ownership in a dedicated sibling process rather than sampling only from the foreground.
# It emits exactly one terminal line and exits: `lost` closes the replacement-listener race;
# `complete` means the runner finished while ownership remained bound to this server tree.
WATCH_PID=""
if [ "$REUSE" != "1" ]; then
  RUNTIME_WATCH_DIR="$(mktemp -d "$EVIDENCE_DIR/runtime-watch.XXXXXX")"
  RUNTIME_WATCH_LOG="$RUNTIME_WATCH_DIR/status"
  (
    while true; do
      if [ -f "$RUNTIME_WATCH_LOG.stop" ]; then
        echo complete >"$RUNTIME_WATCH_LOG"
        exit 0
      fi
      if ! kill -0 "$SERVER_PID" 2>/dev/null \
        || ! port_owned_by_tree "$SERVER_PID" "$PORT"; then
        echo lost >"$RUNTIME_WATCH_LOG"
        exit 1
      fi
      sleep 0.2
    done
  ) &
  WATCH_PID=$!
fi

set +e
BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" \
  python3 "$ROOT/scripts/bot/run_scenarios.py" --all 2>&1 | tee "$SCENARIOS_LOG"
EXIT_CODE=${PIPESTATUS[0]}
set -e

if [ -n "$WATCH_PID" ]; then
  touch "$RUNTIME_WATCH_LOG.stop"
  wait "$WATCH_PID" || true
  watch_result="$(cat "$RUNTIME_WATCH_LOG" 2>/dev/null || true)"
  rm -f "$RUNTIME_WATCH_LOG.stop" "$RUNTIME_WATCH_LOG"
  rmdir "$RUNTIME_WATCH_DIR" 2>/dev/null || true
  RUNTIME_WATCH_LOG=""
  RUNTIME_WATCH_DIR=""
  if [ "$watch_result" != "complete" ]; then
    echo "[bot-e2e] Bot 运行期间本轮 server 退出或失去端口 ownership；拒绝替代 server 伪证" >&2
    EXIT_CODE=1
  fi
fi

if [ "$REUSE" != "1" ] && ! self_started_fixture_runtime_is_current; then
  echo "[bot-e2e] 场景结束时本轮 server/fixture ownership 不再成立，本次证据无效" >&2
  EXIT_CODE=1
fi

if [ "$EXIT_CODE" != "0" ] && [ -f "$SERVER_LOG" ]; then
  echo "[bot-e2e] 场景失败，server log 尾部（完整见 $SERVER_LOG）："
  tail -n 60 "$SERVER_LOG"
fi

exit "$EXIT_CODE"
