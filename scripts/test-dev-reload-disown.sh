#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV_RELOAD="$ROOT/scripts/dev-reload.sh"
TEST_TMP_PARENT="${TMPDIR:-$ROOT/.sisyphus/tmp}"
mkdir -p "$TEST_TMP_PARENT"
TEST_ROOT="$(mktemp -d "$TEST_TMP_PARENT/bong-dev-reload-disown.XXXXXX")"
STUB_SCRIPT="$TEST_ROOT/server-stub.sh"
LAUNCHER_SCRIPT="$TEST_ROOT/launcher.sh"
LAUNCHER_STDIN="$TEST_ROOT/launcher.stdin"
ACTIVE_CHILD_PID=""
ACTIVE_LAUNCHER_PID=""

cleanup() {
    for pid in "$ACTIVE_CHILD_PID" "$ACTIVE_LAUNCHER_PID"; do
        if [[ "$pid" =~ ^[0-9]+$ ]]; then
            kill "$pid" 2>/dev/null || true
        fi
    done
    if [[ "$ACTIVE_LAUNCHER_PID" =~ ^[0-9]+$ ]]; then
        wait "$ACTIVE_LAUNCHER_PID" 2>/dev/null || true
    fi
    rm -rf "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

process_is_running() {
    local pid="$1"
    local state

    kill -0 "$pid" 2>/dev/null || return 1
    state="$(ps -o stat= -p "$pid" 2>/dev/null)" || return 1
    [[ "$state" != Z* ]]
}

wait_for_file() {
    local path="$1"
    local description="$2"
    local attempt

    for ((attempt = 0; attempt < 300; attempt++)); do
        if [ -s "$path" ]; then
            return 0
        fi
        sleep 0.01
    done
    fail "timed out waiting for $description"
}

wait_for_process_exit() {
    local pid="$1"
    local attempt

    for ((attempt = 0; attempt < 300; attempt++)); do
        if ! process_is_running "$pid"; then
            return 0
        fi
        sleep 0.01
    done
    return 1
}

wait_for_process_command() {
    local pid="$1"
    local expected="$2"
    local command
    local attempt

    for ((attempt = 0; attempt < 300; attempt++)); do
        command="$(ps -o comm= -p "$pid" 2>/dev/null | tr -d '[:space:]')"
        if [ "$command" = "$expected" ]; then
            return 0
        fi
        process_is_running "$pid" || return 1
        sleep 0.01
    done
    return 1
}

cat > "$STUB_SCRIPT" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [ -n "${STUB_PID_FILE:-}" ]; then
    printf '%s\n' "$$" > "$STUB_PID_FILE"
fi
printf 'ready\n' > "$READY_FILE"
exec sleep 30
STUB
chmod +x "$STUB_SCRIPT"
: > "$LAUNCHER_STDIN"

cat > "$LAUNCHER_SCRIPT" <<'LAUNCHER'
#!/usr/bin/env bash
set -euo pipefail
source "$DEV_RELOAD"

job_table_contains() {
    local expected_pid="$1"
    local job_pid

    while IFS= read -r job_pid; do
        if [ "$job_pid" = "$expected_pid" ]; then
            return 0
        fi
    done < <({ jobs -p; jobs -pr; } | sort -u)
    return 1
}

case "$TEST_MODE" in
    attached)
        "$STUB_SCRIPT" <&0 &
        child_pid=$!
        ;;
    detached)
        ENV_ARGS=()
        launch_bong_server
        child_pid="$SERVER_PID"
        if [ "$SERVER_PID" = "$DETACHED_PID" ]; then
            printf 'matched\n' > "$SERVER_PID_MATCH_FILE"
        else
            printf 'mismatch\n' > "$SERVER_PID_MATCH_FILE"
        fi
        if job_table_contains "$child_pid"; then
            printf 'present\n' > "$JOB_TABLE_STATE_FILE"
        else
            printf 'detached\n' > "$JOB_TABLE_STATE_FILE"
        fi
        ;;
    *)
        echo "FAIL: unknown launcher mode: $TEST_MODE" >&2
        exit 1
        ;;
esac

printf '%s\n' "$child_pid" > "$CHILD_PID_FILE"
printf '%s\n' "$$" > "$LAUNCHER_PID_FILE"
while :; do
    sleep 1
done
LAUNCHER
chmod +x "$LAUNCHER_SCRIPT"

