#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV_RELOAD="$ROOT/scripts/dev-reload.sh"
TEST_TMP_PARENT="${TMPDIR:-$ROOT/.sisyphus/tmp}"
mkdir -p "$TEST_TMP_PARENT"
TEST_ROOT="$(mktemp -d "$TEST_TMP_PARENT/bong-dev-reload-disown.XXXXXX")"
PID_FILE="$TEST_ROOT/server.pid"
FAIL_LOG="$TEST_ROOT/completed-job.err"
SERVER_PID=""

cleanup() {
    if [[ "$SERVER_PID" =~ ^[0-9]+$ ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

bash -c '
    set -euo pipefail
    source "$1"
    sleep 30 &
    server_pid=$!
    printf "%s\n" "$server_pid" > "$2"
    detach_background_job "$server_pid"
' bash "$DEV_RELOAD" "$PID_FILE"

read -r SERVER_PID < "$PID_FILE"
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "FAIL: detached job $SERVER_PID did not survive launcher shell exit" >&2
    exit 1
fi

if bash -c '
    set -euo pipefail
    source "$1"
    true &
    exited_pid=$!
    wait "$exited_pid"
    detach_background_job "$exited_pid"
' bash "$DEV_RELOAD" 2>"$FAIL_LOG"; then
    echo "FAIL: detach unexpectedly accepted a completed background job" >&2
    exit 1
fi
if ! grep -q "exited before it could be detached" "$FAIL_LOG"; then
    echo "FAIL: completed job rejection did not explain the lifecycle failure" >&2
    exit 1
fi

echo "PASS: detached job survived launcher exit; completed job was rejected"
