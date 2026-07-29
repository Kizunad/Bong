#!/usr/bin/env bash
# run-server-headless.sh — 后台启动 Bong server（preview 用），等到端口 ready
#
# 用法:
#   bash scripts/preview/run-server-headless.sh [--release|--debug] [--timeout 60]
#
# 行为:
#   1. 通过共享 build-token 构建已知 server binary（默认 --release）
#   2. 直接启动该 binary，并发布 PID/starttime/executable identity 权限记录
#   3. 只接受该精确进程持有的 127.0.0.1:25565 listener
#   4. 超时 → 打印 server log + identity-safe 停服 + exit 1
#
# server 已是 offline mode + mock bridge（无 Redis 依赖），见 server/src/main.rs:64,68。
# 退出后 server 仍在后台跑；调用方用 scripts/preview/stop-server-headless.sh 清理。

set -euo pipefail
umask 077

PROFILE_FLAG="--release"
TARGET_PROFILE="release"
TIMEOUT_SECONDS=90
PORT=25565
LOG_FILE="/tmp/bong-preview-server.log"

while [ $# -gt 0 ]; do
  case "$1" in
    --release) PROFILE_FLAG="--release"; TARGET_PROFILE="release"; shift ;;
    --debug)   PROFILE_FLAG=""; TARGET_PROFILE="debug"; shift ;;
    --timeout)
      [ "$#" -ge 2 ] || { echo "--timeout 缺少秒数" >&2; exit 2; }
      TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done
