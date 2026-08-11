#!/usr/bin/env bash
# Bot e2e 编排：起 server（headless offline）→ 跑 scripts/bot/ 协议级黑盒场景 → 收尾。
#
# CI（.github/workflows/e2e.yml「Bot e2e stage」）在 release 二进制已构建、redis 已起的
# job 里调用本脚本，经构建令牌 wrapper cargo run --release 复用缓存。
#
# 本地用法：
#   bash scripts/bot-e2e.sh                          # 自动起 server（release）
#   BOT_E2E_PROFILE=debug bash scripts/bot-e2e.sh    # 用 debug 构建（快）
#   BOT_E2E_REUSE=1 bash scripts/bot-e2e.sh          # 复用已在 25565 跑着的 server
#
# 注意：必须经构建令牌 wrapper 从**当前 checkout**构建运行，不要直接跑共享 target 里的旧
# 二进制——CARGO_MANIFEST_DIR 是编译期烙死的，旧二进制可能指向已删 worktree 的资产路径
# 启动即 panic（loot_pools.json not found 实证）。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST="${BOT_E2E_HOST:-127.0.0.1}"
PORT="${BOT_E2E_PORT:-25565}"
PROFILE="${BOT_E2E_PROFILE:-release}"
REUSE="${BOT_E2E_REUSE:-0}"
AMBIENT_FIXTURE_MODE="${BOT_E2E_AMBIENT_FIXTURE_MODE:-0}"
FALLBACK_MODE="${BOT_E2E_FALLBACK_MODE:-0}"
BOT_E2E_RUN_TAG="${BOT_E2E_RUN_TAG:-$(( $$ % 100000 ))}"
export BOT_E2E_RUN_TAG
BOT_E2E_OPERATOR_TAGS=(
  RGA RGB Clr Fog Give Atk RespawnSfx Cast SwordAV Sword Break Pill Cult Box Herbal Eqp ScDim
  MCA MCB CE1 CE2 Req Scope Tol AmbSur Brew ProdAF Refund Resume Forge Craft ProdLG WoodDrop J1 Poi
  Zlb Zre Alc Bob
  Rein Term NewCh
)
BOT_E2E_OPERATORS=""
for bot_tag in "${BOT_E2E_OPERATOR_TAGS[@]}"; do
  [ -z "$BOT_E2E_OPERATORS" ] || BOT_E2E_OPERATORS+=,
  BOT_E2E_OPERATORS+="B${BOT_E2E_RUN_TAG}${bot_tag}"
done
# 互斥守卫必须先于任何 REUSE 归一化执行，否则归一化把 REUSE 改成 0 后，
# 守卫校验的是已变异值而不是调用方原始请求，排除逻辑被绕过（finding 2）。
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ "$REUSE" = "1" ]; then
  echo "[bot-e2e] BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_REUSE=1 互斥；fixture ownership 仅限本轮自起 server" >&2
  exit 2
fi
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ "$FALLBACK_MODE" = "1" ]; then
  echo "[bot-e2e] BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_FALLBACK_MODE=1 互斥" >&2
  exit 2
fi
if [ "$FALLBACK_MODE" = "1" ] && [ "$REUSE" = "1" ]; then
  echo "[bot-e2e] BOT_E2E_FALLBACK_MODE=1 与 BOT_E2E_REUSE=1 互斥；fallback ownership 仅限本轮自起 server" >&2
  exit 2
fi

# Reuse is safe only when the caller proves that the running offline server has this exact
# run-tag roster and the explicit username-trust opt-in. Otherwise force the fresh-launch path.
if [ "$REUSE" = "1" ] && [ "$AMBIENT_FIXTURE_MODE" != "1" ] && {
  [ "${BONG_OPERATORS:-}" != "$BOT_E2E_OPERATORS" ] ||
  [ "${BONG_OPERATORS_ALLOW_OFFLINE:-}" != "1" ]
}; then
  echo "[bot-e2e] existing server operator roster does not match this run; disabling BOT_E2E_REUSE=1" >&2
  REUSE=0
fi

# Dedicated world ownership modes have intentionally closed values: unset/default, generic 0,
# and strict owned 1. Reject typos before creating files or starting tools, because a misspelled
# mode must never silently run weaker evidence.
for mode_name in BOT_E2E_AMBIENT_FIXTURE_MODE BOT_E2E_FALLBACK_MODE; do
  mode_value="${!mode_name:-0}"
  case "$mode_value" in
    ""|0|1) ;;
    *)
      echo "[bot-e2e] $mode_name 仅接受空值、0 或 1，实际为 $mode_value" >&2
      exit 2
      ;;
  esac