run_hup_case() {
    local mode="$1"
    local expect_child_alive="$2"
    local case_root="$TEST_ROOT/$mode"
    local child_pid_file="$case_root/child.pid"
    local launcher_pid_file="$case_root/launcher.pid"
    local ready_file="$case_root/ready"
    local launcher_log="$case_root/launcher.log"
    local server_log="$case_root/server.log"
    local server_pid_match_file="$case_root/server-pid-match"
    local job_table_state_file="$case_root/job-table-state"
    local spawned_launcher_pid
    local launcher_pgid
    local child_pgid
    local child_stdin

    mkdir -p "$case_root"
    export DEV_RELOAD STUB_SCRIPT
    export TEST_MODE="$mode"
    export CHILD_PID_FILE="$child_pid_file"
    export LAUNCHER_PID_FILE="$launcher_pid_file"
    export READY_FILE="$ready_file"
    export SERVER_PID_MATCH_FILE="$server_pid_match_file"
    export JOB_TABLE_STATE_FILE="$job_table_state_file"
    export BONG_SERVER_WORKDIR="$TEST_ROOT"
    export BONG_SERVER_EXECUTABLE="$STUB_SCRIPT"
    export BONG_SERVER_LOG="$server_log"
    export BONG_SERVER_STARTUP_GRACE_SECONDS=0.05

    setsid bash "$LAUNCHER_SCRIPT" < "$LAUNCHER_STDIN" \
        > "$launcher_log" 2>&1 &
    ACTIVE_LAUNCHER_PID=$!
    spawned_launcher_pid="$ACTIVE_LAUNCHER_PID"

    wait_for_file "$child_pid_file" "$mode child pid"
    wait_for_file "$launcher_pid_file" "$mode launcher pid"
    wait_for_file "$ready_file" "$mode child readiness"
    if [ "$mode" = detached ]; then
        wait_for_file "$server_pid_match_file" "production SERVER_PID assignment"
        wait_for_file "$job_table_state_file" "production job-table detach state"
        grep -Fxq matched "$server_pid_match_file" \
            || fail "production SERVER_PID must equal launch_detached_job DETACHED_PID"
        grep -Fxq detached "$job_table_state_file" \
            || fail "production launch must remove the live server pid from Bash's job table"
    fi
    read -r ACTIVE_CHILD_PID < "$child_pid_file"
    read -r ACTIVE_LAUNCHER_PID < "$launcher_pid_file"

    [[ "$ACTIVE_CHILD_PID" =~ ^[0-9]+$ ]] || fail "$mode returned an invalid child pid"
    [[ "$ACTIVE_LAUNCHER_PID" =~ ^[0-9]+$ ]] || fail "$mode returned an invalid launcher pid"
    [ "$ACTIVE_LAUNCHER_PID" = "$spawned_launcher_pid" ] \
        || fail "$mode reported a launcher pid different from the process started by the test"
    process_is_running "$ACTIVE_CHILD_PID" || fail "$mode child exited before SIGHUP"
    process_is_running "$ACTIVE_LAUNCHER_PID" || fail "$mode launcher exited before SIGHUP"

    launcher_pgid="$(ps -o pgid= -p "$ACTIVE_LAUNCHER_PID" | tr -d '[:space:]')"
    child_pgid="$(ps -o pgid= -p "$ACTIVE_CHILD_PID" | tr -d '[:space:]')"
    [[ "$launcher_pgid" =~ ^[0-9]+$ ]] || fail "$mode launcher has invalid pgid"
    [ "$child_pgid" = "$launcher_pgid" ] \
        || fail "$mode child must begin in the launcher's HUP-exposed process group"
    wait_for_process_command "$ACTIVE_CHILD_PID" sleep \
        || fail "$mode did not exec the final sleep process behind its returned pid"
    child_stdin="$(readlink "/proc/$ACTIVE_CHILD_PID/fd/0")"
    if [ "$mode" = detached ]; then
        [ "$child_stdin" = /dev/null ] \
            || fail "detached child stdin must be /dev/null, actual $child_stdin"
    else
        [ "$child_stdin" = "$LAUNCHER_STDIN" ] \
            || fail "attached control must inherit launcher stdin, actual $child_stdin"
    fi

    # Direct kernel delivery to the shared process group models a terminal
    # foreground-group hangup. It does not consult Bash's job table.
    kill -HUP -- "-$launcher_pgid"
    wait_for_process_exit "$ACTIVE_LAUNCHER_PID" \
        || fail "$mode launcher did not exit after SIGHUP"

    if [ "$expect_child_alive" = true ]; then
        process_is_running "$ACTIVE_CHILD_PID" \
            || fail "detached child did not survive launcher SIGHUP"
        kill "$ACTIVE_CHILD_PID" 2>/dev/null || true
    else
        wait_for_process_exit "$ACTIVE_CHILD_PID" \
            || fail "attached negative-control child survived launcher SIGHUP"
    fi

    wait "$ACTIVE_LAUNCHER_PID" 2>/dev/null || true
    ACTIVE_CHILD_PID=""
    ACTIVE_LAUNCHER_PID=""
}