[[ "$TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] && [ "$TIMEOUT_SECONDS" -gt 0 ] \
  || { echo "--timeout 必须是正整数秒数: $TIMEOUT_SECONDS" >&2; exit 2; }

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$REPO_ROOT/scripts/lib/bong-server-lifecycle.sh"
PID_FILE="${BONG_PREVIEW_PID_FILE:-$(bong_server_runtime_dir)/bong-preview-server.pid}"
export BONG_SERVER_PID_FILE="$PID_FILE"

bong_server_refuse_existing_preview_record() {
  local record status

  record="$(bong_server_pid_file)" || return 1
  if [ ! -e "$record" ] && [ ! -L "$record" ]; then
    return 0
  fi
  if bong_server_read_record; then
    if bong_server_record_matches_process; then
      echo "❌ Server 已在跑 (PID $BONG_SERVER_RECORDED_PID)，先调用 stop-server-headless.sh" >&2
    else
      status=$?
      if [ "$status" -eq 1 ]; then
        echo "❌ Preview server 权限记录已陈旧；先调用 stop-server-headless.sh 安全清理" >&2
      else
        echo "❌ Preview server 权限记录无法确认；拒绝覆盖: $record" >&2
      fi
    fi
  else
    echo "❌ Preview server 权限记录无效；拒绝覆盖: $record" >&2
  fi
  return 1
}

bong_server_refuse_existing_preview_record_locked() {
  bong_server_with_lock bong_server_refuse_existing_preview_record
}

rollback_preview_server() {
  local operation="$1" status

  if bong_server_stop_managed_for_replacement "$operation"; then
    return 0
  else
    status=$?
  fi
  echo "❌ Preview server 回滚未确认 (status=$status); 保留权限记录供诊断" >&2
  return "$status"
}

bong_server_launch_preview_locked() {
  local status

  bong_server_refuse_existing_preview_record || return 1
  : > "$LOG_FILE"
  echo "[run-server-headless] 启动 server (binary=$SERVER_BINARY)..."
  (
    trap '' HUP
    exec </dev/null
    exec env "$SERVER_BINARY" >"$LOG_FILE" 2>&1
  ) &
  SERVER_PID=$!

  SERVER_STARTTIME=""
  SERVER_EXECUTABLE_IDENTITY=""
  for _ in $(seq 1 500); do
    if bong_server_process_is_running "$SERVER_PID" \
        && [ "$(bong_server_process_executable "$SERVER_PID" 2>/dev/null || true)" = "$SERVER_BINARY" ]; then
      SERVER_STARTTIME="$(bong_server_process_starttime "$SERVER_PID")" || SERVER_STARTTIME=""
      SERVER_EXECUTABLE_IDENTITY="$(bong_server_process_executable_identity "$SERVER_PID")" \
        || SERVER_EXECUTABLE_IDENTITY=""
      if [[ "$SERVER_STARTTIME" =~ ^[0-9]+$ ]] \
          && [[ "$SERVER_EXECUTABLE_IDENTITY" =~ ^[0-9]+:[0-9]+$ ]]; then
        break
      fi
    fi
    sleep 0.01
  done

  if [[ ! "$SERVER_STARTTIME" =~ ^[0-9]+$ ]] \
      || [[ ! "$SERVER_EXECUTABLE_IDENTITY" =~ ^[0-9]+:[0-9]+$ ]]; then
    echo "❌ 无法确认直接启动的 server identity；拒绝把数字 PID 当作权限" >&2
    if ! bong_server_process_is_running "$SERVER_PID"; then
      wait "$SERVER_PID" 2>/dev/null || true
    else
      echo "❌ 未记录的 server 仍存活但身份不可确认；拒绝发送信号，交由 CI runner 隔离回收" >&2
    fi
    return 1
  fi

  if ! bong_server_write_record "$SERVER_PID" "$SERVER_BINARY"; then
    echo "❌ 无法发布 server identity 权限记录" >&2
    if bong_server_stop_pinned_process \
        "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_EXECUTABLE_IDENTITY" 10 2; then
      :
    else
      status=$?
      if [ "$status" -ne 1 ] && [ "$status" -ne "$BONG_SERVER_STOP_FORCED" ]; then
        echo "❌ Server identity-safe rollback 未确认 (status=$status)" >&2
      fi
    fi
    wait "$SERVER_PID" 2>/dev/null || true
    return 1
  fi
  if ! disown "$SERVER_PID" 2>/dev/null; then
    echo "❌ 无法将 server 从启动 shell 安全分离" >&2
    rollback_preview_server "preview startup rollback" || true
    return 1
  fi
}

bong_server_refuse_existing_preview_record_locked || exit 1

# CI / preview 无 MINESKIN_API_KEY，跳过皮肤预取（NPC 回退 villager 实体），
# 否则 skin::pool::maintain_skin_pool 会因缺 key 直接 panic（对齐 e2e-redis.sh:892）。
export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"

# headless preview client 无法点击 server resource pack 提示。保持普通 server 默认推
# 资源包，仅 preview wrapper 默认跳过；显式设 BONG_RESOURCE_PACK_ENABLED=true 可覆盖。
export BONG_RESOURCE_PACK_ENABLED="${BONG_RESOURCE_PACK_ENABLED:-false}"

BUILD_ARGS=(build --locked)
if [ -n "$PROFILE_FLAG" ]; then
  BUILD_ARGS+=("$PROFILE_FLAG")
fi
echo "[run-server-headless] 构建 server (profile=$TARGET_PROFILE)..."
(
  cd "$REPO_ROOT/server"
  "$REPO_ROOT/scripts/build-token.sh" cargo "${BUILD_ARGS[@]}"
)

TARGET_ROOT="${CARGO_TARGET_DIR:-$REPO_ROOT/server/target}"
if [[ "$TARGET_ROOT" != /* ]]; then
  TARGET_ROOT="$REPO_ROOT/server/$TARGET_ROOT"
fi
SERVER_BINARY="$(readlink -f -- "$TARGET_ROOT/$TARGET_PROFILE/bong-server")" \
  || { echo "❌ 找不到构建后的 server binary" >&2; exit 1; }
[ -x "$SERVER_BINARY" ] \
  || { echo "❌ Server binary 不可执行: $SERVER_BINARY" >&2; exit 1; }

bong_server_with_lock bong_server_launch_preview_locked || exit 1

echo "[run-server-headless] PID=$SERVER_PID authority=$PID_FILE log=$LOG_FILE"

elapsed=0
while [ "$elapsed" -lt "$TIMEOUT_SECONDS" ]; do
  if bong_server_pinned_process_status \
      "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_EXECUTABLE_IDENTITY"; then
    :
  else
    status=$?
    echo "❌ Server identity 已消失或无法确认 (status=$status)，最后 30 行 log:" >&2
    tail -n 30 "$LOG_FILE" >&2
    if [ "$status" -eq 1 ]; then
      echo "❌ Server 在权限记录发布后提前退出；安全清理匹配记录" >&2
      if ! bong_server_stop_managed_for_replacement "preview failed-start cleanup"; then
        echo "❌ Preview server 退出记录未确认清理；保留权限记录供诊断" >&2
      fi
    fi
    exit 1
  fi

  if bong_server_pinned_process_owns_ipv4_listener \
      "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_EXECUTABLE_IDENTITY" "$PORT"; then
    echo "[run-server-headless] ✅ ready (耗时 ${elapsed}s)，精确进程持有端口 $PORT"
    exit 0
  else
    status=$?
  fi
  if [ "$status" -eq 2 ]; then
    echo "❌ 无法确认端口 $PORT 的 owner；执行 identity-safe 回滚" >&2
    tail -n 30 "$LOG_FILE" >&2
    rollback_preview_server "preview listener-owner rollback" || true
    exit 1
  fi
  sleep 1
  elapsed=$((elapsed + 1))
done

echo "❌ Server 在 ${TIMEOUT_SECONDS}s 内未就绪；执行 identity-safe 回滚" >&2
echo "最后 30 行 log:" >&2
tail -n 30 "$LOG_FILE" >&2
rollback_preview_server "preview timeout rollback" || true
exit 1
