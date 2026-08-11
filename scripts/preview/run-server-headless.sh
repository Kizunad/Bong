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

  if bong_server_rollback_pinned_managed_process \
      "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_BINARY" \
      "$SERVER_EXECUTABLE_IDENTITY" "$operation"; then
    return 0
  else
    status=$?
  fi
  echo "❌ Preview server 回滚未确认 (status=$status); 保留匹配的权限记录供诊断" >&2
  return "$status"
}

# 有界回收：身份-safe 停服未确认时绝不无限 wait（进程可能仍存活，wait 会挂死）。
# 进程若仍存活，只有在确认为「本轮直接 spawn 的子进程」（/proc/<pid>/status 的
# PPid == 本启动器 PID）时才按自有进程树做有界停止 + 端口释放；否则拒绝信号
# （防 PID 复用打到无关进程），有界返回不等待。这是 identity 无法确认时唯一的
# 有界 cleanup 路径（review finding [1]/[4]：原实现把未记录进程裸留 25565
# 无限存活，或对存活进程无限 wait）。
bounded_cleanup_unconfirmed_launch() {
  local operation="$1" ppid

  if ! bong_server_process_is_running "$SERVER_PID"; then
    wait "$SERVER_PID" 2>/dev/null || true
    return 0
  fi
  ppid="$(awk '/^PPid:/{print $2; exit}' "/proc/$SERVER_PID/status" 2>/dev/null || true)"
  if [ "$ppid" != "$$" ]; then
    echo "❌ $operation：进程仍存活但非本启动器直接子进程（PPid=$ppid）；拒绝信号，有界返回" >&2
    return 1
  fi
  if bong_server_stop_process_tree_and_release_port "$SERVER_PID" "$PORT"; then
    wait "$SERVER_PID" 2>/dev/null || true
    return 0
  fi
  echo "❌ $operation：有界回收失败，进程可能仍存活（无权限记录）" >&2
  return 1
}

# 发布后回滚未确认的兜底（review finding 31447830772 [major]）：记录已发布、进程已
# disown 后，identity 持续不可读时 rollback 既不能停服也不能清记录（返回非零并保留
# 记录）。原实现 `rollback ... || true` 把失败当诊断信息吞掉后直接退出——被 spawn 的
# server 作为孤儿无限持有 25565，后续启动还被保留的记录拒绝。进程仍是本启动器直接
# spawn 的子进程（PPid == $$，disown 不改变父子关系），应走有界直接子进程回收停树 +
# 释放端口；回收确认成功后按相同身份安全清除记录（并发 stop 已清/记录被替换则保留）。
bong_server_post_publication_rollback() {
  local operation="$1"

  if rollback_preview_server "$operation"; then
    return 0
  fi
  echo "❌ identity-safe 回滚未确认；改走有界直接子进程回收" >&2
  if bounded_cleanup_unconfirmed_launch "$operation (direct-child fallback)"; then
    bong_server_clear_record_if_matches \
      "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_BINARY" "$SERVER_EXECUTABLE_IDENTITY" \
      || echo "⚠ 回收后权限记录未能安全移除，交由 stop-server-headless.sh 处理" >&2
  fi
  return 1
}

# 取消窗口保护（review finding [2]）：spawn 之后、身份记录发布之前，若启动器被外部
# 信号终止（INT/TERM/HUP）或流程失败（EXIT），被 spawn 的 server 尚未被任何记录
# 跟踪——裸留即为 untracked 孤儿持续持有 25565。两个 trap 处理器只在该窗口生效：
# EXIT 保留原退出码（正常成功/失败都不改写），INT/TERM/HUP 强制非零（被取消的
# 启动不是成功）；二者都只在「SERVER_PID 已设且记录未发布」时回收，记录发布后即
# no-op。disown 成功后 trap 会被清除，readiness 阶段的失败由显式 rollback 处理。
bong_server_pre_publication_maybe_cleanup() {
  if [ -n "${SERVER_PID:-}" ] \
      && [ ! -e "$PID_FILE" ] && [ ! -L "$PID_FILE" ]; then
    bounded_cleanup_unconfirmed_launch "preview pre-publication cancellation" || true
  fi
}
bong_server_pre_publication_on_exit() {
  local rc=$?
  bong_server_pre_publication_maybe_cleanup
  trap - EXIT INT TERM HUP
  exit "$rc"
}
bong_server_pre_publication_on_signal() {
  bong_server_pre_publication_maybe_cleanup
  trap - EXIT INT TERM HUP
  exit 1
}