done

OWNED_WORLD_MODE=0
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ "$FALLBACK_MODE" = "1" ]; then
  OWNED_WORLD_MODE=1
fi

EVIDENCE_ROOT="$ROOT/.sisyphus/evidence/bot-e2e"
EVIDENCE_DIR=""
RUN_ID=""
SERVER_LOG=""
SERVER_RUNTIME_DIR=""
BOT_NOVICE_RASTER_DIR=""
BOT_RASTER_READY_PAYLOAD=""
# 这两个变量必须在安装 cleanup trap 之前初始化为空：SERVER_BINARY/CARGO_TARGET_ROOT
# 若由调用方环境带入，cleanup 会把它当成本轮 harness 拥有的路径处理（review finding：
# 中途失败时 rm 掉调用方任意可写文件 / 无界累积 target 树）。先赋空值切断继承。
SERVER_BINARY=""
CARGO_TARGET_ROOT=""
BOT_FALLBACK_READY_PATTERN='^([[:cntrl:]]\[[0-9;]*m)*[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[^[:space:][:cntrl:]]+([[:cntrl:]]\[[0-9;]*m)*[[:space:]]+([[:cntrl:]]\[[0-9;]*m)*[[:space:]]*INFO([[:cntrl:]]\[[0-9;]*m)*[[:space:]]+\[bong\]\[world\] BOT_FALLBACK_FLAT_READY anchors=[1-9][0-9]* chunks=[1-9][0-9]* view_distance_chunks=[1-9][0-9]*$'

# ownership 只能由本轮 self-start server 的 exact ready marker 授予；拒绝继承调用方
# 或上一轮 shell 留下的声明。REUSE 也没有修改外部 server 启动环境的权限。
unset BOT_E2E_AMBIENT_FIXTURE_OWNED
unset BOT_E2E_FALLBACK_OWNED

# 自起 server 固定由当前 checkout 监听本机 IPv4；若要连接远端或 IPv6 server，
# 必须显式 REUSE，避免 ownership 校验命中 IPv4 子进程、Bot 却连到另一地址族旧服。
if [ "$REUSE" != "1" ] && [ "$HOST" != "127.0.0.1" ]; then
  echo "[bot-e2e] 自起模式仅支持 BOT_E2E_HOST=127.0.0.1；远端/IPv6 请同时设置 BOT_E2E_REUSE=1" >&2
  exit 2
fi

# Dedicated world modes reserve the startup inputs. Generic self-start retains the
# caller's raster/state contract and simply skips the ownership-only scenarios.
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then
  echo "[bot-e2e] ambient fixture mode 不接受外部 BONG_TERRAIN_RASTER_PATH；严格 fixture 必须由本轮 harness 独占生成" >&2
  exit 2
fi
if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then
  echo "[bot-e2e] fallback mode 不接受 BONG_TERRAIN_RASTER_PATH；本轮必须显式无 raster" >&2
  exit 2
fi
if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_WORLD_PATH:-}" ]; then
  echo "[bot-e2e] fallback mode 不接受 BONG_WORLD_PATH；本轮必须显式无 Anvil world" >&2
  exit 2
fi
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then
  echo "[bot-e2e] ambient fixture mode 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH；测试状态必须由本轮 harness 独占" >&2
  exit 2
fi
if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then
  echo "[bot-e2e] fallback mode 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH；测试状态必须由本轮 harness 独占" >&2
  exit 2
fi

mkdir -p "$EVIDENCE_ROOT"
EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"
RUN_ID="${EVIDENCE_DIR##*.}"
SERVER_LOG="$EVIDENCE_DIR/server.log"
if [ "$OWNED_WORLD_MODE" = "1" ]; then
  SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"
  mkdir -p "$SERVER_RUNTIME_DIR/server/data" "$SERVER_RUNTIME_DIR/library-web/public/deceased"
  # botany / forge 的生产 loader 仍从 cwd-relative assets/... 读取；只桥接 checkout
  # 的资产输入，持久化输出继续全部落在本轮私有 runtime。
  ln -s "$ROOT/server/assets" "$SERVER_RUNTIME_DIR/server/assets"
