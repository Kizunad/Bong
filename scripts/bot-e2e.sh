#!/usr/bin/env bash
# Bot e2e 编排：起 server（headless offline）→ 跑 scripts/bot/ 协议级黑盒场景 → 收尾。
#
# CI（.github/workflows/e2e.yml「Bot e2e stage」）在 release 二进制已构建、redis 已起的
# job 里调用本脚本。CI 通过 BONG_E2E_PREBUILT_SERVER_MANIFEST 传入一次构建、
# SHA-256 + checkout HEAD 对拍的 release artifact；每个顺序 stage 复制到自己的 run 目录。
# 未提供 manifest 的本地调用仍走本轮 run-private build 路径。
#
# 本地用法：
#   bash scripts/bot-e2e.sh                          # 自动起 server（release），端口自动分配
#   BOT_E2E_PROFILE=debug bash scripts/bot-e2e.sh    # 用 debug 构建（快）
#   BOT_E2E_PORT=34567 bash scripts/bot-e2e.sh       # 显式端口（自起模式传给 server）
#   BOT_E2E_REUSE=1 bash scripts/bot-e2e.sh          # 复用已在 25565 跑着的 server
# 自起模式不指定 BOT_E2E_PORT 时分配空闲端口，本机可并发跑多套 e2e（各占一个端口）。
#
# 注意：必须从当前 checkout 经 build-token 构建，并复制本轮成功产物后运行，不要直接跑
# 共享 target 里的旧二进制——CARGO_MANIFEST_DIR 是编译期烙死的，旧二进制可能指向已删
# worktree 的资产路径，启动即 panic（loot_pools.json not found 实证）。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/smoke-owned-artifacts.sh
source "$ROOT/scripts/lib/smoke-owned-artifacts.sh"
# shellcheck source=lib/bong-cargo-target.sh
source "$ROOT/scripts/lib/bong-cargo-target.sh"

# 自起模式并发安全：BOT_E2E_PORT 未指定时 bind :0 由内核在临时端口段原子分配——
# 两个 worker 同一瞬间各自 probe 会像 connect 探测一样都看到「空闲」，而 bind 分配
# 拿到即独占。用 0.0.0.0 探测，覆盖 server 的 wildcard listener，避免仅绑定 loopback
# 漏掉第三方占用；close 到 server bind 之间若有第三方抢入，则在下方 bounded retry
# 中换一个端口重启，不把一次可恢复的端口碰撞变成整轮失败。
allocate_free_port() {
  python3 - <<'EOF'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("0.0.0.0", 0))
    print(sock.getsockname()[1])
EOF
}

# 环境变量允许首尾空白，但进入 ss、Python runner 与 BONG_SERVER_PORT 前必须只保留
# 一个 canonical 十进制端口。0 会让 server 绑定不可发现的 OS ephemeral port，必须
# 在启动前 fail closed，不能把错误拖成 600s readiness 假超时。
canonicalize_port() {
  python3 - "$1" <<'EOF'
import sys

raw = sys.argv[1]
value = raw.strip()
if not value or any(char < "0" or char > "9" for char in value):
    print(
        f"[bot-e2e] BOT_E2E_PORT 必须是 1-65535 的十进制整数（可带首尾空白），实际为 {raw!r}",
        file=sys.stderr,
    )
    raise SystemExit(2)
try:
    port = int(value)
except ValueError:
    print(
        f"[bot-e2e] BOT_E2E_PORT 必须是 1-65535 的十进制整数（可带首尾空白），实际为 {raw!r}",
        file=sys.stderr,
    )
    raise SystemExit(2)
if not 1 <= port <= 65535:
    print(
        f"[bot-e2e] BOT_E2E_PORT 必须是 1-65535 的十进制整数（可带首尾空白），实际为 {raw!r}",
        file=sys.stderr,
    )
    raise SystemExit(2)
print(port)
EOF
}