bong_server_launch_preview_locked() {
  local status

  bong_server_refuse_existing_preview_record || return 1
  # 取消窗口 trap：覆盖 spawn → 记录发布区间（见函数定义）。server 子 shell 忽略
  # HUP（trap '' HUP），清理必须走 TERM/KILL 的进程树回收，bounded_cleanup 正是。
  trap bong_server_pre_publication_on_exit EXIT
  trap bong_server_pre_publication_on_signal INT TERM HUP
  : > "$LOG_FILE"
  echo "[run-server-headless] 启动 server (binary=$SERVER_BINARY)..."
  (
    trap '' HUP
    exec </dev/null
    cd "$REPO_ROOT/server"
    exec env "$SERVER_BINARY" >"$LOG_FILE" 2>&1
  ) &
  SERVER_PID=$!

  SERVER_STARTTIME=""
  SERVER_EXECUTABLE_IDENTITY=""
  server_exited_early=0
  for _ in $(seq 1 500); do
    # 进程已死 → 立即 break，不再把剩余 500 次迭代的 sleep 0.01 全跑完（review
    # finding 31436388638 [minor]：FAKE_SERVER_MODE=early 这类启动即退出的场景
    # 白耗 ~5s 才报失败）。真正的启动配置失败应尽早暴露，而非按固定迭代预算拖完。
    if ! bong_server_process_is_running "$SERVER_PID"; then
      server_exited_early=1
      break
    fi
    if [ "$(bong_server_process_executable "$SERVER_PID" 2>/dev/null || true)" = "$SERVER_BINARY" ]; then
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
    if [ "$server_exited_early" -eq 1 ]; then
      echo "❌ 启动的 server 已提前退出（PID $SERVER_PID），最后 30 行 log:" >&2
      tail -n 30 "$LOG_FILE" >&2
    else
      echo "❌ 无法确认直接启动的 server identity；拒绝把数字 PID 当作权限" >&2
      # 身份在 500 次重试内始终不可确认时，进程若仍存活不能裸留 25565——本启动器
      # 已直接 spawn 它（PID 来自 $!），做有界回收而非交给 CI runner 无限隔离
      # （review finding [4]：原实现把未记录的 server 留作孤儿）。
    fi
    bounded_cleanup_unconfirmed_launch "preview identity-pinning"
    return 1
  fi

  if ! bong_server_write_record "$SERVER_PID" "$SERVER_BINARY"; then
    echo "❌ 无法发布 server identity 权限记录" >&2
    if bong_server_stop_pinned_process \
        "$SERVER_PID" "$SERVER_STARTTIME" "$SERVER_EXECUTABLE_IDENTITY" 10 2; then
      status=0
    else
      status=$?
    fi
    case "$status" in
      0|1|"$BONG_SERVER_STOP_FORCED")
        # 已确认停止/消失：wait 立即返回，reap 子进程。
        wait "$SERVER_PID" 2>/dev/null || true
        ;;
      *)
        # 未确认（status 2）：进程可能仍存活。绝不无限 wait——有界重查，仍存活
        # 则按直接子进程回收（review finding [1]：原实现对存活进程无限 wait）。
        echo "❌ Server identity-safe rollback 未确认 (status=$status)" >&2
        bounded_cleanup_unconfirmed_launch "preview record-publish rollback"
        ;;
    esac
    return 1
  fi
  if ! disown "$SERVER_PID" 2>/dev/null; then
    echo "❌ 无法将 server 从启动 shell 安全分离" >&2
    rollback_preview_server "preview startup rollback" || true
    return 1
  fi
  # 记录已发布且进程已分离：取消窗口结束。readiness 阶段的失败走显式 rollback。
  trap - EXIT INT TERM HUP
}