fi

# Owned-fixture mode generates and pins one tokenized raster. Fallback mode must stay
# raster-less by construction. Generic self-start preserves a caller-supplied raster (or the
# historical generated novice fixture) but never claims ownership.
if [ "$REUSE" != "1" ] && [ "$FALLBACK_MODE" != "1" ] && [ -z "${BONG_TERRAIN_RASTER_PATH:-}" ]; then
  BOT_NOVICE_RASTER_DIR="$(mktemp -d "$EVIDENCE_DIR/novice-raster.XXXXXX")"
  # The generator requires a token for every fixture. Generic mode uses a fresh token only to
  # create valid raster metadata; it never exports the ambient witness ownership capability.
  BOT_FIXTURE_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
  if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
    BOT_E2E_AMBIENT_FIXTURE_TOKEN="$BOT_FIXTURE_TOKEN"
    BONG_TERRAIN_RASTER_PATH="$(
      python3 "$ROOT/scripts/bot/make_novice_raster_fixture.py" \
        "$BOT_NOVICE_RASTER_DIR" \
        --fixture-token "$BOT_E2E_AMBIENT_FIXTURE_TOKEN"
    )"
  else
    BONG_TERRAIN_RASTER_PATH="$(
      python3 "$ROOT/scripts/bot/make_novice_raster_fixture.py" \
        "$BOT_NOVICE_RASTER_DIR" \
        --fixture-token "$BOT_FIXTURE_TOKEN"
    )"
  fi
  export BONG_TERRAIN_RASTER_PATH
  echo "[bot-e2e] novice raster fixture: $BONG_TERRAIN_RASTER_PATH"
fi

if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
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

# Dedicated world evidence pins state to its private runtime. Generic self-start keeps a caller
# path untouched and creates the historical per-run temporary state only when none was supplied.
if [ "$OWNED_WORLD_MODE" = "1" ]; then
  SPIRITWOOD_STATE_DIR="$SERVER_RUNTIME_DIR/server/data/spiritwood"
  mkdir -p "$SPIRITWOOD_STATE_DIR"
  export BONG_SPIRITWOOD_HARVESTED_PATH="$SPIRITWOOD_STATE_DIR/harvested.json"