PROFILE="${BOT_E2E_PROFILE:-release}"
REUSE="${BOT_E2E_REUSE:-0}"
HOST="${BOT_E2E_HOST:-127.0.0.1}"
AMBIENT_FIXTURE_MODE="${BOT_E2E_AMBIENT_FIXTURE_MODE:-0}"
FALLBACK_MODE="${BOT_E2E_FALLBACK_MODE:-0}"
BOT_E2E_RUN_TAG="${BOT_E2E_RUN_TAG:-$(( $$ % 100000 ))}"
export BOT_E2E_RUN_TAG
BOT_E2E_OPERATOR_TAGS=(
  RGA RGB Clr Fog Give Atk RespawnSfx Cast SwordAV Sword Break Pill Cult Box Herbal Eqp ScDim
  MCA MCB CE1 CE2 Req Scope Tol AmbSur Brew ProdAF Refund Resume Forge Craft ProdLG WoodDrop J1 Poi
  Zlb Zre Alc Bob CoPl Coffin
  Big Typ Rng Stl
  Rein Term NewCh CoEn CoEnB CoLv CoLvB BR1 BR2 RW1 RW2 OO1 OO2 NRift
  GD2H GD2V GD2I GD2J Abr Hdm Ins Ins2 Rej Dux FoCj
  FoSc
  TA TB DA DB SA
  Charge Throw Switch
)
BOT_E2E_OPERATORS=""
for bot_tag in "${BOT_E2E_OPERATOR_TAGS[@]}"; do
  [ -z "$BOT_E2E_OPERATORS" ] || BOT_E2E_OPERATORS+=,
  BOT_E2E_OPERATORS+="B${BOT_E2E_RUN_TAG}${bot_tag}"
done
# Fixture ownership modes have intentionally closed values. Reject typos before creating files or
# starting tools, because a misspelled mode must never silently run weaker evidence.
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
if [ "$REUSE" = "1" ] && [ "$OWNED_WORLD_MODE" != "1" ] && {
  [ "${BONG_OPERATORS:-}" != "$BOT_E2E_OPERATORS" ] ||
  [ "${BONG_OPERATORS_ALLOW_OFFLINE:-}" != "1" ]
}; then
  echo "[bot-e2e] existing server operator roster does not match this run; disabling BOT_E2E_REUSE=1" >&2
  REUSE=0
fi

# 端口决策必须在 REUSE 可能被上方 roster 守卫降级之后做，否则 REUSE=1 被降级为
# 自起时仍会拿 25565 去抢。显式端口 0/非法值在这里拒绝；自动分配的端口也经过同一
# canonicalizer，后续所有 probe、server env 与 runner 只使用 PORT。
PORT_AUTO_ALLOCATED=0
if [ "$REUSE" = "1" ]; then
  # REUSE：显式端口或默认 25565——只连接既有 server，不分配、不起服。
  RAW_PORT="${BOT_E2E_PORT:-25565}"
else
  # 自起模式：未指定端口就分配空闲端口，而不是默认争抢 25565。
  if [ -n "${BOT_E2E_PORT:-}" ]; then
    RAW_PORT="$BOT_E2E_PORT"
  else
    RAW_PORT="$(allocate_free_port)"
    PORT_AUTO_ALLOCATED=1
  fi
fi
if ! PORT="$(canonicalize_port "$RAW_PORT")"; then
  exit 2
fi
if [ "$PORT_AUTO_ALLOCATED" = "1" ]; then
  echo "[bot-e2e] BOT_E2E_PORT 未设置，自起模式分配空闲端口 $PORT（并发运行互不争抢 25565）"
fi

# Ambient fixture ownership has three intentionally closed values: unset/default, generic 0,
# and strict owned-fixture 1. Reject typos before creating files or starting tools.
EVIDENCE_ROOT="$ROOT/.sisyphus/evidence/bot-e2e"
EVIDENCE_DIR=""
RUN_ID=""
SERVER_LOG=""
CALLER_SERVER_LOG="${BONG_SERVER_LOG:-}"
SERVER_RUNTIME_DIR=""
BOT_NOVICE_RASTER_DIR=""
BOT_RASTER_READY_PAYLOAD=""
# 这两个变量必须在安装 cleanup trap 之前初始化为空：SERVER_BINARY/CARGO_TARGET_ROOT
# 若由调用方环境带入，cleanup 会把它当成本轮 harness 拥有的路径处理（review finding：
# 中途失败时 rm 掉调用方任意可写文件 / 无界累积 target 树）。先赋空值切断继承。
SERVER_BINARY=""
CARGO_TARGET_ROOT=""
BONG_E2E_PREBUILT_SERVER_MANIFEST="${BONG_E2E_PREBUILT_SERVER_MANIFEST:-}"
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

# 回退平世界是钉死 BciJ1/BciJ2/BciFC 用户名的 CI 见证，terrain_join_chunk_delivery
# 簇断言只接受 ci；默认 PID 派生 run tag 会等首个 join 完成才撞断言晚失败，
# 必须在构建/启动前显式拒绝（review finding：fallback 默认自调用自不兼容）。
if [ "$FALLBACK_MODE" = "1" ] && [ "$BOT_E2E_RUN_TAG" != "ci" ]; then
  echo "[bot-e2e] fallback mode 的 terrain_join_chunk_delivery 簇断言只接受 BOT_E2E_RUN_TAG=ci；" >&2
  echo "[bot-e2e] 默认 PID 派生 run tag 与 fallback 见证不兼容，请显式设置 BOT_E2E_RUN_TAG=ci" >&2
  exit 2
