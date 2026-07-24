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
EVIDENCE_ROOT="$ROOT/.sisyphus/evidence/bot-e2e"
EVIDENCE_DIR=""
RUN_ID=""
SERVER_LOG=""
SERVER_RUNTIME_DIR=""
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

# 先做无副作用输入门禁，再创建 evidence/fixture/state。被拒绝的 self-start 调用不会留下
# 空 run 目录，也不会触碰调用方持久化路径。
if [ "$REUSE" != "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then
  echo "[bot-e2e] 自起 server 不接受外部 BONG_TERRAIN_RASTER_PATH；严格 fixture 场景必须由本轮 harness 独占生成" >&2
  exit 2
fi
if [ "$REUSE" != "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then
  echo "[bot-e2e] 自起 server 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH；测试状态必须由本轮 harness 独占" >&2
  exit 2
fi

mkdir -p "$EVIDENCE_ROOT"
EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"
RUN_ID="${EVIDENCE_DIR##*.}"
SERVER_LOG="$EVIDENCE_DIR/server.log"
if [ "$REUSE" != "1" ]; then
  SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"
  mkdir -p "$SERVER_RUNTIME_DIR/server/data" "$SERVER_RUNTIME_DIR/library-web/public/deceased"
  # botany / forge 的生产 loader 仍从 cwd-relative assets/... 读取；只桥接 checkout
  # 的资产输入，持久化输出继续全部落在本轮私有 runtime。
  ln -s "$ROOT/server/assets" "$SERVER_RUNTIME_DIR/server/assets"
fi

# 测试诚实性约束：Bot 必须能黑盒证明真实 manifest → Startup loader →
# PoiNoviceRegistry，而不是只看到 register() 无条件创建的空 resource。默认生成
# 一个 stdlib-only 的 256×256 平地 v2 raster fixture（六类 novice POI 各 1）。
# 自起模式强制使用并 pin 本轮 token；只有这样 ambient 协议场景才能证明 server 实际加载的
# manifest 与它核验的 support/feet/head 二进制属于同一次运行。REUSE 模式无法拥有 server
# 启动环境，故不伪造 ownership；调用方若显式只跑其它场景仍可复用外部 raster。
if [ "$REUSE" != "1" ]; then
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
REDIS_COMPOSE_PROJECT=""
SPIRITWOOD_STATE_DIR=""

# 真实灵木场景会按生产契约持久化已采伐位置。整个 self-start server 已运行在本轮
# 私有 cwd；这里仍显式钉到同一 runtime data tree，让独占输入可由 contract 测试直接核验。
# REUSE 模式无法改变既有 server 环境，仍由调用方负责其世界状态。
if [ "$REUSE" != "1" ]; then
  SPIRITWOOD_STATE_DIR="$SERVER_RUNTIME_DIR/server/data/spiritwood"
  mkdir -p "$SPIRITWOOD_STATE_DIR"
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
  if [ "$STARTED_REDIS" = "1" ] && [ -n "$REDIS_COMPOSE_PROJECT" ]; then
    BONG_TEST_COMPOSE_PROJECT="$REDIS_COMPOSE_PROJECT" BONG_TEST_REDIS_PORT=0 \
      docker compose -f "$ROOT/docker-compose.test.yml" down -v --remove-orphans >/dev/null 2>&1 || true
  fi
  if [ -n "$BOT_NOVICE_RASTER_DIR" ] && [ -d "$BOT_NOVICE_RASTER_DIR" ]; then
    rm -rf "$BOT_NOVICE_RASTER_DIR"
  fi
}
trap cleanup EXIT

# ---- 编解码单测（无需 server；坏了没必要浪费一次 server 启动）----
python3 "$ROOT/scripts/bot/test_protocol.py"

# ---- redis（self-start server 使用每轮私有 compose project + Docker 随机 host port）----
# REUSE 不能改变外部 server 的 Redis；self-start 则永不借用/关闭共享 6379。这样并发 run
# 即使同时发现本机没有 Redis，也只管理各自创建的 project、volume 和 published port。
if [ "$REUSE" != "1" ]; then
  REDIS_COMPOSE_PROJECT="bong-bot-e2e-${RUN_ID,,}"
  STARTED_REDIS=1
  echo "[bot-e2e] 启动本轮私有 Redis project: $REDIS_COMPOSE_PROJECT"
  BONG_TEST_COMPOSE_PROJECT="$REDIS_COMPOSE_PROJECT" BONG_TEST_REDIS_PORT=0 \
    docker compose -f "$ROOT/docker-compose.test.yml" up -d redis --wait
  redis_binding="$(
    BONG_TEST_COMPOSE_PROJECT="$REDIS_COMPOSE_PROJECT" BONG_TEST_REDIS_PORT=0 \
      docker compose -f "$ROOT/docker-compose.test.yml" port redis 6379
  )"
  redis_port="${redis_binding##*:}"
  if [[ ! "$redis_port" =~ ^[0-9]+$ ]] || ! port_open 127.0.0.1 "$redis_port"; then
    echo "[bot-e2e] 无法确认本轮 Redis published port，实际 binding=$redis_binding" >&2
    exit 1
  fi
  export REDIS_URL="redis://127.0.0.1:$redis_port"
fi

# ---- server ----
# 自起模式绝不终止不属于本轮进程树的 listener。端口已占用即 fail-closed；CI 上游若
# 泄漏 server，应修其 owner 的 cleanup，而不是按进程名误杀其它 worktree/用户开发服。
if port_open "$HOST" "$PORT"; then
  if [ "$REUSE" = "1" ]; then
    echo "[bot-e2e] 复用已在 $HOST:$PORT 运行的 server（BOT_E2E_REUSE=1）"
  else
    echo "[bot-e2e] $HOST:$PORT 已被占用。要对着现有 server 跑请设 BOT_E2E_REUSE=1；" >&2
    echo "[bot-e2e] 否则先停掉旧 server，避免测到过期二进制。" >&2
    exit 2
  fi
else
  if [ "$REUSE" = "1" ]; then
    echo "[bot-e2e] BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起" >&2
    exit 2
  fi
  PROFILE_FLAG=""
  if [ "$PROFILE" = "release" ]; then
    PROFILE_FLAG="--release"
  fi
  echo "[bot-e2e] 启动 server（cargo run $PROFILE_FLAG，log: $SERVER_LOG）"
  # 从本轮私有 runtime/server cwd 启动：所有相对 data/ 写入（sqlite/backups/craft/
  # mineral/NPC）均落进 evidence，不触碰 checkout 的 server/data。manifest-path 保持
  # CARGO_MANIFEST_DIR 指向当前 checkout；BONG_ASSETS_DIR 显式钉住只读 body-plan 资产。
  (
    cd "$SERVER_RUNTIME_DIR/server"
    export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
    export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
    export BONG_DORMANT_ROGUE_SEED_COUNT="${BONG_DORMANT_ROGUE_SEED_COUNT:-0}"
    export BONG_ASSETS_DIR="$ROOT/server"
    # private cwd 不会自动发现 checkout/server/.cargo/config.toml；显式保持同一 dev profile，
    # 否则 debug 构建会切回 full debuginfo、重编整棵依赖并突破 600s readiness 门。
    export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-line-tables-only}"
    # 协议 Bot 覆盖大量 dev-only 命令；生产默认不暴露这些确定性测试接缝。
    export BONG_DEV_MODE="${BONG_DEV_MODE:-1}"
    exec cargo run --locked --manifest-path "$ROOT/server/Cargo.toml" $PROFILE_FLAG
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
