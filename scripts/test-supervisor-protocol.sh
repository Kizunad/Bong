#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-supervisor-protocol.XXXXXX")"
SUPERVISOR="$ROOT/scripts/lib/bong-process-group-supervisor.py"
ACTIVE_OWNER_PID=""
ACTIVE_OWNER_STARTTIME=""
ACTIVE_OWNER_IDENTITY=""
ACTIVE_OWNER_PGID=""

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

close_fd_if_open() {
    local fd="${1:-}" direction="${2:-}"
    [[ "$fd" =~ ^[0-9]+$ ]] || return 0
    case "$direction" in
        read) eval "exec ${fd}<&-" 2>/dev/null || true ;;
        write) eval "exec ${fd}>&-" 2>/dev/null || true ;;
    esac
}

clear_active_owner() {
    ACTIVE_OWNER_PID=""
    ACTIVE_OWNER_STARTTIME=""
    ACTIVE_OWNER_IDENTITY=""
    ACTIVE_OWNER_PGID=""
}

track_active_owner() {
    local pid="$1" snapshot starttime pgid identity
    snapshot="$(bong_server_process_starttime_and_group "$pid")" \
        || fail "could not snapshot fixture owner $pid"
    read -r starttime pgid <<< "$snapshot"
    identity="$(bong_server_process_executable_identity "$pid")" \
        || fail "could not snapshot fixture owner executable identity"
    [ "$pgid" = "$pid" ] \
        || fail "fixture owner $pid did not establish a private process group (pgid=$pgid)"
    ACTIVE_OWNER_PID="$pid"
    ACTIVE_OWNER_STARTTIME="$starttime"
    ACTIVE_OWNER_IDENTITY="$identity"
    ACTIVE_OWNER_PGID="$pgid"
}

cleanup_active_owner() {
    local snapshot="" actual_starttime="" actual_pgid="" actual_identity=""
    [ -n "$ACTIVE_OWNER_PID" ] || return 0
    snapshot="$(bong_server_process_starttime_and_group "$ACTIVE_OWNER_PID" 2>/dev/null || true)"
    read -r actual_starttime actual_pgid <<< "$snapshot"
    actual_identity="$(bong_server_process_executable_identity "$ACTIVE_OWNER_PID" 2>/dev/null || true)"
    if [ "$actual_starttime" = "$ACTIVE_OWNER_STARTTIME" ] \
        && [ "$actual_pgid" = "$ACTIVE_OWNER_PGID" ] \
        && [ "$actual_identity" = "$ACTIVE_OWNER_IDENTITY" ]; then
        kill -KILL -- "-$ACTIVE_OWNER_PGID" 2>/dev/null || true
    fi
    wait "$ACTIVE_OWNER_PID" 2>/dev/null || true
    clear_active_owner
}

