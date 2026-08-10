#!/usr/bin/env bash
# stop-server-headless.sh — 按 PID/starttime/executable identity 停止 preview server。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$REPO_ROOT/scripts/lib/bong-server-lifecycle.sh"
PID_FILE="${BONG_PREVIEW_PID_FILE:-$(bong_server_runtime_dir)/bong-preview-server.pid}"
export BONG_SERVER_PID_FILE="$PID_FILE"

# stop 的「无记录 → 无事可做」判定必须在生命周期锁内做（review finding 31436388638
# [major]：旧实现在锁外先看 PID 文件，start 正在持锁构建（记录尚未发布）时这里看到
# 无文件 → exit 0 静默成功，随后 start 完成构建启动出 server——stop 报了成功但 server
# 事后出现并持续运行）。持锁后：若 start 在途则阻塞到其结束（构建 < 锁超时）再按其
# 记录停服，或锁超时诚实失败；绝无「stop 成功 + 后续 server 出现」。
bong_server_stop_preview_locked() {
  if [ ! -e "$PID_FILE" ] && [ ! -L "$PID_FILE" ]; then
    return 0
  fi
  bong_server_stop_managed_for_replacement "preview cleanup"
}
bong_server_with_lock bong_server_stop_preview_locked
