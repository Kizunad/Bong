#!/usr/bin/env bash
set -euo pipefail
umask 077

# This test creates a real tmux server and delivers real TERM/KILL signals. It is
# quarantined to the disposable GitHub Actions runner and requires an explicit
# workflow opt-in; local lifecycle suites must never activate it implicitly.
if [ "${GITHUB_ACTIONS:-}" != true ] \
    || [ "${BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST:-0}" != 1 ]; then
    printf 'SKIP: tmux shutdown-order signal test is quarantined to opted-in GitHub Actions e2e\n'
    exit 0
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-tmux-shutdown-order.XXXXXX")"
TMUX_SOCKET="$TEST_ROOT/tmux.sock"
SESSION="flush-order"
ACTIVE_PID=""
ACTIVE_STARTTIME=""
ACTIVE_IDENTITY=""

cleanup() {
    local stop_status

    if bong_server_validate_signal_id "$ACTIVE_PID" \
        && [[ "$ACTIVE_STARTTIME" =~ ^[0-9]+$ ]] \
        && [[ "$ACTIVE_IDENTITY" =~ ^[0-9]+:[0-9]+$ ]]; then
        if bong_server_stop_pinned_process \
            "$ACTIVE_PID" "$ACTIVE_STARTTIME" "$ACTIVE_IDENTITY" 2 1; then
            stop_status=0
        else
            stop_status=$?
        fi
        case "$stop_status" in
            0|1|"$BONG_SERVER_STOP_FORCED") ;;
            *) printf 'WARN: shutdown probe cleanup could not safely signal pinned pid %s (status=%s)\n' \
                "$ACTIVE_PID" "$stop_status" >&2 ;;
        esac
    fi
    # The absolute socket lives inside this invocation's 0700 directory. Session
    # teardown cannot resolve to the user's default tmux server.
    tmux -S "$TMUX_SOCKET" kill-session -t "$SESSION" 2>/dev/null || true
    if bong_server_validate_signal_id "$ACTIVE_PID"; then
        wait "$ACTIVE_PID" 2>/dev/null || true
    fi
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

wait_for_path() {
    local path="$1"
    for _ in $(seq 1 3000); do
        [ -e "$path" ] && return 0
        sleep 0.01
    done
    return 1
}

server_binary="${CARGO_TARGET_DIR:-$ROOT/server/target}/debug/bong-server"
[ -x "$server_binary" ] || fail "shutdown-order fixture requires built binary at $server_binary"
unlock_path="$TEST_ROOT/data/craft/recipe_unlocks.json"
ready_path="$TEST_ROOT/probe.ready"
stderr_path="$TEST_ROOT/probe.stderr"

# The pane shell execs the real probe binary, matching start.sh's process identity
# model. The dirty unlock stays below its 600-tick runtime interval until TERM.
tmux -S "$TMUX_SOCKET" new-session -d -s "$SESSION" \
    "cd '$ROOT/server' && exec env \
BONG_SHUTDOWN_SIGNAL_PROBE=1 \
BONG_SHUTDOWN_SIGNAL_PROBE_UNLOCK_PATH='$unlock_path' \
BONG_SHUTDOWN_SIGNAL_PROBE_READY_PATH='$ready_path' \
BONG_SKIP_SKIN_PREFETCH=1 \
REDIS_URL='redis://127.0.0.1:1' \
'$server_binary' 2>'$stderr_path'"
ACTIVE_PID="$(tmux -S "$TMUX_SOCKET" display-message -p -t "$SESSION" '#{pane_pid}')"
bong_server_validate_signal_id "$ACTIVE_PID" \
    || fail "tmux returned reserved or malformed pane pid: ${ACTIVE_PID:-empty}"
bong_server_wait_for_executable "$ACTIVE_PID" "$server_binary" 500 \
    || fail "tmux pane did not exec the shutdown probe binary"
ACTIVE_STARTTIME="$(bong_server_process_starttime "$ACTIVE_PID")" \
    || fail "could not pin shutdown probe starttime"
ACTIVE_IDENTITY="$(bong_server_process_executable_identity "$ACTIVE_PID")" \
    || fail "could not pin shutdown probe executable identity"
wait_for_path "$ready_path" || {
    printf '%s\n' "--- shutdown probe stderr ---" >&2
    if [ -f "$stderr_path" ]; then
        python3 - "$stderr_path" <<'PY' >&2
from pathlib import Path
import sys
lines = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").splitlines()
print("\n".join(lines[-80:]))
PY
    fi
    fail "shutdown probe did not publish post-startup signal readiness"
}
[ ! -e "$unlock_path" ] \
    || fail "unlock log must remain dirty before the shutdown TERM"

# This is the production ordering contract: pinned TERM/wait drives AppExit and
# Last flush first. Only after that helper returns may the tmux session be killed.
if bong_server_stop_pinned_process \
    "$ACTIVE_PID" "$ACTIVE_STARTTIME" "$ACTIVE_IDENTITY" 10 2; then
    stop_status=0
else
    stop_status=$?
fi
if [ "$stop_status" -ne 0 ]; then
    printf '%s\n' "--- shutdown probe stderr ---" >&2
    if [ -f "$stderr_path" ]; then
        python3 - "$stderr_path" <<'PY' >&2
from pathlib import Path
import sys
lines = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace").splitlines()
print("\n".join(lines[-80:]))
PY
    fi
    fail "pinned TERM did not complete the shutdown probe (status=$stop_status)"
fi
ACTIVE_PID=""
ACTIVE_STARTTIME=""
ACTIVE_IDENTITY=""
[ -f "$unlock_path" ] \
    || fail "Last must durably publish recipe unlocks before tmux teardown"
[ ! -e "${unlock_path%.json}.tmp" ] \
    || fail "Last flush must not leave a temporary unlock file"
python3 - "$unlock_path" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
assert payload["version"] == 1
assert "craft.probe.shutdown.flush" in payload["by_player"]["offline:shutdown-probe"]
PY

# The pane may already have closed with the server. tmux teardown is deliberately
# idempotent, but it occurs only after persistence hydration above succeeded.
tmux -S "$TMUX_SOCKET" kill-session -t "$SESSION" 2>/dev/null || true
if tmux -S "$TMUX_SOCKET" has-session -t "$SESSION" 2>/dev/null; then
    fail "tmux session survived post-flush teardown"
fi

printf 'PASS: pinned TERM completes AppExit/Last persistence before tmux teardown\n'