command -v setsid >/dev/null 2>&1 || fail "util-linux setsid command is required"

# Both children start in the launcher's process group. The negative control dies
# on direct group HUP; the production path survives via its inherited HUP ignore.
run_hup_case attached false
run_hup_case detached true

source "$DEV_RELOAD"

NO_COMMAND_LOG="$TEST_ROOT/no-command.err"
if launch_detached_job 2> "$NO_COMMAND_LOG"; then
    fail "launch accepted an empty command"
fi
grep -Fq "no background command provided" "$NO_COMMAND_LOG" \
    || fail "empty command rejection did not include its diagnostic"
[ -z "$DETACHED_PID" ] || fail "failed launch left a detached pid behind"

EMPTY_PID_LOG="$TEST_ROOT/empty-pid.err"
if detach_background_job "" 2> "$EMPTY_PID_LOG"; then
    fail "detach accepted an empty pid"
fi
grep -Fq "invalid background job pid: <empty>" "$EMPTY_PID_LOG" \
    || fail "empty pid rejection did not include its diagnostic"

NON_NUMERIC_PID_LOG="$TEST_ROOT/non-numeric-pid.err"
if detach_background_job "not-a-pid" 2> "$NON_NUMERIC_PID_LOG"; then
    fail "detach accepted a non-numeric pid"
fi
grep -Fq "invalid background job pid: not-a-pid" "$NON_NUMERIC_PID_LOG" \
    || fail "non-numeric pid rejection did not include its diagnostic"

COMPLETED_MARKER="$TEST_ROOT/completed"
COMPLETED_JOB_LOG="$TEST_ROOT/completed-job.err"
(printf 'done\n' > "$COMPLETED_MARKER") &
completed_pid=$!
wait_for_file "$COMPLETED_MARKER" "completed-job marker"
wait_for_process_exit "$completed_pid" \
    || fail "completed-job fixture did not exit naturally"

completed_job_recorded=false
while IFS= read -r job_pid; do
    if [ "$job_pid" = "$completed_pid" ]; then
        completed_job_recorded=true
        break
    fi
done < <(jobs -p)
[ "$completed_job_recorded" = true ] \
    || fail "completed-job fixture disappeared from the job table before detach"

# Deliberately do not wait: this locks the boundary where disown alone would
# accept a completed job which still has an entry in Bash's job table.
if detach_background_job "$completed_pid" 2> "$COMPLETED_JOB_LOG"; then
    fail "detach accepted a completed background job"
fi
grep -Fq "is not running in this shell" "$COMPLETED_JOB_LOG" \
    || fail "completed job rejection did not include its lifecycle diagnostic"

MISSING_SERVER_LOG="$TEST_ROOT/missing-server.err"
ENV_ARGS=()
BONG_SERVER_WORKDIR="$TEST_ROOT"
BONG_SERVER_EXECUTABLE="$TEST_ROOT/does-not-exist"
BONG_SERVER_LOG="$TEST_ROOT/missing-server.log"
BONG_SERVER_STARTUP_GRACE_SECONDS=0.05
if launch_bong_server 2> "$MISSING_SERVER_LOG"; then
    fail "production server launch accepted an exec failure"
fi
grep -Eq "is not running in this shell|exited during launch|exited during .* startup grace" "$MISSING_SERVER_LOG" \
    || fail "exec failure did not include its lifecycle diagnostic"
[ -z "$SERVER_PID" ] || fail "failed production launch left SERVER_PID=$SERVER_PID"
[ -z "$DETACHED_PID" ] || fail "failed production launch left DETACHED_PID=$DETACHED_PID"

BAD_WORKDIR_LOG="$TEST_ROOT/bad-workdir.err"
BAD_WORKDIR_READY="$TEST_ROOT/bad-workdir.ready"
READY_FILE="$BAD_WORKDIR_READY"
STUB_PID_FILE=""
BONG_SERVER_WORKDIR="$TEST_ROOT/does-not-exist"
BONG_SERVER_EXECUTABLE="$STUB_SCRIPT"
BONG_SERVER_LOG="$TEST_ROOT/bad-workdir.log"
BONG_SERVER_STARTUP_GRACE_SECONDS=0.05
if launch_bong_server 2> "$BAD_WORKDIR_LOG"; then
    fail "production server launch accepted a missing workdir"
fi
grep -Fq "could not enter bong server workdir" "$BAD_WORKDIR_LOG" \
    || fail "missing workdir rejection did not include its diagnostic"