elif [ "$REUSE" != "1" ] && [ -z "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then
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
    && port_open "$HOST" "$PORT" \
    && port_owned_by_tree "$SERVER_PID" "$PORT" \
    && { [ "$AMBIENT_FIXTURE_MODE" != "1" ] || grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"; } \
    && { [ "$FALLBACK_MODE" != "1" ] || grep -Eq -- "$BOT_FALLBACK_READY_PATTERN" "$SERVER_LOG"; }
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
  # 每次自起都会向 evidence 目录 install 一份完整 server 可执行文件；server 已终止后
  # 在这里移除，避免每个本地 run 都永久保留一份整二进制（evidence 目录只留 log/场景证据）。
  # 删除前必须做证据目录包含性校验：SERVER_BINARY 只允许是本轮
  # "$EVIDENCE_DIR/bong-server-*" install 路径（review finding：继承自调用方环境的
  # SERVER_BINARY 若直接 rm，中途失败会删掉调用方任意可写文件）。
  if [ -n "$SERVER_BINARY" ] \
    && [[ "$SERVER_BINARY" == "$EVIDENCE_DIR/bong-server-"* ]] \
    && [ -f "$SERVER_BINARY" ]; then
    rm -f "$SERVER_BINARY"
  fi
  # run-private Cargo target（$EVIDENCE_DIR/bong-target）每轮自起都会留下整棵依赖/
  # 增量产物树；evidence 目录有意保留 log/场景证据，但这棵树只服务于本轮构建，server
  # 已终止后必须整棵移除（review finding：不删则每次本地 run 无界累积 target 树）。
  if [ -n "$CARGO_TARGET_ROOT" ] && [ -d "$CARGO_TARGET_ROOT" ]; then
    rm -rf "$CARGO_TARGET_ROOT"
  fi
  if [ "$OWNED_WORLD_MODE" != "1" ] && [ -n "$SPIRITWOOD_STATE_DIR" ]; then
    rm -f "$SPIRITWOOD_STATE_DIR/harvested.json" "$SPIRITWOOD_STATE_DIR/harvested.tmp"
    rmdir "$SPIRITWOOD_STATE_DIR" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# ---- 编解码单测（无需 server；坏了没必要浪费一次 server 启动）----
python3 "$ROOT/scripts/bot/test_protocol.py"

# ---- redis ----
# Owned fixture evidence always receives an isolated Redis, even if CI/global environment exports
# REDIS_URL. Generic self-start preserves an explicit caller URL. With no explicit URL it first
# adopts the historical caller Redis at 127.0.0.1:6379; only an absent default listener gets a
# private Compose Redis owned and cleaned up by this run.
if [ "$REUSE" != "1" ] && [ "$OWNED_WORLD_MODE" != "1" ] && [ -z "${REDIS_URL:-}" ] && port_open 127.0.0.1 6379; then
  echo "[bot-e2e] 沿用调用方默认 Redis 127.0.0.1:6379"
elif [ "$REUSE" != "1" ] && { [ "$OWNED_WORLD_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then
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
  PROFILE_FLAG=()
  TARGET_PROFILE=debug
  if [ "$PROFILE" = "release" ]; then
    PROFILE_FLAG=(--release)
    TARGET_PROFILE=release
  fi
  echo "[bot-e2e] 构建并启动本轮 immutable server（profile=$PROFILE，log: $SERVER_LOG）"
  export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
  export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
  # review finding：build-token 只锁 cargo 子进程；从共享 target 读产物时，build
  # 返回后、install 前的 bong-server 会被并发 job 替换（stale-read TOCTOU）。build
  # 到 run-private target（EVIDENCE_DIR 每轮唯一），源产物不可能被并发替换。
  CARGO_TARGET_ROOT="$EVIDENCE_DIR/bong-target"
  export CARGO_TARGET_DIR="$CARGO_TARGET_ROOT"
  if ! (
    cd "$ROOT/server"
    "$ROOT/scripts/build-token.sh" cargo build --locked "${PROFILE_FLAG[@]}"
  ) >>"$SERVER_LOG" 2>&1; then
    echo "[bot-e2e] server build failed" >&2
    tail -n 40 "$SERVER_LOG" >&2
    exit 1
  fi
  SERVER_BINARY="$EVIDENCE_DIR/bong-server-$TARGET_PROFILE"
  install -m 700 "$CARGO_TARGET_ROOT/$TARGET_PROFILE/bong-server" "$SERVER_BINARY"
  (
    if [ "$OWNED_WORLD_MODE" = "1" ]; then
      cd "$SERVER_RUNTIME_DIR/server"
      export BONG_DORMANT_ROGUE_SEED_COUNT="${BONG_DORMANT_ROGUE_SEED_COUNT:-0}"
      export BONG_ASSETS_DIR="$ROOT/server"
    else
      cd "$ROOT/server"
    fi
    export BONG_OPERATORS="$BOT_E2E_OPERATORS"
    export BONG_OPERATORS_ALLOW_OFFLINE=1
    if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
      # The owned server is the harness capability boundary; REUSE never enters this branch.
      export BONG_DEV_MODE=1
    fi
    exec "$SERVER_BINARY"
  ) >>"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!

  # Owned-fixture mode needs an exact fixture marker. Generic mode keeps the prior common
  # world bootstrap anchor while still requiring the listener to belong to this process tree.
  BOOT_ANCHOR="spawned tsy dimension layer (empty, awaits worldgen)"
  echo "[bot-e2e] 等待 $HOST:$PORT 就绪（最长 600s，冷编译会慢）"
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
    # Dedicated world modes need an exact startup marker. Generic mode keeps the prior common
    # world bootstrap anchor while still requiring the listener to belong to this process tree.
    ready_marker_ok=0
    if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
      grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" && ready_marker_ok=1
    elif [ "$FALLBACK_MODE" = "1" ]; then
      grep -Eq -- "$BOT_FALLBACK_READY_PATTERN" "$SERVER_LOG" && ready_marker_ok=1
    else
      grep -Fq "$BOOT_ANCHOR" "$SERVER_LOG" && ready_marker_ok=1
    fi
    if [ "$ready_marker_ok" = "1" ] \
      && port_open "$HOST" "$PORT" \
      && port_owned_by_tree "$SERVER_PID" "$PORT"; then
      if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
        export BOT_E2E_AMBIENT_FIXTURE_OWNED=1
      elif [ "$FALLBACK_MODE" = "1" ]; then
        export BOT_E2E_FALLBACK_OWNED=1
      fi
      break
    fi
    sleep 2
  done
  ready_marker_ok=0
  if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
    grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" && ready_marker_ok=1
  elif [ "$FALLBACK_MODE" = "1" ]; then
    grep -Eq -- "$BOT_FALLBACK_READY_PATTERN" "$SERVER_LOG" && ready_marker_ok=1
  else
    grep -Fq "$BOOT_ANCHOR" "$SERVER_LOG" && ready_marker_ok=1
  fi
  if { [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ "${BOT_E2E_AMBIENT_FIXTURE_OWNED:-0}" != "1" ]; } \
    || { [ "$FALLBACK_MODE" = "1" ] && [ "${BOT_E2E_FALLBACK_OWNED:-0}" != "1" ]; } \
    || [ "$ready_marker_ok" != "1" ] \
    || ! port_open "$HOST" "$PORT" \
    || ! port_owned_by_tree "$SERVER_PID" "$PORT"; then
    echo "[bot-e2e] 600s 内未同时满足「本轮就绪锚点 + 当前 server 进程树持有端口 $PORT」，log 尾部：" >&2
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

if { [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ "$FALLBACK_MODE" = "1" ]; } \
  && ! self_started_fixture_runtime_is_current; then
  echo "[bot-e2e] 场景启动前本轮专用 world ownership 已失效，拒绝运行 Bot" >&2
  exit 1
fi

# Watch ownership in a dedicated sibling process rather than sampling only from the foreground.
# It emits exactly one terminal line and exits: `lost` closes the replacement-listener race;
# `complete` means the runner finished while ownership remained bound to this server tree.
WATCH_PID=""
if [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ "$FALLBACK_MODE" = "1" ]; then
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

SCENARIO_ARGS=(--all)
if [ "$FALLBACK_MODE" = "1" ]; then
  # Fallback-flat is a dedicated world-layout witness, not a second run of every gameplay suite.
  # Keeping the runner explicit also ensures dev-command scenarios cannot accidentally enter this
  # production-mode server and turn unrelated persistence/setup into fallback evidence.
  SCENARIO_ARGS=(--scenario terrain_join_chunk_delivery)
fi

set +e
BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" \
  python3 "$ROOT/scripts/bot/run_scenarios.py" "${SCENARIO_ARGS[@]}" 2>&1 | tee "$SCENARIOS_LOG"
pipeline_status=("${PIPESTATUS[@]}")
if [ "${pipeline_status[0]}" -ne 0 ]; then
  EXIT_CODE=${pipeline_status[0]}
else
  EXIT_CODE=${pipeline_status[1]}
fi
set -e

if [ -n "$WATCH_PID" ]; then
  touch "$RUNTIME_WATCH_LOG.stop"
  wait "$WATCH_PID" || true
  watch_result="$(cat "$RUNTIME_WATCH_LOG" 2>/dev/null || true)"
  if [ -n "${BOT_E2E_WATCH_STATUS_EVIDENCE_PATH:-}" ]; then
    printf '%s\n' "$watch_result" >"$BOT_E2E_WATCH_STATUS_EVIDENCE_PATH"
  fi
  rm -f "$RUNTIME_WATCH_LOG.stop" "$RUNTIME_WATCH_LOG"
  rmdir "$RUNTIME_WATCH_DIR" 2>/dev/null || true
  RUNTIME_WATCH_LOG=""
  RUNTIME_WATCH_DIR=""
  if [ "$watch_result" != "complete" ]; then
    echo "[bot-e2e] Bot 运行期间本轮 server 退出或失去端口 ownership；拒绝替代 server 伪证" >&2
    EXIT_CODE=1
  fi
fi

if { [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ "$FALLBACK_MODE" = "1" ]; } \
  && ! self_started_fixture_runtime_is_current; then
  echo "[bot-e2e] 场景结束时本轮专用 world ownership 不再成立，本次证据无效" >&2
  EXIT_CODE=1
fi

if [ "$EXIT_CODE" != "0" ] && [ -f "$SERVER_LOG" ]; then
  echo "[bot-e2e] 场景失败，server log 尾部（完整见 $SERVER_LOG）："
  tail -n 60 "$SERVER_LOG"
fi

exit "$EXIT_CODE"
