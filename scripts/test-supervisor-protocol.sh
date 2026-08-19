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
    local stop_status original_port_check
    [ -n "$ACTIVE_OWNER_PID" ] || return 0
    if ! bong_server_validate_signal_id "$ACTIVE_OWNER_PID" \
        || ! bong_server_validate_signal_id "$ACTIVE_OWNER_PGID" \
        || [[ ! "$ACTIVE_OWNER_STARTTIME" =~ ^[0-9]+$ ]] \
        || [[ ! "$ACTIVE_OWNER_IDENTITY" =~ ^[0-9]+:[0-9]+$ ]]; then
        printf 'WARN: refusing cleanup without complete pinned fixture authority\n' >&2
        clear_active_owner
        return 0
    fi

    # Reuse production's per-member pidfd teardown. The protocol fixture never
    # owns a TCP listener, so port release is an invariant rather than a signal
    # authority and is kept deterministic here.
    original_port_check="$(declare -f bong_server_port_is_open || true)"
    bong_server_port_is_open() { return 1; }
    if bong_server_stop_owned_process_group_and_release_port \
        "$ACTIVE_OWNER_PID" "$ACTIVE_OWNER_STARTTIME" "$ACTIVE_OWNER_IDENTITY" \
        "$ACTIVE_OWNER_PGID" 25565; then
        stop_status=0
    else
        stop_status=$?
    fi
    if [ -n "$original_port_check" ]; then
        unset -f bong_server_port_is_open
        eval "$original_port_check"
    else
        unset -f bong_server_port_is_open
    fi
    case "$stop_status" in
        0|"$BONG_SERVER_STOP_FORCED") wait "$ACTIVE_OWNER_PID" 2>/dev/null || true ;;
        *) printf 'WARN: pinned fixture cleanup failed closed (status=%s)\n' "$stop_status" >&2 ;;
    esac
    clear_active_owner
}

