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
if bong_server_stop_managed; then
    fail "malformed record must fail closed"
fi
[ -e "$BONG_SERVER_PID_FILE" ] || fail "malformed record must remain for operator diagnosis"
rm -f -- "$BONG_SERVER_PID_FILE"

sleep 30 &
foreign_pid=$!
foreign_starttime="$(bong_server_process_starttime "$foreign_pid")"
foreign_identity="$(stat -Lc '%d:%i' -- "$(command -v bash)")"
printf 'pid=%s\nstarttime=%s\nexecutable=%s\nexecutable_identity=%s\n' \
    "$foreign_pid" "$foreign_starttime" /definitely/wrong "$foreign_identity" > "$BONG_SERVER_PID_FILE"
if bong_server_stop_managed; then
    fail "foreign identity record must fail closed"
fi
kill -0 "$foreign_pid" 2>/dev/null || fail "foreign PID was killed by mismatched record"
[ -e "$BONG_SERVER_PID_FILE" ] || fail "foreign identity record must remain for operator diagnosis"
rm -f -- "$BONG_SERVER_PID_FILE"
kill "$foreign_pid"
wait "$foreign_pid" 2>/dev/null || true

lock_marker="$TEST_ROOT/lock-held.marker"
term_marker="$TEST_ROOT/serialized-term.marker"
spawn_fixture graceful "$term_marker"
bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
    || fail "could not record serialized stop fixture"
(
    source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
    export BONG_SERVER_PID_FILE
    hold_lifecycle_lock() {
        : > "$lock_marker"
        sleep 0.3
    }
    bong_server_with_lock hold_lifecycle_lock
) &
lock_holder_pid=$!
for _ in $(seq 1 100); do
    [ -e "$lock_marker" ] && break
    sleep 0.01
done
[ -e "$lock_marker" ] || fail "lifecycle lock holder did not acquire the shared lock"
bong_server_stop_managed &
serialized_stop_pid=$!
sleep 0.05
[ ! -e "$term_marker" ] || fail "stop signaled the managed PID before the shared lifecycle lock released"
wait "$lock_holder_pid" || fail "lifecycle lock holder failed"
wait "$serialized_stop_pid" || fail "serialized stop failed after lock release"
[ -e "$term_marker" ] || fail "serialized stop did not deliver TERM after lock release"
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "serialized stop did not clear the matching PID record"
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

replacement_root="$TEST_ROOT/replaced-executable"
mkdir -p "$replacement_root"
replacement_executable="$replacement_root/bong-server"
cp "$(command -v sleep)" "$replacement_executable"
chmod +x "$replacement_executable"
"$replacement_executable" 30 &
ACTIVE_PID=$!
bong_server_wait_for_executable "$ACTIVE_PID" "$replacement_executable" 500 \
    || fail "replacement fixture did not exec its managed image"
bong_server_write_record "$ACTIVE_PID" "$replacement_executable" \
    || fail "could not record replacement fixture"
original_identity="$(bong_server_process_executable_identity "$ACTIVE_PID")"
mv "$replacement_executable" "$replacement_executable.previous"
cp "$(command -v sleep)" "$replacement_executable"
new_identity="$(stat -Lc '%d:%i' -- "$replacement_executable")"
[ "$original_identity" != "$new_identity" ] \
    || fail "replacement fixture did not produce a distinct executable image"
bong_server_stop_managed \
    || fail "managed stop must accept the original executable image after replacement"
if kill -0 "$ACTIVE_PID" 2>/dev/null; then
    fail "replaced executable fixture survived managed TERM"
fi
[ ! -e "$BONG_SERVER_PID_FILE" ] \
    || fail "replaced executable stop did not remove its matching record"
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

tree_script="$TEST_ROOT/pane-tree.sh"
cat > "$tree_script" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
child=""
cleanup_child() {
    [ -n "$child" ] && kill "$child" 2>/dev/null || true
    [ -n "$child" ] && wait "$child" 2>/dev/null || true
    exit 0
}
trap cleanup_child TERM
bash -c 'exec -a bong-server sleep 30' &
child=$!
wait "$child"
SCRIPT
chmod +x "$tree_script"
"$tree_script" &
tree_root_pid=$!
for _ in $(seq 1 100); do
    bong_server_process_tree_has_server "$tree_root_pid" && break
    sleep 0.01
done
bong_server_process_tree_has_server "$tree_root_pid" \
    || fail "recursive pane process scan must detect a bong-server descendant"
tmux() {
    [ "$1" = list-panes ] && [ "$2" = -a ] && [ "$3" = -t ] && [ "$4" = bong ] \
        || return 1
    printf '%s\n' "$tree_root_pid"
}
bong_server_tmux_has_unmanaged_server bong \
    || fail "tmux scan must inspect a bong-server descendant in another session window"
unset -f tmux
kill -TERM "$tree_root_pid"
wait "$tree_root_pid" 2>/dev/null || true

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

grep -Fq 'tmux list-panes -a -t "$session"' "$ROOT/scripts/lib/bong-server-lifecycle.sh" \
    || fail "tmux unmanaged-server scan must include every session window"
grep -Fq 'bong_server_with_lock stop_bong_stack' "$ROOT/scripts/stop.sh" \
    || fail "stop.sh must hold the lifecycle lock through tmux teardown"

for script in scripts/dev-reload.sh scripts/start.sh scripts/stop.sh; do
    if grep -Eq "pkill[[:space:]].*(bong-server|target/debug/bong-server)" "$ROOT/$script"; then
        fail "$script must not kill bong-server by name"
    fi
done

[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "no-record stop must leave no record"
bong_server_stop_managed || fail "missing record was not a no-op"
echo "PASS: managed PID lifecycle validates identity, TERM/KILL, stale records, and no name-based server kills"