fi

# 每次 harness invocation 都有独立 evidence namespace。test_protocol.py 内嵌的 fake
# server 可通过 BOT_E2E_EVIDENCE_ROOT 直接钉住自己的 invocation 根目录；普通运行则
# 在共享 artifact 根下创建唯一 session，避免同一 checkout 的并发 bot-e2e 互相删除/
# 读取对方的 run 目录。
if [ -n "${BOT_E2E_EVIDENCE_ROOT:-}" ]; then
  EVIDENCE_ROOT="$BOT_E2E_EVIDENCE_ROOT"
else
  mkdir -p "$EVIDENCE_ROOT"
  EVIDENCE_ROOT="$(mktemp -d "$EVIDENCE_ROOT/session.XXXXXXXXXX")"
  export BOT_E2E_EVIDENCE_ROOT="$EVIDENCE_ROOT"
fi
mkdir -p "$EVIDENCE_ROOT"
EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"
# mktemp 返回的文本路径可能带符号链接 checkout 的别名祖先；smoke_cleanup_owned_artifacts 要求 run dir 与 realpath 精确相等，立即归一化。
EVIDENCE_DIR="$(realpath -e -- "$EVIDENCE_DIR")"
RUN_ID="${EVIDENCE_DIR##*.}"
SERVER_LOG="$EVIDENCE_DIR/server.log"
if [ "$OWNED_WORLD_MODE" = "1" ]; then
  SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"
  mkdir -p "$SERVER_RUNTIME_DIR/server/data" "$SERVER_RUNTIME_DIR/library-web/public/deceased"
  export BONG_SERVER_DB="$SERVER_RUNTIME_DIR/server/data/bong.db"
  # Dedicated world evidence must not inherit checkout persistence. Production loaders that still
  # use cwd-relative assets receive a read-only bridge to this exact checkout; all writes remain
  # inside the per-run evidence runtime.
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

fallback_ready_marker_present() {
  # tracing-subscriber emits ANSI style codes even when stderr is redirected in CI. Strip only
  # those terminal controls, then apply the same anchored INFO-level payload contract.
  sed -E $'s/\x1b\\[[0-9;]*[[:alpha:]]//g' "$SERVER_LOG" \
    | grep -E -- "$BOT_FALLBACK_READY_PATTERN" >/dev/null
}

# Port reachability is insufficient: the immutable server may still be starting while an old server
# listens on the same port. Only a listener in this launch's process tree is authoritative.
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
    && { [ "$FALLBACK_MODE" != "1" ] || fallback_ready_marker_present; }
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
  local original_status=$?
  local cleanup_status=0
  local final_status
  trap - EXIT
  set +e

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
  # Retain logs/evidence, but remove only the exact run-private target and binary.
  # The shared helper validates the canonical run root and both candidates before
  # either deletion, then revalidates each immediately before rm.
  if ! smoke_cleanup_owned_artifacts "$EVIDENCE_DIR" "$CARGO_TARGET_ROOT" "$SERVER_BINARY"; then
    cleanup_status=1
    echo "[bot-e2e] refusing or failing run-private artifact cleanup; logs retained" >&2
  fi
  if [ "$OWNED_WORLD_MODE" != "1" ] && [ -n "$SPIRITWOOD_STATE_DIR" ]; then
    rm -f "$SPIRITWOOD_STATE_DIR/harvested.json" "$SPIRITWOOD_STATE_DIR/harvested.tmp"
    rmdir "$SPIRITWOOD_STATE_DIR" 2>/dev/null || true
  fi
  final_status=$original_status
  if [ "$final_status" -eq 0 ] && [ "$cleanup_status" -ne 0 ]; then
    final_status=1
  fi
  exit "$final_status"
}
trap cleanup EXIT

# ---- 编解码单测（无需 server；坏了没必要浪费一次 server 启动）----
python3 "$ROOT/scripts/bot/test_protocol.py"

# ---- redis ----
# Owned fixture and fallback evidence always receive isolated Redis, even if a caller exports
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
# 自动分配端口在 close(:0) 到 cargo bind 之间仍有可恢复的碰撞窗口；只针对明确的
# listener bind 失败做有限次换端口重启，避免一次碰撞丢掉整轮，也不把编译/业务故障
# 伪装成端口问题。
MAX_PORT_RETRIES=3

