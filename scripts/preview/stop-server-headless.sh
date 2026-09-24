#!/usr/bin/env bash
# stop-server-headless.sh — 按 PID/starttime/executable identity 停止 preview server。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$REPO_ROOT/scripts/lib/bong-server-lifecycle.sh"
PID_FILE="${BONG_PREVIEW_PID_FILE:-$(bong_server_runtime_dir)/bong-preview-server.pid}"
export BONG_SERVER_PID_FILE="$PID_FILE"

# A preview runner may retain a launch-specific identity handoff after the
# wrapper exits non-zero.  These values are only a cleanup capability, never a
# replacement for the lifecycle record: the record is re-read and compared
# while the lifecycle lock is held immediately before any stop operation.
EXPECTED_PID="${BONG_PREVIEW_EXPECTED_PID:-}"
EXPECTED_STARTTIME="${BONG_PREVIEW_EXPECTED_STARTTIME:-}"
EXPECTED_EXECUTABLE="${BONG_PREVIEW_EXPECTED_EXECUTABLE:-}"
EXPECTED_EXECUTABLE_IDENTITY="${BONG_PREVIEW_EXPECTED_EXECUTABLE_IDENTITY:-}"

expected_identity_is_complete=0
if [[ -n "$EXPECTED_PID$EXPECTED_STARTTIME$EXPECTED_EXECUTABLE$EXPECTED_EXECUTABLE_IDENTITY" ]]; then
  [[ -n "$EXPECTED_PID" && -n "$EXPECTED_STARTTIME" && -n "$EXPECTED_EXECUTABLE" \
      && -n "$EXPECTED_EXECUTABLE_IDENTITY" ]] || {
    echo "FAIL: preview expected identity handoff 不完整；拒绝 stop" >&2
    exit 2
  }
  [[ "$EXPECTED_PID" =~ ^[0-9]+$ ]] && [ "$EXPECTED_PID" -gt 1 ] \
      && [[ "$EXPECTED_STARTTIME" =~ ^[0-9]+$ ]] \
      && [[ "$EXPECTED_EXECUTABLE_IDENTITY" =~ ^[0-9]+:[0-9]+$ ]] || {
    echo "FAIL: preview expected identity handoff 格式无效；拒绝 stop" >&2
    exit 2
  }
  expected_identity_is_complete=1
fi

# stop 的「无记录 → 无事可做」判定必须在生命周期锁内做（review finding 31436388638
# [major]：旧实现在锁外先看 PID 文件，start 正在持锁构建（记录尚未发布）时这里看到
# 无文件 → exit 0 静默成功，随后 start 完成构建启动出 server——stop 报了成功但 server
# 事后出现并持续运行）。持锁后：若 start 在途则阻塞到其结束（构建 < 锁超时）再按其
# 记录停服，或锁超时诚实失败；绝无「stop 成功 + 后续 server 出现」。
bong_server_stop_preview_locked() {
  if [ ! -e "$PID_FILE" ] && [ ! -L "$PID_FILE" ]; then
    return 0
  fi
  if [ "$expected_identity_is_complete" -eq 1 ]; then
    if ! bong_server_read_record; then
      echo "FAIL: preview expected identity 对应的 authority record 无法读取；拒绝 stop" >&2
      return 2
    fi
    if [ "$BONG_SERVER_RECORDED_PID" != "$EXPECTED_PID" ] \
        || [ "$BONG_SERVER_RECORDED_STARTTIME" != "$EXPECTED_STARTTIME" ] \
        || [ "$BONG_SERVER_RECORDED_EXECUTABLE" != "$EXPECTED_EXECUTABLE" ] \
        || [ "$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY" != "$EXPECTED_EXECUTABLE_IDENTITY" ]; then
      echo "INFO: preview authority record 已被 successor 替换；保留 successor，拒绝旧 launch stop" >&2
      return 1
    fi
  fi
  bong_server_stop_managed_for_replacement "preview cleanup"
}
bong_server_with_lock bong_server_stop_preview_locked
