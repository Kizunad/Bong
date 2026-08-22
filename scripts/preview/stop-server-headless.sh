#!/usr/bin/env bash
# stop-server-headless.sh — 按 PID/starttime/executable identity 停止 preview server。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
source "$REPO_ROOT/scripts/lib/bong-server-lifecycle.sh"
PID_FILE="${BONG_PREVIEW_PID_FILE:-$(bong_server_runtime_dir)/bong-preview-server.pid}"
export BONG_SERVER_PID_FILE="$PID_FILE"

if [ ! -e "$PID_FILE" ] && [ ! -L "$PID_FILE" ]; then
  exit 0
fi
bong_server_stop_managed_for_replacement "preview cleanup"