start_self_server_attempt() {
  local attempt="$1"
  local ready_marker_ok=0

  # 清掉上一次尝试的 marker，避免失败尝试留下的日志让本轮 readiness 误判；失败日志
  # 另存为 attempt-N，最终 evidence 仍保留每次实际启动的事实。
  if [ -s "$SERVER_LOG" ]; then
    cp -- "$SERVER_LOG" "$EVIDENCE_DIR/server-attempt-$((attempt - 1)).log"
  fi
  : >"$SERVER_LOG"

  PROFILE_FLAG=()
  TARGET_PROFILE=debug
  if [ "$PROFILE" = "release" ]; then
    PROFILE_FLAG=(--release)
    TARGET_PROFILE=release
  fi
  export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
  export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
  if [ -n "$BONG_E2E_PREBUILT_SERVER_MANIFEST" ]; then
    SERVER_BINARY="$EVIDENCE_DIR/bong-server-release"
    if ! python3 "$ROOT/scripts/lib/bong_server_provenance.py" copy \
      "$BONG_E2E_PREBUILT_SERVER_MANIFEST" "$ROOT" "$SERVER_BINARY" \
      >>"$SERVER_LOG" 2>&1; then
      echo "[bot-e2e] provenance-checked prebuilt server rejected" >&2
      tail -n 40 "$SERVER_LOG" >&2
      exit 1
    fi
    echo "[bot-e2e] reused one provenance-checked release artifact via run-owned copy: $SERVER_BINARY" >>"$SERVER_LOG"
  else
    echo "[bot-e2e] 构建并启动本轮 immutable server（profile=$PROFILE，log: $SERVER_LOG）"
    # Local fallback: build into a unique run-private target so no shared artifact
    # can be replaced between cargo build and install.
    CARGO_TARGET_ROOT="$(realpath -e -- "$EVIDENCE_DIR")/bong-target"
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
  fi
  echo "[bot-e2e] 启动 server（cargo run $PROFILE_FLAG，端口 $PORT，尝试 $attempt/$MAX_PORT_RETRIES，log: $SERVER_LOG）"
  # Owned-fixture mode moves all relative persistent outputs into its evidence runtime. Generic
  # mode retains the historical checkout/server CWD for callers that rely on that contract.
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
    # 自起模式必须把本轮选定的端口传给 server（BONG_SERVER_PORT），否则 server
    # 永远绑 valence 默认 25565，分配的端口将无人监听、readiness 假超时。
    export BONG_SERVER_PORT="$PORT"
    if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
      # The owned server is the harness capability boundary; REUSE never enters this branch.
      export BONG_DEV_MODE=1
    fi
    exec "$SERVER_BINARY"
  ) >>"$SERVER_LOG" 2>&1 &
  SERVER_PID=$!

  # Dedicated world modes need an exact startup marker. Generic mode keeps the prior common
  # bootstrap anchor while still requiring the listener to belong to this process tree.
  BOOT_ANCHOR="spawned tsy dimension layer (empty, awaits worldgen)"
  echo "[bot-e2e] 等待 $HOST:$PORT 就绪（最长 600s，冷编译会慢）"
  for _ in $(seq 1 300); do
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      if grep -Fq "failed to start TCP listener" "$SERVER_LOG"; then
        echo "[bot-e2e] 当前 server TCP listener 启动失败（端口 $PORT），可重试：" >&2
        tail -n 40 "$SERVER_LOG" >&2
        kill_tree "$SERVER_PID"
        wait "$SERVER_PID" 2>/dev/null || true
        return 75
      fi
      echo "[bot-e2e] server 进程提前退出，log 尾部：" >&2
      tail -n 40 "$SERVER_LOG" >&2
      return 1
    fi
    if grep -Fq "failed to start TCP listener" "$SERVER_LOG"; then
      echo "[bot-e2e] 当前 server TCP listener 启动失败（端口 $PORT），可重试：" >&2
      tail -n 40 "$SERVER_LOG" >&2
      kill_tree "$SERVER_PID"
      wait "$SERVER_PID" 2>/dev/null || true
      return 75
    fi
    # Dedicated world modes need an exact startup marker. Generic mode keeps the prior common
    # world bootstrap anchor while still requiring the listener to belong to this process tree.
    ready_marker_ok=0
    if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then
      grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG" && ready_marker_ok=1
    elif [ "$FALLBACK_MODE" = "1" ]; then
      fallback_ready_marker_present && ready_marker_ok=1
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
    fallback_ready_marker_present && ready_marker_ok=1
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
    return 1
  fi
  return 0
}