# CI / preview 无 MINESKIN_API_KEY，跳过皮肤预取（NPC 回退 villager 实体），
# 否则 skin::pool::maintain_skin_pool 会因缺 key 直接 panic（对齐 e2e-redis.sh:892）。
export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"

# headless preview client 无法点击 server resource pack 提示。保持普通 server 默认推
# 资源包，仅 preview wrapper 默认跳过；显式设 BONG_RESOURCE_PACK_ENABLED=true 可覆盖。
export BONG_RESOURCE_PACK_ENABLED="${BONG_RESOURCE_PACK_ENABLED:-false}"

# 整个 start（拒绝已有记录 → 构建 → 启动）在同一生命周期锁内原子执行（review finding
# 31436388638 [major]：旧实现 refuse 与 launch 各自独立取锁，中间释放锁的构建窗口里
# stop 观察到无记录 → 静默成功退出，随后 start 完成构建又启动出 server——stop 报了成功
# 但 server 事后出现并持续运行）。持锁期间并发 stop 要么阻塞到本 start 结束再停（构建
# < 锁超时），要么锁超时诚实失败，绝无「stop 成功 + 后续 server 出现」。
bong_server_preview_start_locked() {
  bong_server_refuse_existing_preview_record || return 1

  BUILD_ARGS=(build --locked)
  if [ -n "$PROFILE_FLAG" ]; then
    BUILD_ARGS+=("$PROFILE_FLAG")
  fi
  echo "[run-server-headless] 构建 server (profile=$TARGET_PROFILE)..."
  (
    cd "$REPO_ROOT/server"
    "$REPO_ROOT/scripts/build-token.sh" cargo "${BUILD_ARGS[@]}"
  ) || return 1

  TARGET_ROOT="${CARGO_TARGET_DIR:-$REPO_ROOT/server/target}"
  if [[ "$TARGET_ROOT" != /* ]]; then
    TARGET_ROOT="$REPO_ROOT/server/$TARGET_ROOT"
  fi
  SERVER_BINARY="$(readlink -f -- "$TARGET_ROOT/$TARGET_PROFILE/bong-server")" \
    || { echo "❌ 找不到构建后的 server binary" >&2; return 1; }
  [ -x "$SERVER_BINARY" ] \
    || { echo "❌ Server binary 不可执行: $SERVER_BINARY" >&2; return 1; }

  bong_server_launch_preview_locked
}

bong_server_with_lock bong_server_preview_start_locked || exit 1

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
      rollback_preview_server "preview failed-start cleanup" || true
    elif [ "$status" -eq 2 ]; then
      # 身份无法确认（非「已消失」）：进程可能仍存活。同样必须走 identity-safe
      # 回滚——它重查 pinned identity，能确认则停服并清记录，不能则保留记录供
      # stop-server-headless.sh 事后清理，不能带着存活进程 + 记录直接退出
      # （review finding [2]：原实现跳过回滚，把进程和 PID 记录都留下）。回滚
      # 本身未确认（identity 持续不可读）时绝不能忽略失败退出——走有界直接
      # 子进程回收（review finding 31447830772 [major]）。
      echo "❌ Server identity 无法确认（status 2）；尝试 identity-safe 回滚" >&2
      bong_server_post_publication_rollback "preview identity-unconfirmed cleanup"
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
    # 回滚未确认时同样走有界直接子进程回收（见 bong_server_post_publication_rollback）。
    bong_server_post_publication_rollback "preview listener-owner rollback"
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