cleanup() {
    close_fd_if_open "${DIRECT_CONTROL_FD:-}" write
    close_fd_if_open "${DIRECT_READY_FD:-}" read
    close_fd_if_open "${SERVER_STARTUP_CONTROL_FD:-}" write
    close_fd_if_open "${SERVER_STARTUP_READY_FD:-}" read
    cleanup_active_owner
    if [[ "${BONG_SERVER_SUPERVISOR_PID:-}" =~ ^[0-9]+$ ]]; then
        local fallback_pid="$BONG_SERVER_SUPERVISOR_PID" fallback_snapshot fallback_starttime fallback_pgid
        fallback_snapshot="$(bong_server_process_starttime_and_group "$fallback_pid" 2>/dev/null || true)"
        read -r fallback_starttime fallback_pgid <<< "$fallback_snapshot"
        if [ "$fallback_pgid" = "$fallback_pid" ]; then
            kill -KILL -- "-$fallback_pgid" 2>/dev/null || true
        fi
        wait "$fallback_pid" 2>/dev/null || true
    fi
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT

wait_for_file() {
    local path="$1"
    for _ in $(seq 1 500); do
        [ -s "$path" ] && return 0
        sleep 0.01
    done
    return 1
}

assert_dev_null_fd0() {
    local pid="$1" role="$2" target
    target="$(readlink -f -- "/proc/$pid/fd/0" 2>/dev/null || true)"
    [ "$target" = /dev/null ] \
        || fail "$role stdin must be /dev/null, got ${target:-missing}"
}

fixture_dir="$TEST_ROOT/real-supervisor"
fixture_bin="$fixture_dir/bin"
fixture_server="$fixture_dir/server"
cargo_pid_file="$fixture_dir/cargo.pid"
descendant_pid_file="$fixture_dir/descendant.pid"
mkdir -p "$fixture_bin" "$fixture_server"
cat > "$fixture_bin/cargo" <<'FIXTURE'
#!/usr/bin/env bash
set -euo pipefail
: "${SUPERVISOR_FIXTURE_CARGO_PID:?}"
: "${SUPERVISOR_FIXTURE_DESCENDANT_PID:?}"
printf '%s\n' "$BASHPID" > "$SUPERVISOR_FIXTURE_CARGO_PID"
(
    trap '' TERM
    printf '%s\n' "$BASHPID" > "$SUPERVISOR_FIXTURE_DESCENDANT_PID"
    while :; do sleep 1; done
) &
descendant=$!
trap 'kill -KILL "$descendant" 2>/dev/null || true; wait "$descendant" 2>/dev/null || true; exit 0' TERM
while :; do sleep 1; done
FIXTURE
chmod +x "$fixture_bin/cargo"

start_direct_supervisor() {
    local ready_line=""
    rm -f -- "$cargo_pid_file" "$descendant_pid_file"
    coproc DIRECT_SUPERVISOR {
        exec env \
            PATH="$fixture_bin:$PATH" \
            SUPERVISOR_FIXTURE_CARGO_PID="$cargo_pid_file" \
            SUPERVISOR_FIXTURE_DESCENDANT_PID="$descendant_pid_file" \
            python3 "$SUPERVISOR" "$fixture_server" \
            2>"$fixture_dir/supervisor.log"
    }
    DIRECT_OWNER_PID=""
    DIRECT_READY_FD="${DIRECT_SUPERVISOR[0]}"
    DIRECT_CONTROL_FD="${DIRECT_SUPERVISOR[1]}"
    IFS= read -r -t 5 -u "$DIRECT_READY_FD" ready_line \
        || fail "supervisor did not publish READY"
    [[ "$ready_line" = 'READY pid='[0-9]* ]] \
        || fail "supervisor published malformed READY: $ready_line"
    DIRECT_OWNER_PID="${ready_line#READY pid=}"
    track_active_owner "$DIRECT_OWNER_PID"
    wait_for_file "$cargo_pid_file" || fail "cargo fixture did not publish its PID"
    wait_for_file "$descendant_pid_file" || fail "cargo descendant did not publish its PID"
    DIRECT_CARGO_PID="$(<"$cargo_pid_file")"
    DIRECT_DESCENDANT_PID="$(<"$descendant_pid_file")"
    assert_dev_null_fd0 "$DIRECT_CARGO_PID" "cargo child before commit"
    assert_dev_null_fd0 "$DIRECT_DESCENDANT_PID" "cargo descendant before commit"
}

assert_direct_rollback() {
    local owner="$DIRECT_OWNER_PID" descendant="$DIRECT_DESCENDANT_PID"
    close_fd_if_open "$DIRECT_CONTROL_FD" write
    DIRECT_CONTROL_FD=""
    close_fd_if_open "$DIRECT_READY_FD" read
    DIRECT_READY_FD=""
    if wait "$owner" 2>/dev/null; then
        fail "uncommitted supervisor must exit nonzero"
    fi
    kill -0 "$descendant" 2>/dev/null \
        && fail "uncommitted supervisor left its descendant alive"
    clear_active_owner
}

# EOF and a wrong command byte are both explicit rollback paths.
start_direct_supervisor
assert_direct_rollback

start_direct_supervisor
printf X >&"$DIRECT_CONTROL_FD" || fail "could not send invalid control byte"
assert_direct_rollback

# The exact C -> COMMITTED boundary keeps the supervisor as the persistent owner.
start_direct_supervisor
printf C >&"$DIRECT_CONTROL_FD" || fail "could not send commit byte"
close_fd_if_open "$DIRECT_CONTROL_FD" write
DIRECT_CONTROL_FD=""
acknowledgement=""
IFS= read -r -t 5 -u "$DIRECT_READY_FD" acknowledgement \
    || fail "supervisor did not publish COMMITTED"
[ "$acknowledgement" = COMMITTED ] \
    || fail "supervisor acknowledgement must be exact, got $acknowledgement"
close_fd_if_open "$DIRECT_READY_FD" read
DIRECT_READY_FD=""
assert_dev_null_fd0 "$DIRECT_CARGO_PID" "cargo child after commit"
assert_dev_null_fd0 "$DIRECT_DESCENDANT_PID" "cargo descendant after commit"
kill -0 "$DIRECT_OWNER_PID" 2>/dev/null \
    || fail "committed supervisor did not retain owner identity"
bong_server_port_is_open() { return 1; }
bong_server_stop_owned_process_group_and_release_port \
    "$ACTIVE_OWNER_PID" "$ACTIVE_OWNER_STARTTIME" "$ACTIVE_OWNER_IDENTITY" \
    "$ACTIVE_OWNER_PGID" 25565 \
    || fail "committed supervisor did not support owner-bound teardown"
unset -f bong_server_port_is_open
clear_active_owner

# Exercise the actual Bash parent state machine instead of pinning it with grep.
parent_fixture="$TEST_ROOT/start-server-process-group.sh"
python3 - "$ROOT/scripts/e2e-redis.sh" > "$parent_fixture" <<'PY'
from pathlib import Path
import sys

text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("start_server_process_group() {")
end = text.index("\n}\n\nstop_server() {", start) + 2
print(text[start:end])
PY
# shellcheck source=/dev/null
source "$parent_fixture"
RUST_PATH="$fixture_bin:$PATH"
SERVER_PID=""
SERVER_PGID=""
SERVER_OWNER_STARTTIME=""
SERVER_OWNER_EXECUTABLE_IDENTITY=""
SERVER_AUTHORITY_UNCERTAIN=1
SERVER_STARTUP_CONTROL_FD=""
SERVER_STARTUP_READY_FD=""

assert_parent_unpublished() {
    [ "$SERVER_AUTHORITY_UNCERTAIN" -eq 1 ] \
        || fail "failed startup must retain uncertain authority"
    [ -z "$SERVER_PID" ] && [ -z "$SERVER_PGID" ] \
        && [ -z "$SERVER_OWNER_STARTTIME" ] \
        && [ -z "$SERVER_OWNER_EXECUTABLE_IDENTITY" ] \
        || fail "failed startup must not publish partial process-group authority"
    [ -z "$SERVER_STARTUP_CONTROL_FD" ] && [ -z "$SERVER_STARTUP_READY_FD" ] \
        || fail "failed startup must close and clear protocol descriptors"
}

# The removed two-argument protocol must fail before spawning any supervisor.
if BONG_E2E_BUILD_TOKEN="$ROOT/scripts/build-token.sh" \
    start_server_process_group "$TEST_ROOT/parent-obsolete-build-token.log" 0; then
    fail "parent accepted obsolete BONG_E2E_BUILD_TOKEN override"
fi
assert_parent_unpublished

# Happy path uses the production supervisor and publishes only after exact ACK.
rm -f -- "$cargo_pid_file" "$descendant_pid_file"
SUPERVISOR_FIXTURE_CARGO_PID="$cargo_pid_file" \
SUPERVISOR_FIXTURE_DESCENDANT_PID="$descendant_pid_file" \
BONG_E2E_SUPERVISOR_TEST_MODE=1 \
BONG_E2E_SUPERVISOR="$SUPERVISOR" \
BONG_E2E_SERVER_DIRECTORY="$fixture_server" \
start_server_process_group "$TEST_ROOT/parent-normal.log" 0 1 \
    || fail "parent rejected the production READY -> C -> COMMITTED protocol"
[ "$SERVER_AUTHORITY_UNCERTAIN" -eq 0 ] \
    || fail "successful startup must publish certain authority"
[ "$SERVER_PID" = "$SERVER_PGID" ] \
    || fail "published supervisor must remain process-group owner"
[ -n "$SERVER_OWNER_STARTTIME" ] && [ -n "$SERVER_OWNER_EXECUTABLE_IDENTITY" ] \
    || fail "successful startup must publish the complete owner pin"
[ -z "$SERVER_STARTUP_CONTROL_FD" ] && [ -z "$SERVER_STARTUP_READY_FD" ] \
    || fail "successful startup must close and clear protocol descriptors"
track_active_owner "$SERVER_PID"
wait_for_file "$cargo_pid_file" || fail "parent cargo fixture did not publish its PID"
wait_for_file "$descendant_pid_file" || fail "parent cargo descendant did not publish its PID"
assert_dev_null_fd0 "$(<"$cargo_pid_file")" "parent cargo child after commit"
assert_dev_null_fd0 "$(<"$descendant_pid_file")" "parent cargo descendant after commit"
bong_server_port_is_open() { return 1; }
bong_server_stop_owned_process_group_and_release_port \
    "$SERVER_PID" "$SERVER_OWNER_STARTTIME" "$SERVER_OWNER_EXECUTABLE_IDENTITY" \
    "$SERVER_PGID" 25565 \
    || fail "parent-published authority did not support teardown"
unset -f bong_server_port_is_open
clear_active_owner

fake_supervisor="$TEST_ROOT/fake-supervisor.py"
cat > "$fake_supervisor" <<'PY'
#!/usr/bin/env python3
import os
import signal
import sys
import time

if len(sys.argv) != 2:
    print("usage: fake-supervisor.py SERVER_DIRECTORY", file=sys.stderr)
    raise SystemExit(2)

os.setsid()
for signal_number in (signal.SIGINT, signal.SIGHUP, signal.SIGTERM):
    signal.signal(signal_number, signal.SIG_IGN)
mode = os.environ.get("FAKE_SUPERVISOR_MODE", "normal")
pid_file = os.environ.get("FAKE_SUPERVISOR_PID_FILE")
if pid_file:
    with open(pid_file, "w", encoding="utf-8") as handle:
        handle.write(f"{os.getpid()}\n")
sys.stdout.buffer.write(f"READY pid={os.getpid()}\n".encode())
sys.stdout.buffer.flush()
if sys.stdin.buffer.read(1) != b"C":
    raise SystemExit(2)
if mode == "eof":
    raise SystemExit(3)
if mode == "malformed":
    sys.stdout.buffer.write(b"NOT-COMMITTED\n")
else:
    sys.stdout.buffer.write(b"COMMITTED\n")
sys.stdout.buffer.flush()
while True:
    time.sleep(1)
PY
chmod +x "$fake_supervisor"
fake_pid_file="$TEST_ROOT/fake-supervisor.pid"
bong_server_port_is_open() { return 1; }

# The strict fake accepts the same single SERVER_DIRECTORY argument as production.
# An accidental second argv must fail before READY so caller/supervisor drift cannot hide.
if python3 "$fake_supervisor" "$fixture_server" "$ROOT/scripts/build-token.sh" \
    >"$TEST_ROOT/fake-extra-argv.out" 2>"$TEST_ROOT/fake-extra-argv.err"; then
    fail "strict fake supervisor accepted an obsolete second argument"
fi
grep -Fq "usage: fake-supervisor.py SERVER_DIRECTORY" "$TEST_ROOT/fake-extra-argv.err" \
    || fail "strict fake supervisor did not reject extra argv with its one-argument usage"

run_failed_parent_mode() {
    local mode="$1" after_commit_hook="${2:-}" after_ack_hook="${3:-}" owner wait_status=0
    rm -f -- "$fake_pid_file"
    if FAKE_SUPERVISOR_MODE="$mode" \
        FAKE_SUPERVISOR_PID_FILE="$fake_pid_file" \
        BONG_E2E_SUPERVISOR_TEST_MODE=1 \
        BONG_E2E_SUPERVISOR="$fake_supervisor" \
        BONG_E2E_SERVER_DIRECTORY="$fixture_server" \
        BONG_E2E_TEST_AFTER_COMMIT_WRITE_HOOK="$after_commit_hook" \
        BONG_E2E_TEST_AFTER_ACK_HOOK="$after_ack_hook" \
        start_server_process_group "$TEST_ROOT/parent-$mode.log" 0 1; then
        fail "parent unexpectedly accepted failed supervisor mode $mode"
    fi
    assert_parent_unpublished
    wait_for_file "$fake_pid_file" || fail "failed parent mode $mode did not publish owner PID"
    owner="$(<"$fake_pid_file")"
    if kill -0 "$owner" 2>/dev/null; then
        fail "failed parent mode $mode left its pinned supervisor alive"
    fi
    wait "$owner" 2>/dev/null || wait_status=$?
    [ "$wait_status" -ne 0 ] || fail "failed parent mode $mode exited successfully"
}

run_failed_parent_mode malformed
run_failed_parent_mode eof

stop_before_commit_hook="$TEST_ROOT/stop-before-commit.sh"
cat > "$stop_before_commit_hook" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
kill -STOP "$1"
printf 'stopped\n' > "${BONG_TEST_STOP_MARKER:?}"
HOOK
chmod +x "$stop_before_commit_hook"

# A producer stopped before consuming C leaves the write successful but cannot
# emit ACK. This isolates the protocol boundary from scheduler timing.
no_ack_supervisor="$TEST_ROOT/no-ack-supervisor.py"
cat > "$no_ack_supervisor" <<'PY'
#!/usr/bin/env python3
import os
import signal
import sys

os.setsid()
for signal_number in (signal.SIGINT, signal.SIGHUP, signal.SIGTERM):
    signal.signal(signal_number, signal.SIG_IGN)
pid_file = os.environ["FAKE_SUPERVISOR_PID_FILE"]
with open(pid_file, "w", encoding="utf-8") as handle:
    handle.write(f"{os.getpid()}\n")
sys.stdout.buffer.write(f"READY pid={os.getpid()}\n".encode())
sys.stdout.buffer.flush()
while True:
    signal.pause()
PY
chmod +x "$no_ack_supervisor"
rm -f -- "$fake_pid_file"
stop_marker="$TEST_ROOT/fake-supervisor.stopped"
FAKE_SUPERVISOR_PID_FILE="$fake_pid_file" \
BONG_E2E_SUPERVISOR_TEST_MODE=1 \
BONG_E2E_SUPERVISOR="$no_ack_supervisor" \
BONG_E2E_SERVER_DIRECTORY="$fixture_server" \
BONG_TEST_STOP_MARKER="$stop_marker" \
BONG_E2E_TEST_AFTER_COMMIT_WRITE_HOOK="$stop_before_commit_hook" \
start_server_process_group "$TEST_ROOT/parent-stopped.log" 0 1 &
stopped_parent_pid=$!
wait_for_file "$stop_marker" || fail "SIGSTOP fixture did not stop the supervisor after C write"
wait_for_file "$fake_pid_file" || fail "SIGSTOP fixture did not publish owner PID"
stopped_owner="$(<"$fake_pid_file")"
sleep 0.1
kill -0 "$stopped_parent_pid" 2>/dev/null \
    || fail "parent returned before the stopped supervisor could acknowledge C"
kill -KILL "$stopped_owner" 2>/dev/null || true
if wait "$stopped_parent_pid"; then
    fail "parent accepted startup after the stopped supervisor was killed before ACK"
fi
assert_parent_unpublished

kill_after_ack_hook="$TEST_ROOT/kill-after-ack.sh"
cat > "$kill_after_ack_hook" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail
kill -KILL "$1" 2>/dev/null || true
HOOK
chmod +x "$kill_after_ack_hook"
run_failed_parent_mode normal "" "$kill_after_ack_hook"
unset -f bong_server_port_is_open

printf 'PASS: supervisor commit protocol and parent authority publication are fail-closed\n'