server_ready=0
for attempt in $(seq 1 "$MAX_PORT_RETRIES"); do
  if port_open "$HOST" "$PORT"; then
    if [ "$REUSE" = "1" ]; then
      echo "[bot-e2e] 复用已在 $HOST:$PORT 运行的 server（BOT_E2E_REUSE=1）"
      server_ready=1
      break
    fi
    if [ "$PORT_AUTO_ALLOCATED" = "1" ] && [ "$attempt" -lt "$MAX_PORT_RETRIES" ]; then
      old_port="$PORT"
      PORT="$(canonicalize_port "$(allocate_free_port)")"
      echo "[bot-e2e] 自动端口 $old_port 已被占用，改用 $PORT（预启动重试 $((attempt + 1))/$MAX_PORT_RETRIES）" >&2
      continue
    fi
    echo "[bot-e2e] $HOST:$PORT 已被占用。要对着现有 server 跑请设 BOT_E2E_REUSE=1；" >&2
    echo "[bot-e2e] 否则先停掉旧 server，避免测到过期二进制。" >&2
    exit 2
  fi

  if [ "$REUSE" = "1" ]; then
    echo "[bot-e2e] BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起" >&2
    exit 2
  fi
  if start_self_server_attempt "$attempt"; then
    server_ready=1
    break
  else
    attempt_status=$?
  fi
  if [ "$attempt_status" -eq 75 ] && [ "$PORT_AUTO_ALLOCATED" = "1" ] && [ "$attempt" -lt "$MAX_PORT_RETRIES" ]; then
    old_port="$PORT"
    PORT="$(canonicalize_port "$(allocate_free_port)")"
    echo "[bot-e2e] 自动端口 $old_port 与其它 listener 碰撞，改用 $PORT（启动重试 $((attempt + 1))/$MAX_PORT_RETRIES）" >&2
    continue
  fi
  exit 1
done
if [ "$server_ready" != "1" ]; then
  echo "[bot-e2e] 端口自动分配重试 $MAX_PORT_RETRIES 次仍未建立 server ownership" >&2
  exit 1
fi

# Reuse mode does not start a child server, so keep the caller-owned log as the
# evidence source. Never overwrite it with this run's unwritten evidence path;
# without a readable caller log, the log-backed anqi scenarios remain disabled.
if [ "$REUSE" = "1" ]; then
  if [ -n "$CALLER_SERVER_LOG" ] && [ -r "$CALLER_SERVER_LOG" ]; then
    SERVER_LOG="$CALLER_SERVER_LOG"
  else
    SERVER_LOG=""
  fi
fi

# The anqi scenarios are part of --all when this harness owns both prerequisites:
# Redis is reachable and a readable server log is available for consumer guards.
# CI sets the same gate explicitly; this local guard prevents reuse mode without
# an external log from pretending its negative evidence is covered.
if [ -n "$SERVER_LOG" ] && [ -r "$SERVER_LOG" ] \
  && { [ -n "${REDIS_URL:-}" ] || port_open 127.0.0.1 6379; }; then
  export BOT_E2E_ANQI_REDIS=1
else
  unset BOT_E2E_ANQI_REDIS
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
elif [ -n "${BOT_E2E_SCENARIOS:-}" ]; then
  SCENARIO_ARGS=()
  if [[ "$BOT_E2E_SCENARIOS" == ,* || "$BOT_E2E_SCENARIOS" == *, || "$BOT_E2E_SCENARIOS" == *,,* ]]; then
    echo "[bot-e2e] BOT_E2E_SCENARIOS 含空场景名：$BOT_E2E_SCENARIOS" >&2
    exit 2
  fi
  IFS=',' read -r -a requested_scenarios <<<"$BOT_E2E_SCENARIOS"
  for scenario in "${requested_scenarios[@]}"; do
    trimmed_scenario="${scenario#"${scenario%%[![:space:]]*}"}"
    trimmed_scenario="${trimmed_scenario%"${trimmed_scenario##*[![:space:]]}"}"
    if [ -z "$trimmed_scenario" ] || [ "$trimmed_scenario" != "$scenario" ]; then
      echo "[bot-e2e] BOT_E2E_SCENARIOS 含空白或带首尾空白的场景名：$BOT_E2E_SCENARIOS" >&2
      exit 2
    fi
    SCENARIO_ARGS+=(--scenario "$scenario")
  done
fi

set +e
# BONG_SERVER_LOG 交给场景做正向派发证据：combat_anqi_throw_carrier 的空手
# no-op 契约对 server→client 无可观测副作用，只能用 server 日志的
# `client_request received` 证明意图确实抵达并反序列化成功。
BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" BONG_SERVER_LOG="$SERVER_LOG" \
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
