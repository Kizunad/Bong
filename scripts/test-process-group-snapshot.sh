#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-process-group-snapshot.XXXXXX")"
ACTIVE_PID=""

cleanup() {
    for pid in "${ACTIVE_PID:-}" "${owner_pid:-}"; do
        [[ "$pid" =~ ^[0-9]+$ ]] || continue
        kill -KILL "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

# expected PGID is checked from the same /proc stat snapshot as starttime.
pidfd_fixture="$TEST_ROOT/pidfd-fixture.sh"
pidfd_marker="$TEST_ROOT/pidfd.marker"
cat > "$pidfd_fixture" <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
trap 'printf TERM > "$pidfd_marker"; exit 0' TERM
while :; do sleep 1; done
SCRIPT
chmod +x "$pidfd_fixture"
"$pidfd_fixture" &
ACTIVE_PID=$!
starttime="$(bong_server_process_starttime "$ACTIVE_PID")"
identity="$(bong_server_process_executable_identity "$ACTIVE_PID")"
pgrp="$(ps -o pgrp= -p "$ACTIVE_PID" 2>/dev/null | tr -d '[:space:]')"
if bong_server_pidfd_signal "$ACTIVE_PID" "$starttime" "$identity" TERM "$((pgrp + 1))"; then
    fail "pidfd expected-PGID mismatch must not report success"
else
    mismatch_status=$?
fi
[ "$mismatch_status" -eq 1 ] || fail "pidfd expected-PGID mismatch must return 1, got $mismatch_status"
[ ! -e "$pidfd_marker" ] || fail "pidfd expected-PGID mismatch must not deliver TERM"
kill -0 "$ACTIVE_PID" 2>/dev/null || fail "pidfd expected-PGID mismatch must preserve process"
kill -KILL "$ACTIVE_PID" 2>/dev/null || true
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

# Feed a stale member starttime snapshot to both TERM and KILL loops while its
# numeric PID names a real foreign process. pidfd must reject the stale identity.
owner_fixture="$TEST_ROOT/owner-fixture.sh"
cat > "$owner_fixture" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
while :; do sleep 1; done
SCRIPT
chmod +x "$owner_fixture"
setsid "$owner_fixture" &
owner_pid=$!
for _ in $(seq 1 100); do
    owner_pgid="$(ps -o pgid= -p "$owner_pid" 2>/dev/null | tr -d '[:space:]')"
    [ "$owner_pgid" = "$owner_pid" ] && break
    sleep 0.01
done
[ "$owner_pgid" = "$owner_pid" ] || fail "owner fixture did not become process-group leader"
owner_starttime="$(bong_server_process_starttime "$owner_pid")"
owner_identity="$(bong_server_process_executable_identity "$owner_pid")"
foreign_marker="$TEST_ROOT/foreign.marker"
foreign_fixture="$TEST_ROOT/foreign-fixture.sh"
cat > "$foreign_fixture" <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
trap 'printf TERM > "$foreign_marker"; exit 0' TERM
while :; do sleep 1; done
SCRIPT
chmod +x "$foreign_fixture"
"$foreign_fixture" &
ACTIVE_PID=$!
foreign_starttime="$(bong_server_process_starttime "$ACTIVE_PID")"
foreign_identity="$(bong_server_process_executable_identity "$ACTIVE_PID")"
stale_starttime=$((foreign_starttime + 1))

# Direct parser fixture: the final delimiter, not the first `) ` in comm, must
# determine fields. state=S, pgrp=4242 and starttime=777 are the pinned values.
stat_with_right_paren='99 (comm ) contains delimiter) S 1 4242 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 777'
[ "$(bong_server_parse_stat_starttime_and_group "$stat_with_right_paren")" = '777 4242' ] \
    || fail "stat parser must use the final comm delimiter"

original_members="$(declare -f bong_server_process_group_members)"
original_wait_children="$(declare -f bong_server_wait_for_owned_process_group_children)"
original_wait_exit="$(declare -f bong_server_wait_for_pinned_process_exit)"
original_signal="$(declare -f bong_server_pidfd_signal)"
term_status=""
kill_status=""
bong_server_process_group_members() {
    printf '%s %s %s\n' "$ACTIVE_PID" "$stale_starttime" "$foreign_identity"
}
wait_calls=0
bong_server_wait_for_owned_process_group_children() {
    wait_calls=$((wait_calls + 1))
    [ "$wait_calls" -eq 1 ] && return 1
    return 0
}
bong_server_wait_for_pinned_process_exit() { return 0; }
bong_server_pidfd_signal() {
    local signal_name="${4:-}" status
    if [ "${1:-}" = "$owner_pid" ]; then
        if [ "$signal_name" = KILL ]; then
            kill -KILL "$owner_pid" 2>/dev/null || true
            return 0
        fi
        return 1
    fi
    if python3 "$BONG_SERVER_LIFECYCLE_LIBRARY_DIR/bong-pidfd-signal.py" "$@"; then
        status=0
    else
        status=$?
    fi
    case "$signal_name" in
        TERM) term_status="$status" ;;
        KILL) kill_status="$status" ;;
    esac
    return "$status"
}
bong_server_port_is_open() { return 1; }
bong_server_stop_owned_process_group_and_release_port \
    "$owner_pid" "$owner_starttime" "$owner_identity" "$owner_pgid" 25565 \
    || fail "stale member fixture should complete through safe absence paths"
eval "$original_members"
eval "$original_wait_children"
eval "$original_wait_exit"
eval "$original_signal"
unset -f bong_server_port_is_open
[ "$term_status" -eq 1 ] || fail "stale TERM member snapshot must return 1, got $term_status"
[ "$kill_status" -eq 1 ] || fail "stale KILL member snapshot must return 1, got $kill_status"
[ ! -e "$foreign_marker" ] || fail "stale snapshots must not TERM/KILL foreign process"
kill -0 "$ACTIVE_PID" 2>/dev/null || fail "stale snapshots must preserve foreign process"

# A live member whose identity cannot be captured is uncertainty, not absence.
# Use an independent setsid fixture, not a surviving child of the killed owner.
inspection_fixture="$TEST_ROOT/inspection-fixture.sh"
cat > "$inspection_fixture" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
exec sleep 30
SCRIPT
chmod +x "$inspection_fixture"
setsid "$inspection_fixture" &
inspection_pid=$!
for _ in $(seq 1 100); do
    inspection_pgid="$(ps -o pgid= -p "$inspection_pid" 2>/dev/null | tr -d '[:space:]')"
    [ "$inspection_pgid" = "$inspection_pid" ] && break
    sleep 0.01
done
[ "$inspection_pgid" = "$inspection_pid" ] \
    || fail "inspection fixture did not become an independent group leader"
original_starttime_group="$(declare -f bong_server_process_starttime_and_group)"
bong_server_process_starttime_and_group() { return 1; }
if bong_server_process_group_members "$inspection_pgid" >/dev/null; then
    eval "$original_starttime_group"
    fail "live member inspection failure must not enumerate as success"
else
    inspection_status=$?
fi
eval "$original_starttime_group"
[ "$inspection_status" -eq 2 ] || fail "live member inspection failure must return 2, got $inspection_status"
kill -KILL "$inspection_pid" 2>/dev/null || true
wait "$inspection_pid" 2>/dev/null || true

printf 'PASS: pidfd expected-PGID and stale TERM/KILL member snapshots are fail-closed\n'