cleanup() {
    close_fd_if_open "${DIRECT_CONTROL_FD:-}" write
    close_fd_if_open "${DIRECT_READY_FD:-}" read
    close_fd_if_open "${SERVER_STARTUP_CONTROL_FD:-}" write
    close_fd_if_open "${SERVER_STARTUP_READY_FD:-}" read
    cleanup_active_owner
    if bong_server_validate_signal_id "${BONG_SERVER_SUPERVISOR_PID:-}"; then
        local fallback_pid="$BONG_SERVER_SUPERVISOR_PID" fallback_snapshot fallback_starttime fallback_pgid fallback_identity
        fallback_snapshot="$(bong_server_process_starttime_and_group "$fallback_pid" 2>/dev/null || true)"
        read -r fallback_starttime fallback_pgid <<< "$fallback_snapshot"
        fallback_identity="$(bong_server_process_executable_identity "$fallback_pid" 2>/dev/null || true)"
        if [ "$fallback_pgid" = "$fallback_pid" ] \
            && [[ "$fallback_starttime" =~ ^[0-9]+$ ]] \
            && [[ "$fallback_identity" =~ ^[0-9]+:[0-9]+$ ]]; then
            ACTIVE_OWNER_PID="$fallback_pid"
            ACTIVE_OWNER_STARTTIME="$fallback_starttime"
            ACTIVE_OWNER_IDENTITY="$fallback_identity"
            ACTIVE_OWNER_PGID="$fallback_pgid"
            cleanup_active_owner
        else
            printf 'WARN: refusing fallback cleanup without complete private-group authority\n' >&2
        fi
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
fixture_target="$fixture_dir/target"
cargo_pid_file="$fixture_dir/server.pid"
descendant_pid_file="$fixture_dir/descendant.pid"
build_token_args_file="$fixture_dir/build-token.args"
build_stdin_record="$fixture_dir/build-stdin.record"
build_fd_record="$fixture_dir/build-fd.record"
build_token="$fixture_dir/build-token.sh"
mkdir -p "$fixture_bin" "$fixture_server" "$fixture_target"
cat > "$fixture_bin/cargo" <<'FIXTURE'
#!/usr/bin/env bash
set -euo pipefail
[ "$#" -eq 2 ] && [ "$1" = build ] && [ "$2" = --release ] || exit 43
: "${CARGO_TARGET_DIR:?}"
printf 'fixture release build completed\n' >&2
if [ -n "${SUPERVISOR_FIXTURE_BUILD_STDIN_RECORD:-}" ]; then
    readlink -f -- /proc/self/fd/0 > "$SUPERVISOR_FIXTURE_BUILD_STDIN_RECORD"
fi
if [ -n "${SUPERVISOR_FIXTURE_EXPECT_CLOSED_FD:-}" ]; then
    if [ -e "/proc/self/fd/$SUPERVISOR_FIXTURE_EXPECT_CLOSED_FD" ]; then
        printf inherited > "${SUPERVISOR_FIXTURE_BUILD_FD_RECORD:?}"
    else
        printf closed > "${SUPERVISOR_FIXTURE_BUILD_FD_RECORD:?}"
    fi
fi
if [ -n "${SUPERVISOR_FIXTURE_BUILD_TIMEOUT_PID_FILE:-}" ]; then
    printf '%s\n' "$BASHPID" > "$SUPERVISOR_FIXTURE_BUILD_TIMEOUT_PID_FILE"
    (
        trap '' TERM
        printf '%s\n' "$BASHPID" > "${SUPERVISOR_FIXTURE_BUILD_TIMEOUT_DESCENDANT_FILE:?}"
        while :; do sleep 1; done
    ) &
    wait
fi
sleep "${SUPERVISOR_FIXTURE_BUILD_DELAY_SECONDS:-0}"
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/bong-server" <<'SERVER'
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
SERVER
chmod +x "$CARGO_TARGET_DIR/release/bong-server"
FIXTURE
chmod +x "$fixture_bin/cargo"
cat > "$build_token" <<'FIXTURE'
#!/usr/bin/env bash
set -euo pipefail
: "${SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS:?}"
printf '%s\n' "$@" > "$SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS"
[ "$#" -ge 1 ] && [ "$1" = cargo ] || exit 42
shift
exec cargo "$@"
FIXTURE
chmod +x "$build_token"

build_fixture_server() {
    local delay_seconds="${1:-0}"
    rm -f -- "$build_token_args_file"
    env \
        CARGO_TARGET_DIR="$fixture_target" \
        PATH="$fixture_bin:$PATH" \
        SUPERVISOR_FIXTURE_BUILD_DELAY_SECONDS="$delay_seconds" \
        SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$build_token_args_file" \
        "$build_token" cargo build --release
    wait_for_file "$build_token_args_file" || fail "build-token fixture did not publish argv"
    [ "$(tr '\n' ' ' < "$build_token_args_file")" = "cargo build --release " ] \
        || fail "fixture did not route exact cargo build --release argv through build-token"
    [ -x "$fixture_target/release/bong-server" ] \
        || fail "fixture build did not produce an executable server artifact"
}

start_direct_supervisor() {
    local ready_line=""
    rm -f -- "$cargo_pid_file" "$descendant_pid_file"
    build_fixture_server
    coproc DIRECT_SUPERVISOR {
        exec env \
            SUPERVISOR_FIXTURE_CARGO_PID="$cargo_pid_file" \
            SUPERVISOR_FIXTURE_DESCENDANT_PID="$descendant_pid_file" \
            python3 "$SUPERVISOR" "$fixture_server" \
                "$fixture_target/release/bong-server" \
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
    wait_for_file "$cargo_pid_file" || fail "server artifact did not publish its PID"
    wait_for_file "$descendant_pid_file" || fail "server descendant did not publish its PID"
    DIRECT_CARGO_PID="$(<"$cargo_pid_file")"
    DIRECT_DESCENDANT_PID="$(<"$descendant_pid_file")"
    assert_dev_null_fd0 "$DIRECT_CARGO_PID" "server artifact before commit"
    assert_dev_null_fd0 "$DIRECT_DESCENDANT_PID" "server descendant before commit"
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
assert_dev_null_fd0 "$DIRECT_CARGO_PID" "server artifact after commit"
assert_dev_null_fd0 "$DIRECT_DESCENDANT_PID" "server descendant after commit"
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
start = text.index("resolve_server_cargo_target() {")
end = text.index("\n}\n\nstop_server() {", start) + 2
print(text[start:end])
PY
# shellcheck source=/dev/null
source "$ROOT/scripts/lib/bong-cargo-target.sh"
source "$parent_fixture"
RUST_PATH="$fixture_bin:$PATH"
SERVER_PID=""
SERVER_PGID=""
SERVER_OWNER_STARTTIME=""
SERVER_OWNER_EXECUTABLE_IDENTITY=""
SERVER_AUTHORITY_UNCERTAIN=1
SERVER_STARTUP_CONTROL_FD=""
SERVER_STARTUP_READY_FD=""

expected_default_target="$(env -u CARGO_TARGET_DIR bash -c \
    'source "$1"; bong_scoped_cargo_target "$2"' _ \
    "$ROOT/scripts/lib/bong-cargo-target.sh" "$fixture_server")"
[ "$(env -u CARGO_TARGET_DIR bash -c \
    'source "$1"; source "$2"; resolve_server_cargo_target "$3"' _ \
    "$ROOT/scripts/lib/bong-cargo-target.sh" "$parent_fixture" "$fixture_server")" \
    = "$expected_default_target" ] \
    || fail "unset CARGO_TARGET_DIR must resolve to a checkout-scoped target"
expected_relative_target="$(CARGO_TARGET_DIR=relative-target bong_scoped_cargo_target "$fixture_server")"
[ "$(CARGO_TARGET_DIR=relative-target resolve_server_cargo_target "$fixture_server")" \
    = "$expected_relative_target" ] \
    || fail "relative CARGO_TARGET_DIR must resolve to a checkout-scoped target"

default_target_probe="$fixture_dir/default-target-probe.sh"
default_target_record="$fixture_dir/default-target.record"
cat > "$default_target_probe" <<'PROBE'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "${CARGO_TARGET_DIR:-missing}" > "${SUPERVISOR_FIXTURE_DEFAULT_TARGET_RECORD:?}"
exit 44
PROBE
chmod +x "$default_target_probe"
if (
    unset CARGO_TARGET_DIR
    SUPERVISOR_FIXTURE_DEFAULT_TARGET_RECORD="$default_target_record" \
    BONG_E2E_SUPERVISOR_TEST_MODE=1 \
    BONG_E2E_SUPERVISOR="$SUPERVISOR" \
    BONG_E2E_BUILD_TOKEN="$default_target_probe" \
    BONG_E2E_SERVER_DIRECTORY="$fixture_server" \
    start_server_process_group "$TEST_ROOT/parent-default-target.log" 0 1
); then
    fail "default-target build probe unexpectedly succeeded"
fi
[ "$(<"$default_target_record")" = "$expected_default_target" ] \
    || fail "unset CARGO_TARGET_DIR was not explicitly passed as the checkout-scoped target"

assert_parent_unpublished() {
    [ "$SERVER_AUTHORITY_UNCERTAIN" -eq 1 ] \
        || fail "failed READY transaction must retain uncertain authority"
    [ -z "$SERVER_PID" ] && [ -z "$SERVER_PGID" ] \
        && [ -z "$SERVER_OWNER_STARTTIME" ] \
        && [ -z "$SERVER_OWNER_EXECUTABLE_IDENTITY" ] \
        || fail "failed startup must not publish partial process-group authority"
    [ -z "$SERVER_STARTUP_CONTROL_FD" ] && [ -z "$SERVER_STARTUP_READY_FD" ] \
        || fail "failed startup must close and clear protocol descriptors"
}

# A failed pre-handshake build owns no server process authority. Even a
# TERM-resistant descendant is bounded inside the helper's private build group.
timeout_build_pid_file="$fixture_dir/timeout-build.pid"
timeout_build_descendant_file="$fixture_dir/timeout-build-descendant.pid"
rm -f -- "$timeout_build_pid_file" "$timeout_build_descendant_file"
SERVER_AUTHORITY_UNCERTAIN=1
if CARGO_TARGET_DIR="$fixture_target" \
SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$build_token_args_file" \
SUPERVISOR_FIXTURE_BUILD_TIMEOUT_PID_FILE="$timeout_build_pid_file" \
SUPERVISOR_FIXTURE_BUILD_TIMEOUT_DESCENDANT_FILE="$timeout_build_descendant_file" \
BONG_E2E_BUILD_TIMEOUT_SECONDS=1 \
BONG_E2E_SUPERVISOR_TEST_MODE=1 \
BONG_E2E_SUPERVISOR="$SUPERVISOR" \
BONG_E2E_BUILD_TOKEN="$build_token" \
BONG_E2E_SERVER_DIRECTORY="$fixture_server" \
start_server_process_group "$TEST_ROOT/parent-build-timeout.log" 0 1; then
    fail "parent accepted a timed-out release build"
fi
[ "$SERVER_AUTHORITY_UNCERTAIN" -eq 0 ] \
    || fail "pre-handshake build failure must retain certain empty authority"
[ -z "$SERVER_PID$SERVER_PGID$SERVER_OWNER_STARTTIME$SERVER_OWNER_EXECUTABLE_IDENTITY" ] \
    || fail "pre-handshake build failure published server authority"
wait_for_file "$timeout_build_pid_file" || fail "timeout build did not publish its PID"
wait_for_file "$timeout_build_descendant_file" \
    || fail "timeout build did not publish its descendant PID"
for stopped_pid_file in "$timeout_build_pid_file" "$timeout_build_descendant_file"; do
    stopped_pid="$(<"$stopped_pid_file")"
    for _ in $(seq 1 100); do
        if ! kill -0 "$stopped_pid" 2>/dev/null; then
            break
        fi
        stopped_state="$(ps -o stat= -p "$stopped_pid" 2>/dev/null | tr -d '[:space:]')"
        [[ "$stopped_state" = Z* ]] && break
        sleep 0.01
    done
    if kill -0 "$stopped_pid" 2>/dev/null; then
        stopped_state="$(ps -o stat= -p "$stopped_pid" 2>/dev/null | tr -d '[:space:]')"
        [[ "$stopped_state" = Z* ]] \
            || fail "timed-out build process $stopped_pid survived bounded group cleanup"
    fi
done

# A build slower than the five-second READY budget completes first into a target
# relative to a relative server override; only then does the handshake begin.
parent_target_relative=relative-target
parent_target="$fixture_server/$parent_target_relative"
relative_fixture_server="$(realpath --relative-to="$PWD" "$fixture_server")"
rm -rf -- "$parent_target"
rm -f -- "$cargo_pid_file" "$descendant_pid_file" "$build_token_args_file" \
    "$build_stdin_record" "$build_fd_record"
exec 19>"$fixture_dir/parent-private-fd"
CARGO_TARGET_DIR="$parent_target_relative" \
SUPERVISOR_FIXTURE_CARGO_PID="$cargo_pid_file" \
SUPERVISOR_FIXTURE_DESCENDANT_PID="$descendant_pid_file" \
SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$build_token_args_file" \
SUPERVISOR_FIXTURE_BUILD_STDIN_RECORD="$build_stdin_record" \
SUPERVISOR_FIXTURE_EXPECT_CLOSED_FD=19 \
SUPERVISOR_FIXTURE_BUILD_FD_RECORD="$build_fd_record" \
SUPERVISOR_FIXTURE_BUILD_DELAY_SECONDS=6 \
BONG_E2E_BUILD_TIMEOUT_SECONDS=10 \
BONG_E2E_SUPERVISOR_TEST_MODE=1 \
BONG_E2E_SUPERVISOR="$SUPERVISOR" \
BONG_E2E_BUILD_TOKEN="$build_token" \
BONG_E2E_SERVER_DIRECTORY="$relative_fixture_server" \
start_server_process_group "$TEST_ROOT/parent-normal.log" 0 1 \
    || fail "parent included a slow successful build in the READY handshake budget"
exec 19>&-
[ "$(<"$build_stdin_record")" = /dev/null ] \
    || fail "pre-handshake build stdin was not isolated to /dev/null"
[ "$(<"$build_fd_record")" = closed ] \
    || fail "pre-handshake build inherited a parent-private descriptor"
grep -Fq 'fixture release build completed' "$TEST_ROOT/parent-normal.log" \
    || fail "supervisor launch truncated the pre-handshake build diagnostics"
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
assert_dev_null_fd0 "$(<"$cargo_pid_file")" "parent server artifact after commit"
assert_dev_null_fd0 "$(<"$descendant_pid_file")" "parent server descendant after commit"
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

run_failed_parent_mode() {
    local mode="$1" after_commit_hook="${2:-}" after_ack_hook="${3:-}" owner wait_status=0
    rm -f -- "$fake_pid_file"
    if FAKE_SUPERVISOR_MODE="$mode" \
        FAKE_SUPERVISOR_PID_FILE="$fake_pid_file" \
        CARGO_TARGET_DIR="$fixture_target" \
        SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$build_token_args_file" \
        BONG_E2E_SUPERVISOR_TEST_MODE=1 \
        BONG_E2E_SUPERVISOR="$fake_supervisor" \
        BONG_E2E_BUILD_TOKEN="$build_token" \
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
CARGO_TARGET_DIR="$fixture_target" \
SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$build_token_args_file" \
BONG_E2E_SUPERVISOR_TEST_MODE=1 \
BONG_E2E_SUPERVISOR="$no_ack_supervisor" \
BONG_E2E_BUILD_TOKEN="$build_token" \
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
