#!/usr/bin/env bash
# stop.sh — 停止由 Bong 生命周期记录管理的进程
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"

stop_bong_stack() {
    local managed_stop_status=0 tmux_scan_status

    if bong_server_stop_managed; then
        managed_stop_status=0
    else
        managed_stop_status=$?
    fi
    case "$managed_stop_status" in
        0)
            ;;
        "$BONG_SERVER_STOP_FORCED")
            echo "WARN: managed bong-server required identity-safe SIGKILL; AppExit/Last persistence is unconfirmed" >&2
            ;;
        *)
            echo "FAIL: managed bong-server stop did not complete safely (status=$managed_stop_status); refusing tmux teardown" >&2
            return "$managed_stop_status"
            ;;
    esac
    if bong_server_tmux_has_unmanaged_server bong; then
        echo "FAIL: tmux session 'bong' still owns an unrecorded bong-server; refusing HUP shutdown" >&2
        return 1
    else
        tmux_scan_status=$?
        if [ "$tmux_scan_status" -eq 2 ]; then
            echo "FAIL: could not verify tmux session 'bong'; refusing teardown" >&2
            return 2
        fi
    fi

    echo "Stopped managed bong-server"
    tmux kill-session -t bong 2>/dev/null && echo "Killed tmux session 'bong'" || echo "No session 'bong' found"
    pkill -f "tiandao/src/main.ts" 2>/dev/null && echo "Killed tiandao agent" || true
    redis-cli shutdown nosave 2>/dev/null && echo "Stopped Redis" || true
    return "$managed_stop_status"
}

run_stop_bong_stack() {
    local status

    if bong_server_with_lock stop_bong_stack; then
        echo "Done"
        return 0
    else
        status=$?
    fi
    if [ "$status" -eq "$BONG_SERVER_STOP_FORCED" ]; then
        echo "Done (managed bong-server required forced shutdown)" >&2
    fi
    return "$status"
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
    return 0
fi

run_stop_bong_stack
