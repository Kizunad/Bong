#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-server-lifecycle.XXXXXX")"
export BONG_SERVER_PID_FILE="$TEST_ROOT/managed.pid"
ACTIVE_PID=""

cleanup() {
    if [[ "$ACTIVE_PID" =~ ^[0-9]+$ ]]; then
        kill -KILL "$ACTIVE_PID" 2>/dev/null || true
        wait "$ACTIVE_PID" 2>/dev/null || true
    fi
    rm -rf -- "$TEST_ROOT"
}
trap cleanup EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

spawn_fixture() {
    local mode="$1"
    local marker="$2"
    local script="$TEST_ROOT/$mode.sh"

    cat > "$script" <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
trap 'printf TERM > "$marker"; exit 0' TERM
if [ "$mode" = ignore ]; then
    trap '' TERM
fi
while :; do
    sleep 1
done
SCRIPT
    chmod +x "$script"
    "$script" &
    ACTIVE_PID=$!
    bong_server_wait_for_executable "$ACTIVE_PID" "$(command -v bash)" 500 \
        || fail "$mode fixture did not remain in its TERM-aware shell"
}

term_marker="$TEST_ROOT/term.marker"
spawn_fixture graceful "$term_marker"
bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
    || fail "could not create managed PID record"
bong_server_stop_managed || fail "graceful managed stop failed"
[ -f "$term_marker" ] || fail "graceful stop did not deliver TERM"
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "graceful stop did not remove PID record"
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

kill_marker="$TEST_ROOT/kill.marker"
spawn_fixture ignore "$kill_marker"
bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
    || fail "could not record TERM-ignoring fixture"
BONG_SERVER_STOP_GRACE_SECONDS=0 bong_server_stop_managed \
    || fail "TERM-ignoring managed stop failed"
[ ! -f "$kill_marker" ] || fail "TERM-ignoring fixture unexpectedly handled TERM"
if kill -0 "$ACTIVE_PID" 2>/dev/null; then
    fail "TERM-ignoring fixture survived KILL escalation"
fi
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "KILL escalation did not remove PID record"
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

printf 'malformed\n' > "$BONG_SERVER_PID_FILE"
bong_server_stop_managed || fail "malformed record was not a safe no-op"
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "malformed record was not cleared"

sleep 30 &
foreign_pid=$!
foreign_starttime="$(bong_server_process_starttime "$foreign_pid")"
printf 'pid=%s\nstarttime=%s\nexecutable=%s\n' "$foreign_pid" "$foreign_starttime" /definitely/wrong \
    > "$BONG_SERVER_PID_FILE"
bong_server_stop_managed || fail "foreign identity record was not a safe no-op"
kill -0 "$foreign_pid" 2>/dev/null || fail "foreign PID was killed by mismatched record"
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "foreign identity record was not cleared"
kill "$foreign_pid"
wait "$foreign_pid" 2>/dev/null || true

relative_root="$TEST_ROOT/relative-target"
mkdir -p "$relative_root/target/release"
relative_executable="$relative_root/target/release/bong-server"
printf '#!/usr/bin/env bash\nexit 0\n' > "$relative_executable"
chmod +x "$relative_executable"
resolved_relative_executable="$(bong_server_resolve_executable "$relative_root" target/release/bong-server)" \
    || fail "relative executable path could not be resolved from its launch directory"
[ "$resolved_relative_executable" = "$relative_executable" ] \
    || fail "relative executable path resolved outside its launch directory"
grep -Fq 'bong_server_resolve_executable "$ROOT/server" "$CARGO_TARGET_DIR/release/bong-server"' "$ROOT/scripts/start.sh" \
    || fail "start.sh must resolve CARGO_TARGET_DIR from server before PID identity checks"

for script in scripts/dev-reload.sh scripts/start.sh scripts/stop.sh; do
    if grep -Eq "pkill[[:space:]].*(bong-server|target/debug/bong-server)" "$ROOT/$script"; then
        fail "$script must not kill bong-server by name"
    fi
done

[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "no-record stop must leave no record"
bong_server_stop_managed || fail "missing record was not a no-op"
echo "PASS: managed PID lifecycle validates identity, TERM/KILL, stale records, and no name-based server kills"