[ ! -e "$BAD_WORKDIR_READY" ] \
    || fail "missing workdir still executed the absolute server stub"
[ -z "$SERVER_PID" ] || fail "missing workdir left SERVER_PID=$SERVER_PID"
[ -z "$DETACHED_PID" ] || fail "missing workdir left DETACHED_PID=$DETACHED_PID"

invalid_grace_case=0
for invalid_grace in not-a-duration -1; do
    ((invalid_grace_case += 1))
    INVALID_GRACE_LOG="$TEST_ROOT/invalid-grace-$invalid_grace_case.err"
    INVALID_GRACE_READY="$TEST_ROOT/invalid-grace-$invalid_grace_case.ready"
    READY_FILE="$INVALID_GRACE_READY"
    BONG_SERVER_WORKDIR="$TEST_ROOT"
    BONG_SERVER_EXECUTABLE="$STUB_SCRIPT"
    BONG_SERVER_LOG="$TEST_ROOT/invalid-grace-$invalid_grace_case.log"
    BONG_SERVER_STARTUP_GRACE_SECONDS="$invalid_grace"
    if launch_bong_server 2> "$INVALID_GRACE_LOG"; then
        fail "production server launch accepted invalid startup grace $invalid_grace"
    fi
    grep -Fq "BONG_SERVER_STARTUP_GRACE_SECONDS must be a non-negative number" "$INVALID_GRACE_LOG" \
        || fail "invalid startup grace $invalid_grace rejection lacked its diagnostic"
    [ ! -e "$INVALID_GRACE_READY" ] \
        || fail "invalid startup grace $invalid_grace started the server before validation"
    [ -z "$SERVER_PID" ] \
        || fail "invalid startup grace $invalid_grace left SERVER_PID=$SERVER_PID"
    [ -z "$DETACHED_PID" ] \
        || fail "invalid startup grace $invalid_grace left DETACHED_PID=$DETACHED_PID"
done

FAIL_SLEEP_DIR="$TEST_ROOT/fail-sleep-bin"
FAIL_SLEEP_SCRIPT="$FAIL_SLEEP_DIR/sleep"
RUNTIME_SLEEP_LOG="$TEST_ROOT/runtime-sleep.err"
RUNTIME_SLEEP_READY="$TEST_ROOT/runtime-sleep.ready"
RUNTIME_SLEEP_PID_FILE="$TEST_ROOT/runtime-sleep.pid"
mkdir -p "$FAIL_SLEEP_DIR"
cat > "$FAIL_SLEEP_SCRIPT" <<'SLEEP_STUB'
#!/usr/bin/env bash
if [ "${1:-}" = "0.05" ]; then
    for _ in $(seq 1 100); do
        [ -s "${READY_FILE:-}" ] && exit 1
        /bin/sleep 0.01
    done
    exit 1
fi
exec /bin/sleep "$@"
SLEEP_STUB
chmod +x "$FAIL_SLEEP_SCRIPT"

READY_FILE="$RUNTIME_SLEEP_READY"
export STUB_PID_FILE="$RUNTIME_SLEEP_PID_FILE"
BONG_SERVER_WORKDIR="$TEST_ROOT"
BONG_SERVER_EXECUTABLE="$STUB_SCRIPT"
BONG_SERVER_LOG="$TEST_ROOT/runtime-sleep.log"
BONG_SERVER_STARTUP_GRACE_SECONDS=0.05
ORIGINAL_PATH="$PATH"
PATH="$FAIL_SLEEP_DIR:$PATH"
if launch_bong_server 2> "$RUNTIME_SLEEP_LOG"; then
    PATH="$ORIGINAL_PATH"
    fail "production server launch accepted a runtime startup grace failure"
fi
PATH="$ORIGINAL_PATH"
grep -Fq "startup grace wait failed" "$RUNTIME_SLEEP_LOG" \
    || fail "runtime startup grace failure did not include its diagnostic"
wait_for_file "$RUNTIME_SLEEP_PID_FILE" "runtime sleep failure server pid"
read -r ACTIVE_CHILD_PID < "$RUNTIME_SLEEP_PID_FILE"
wait_for_process_exit "$ACTIVE_CHILD_PID" \
    || fail "runtime startup grace failure leaked detached server pid $ACTIVE_CHILD_PID"
[ -z "$SERVER_PID" ] || fail "runtime startup grace failure left SERVER_PID=$SERVER_PID"
[ -z "$DETACHED_PID" ] || fail "runtime startup grace failure left DETACHED_PID=$DETACHED_PID"
ACTIVE_CHILD_PID=""

echo "PASS: production launch survived SIGHUP and all initialization failures were cleaned up"
