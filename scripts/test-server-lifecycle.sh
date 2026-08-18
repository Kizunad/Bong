#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
TEST_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/bong-server-lifecycle.XXXXXX")"
export BONG_SERVER_PID_FILE="$TEST_ROOT/managed.pid"
ACTIVE_PID=""

cleanup() {
    if bong_server_validate_signal_id "$ACTIVE_PID"; then
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
if BONG_SERVER_STOP_GRACE_SECONDS=0 bong_server_stop_managed; then
    fail "TERM-ignoring managed stop must report forced shutdown"
else
    forced_stop_status=$?
fi
[ "$forced_stop_status" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "TERM-ignoring managed stop must return forced status $BONG_SERVER_STOP_FORCED, got $forced_stop_status"
[ ! -f "$kill_marker" ] || fail "TERM-ignoring fixture unexpectedly handled TERM"
if kill -0 "$ACTIVE_PID" 2>/dev/null; then
    fail "TERM-ignoring fixture survived KILL escalation"
fi
[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "forced KILL cleanup did not remove PID record"
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

# Replacement callers may continue only after identity-safe forced cleanup has
# removed both the exact process and its authority record. The degraded outcome
# stays visible as a warning; all other non-zero statuses remain fail-closed.
original_stop_managed="$(declare -f bong_server_stop_managed)"
replacement_stop_status=0
bong_server_stop_managed() { return "$replacement_stop_status"; }
replacement_stop_log="$TEST_ROOT/replacement-stop.log"
replacement_stop_status=0
bong_server_stop_managed_for_replacement "test replacement" 2> "$replacement_stop_log" \
    || fail "graceful managed stop must authorize replacement"
[ ! -s "$replacement_stop_log" ] \
    || fail "graceful replacement stop must not emit a degraded warning"
replacement_stop_status="$BONG_SERVER_STOP_FORCED"
bong_server_stop_managed_for_replacement "test replacement" 2> "$replacement_stop_log" \
    || fail "identity-safe forced cleanup must authorize replacement"
grep -Fq 'AppExit/Last persistence is unconfirmed' "$replacement_stop_log" \
    || fail "forced replacement stop must expose the unproven AppExit/Last outcome"
for replacement_stop_status in 1 2; do
    if bong_server_stop_managed_for_replacement "test replacement" 2> "$replacement_stop_log"; then
        fail "managed stop status $replacement_stop_status must not authorize replacement"
    else
        replacement_result=$?
    fi
    [ "$replacement_result" -eq "$replacement_stop_status" ] \
        || fail "replacement stop must preserve unsafe status $replacement_stop_status"
    grep -Fq "status=$replacement_stop_status" "$replacement_stop_log" \
        || fail "unsafe replacement refusal must report status $replacement_stop_status"
done
unset -f bong_server_stop_managed
eval "$original_stop_managed"

# If TERM wins the race immediately before KILL delivery, a reliable pinned
# absence is still a forced outcome; an explicit signal-helper status 1 while the
# identity remains alive is inconsistent and must fail closed as inspection error.
original_pidfd_signal="$(declare -f bong_server_pidfd_signal)"
original_wait_for_pinned_exit="$(declare -f bong_server_wait_for_pinned_process_exit)"
original_pinned_status="$(declare -f bong_server_pinned_process_status)"
pidfd_signal_calls=0
bong_server_pidfd_signal() {
    pidfd_signal_calls=$((pidfd_signal_calls + 1))
    [ "$pidfd_signal_calls" -eq 1 ] && return 0
    return 1
}
bong_server_wait_for_pinned_process_exit() {
    [ "${4:-}" -eq 0 ] && return 1
    return 0
}
bong_server_pinned_process_status() { return 1; }
if bong_server_stop_pinned_process 424242 1 1:1 0 2; then
    fail "pre-KILL process disappearance must remain a forced stop outcome"
else
    pinned_race_status=$?
fi
[ "$pinned_race_status" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "pre-KILL disappearance must return forced status"
pidfd_signal_calls=0
bong_server_pinned_process_status() { return 0; }
if bong_server_stop_pinned_process 424242 1 1:1 0 2; then
    fail "signal status 1 for a still-live pinned identity must fail closed"
else
    pinned_race_status=$?
fi
[ "$pinned_race_status" -eq 2 ] \
    || fail "inconsistent KILL absence must return inspection status 2"
unset -f bong_server_pidfd_signal bong_server_wait_for_pinned_process_exit bong_server_pinned_process_status
eval "$original_pidfd_signal"
eval "$original_wait_for_pinned_exit"
eval "$original_pinned_status"

printf 'malformed\n' > "$BONG_SERVER_PID_FILE"
if bong_server_stop_managed; then
    fail "malformed record must fail closed"
fi
[ -e "$BONG_SERVER_PID_FILE" ] || fail "malformed record must remain for operator diagnosis"
rm -f -- "$BONG_SERVER_PID_FILE"

# PID records are an authority boundary even for explicit overrides: reject
# attacker-controlled metadata before parsing, and never touch a victim.
pid_victim="$TEST_ROOT/pid-victim"
printf 'pid-victim-content\n' > "$pid_victim"
ln -s "$pid_victim" "$BONG_SERVER_PID_FILE"
if bong_server_read_record; then
    fail "PID record symlink must be rejected before parsing"
fi
[ "$(cat "$pid_victim")" = "pid-victim-content" ] || fail "PID record symlink must not alter victim"
rm -f -- "$BONG_SERVER_PID_FILE"
ln -s "$TEST_ROOT/missing-pid-record-target" "$BONG_SERVER_PID_FILE"
if bong_server_stop_managed; then
    fail "dangling PID record symlink must fail closed rather than look absent"
fi
[ -L "$BONG_SERVER_PID_FILE" ] || fail "dangling PID record symlink must remain for diagnosis"
rm -f -- "$BONG_SERVER_PID_FILE"
printf 'pid=1\nstarttime=1\nexecutable=/bin/bash\nexecutable_identity=1:1\n' > "$BONG_SERVER_PID_FILE"
chmod 644 "$BONG_SERVER_PID_FILE"
if bong_server_read_record; then
    fail "mode 0644 PID record must be rejected"
fi
chmod 600 "$BONG_SERVER_PID_FILE"
rm -f -- "$BONG_SERVER_PID_FILE"
ln "$pid_victim" "$BONG_SERVER_PID_FILE"
if bong_server_read_record; then
    fail "hardlinked PID record must be rejected"
fi
[ "$(cat "$pid_victim")" = "pid-victim-content" ] || fail "hardlinked PID record must not alter victim"
rm -f -- "$BONG_SERVER_PID_FILE"

# Override parents are authority boundaries too. A group-writable parent must
# fail closed without reading, signaling, or removing a victim path.
insecure_pid_parent="$TEST_ROOT/insecure-pid-parent"
mkdir "$insecure_pid_parent"
chmod 775 "$insecure_pid_parent"
insecure_pid_file="$insecure_pid_parent/managed.pid"
insecure_term_marker="$TEST_ROOT/insecure-parent-term.marker"
spawn_fixture graceful "$insecure_term_marker"
insecure_starttime="$(bong_server_process_starttime "$ACTIVE_PID")"
insecure_executable="$(bong_server_process_executable "$ACTIVE_PID")"
insecure_executable_identity="$(bong_server_process_executable_identity "$ACTIVE_PID")"
printf 'pid=%s\nstarttime=%s\nexecutable=%s\nexecutable_identity=%s\n' \
    "$ACTIVE_PID" "$insecure_starttime" "$insecure_executable" "$insecure_executable_identity" > "$insecure_pid_file"
if BONG_SERVER_PID_FILE="$insecure_pid_file" bong_server_read_record; then
    fail "PID record under mode 0775 override parent must be rejected"
fi
if BONG_SERVER_PID_FILE="$insecure_pid_file" bong_server_stop_managed; then
    fail "stop must reject mode 0775 override parent"
fi
[ ! -e "$insecure_term_marker" ] || fail "insecure override parent must not signal victim"
kill -0 "$ACTIVE_PID" 2>/dev/null || fail "insecure override parent stop killed victim"
[ -e "$insecure_pid_file" ] || fail "insecure override parent must not delete victim record"
kill -KILL "$ACTIVE_PID" 2>/dev/null || true
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""

# Swap immediately after the actual record open. The revalidation must reject
# open-followed symlinks before their contents are parsed or used for a signal.
printf 'pid=1\nstarttime=1\nexecutable=/bin/bash\nexecutable_identity=1:1\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
opened_record="$BONG_SERVER_PID_FILE.opened"
bong_server_after_record_open() {
    mv -- "$1" "$opened_record"
    ln -s "$pid_victim" "$1"
}
if bong_server_read_record; then
    fail "read must reject a record path swapped to symlink after open"
fi
bong_server_after_record_open() {
    :
}
[ "$(cat "$pid_victim")" = "pid-victim-content" ] || fail "open swap must not parse victim"
[ -L "$BONG_SERVER_PID_FILE" ] || fail "open swap fixture did not install symlink"
rm -f -- "$BONG_SERVER_PID_FILE" "$opened_record"

# Clear opens and validates the exact record inode itself. Swapping the path at
# the injected post-open boundary must preserve both replacement and victim.
printf 'clear-original\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
clear_identity="$(bong_server_path_identity "$BONG_SERVER_PID_FILE")"
clear_replacement="$BONG_SERVER_PID_FILE.clear-replacement"
bong_server_before_record_clear_remove() {
    mv -- "$1" "$opened_record"
    printf 'clear-replacement\n' > "$clear_replacement"
    chmod 600 "$clear_replacement"
    mv -- "$clear_replacement" "$1"
}
if bong_server_clear_record "$clear_identity"; then
    fail "clear must reject a path swapped after its verified open"
fi
bong_server_before_record_clear_remove() {
    :
}
[ "$(cat "$BONG_SERVER_PID_FILE")" = "clear-replacement" ] || fail "clear swap must preserve replacement"
[ -f "$opened_record" ] || fail "clear swap must preserve opened original"
rm -f -- "$BONG_SERVER_PID_FILE" "$opened_record"

# Clearing pins the inode observed during a validated read, so a pathname swap
# cannot make lifecycle cleanup delete a replacement/victim record.
printf 'old-record\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
pid_record_identity="$(bong_server_path_identity "$BONG_SERVER_PID_FILE")"
mv "$BONG_SERVER_PID_FILE" "$BONG_SERVER_PID_FILE.original"
printf 'replacement-record\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
if bong_server_clear_record "$pid_record_identity"; then
    fail "clear must reject a PID pathname replaced after validation"
fi
[ "$(cat "$BONG_SERVER_PID_FILE")" = "replacement-record" ] || fail "clear swap must preserve replacement record"
rm -f -- "$BONG_SERVER_PID_FILE" "$BONG_SERVER_PID_FILE.original"

# A descriptor opened before a pathname replacement must never be treated as
# the replacement record; this is the open-then-swap race boundary.
printf 'fd-original\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
exec {pid_record_fd}<"$BONG_SERVER_PID_FILE"
mv "$BONG_SERVER_PID_FILE" "$BONG_SERVER_PID_FILE.opened"
printf 'fd-replacement\n' > "$BONG_SERVER_PID_FILE"
chmod 600 "$BONG_SERVER_PID_FILE"
if bong_server_fd_matches_path "$pid_record_fd" "$BONG_SERVER_PID_FILE"; then
    fail "PID record FD/path identity check must reject an open-path swap"
fi
exec {pid_record_fd}<&-
[ "$(cat "$BONG_SERVER_PID_FILE")" = "fd-replacement" ] || fail "open-path swap must preserve replacement record"
rm -f -- "$BONG_SERVER_PID_FILE" "$BONG_SERVER_PID_FILE.opened"

# Open-descriptor validation is an authority boundary in its own right. It must
# accept only private, single-link regular files and reject every other FD kind.
fd_validation_root="$TEST_ROOT/fd-validation"
mkdir -p "$fd_validation_root"
fd_empty="$fd_validation_root/empty"
: > "$fd_empty"
chmod 600 "$fd_empty"
exec {fd_empty_handle}<"$fd_empty"
bong_server_validate_fd_secure_regular_file "$fd_empty_handle" \
    || fail "empty mode-0600 single-link regular FD must validate"
exec {fd_empty_handle}<&-

fd_nonempty="$fd_validation_root/nonempty"
printf 'fd-bytes\n' > "$fd_nonempty"
chmod 600 "$fd_nonempty"
exec {fd_nonempty_handle}<"$fd_nonempty"
bong_server_validate_fd_secure_regular_file "$fd_nonempty_handle" \
    || fail "nonempty mode-0600 single-link regular FD must validate"
exec {fd_nonempty_handle}<&-

fd_fifo="$fd_validation_root/fifo"
mkfifo -m 600 "$fd_fifo"
exec {fd_fifo_handle}<>"$fd_fifo"
if bong_server_validate_fd_secure_regular_file "$fd_fifo_handle"; then
    fail "mode-0600 FIFO FD must not validate as a regular authority file"
fi
exec {fd_fifo_handle}>&-

fd_permissive="$fd_validation_root/permissive"
: > "$fd_permissive"
chmod 644 "$fd_permissive"
exec {fd_permissive_handle}<"$fd_permissive"
if bong_server_validate_fd_secure_regular_file "$fd_permissive_handle"; then
    fail "mode-0644 regular FD must be rejected"
fi
exec {fd_permissive_handle}<&-

fd_hardlink="$fd_validation_root/hardlink"
printf 'linked\n' > "$fd_hardlink"
chmod 600 "$fd_hardlink"
ln "$fd_hardlink" "$fd_hardlink.peer"
exec {fd_hardlink_handle}<"$fd_hardlink"
if bong_server_validate_fd_secure_regular_file "$fd_hardlink_handle"; then
    fail "hardlinked regular FD must be rejected"
fi
exec {fd_hardlink_handle}<&-
if bong_server_validate_fd_secure_regular_file 999999; then
    fail "closed or invalid FD must be rejected"
fi

# The default runtime record gets the same metadata discipline as overrides.
default_pid_runtime="$TEST_ROOT/default-pid-runtime"
mkdir -p "$default_pid_runtime"
chmod 700 "$default_pid_runtime"
default_pid_file="$default_pid_runtime/bong/bong-server.pid"
mkdir -p "$(dirname "$default_pid_file")"
chmod 700 "$(dirname "$default_pid_file")"
BONG_SERVER_PID_FILE="$default_pid_file" :
printf 'pid=1\nstarttime=1\nexecutable=/bin/bash\nexecutable_identity=1:1\n' > "$default_pid_file"
chmod 644 "$default_pid_file"
BONG_SERVER_PID_FILE="$default_pid_file" bong_server_read_record && fail "default PID record mode 0644 must be rejected"
rm -f -- "$default_pid_file"

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

# A leaf process makes pgrep return 1. That is ordinary "no children", not an
# empty child record: the parent itself must still receive TERM and exit.
leaf_term_marker="$TEST_ROOT/kill-tree-leaf-term.marker"
leaf_script="$TEST_ROOT/kill-tree-leaf.sh"
cat > "$leaf_script" <<SCRIPT
#!/usr/bin/env bash
trap 'printf TERM > "$leaf_term_marker"; exit 0' TERM
while :; do
    read -rt 1 _ || true
done
SCRIPT
chmod +x "$leaf_script"
"$leaf_script" &
ACTIVE_PID=$!
leaf_pid="$ACTIVE_PID"
bong_server_kill_tree "$leaf_pid" \
    || fail "kill tree must terminate a leaf when pgrep returns status 1"
[ -e "$leaf_term_marker" ] \
    || fail "kill tree leaf status 1 must deliver TERM to the leaf itself"
if kill -0 "$leaf_pid" 2>/dev/null; then
    fail "kill tree leaf status 1 left the leaf process alive"
fi
wait "$leaf_pid" 2>/dev/null || true
ACTIVE_PID=""

# A successful pgrep with a blank child line is malformed. It must fail closed
# before the target receives any signal, unlike pgrep's status-1 empty result.
malformed_tree_term_marker="$TEST_ROOT/kill-tree-malformed-term.marker"
pgrep() { printf '\n'; return 0; }
kill() {
    if [ "${1:-}" = 424242 ]; then
        : > "$malformed_tree_term_marker"
        return 0
    fi
    command kill "$@"
}
if bong_server_kill_tree 424242; then
    unset -f pgrep kill
    fail "kill tree must reject a status-0 blank child line"
else
    malformed_tree_status=$?
fi
unset -f pgrep kill
[ "$malformed_tree_status" -eq 1 ] \
    || fail "status-0 blank child line must return failure"
[ ! -e "$malformed_tree_term_marker" ] \
    || fail "malformed child enumeration must not signal the target"


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
# pgrep infrastructure failure is tri-state 2, not evidence that this pane is
# safe to tear down. A function override models the command failing after the
# process itself was proved live.
if bong_server_process_tree_has_server "99999999"; then
    fail "a non-existent process must not contain a server"
else
    [ "$?" -eq 1 ] || fail "a provably absent process must return tree tri-state 1"
fi
# Every inspection failure on a still-live process is an unsafe tri-state 2.
ps() { return 2; }
if bong_server_process_tree_has_server "$$"; then
    fail "ps inspection failure must not be reported absent"
else
    [ "$?" -eq 2 ] || fail "ps inspection failure must return tree tri-state 2"
fi
unset -f ps
readlink() { return 2; }
if bong_server_process_tree_has_server "$$"; then
    fail "readlink inspection failure must not be reported absent"
else
    [ "$?" -eq 2 ] || fail "readlink inspection failure must return tree tri-state 2"
fi
unset -f readlink
tr() { return 2; }
if bong_server_process_tree_has_server "$$"; then
    fail "cmdline translation failure must not be reported absent"
else
    [ "$?" -eq 2 ] || fail "cmdline translation failure must return tree tri-state 2"
fi
unset -f tr
# A failed inspection followed by genuine disappearance is ordinary absence.
vanishing_tree_pid=""
readlink() {
    if [ "$1" = -f ]; then
        kill "$vanishing_tree_pid" 2>/dev/null || true
        return 2
    fi
    command readlink "$@"
}
sleep 30 &
vanishing_tree_pid=$!
if bong_server_process_tree_has_server "$vanishing_tree_pid"; then
    unset -f readlink
    fail "vanishing process must not contain a server"
else
    [ "$?" -eq 1 ] || { unset -f readlink; fail "post-failure disappearance must return tree tri-state 1"; }
fi
unset -f readlink
wait "$vanishing_tree_pid" 2>/dev/null || true

pgrep() { return 2; }
if bong_server_process_tree_has_server "$$"; then
    fail "pgrep infrastructure failure must not be reported as no bong-server"
else
    [ "$?" -eq 2 ] || fail "pgrep infrastructure failure must return process-tree tri-state 2"
fi
unset -f pgrep
# A failed descendant enumeration remains unsafe if its recheck cannot inspect
# the still-live parent. This specifically guards pgrep=2 -> ps=2.
pgrep() { return 2; }
ps() { return 2; }
if bong_server_process_tree_has_server "$$"; then
    unset -f pgrep ps
    fail "pgrep failure followed by ps failure must not be reported absent"
else
    [ "$?" -eq 2 ] || { unset -f pgrep ps; fail "pgrep=2 then ps=2 must return process-tree tri-state 2"; }
fi
unset -f pgrep ps
# Command substitution preserves pgrep status but strips a newline-only child
# list. Its synthetic empty iteration must use the same tri-state recheck.
ps_calls=0
ps() {
    ps_calls=$((ps_calls + 1))
    [ "$ps_calls" -eq 1 ] && command ps "$@" || return 2
}
pgrep() { printf '\n'; }
if bong_server_process_tree_has_server "$$"; then
    unset -f pgrep ps
    fail "empty child followed by ps failure must not be reported absent"
else
    [ "$?" -eq 2 ] || { unset -f pgrep ps; fail "empty child then ps=2 must return process-tree tri-state 2"; }
fi
unset -f pgrep ps
# Conversely a real parent exit after enumeration failure is reliable absence.
sleep 30 &
disappearing_tree_pid=$!
pgrep() {
    kill "$disappearing_tree_pid" 2>/dev/null || true
    return 2
}
if bong_server_process_tree_has_server "$disappearing_tree_pid"; then
    unset -f pgrep
    fail "a process that exits during failed enumeration must not contain a server"
else
    [ "$?" -eq 1 ] || { unset -f pgrep; fail "post-pgrep disappearance must return process-tree tri-state 1"; }
fi
unset -f pgrep
wait "$disappearing_tree_pid" 2>/dev/null || true
# Keep the original tmux propagation fixture on its explicit pgrep failure seam.
pgrep() { return 2; }
tmux() {
    if [ "$1" = list-sessions ] && [ "$2" = -F ] && [ "$3" = '#{session_name}' ]; then
        printf '%s\n' bong
        return 0
    fi
    [ "$1" = list-panes ] && [ "$2" = -s ] && [ "$3" = -t ] && [ "$4" = bong ] && [ "$5" = -F ] && [ "$6" = '#{pane_pid}' ] \
        || return 77
    printf '%s\n' "$$"
}
if bong_server_tmux_has_unmanaged_server bong; then
    fail "pgrep failure under tmux must not be reported safe"
else
    [ "$?" -eq 2 ] || fail "tmux helper must propagate process-tree tri-state 2"
fi
unset -f pgrep
# Caller contract is the start/stop explicit status-2 teardown refusal.
grep -Fq 'tmux_scan_status=$?' "$ROOT/scripts/start.sh" \
    || fail "start caller must capture tmux tri-state status"
grep -Fq '[ "$tmux_scan_status" -eq 2 ]' "$ROOT/scripts/start.sh" \
    || fail "start caller must refuse teardown for tmux tri-state 2"
grep -Fq '[ "$tmux_scan_status" -eq 2 ]' "$ROOT/scripts/stop.sh" \
    || fail "stop caller must refuse teardown for tmux tri-state 2"
tmux() {
    if [ "$1" = list-sessions ] && [ "$2" = -F ] && [ "$3" = '#{session_name}' ]; then
        printf '%s\n' bong
        return 0
    fi
    if [ "$1" = has-session ] && [ "$2" = -t ] && [ "$3" = bong ]; then
        return 0
    fi
    [ "$1" = list-panes ] && [ "$2" = -s ] && [ "$3" = -t ] && [ "$4" = bong ] && [ "$5" = -F ] && [ "$6" = '#{pane_pid}' ] \
        || return 77
    printf '%s\n%s\n' "$$" "$tree_root_pid"
}
bong_server_tmux_has_unmanaged_server bong \
    || fail "tmux scan must inspect every window pane inside the requested session"
# A server in a different session is deliberately omitted from `bong` output;
# it must not contaminate the target-session safety decision.
sleep 30 &
other_session_pane_pid=$!
tmux() {
    if [ "$1" = list-sessions ] && [ "$2" = -F ] && [ "$3" = '#{session_name}' ]; then
        printf '%s\n' bong
        return 0
    fi
    if [ "$1" = has-session ] && [ "$2" = -t ] && [ "$3" = bong ]; then
        return 0
    fi
    [ "$1" = list-panes ] && [ "$2" = -s ] && [ "$3" = -t ] && [ "$4" = bong ] && [ "$5" = -F ] && [ "$6" = '#{pane_pid}' ] \
        || return 77
    printf '%s\n' "$other_session_pane_pid"
}
if bong_server_tmux_has_unmanaged_server bong; then
    fail "a bong-server in another tmux session must not make target session scan positive"
else
    [ "$?" -eq 1 ] || fail "successful target tmux enumeration without server must return 1"
fi
# A real tmux no-server diagnostic is expected absence; an unclassified status 1
# remains an operational error rather than permission to tear down.
tmux() { printf 'no server running on /tmp/tmux-test\n' >&2; return 1; }
if bong_server_tmux_has_unmanaged_server bong; then
    fail "tmux no-server must report target absence"
else
    [ "$?" -eq 1 ] || fail "tmux no-server diagnostic must return tri-state 1"
fi
tmux() { printf 'permission denied\n' >&2; return 1; }
if bong_server_tmux_has_unmanaged_server bong; then
    fail "tmux permission failure must not be reported safe"
else
    [ "$?" -eq 2 ] || fail "tmux permission failure must return tri-state 2"
fi

# Session-list enumeration failure is also tri-state 2; it must never be
# treated as an absent target session.
tmux() { return 1; }
if bong_server_tmux_has_unmanaged_server bong; then
    fail "tmux session-list failure must not be reported safe"
else
    [ "$?" -eq 2 ] || fail "tmux session-list failure must return tri-state 2"
fi

# Enumeration failures are a distinct fail-closed state, not ordinary false.
tmux() {
    if [ "$1" = list-sessions ] && [ "$2" = -F ] && [ "$3" = '#{session_name}' ]; then
        printf '%s\n' bong
        return 0
    fi
    if [ "$1" = has-session ] && [ "$2" = -t ] && [ "$3" = bong ]; then
        return 0
    fi
    return 1
}
if bong_server_tmux_has_unmanaged_server bong; then
    fail "tmux enumeration failure must not be reported as a safe no-server result"
else
    [ "$?" -eq 2 ] || fail "tmux enumeration failure must return tri-state 2"
fi
unset -f tmux
kill "$other_session_pane_pid" 2>/dev/null || true
wait "$other_session_pane_pid" 2>/dev/null || true
kill -TERM "$tree_root_pid"
wait "$tree_root_pid" 2>/dev/null || true

# The default runtime path is a private leaf; malformed permissions and
# symlinks are rejected. Explicit PID overrides retain test compatibility.
runtime_root="$TEST_ROOT/runtime"
mkdir -p "$runtime_root"
chmod 700 "$runtime_root"
XDG_RUNTIME_DIR="$runtime_root" default_runtime="$(bong_server_runtime_dir)" \
    || fail "secure XDG runtime directory must create a private bong leaf"
[ "$(stat -Lc '%a' "$default_runtime")" = 700 ] \
    || fail "default runtime leaf must be mode 0700"
[ "$(stat -Lc '%u' "$default_runtime")" = "$UID" ] \
    || fail "default runtime leaf must be owned by current UID"
chmod 755 "$default_runtime"
if XDG_RUNTIME_DIR="$runtime_root" bong_server_runtime_dir >/dev/null; then
    fail "insecure existing runtime leaf mode must be rejected rather than repaired silently"
fi
rm -rf "$default_runtime"
mkdir -p "$default_runtime"
chmod 700 "$default_runtime"
rm -rf "$default_runtime"
ln -s /tmp "$default_runtime"
if XDG_RUNTIME_DIR="$runtime_root" bong_server_runtime_dir >/dev/null; then
    fail "symlink runtime leaf must be rejected"
fi
rm -f "$default_runtime"

lock_override="$TEST_ROOT/hardened.pid"
export BONG_SERVER_PID_FILE="$lock_override"
lock_path="${lock_override}.lock"
ln -s /dev/null "$lock_path"
if bong_server_with_lock true; then
    fail "lifecycle lock symlink must be rejected without opening/truncating it"
fi
rm -f "$lock_path"
: > "$lock_path"
chmod 644 "$lock_path"
if bong_server_with_lock true; then
    fail "lifecycle lock with unsafe mode must be rejected"
fi
chmod 600 "$lock_path"
lock_timeout_ready="$TEST_ROOT/lock-timeout-holder.ready"
(
    source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
    export BONG_SERVER_PID_FILE="$lock_override"
    hold_timeout_lock() {
        : > "$lock_timeout_ready"
        sleep 1
    }
    BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=1 bong_server_with_lock hold_timeout_lock
) &
lock_timeout_holder=$!
for _ in $(seq 1 100); do
    [ -e "$lock_timeout_ready" ] && break
    sleep 0.01
done
[ -e "$lock_timeout_ready" ] || fail "lock timeout holder did not acquire the lifecycle lock"
if BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=0 bong_server_with_lock true; then
    fail "bounded lifecycle lock with zero timeout must reject contention instead of waiting forever"
fi
wait "$lock_timeout_holder" || fail "lock timeout holder unexpectedly failed"

# Externally supplied depth/FD variables are not reentrancy authority. A forged
# depth must still contend on the real lock rather than entering its callback.
forged_lock_entered="$TEST_ROOT/forged-lock-entered"
(
    source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
    export BONG_SERVER_PID_FILE="$lock_override"
    hold_forged_lock() {
        : > "$forged_lock_entered"
        sleep 0.3
    }
    BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=1 bong_server_with_lock hold_forged_lock
) &
forged_lock_holder=$!
for _ in $(seq 1 100); do
    [ -e "$forged_lock_entered" ] && break
    sleep 0.01
done
[ -e "$forged_lock_entered" ] || fail "forged-depth lock holder did not acquire the lifecycle lock"
if BONG_SERVER_LIFECYCLE_LOCK_DEPTH=1 \
    BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=0 \
    bong_server_with_lock true; then
    wait "$forged_lock_holder" || true
    fail "forged lifecycle depth must not bypass flock contention"
fi
wait "$forged_lock_holder" || fail "forged-depth lock holder unexpectedly failed"

# A genuine same-shell nested call reuses only the verified open lock descriptor
# and therefore succeeds without deadlocking.
nested_lock_marker="$TEST_ROOT/nested-lock.marker"
nested_lock_inner() { printf nested > "$nested_lock_marker"; }
nested_lock_outer() { bong_server_with_lock nested_lock_inner; }
BONG_SERVER_PID_FILE="$lock_override" bong_server_with_lock nested_lock_outer \
    || fail "verified same-shell lifecycle lock nesting must succeed"
[ "$(cat "$nested_lock_marker")" = nested ] \
    || fail "verified nested lifecycle lock callback did not run"

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

grep -Fq 'tmux list-panes -s -t "$session"' "$ROOT/scripts/lib/bong-server-lifecycle.sh" \
    || fail "tmux unmanaged-server scan must scope every window to the requested session"
grep -Fq 'bong_server_with_lock stop_bong_stack' "$ROOT/scripts/stop.sh" \
    || fail "stop.sh must hold the lifecycle lock through tmux teardown"
grep -Fq 'tmux_scan_status=$?' "$ROOT/scripts/start.sh" \
    || fail "start.sh must explicitly reject tmux scan tri-state 2 before teardown"
grep -Fq 'tmux_scan_status=$?' "$ROOT/scripts/stop.sh" \
    || fail "stop.sh must explicitly reject tmux scan tri-state 2 before teardown"

for script in scripts/dev-reload.sh scripts/start.sh scripts/stop.sh; do
    if grep -Eq "pkill[[:space:]].*(bong-server|target/debug/bong-server)" "$ROOT/$script"; then
        fail "$script must not kill bong-server by name"
    fi
done

[ ! -e "$BONG_SERVER_PID_FILE" ] || fail "no-record stop must leave no record"
bong_server_stop_managed || fail "missing record was not a no-op"

# Unlocked persistence helpers must reject without changing developer data.
unlocked_root="$TEST_ROOT/persistence-unlocked"
unlocked_data="$unlocked_root/data"
unlocked_stash="$unlocked_root/stash"
mkdir -p "$unlocked_data"
printf 'unlocked-developer-db\n' > "$unlocked_data/bong.db"
if bong_server_stash_persistence "$unlocked_data" "$unlocked_stash"; then
    fail "stash without an active transaction must fail closed"
fi
[ "$(cat "$unlocked_data/bong.db")" = "unlocked-developer-db" ] \
    || fail "unlocked stash rejection must leave developer DB untouched"
if bong_server_restore_persistence "$unlocked_data" "$unlocked_stash"; then
    fail "restore without an active READY transaction must fail closed"
fi
[ "$(cat "$unlocked_data/bong.db")" = "unlocked-developer-db" ] \
    || fail "unlocked restore rejection must leave developer DB untouched"

transaction_state_dir() {
    bong_server_persistence_transaction_state_dir "$1" \
        || fail "could not derive secure persistence transaction state directory"
}

# Transaction lock hardening rejects symlinks, permissive modes, and a symlink
# data directory before creating/truncating a victim.
# Transaction state authority must never be created in a permissive data parent.
transaction_lock_root="$TEST_ROOT/persistence-lock-security"
transaction_lock_data="$transaction_lock_root/data"
transaction_victim="$transaction_lock_root/victim"
mkdir -p "$transaction_lock_data"
printf 'victim-content\n' > "$transaction_victim"
chmod 775 "$transaction_lock_root"
transaction_state="$(transaction_state_dir "$transaction_lock_data")"
case "$transaction_state" in
    "$transaction_lock_root"/*) fail "transaction authority must not live under data parent" ;;
esac
[ "$(stat -Lc '%a' "$transaction_state")" = 700 ] || fail "transaction state leaf must be mode 0700"
transaction_lock_path="$transaction_state/transaction.lock"
ln -s "$transaction_victim" "$transaction_lock_path"
if bong_server_persistence_transaction_begin "$transaction_lock_data"; then
    fail "transaction lock symlink must be rejected"
fi
[ "$(cat "$transaction_victim")" = "victim-content" ] \
    || fail "transaction lock symlink rejection must not truncate victim"
rm -f -- "$transaction_lock_path"
: > "$transaction_lock_path"
transaction_hardlink_victim="$transaction_lock_root/hardlink-victim"
printf 'hardlink-victim-content\n' > "$transaction_hardlink_victim"
rm -f "$transaction_lock_path"
ln "$transaction_hardlink_victim" "$transaction_lock_path"
if bong_server_persistence_transaction_begin "$transaction_lock_data"; then
    fail "transaction lock hardlink must be rejected"
fi
[ "$(cat "$transaction_hardlink_victim")" = "hardlink-victim-content" ] \
    || fail "transaction lock hardlink rejection must not alter victim content"
rm -f "$transaction_lock_path"
: > "$transaction_lock_path"
chmod 644 "$transaction_lock_path"
if bong_server_persistence_transaction_begin "$transaction_lock_data"; then
    fail "transaction lock unsafe mode must be rejected"
fi
rm -f "$transaction_lock_path"
transaction_link="$transaction_lock_root/data-link"
ln -s "$transaction_lock_data" "$transaction_link"
if bong_server_persistence_transaction_begin "$transaction_link"; then
    fail "transaction data directory symlink must be rejected"
fi

# Lifecycle lock gets the same one-link ownership rule as persistence locks.
lifecycle_hardlink_record="$TEST_ROOT/hardlink-lifecycle.pid"
lifecycle_hardlink_victim="$TEST_ROOT/hardlink-lifecycle-victim"
printf 'lifecycle-hardlink-victim\n' > "$lifecycle_hardlink_victim"
ln "$lifecycle_hardlink_victim" "$lifecycle_hardlink_record.lock"
BONG_SERVER_PID_FILE="$lifecycle_hardlink_record"
if bong_server_with_lock true; then
    fail "lifecycle lock hardlink must be rejected"
fi
[ "$(cat "$lifecycle_hardlink_victim")" = "lifecycle-hardlink-victim" ] \
    || fail "lifecycle lock hardlink rejection must preserve victim content"
unset BONG_SERVER_PID_FILE

# Persistence operations are deliberately impossible outside the transaction
# gate. These fixtures use the same begin → stash → restore → complete protocol
# as e2e, keeping direct unsafe calls available only as negative tests.
test_persistence_stash() {
    local data_dir="$1" stash_dir="$2" status
    if [ -z "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ]; then
        bong_server_persistence_transaction_begin "$data_dir" || return 1
    fi
    bong_server_stash_persistence "$data_dir" "$stash_dir"
    status=$?
    if [ "$status" -ne 0 ] && [ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -ne 1 ]; then
        bong_server_persistence_transaction_complete || bong_server_persistence_transaction_release
    fi
    return "$status"
}

test_persistence_restore() {
    local data_dir="$1" stash_dir="$2" status
    bong_server_restore_persistence "$data_dir" "$stash_dir"
    status=$?
    if [ "$status" -eq 0 ]; then
        bong_server_persistence_transaction_complete || return 1
    else
        # Negative fixtures assert the data stayed untouched; release their
        # private test holder so the next fixture cannot inherit authority.
        bong_server_persistence_transaction_release
    fi
    return "$status"
}

# --- north-rift dedicated preview server 持久化 stash/restore 回归 ---
# 优雅关服刷盘可达后，e2e-redis.sh 的 north-rift 阶段必须先把开发者本地
# server/data/bong.db{,-wal,-shm} 挪走再起专用 preview server，跑完精确
# 还原，绝不允许把专用 server 写脏的存档留给开发者，也绝不允许"精确还原"
# 语义打折（该删的没删、该拷回的没拷回）。

persistence_root="$TEST_ROOT/persistence"
data_dir="$persistence_root/data"
stash_dir="$persistence_root/stash"
mkdir -p "$data_dir"
printf 'original-db\n' > "$data_dir/bong.db"
printf 'original-wal\n' > "$data_dir/bong.db-wal"
printf 'original-shm\n' > "$data_dir/bong.db-shm"

test_persistence_stash "$data_dir" "$stash_dir" \
    || fail "stash must succeed when data_dir holds all three persistence files"
[ ! -e "$data_dir/bong.db" ] || fail "stash must remove bong.db from data_dir"
[ ! -e "$data_dir/bong.db-wal" ] || fail "stash must remove bong.db-wal from data_dir"
[ ! -e "$data_dir/bong.db-shm" ] || fail "stash must remove bong.db-shm from data_dir"
[ "$(cat "$stash_dir/bong.db")" = "original-db" ] \
    || fail "stash must preserve bong.db content byte-for-byte in stash_dir, expected 'original-db' because the source file was moved not copied-and-mutated"
[ "$(cat "$stash_dir/bong.db-wal")" = "original-wal" ] \
    || fail "stash must preserve bong.db-wal content byte-for-byte in stash_dir"
[ "$(cat "$stash_dir/bong.db-shm")" = "original-shm" ] \
    || fail "stash must preserve bong.db-shm content byte-for-byte in stash_dir"

# 模拟专用 preview server 在 data_dir 里新造了一份内容不同的 bong.db
printf 'preview-created-db\n' > "$data_dir/bong.db"

test_persistence_restore "$data_dir" "$stash_dir" \
    || fail "restore must succeed after a stash + preview-server-write cycle"
[ "$(cat "$data_dir/bong.db")" = "original-db" ] \
    || fail "restore must overwrite the preview-created bong.db with the pre-stash original because restore is authoritative, not a merge"
[ "$(cat "$data_dir/bong.db-wal")" = "original-wal" ] \
    || fail "restore must bring back bong.db-wal exactly as it was before stash"
[ "$(cat "$data_dir/bong.db-shm")" = "original-shm" ] \
    || fail "restore must bring back bong.db-shm exactly as it was before stash"
[ ! -d "$stash_dir" ] \
    || fail "restore must remove the now-empty stash_dir so a repeated cleanup-trap restore call falls into the safe no-op branch instead of re-deleting what it just restored"

# READY/STASHED is not completion evidence. Losing the stash path must retain the
# durable handoff instead of converting absence into a false successful restore.
missing_ready_root="$TEST_ROOT/persistence-ready-stash-missing"
missing_ready_data="$missing_ready_root/data"
missing_ready_stash="$missing_ready_root/stash"
mkdir -p "$missing_ready_data"
printf 'ready-stash-must-remain-recoverable\n' > "$missing_ready_data/bong.db"
test_persistence_stash "$missing_ready_data" "$missing_ready_stash" \
    || fail "READY-stash-missing fixture must create a durable stash"
missing_ready_marker="$(transaction_state_dir "$missing_ready_data")/recovery-handoff"
rm -rf -- "$missing_ready_stash"
if bong_server_persistence_transaction_complete; then
    fail "missing READY stash must not be mistaken for a completed restore"
fi
[ -f "$missing_ready_marker" ] || fail "missing READY stash must preserve recovery handoff"
grep -Fqx 'state=STASHED' "$missing_ready_marker" \
    || fail "missing READY stash must retain STASHED state for recovery"
bong_server_persistence_transaction_release

# Directory metadata must become durable before RESTORED is published. Inject a
# failure at the data-directory sync and ensure complete cannot erase STASHED.
sync_failure_root="$TEST_ROOT/persistence-restore-sync-failure"
sync_failure_data="$sync_failure_root/data"
sync_failure_stash="$sync_failure_root/stash"
mkdir -p "$sync_failure_data"
printf 'sync-failure-original\n' > "$sync_failure_data/bong.db"
test_persistence_stash "$sync_failure_data" "$sync_failure_stash" \
    || fail "restore sync-failure fixture must create a READY stash"
printf 'sync-failure-preview\n' > "$sync_failure_data/bong.db"
original_sync="$(declare -f sync 2>/dev/null || true)"
sync() {
    if [ "${1:-}" = -f ] && [ "${3:-}" = "$sync_failure_data" ]; then
        return 1
    fi
    command sync "$@"
}
if bong_server_restore_persistence "$sync_failure_data" "$sync_failure_stash"; then
    unset -f sync
    fail "restore data-directory sync failure must not report success"
fi
unset -f sync
sync_failure_marker="$(transaction_state_dir "$sync_failure_data")/recovery-handoff"
grep -Fqx 'state=STASHED' "$sync_failure_marker" \
    || fail "restore sync failure must retain STASHED recovery state"
if bong_server_persistence_transaction_complete; then
    fail "restore sync failure must not permit transaction completion"
fi
[ -f "$sync_failure_marker" ] || fail "restore sync failure must preserve recovery handoff"
bong_server_persistence_transaction_release

# A fully restored and durably synced transaction advances to RESTORED before
# complete clears its marker.
restored_state_root="$TEST_ROOT/persistence-restored-state"
restored_state_data="$restored_state_root/data"
restored_state_stash="$restored_state_root/stash"
mkdir -p "$restored_state_data"
printf 'restored-state-original\n' > "$restored_state_data/bong.db"
test_persistence_stash "$restored_state_data" "$restored_state_stash" \
    || fail "RESTORED-state fixture must create a READY stash"
printf 'restored-state-preview\n' > "$restored_state_data/bong.db"
bong_server_restore_persistence "$restored_state_data" "$restored_state_stash" \
    || fail "RESTORED-state fixture restore must succeed"
restored_state_marker="$(transaction_state_dir "$restored_state_data")/recovery-handoff"
grep -Fqx 'state=RESTORED' "$restored_state_marker" \
    || fail "successful durable restore must publish RESTORED before completion"
[ -f "$restored_state_stash/stashed-files" ] \
    || fail "RESTORED publication must precede destructive stash evidence cleanup"
[ "${BONG_SERVER_PERSISTENCE_RESTORE_DURABLE:-0}" -eq 1 ] \
    || fail "successful durable restore must set its in-memory completion guard"
bong_server_persistence_transaction_complete \
    || fail "durable RESTORED transaction must complete"
[ ! -e "$restored_state_marker" ] || fail "completed RESTORED transaction must clear handoff"
[ ! -e "$restored_state_stash" ] || fail "completed RESTORED transaction must remove stash evidence"

# Completion owns destructive evidence cleanup after RESTORED is durable. A
# transient stash-directory fsync failure must preserve RESTORED and be retryable
# even though the manifest unlink already succeeded.
cleanup_sync_root="$TEST_ROOT/persistence-restored-cleanup-sync"
cleanup_sync_data="$cleanup_sync_root/data"
cleanup_sync_stash="$cleanup_sync_root/stash"
mkdir -p "$cleanup_sync_data"
printf 'cleanup-sync-original\n' > "$cleanup_sync_data/bong.db"
test_persistence_stash "$cleanup_sync_data" "$cleanup_sync_stash" \
    || fail "RESTORED cleanup-sync fixture must create a READY stash"
printf 'cleanup-sync-preview\n' > "$cleanup_sync_data/bong.db"
bong_server_restore_persistence "$cleanup_sync_data" "$cleanup_sync_stash" \
    || fail "RESTORED cleanup-sync fixture restore must succeed"
cleanup_sync_marker="$(transaction_state_dir "$cleanup_sync_data")/recovery-handoff"
sync() {
    if [ "${1:-}" = -f ] && [ "${3:-}" = "$cleanup_sync_stash" ]; then
        return 1
    fi
    command sync "$@"
}
if bong_server_persistence_transaction_complete; then
    unset -f sync
    fail "stash cleanup sync failure must not report transaction completion"
fi
unset -f sync
grep -Fqx 'state=RESTORED' "$cleanup_sync_marker" \
    || fail "stash cleanup sync failure must preserve RESTORED handoff"
[ -d "$cleanup_sync_stash" ] \
    || fail "stash cleanup sync failure must retain the directory for retry"
[ ! -e "$cleanup_sync_stash/stashed-files" ] \
    || fail "stash cleanup sync fixture must exercise retry after manifest unlink"
bong_server_restore_persistence "$cleanup_sync_data" "$cleanup_sync_stash" \
    || fail "durable RESTORED restore retry must not require the removed manifest"
bong_server_persistence_transaction_complete \
    || fail "RESTORED completion must retry after transient stash sync failure"
[ ! -e "$cleanup_sync_marker" ] || fail "retried completion must clear RESTORED handoff"
[ ! -e "$cleanup_sync_stash" ] || fail "retried completion must remove residual stash directory"

# If rmdir succeeds but the stash-parent fsync fails, RESTORED must remain and a
# second completion must durably confirm the already-absent directory.
parent_sync_root="$TEST_ROOT/persistence-restored-parent-sync"
parent_sync_data="$parent_sync_root/data"
parent_sync_stash="$parent_sync_root/stash"
mkdir -p "$parent_sync_data"
printf 'parent-sync-original\n' > "$parent_sync_data/bong.db"
test_persistence_stash "$parent_sync_data" "$parent_sync_stash" \
    || fail "RESTORED parent-sync fixture must create a READY stash"
printf 'parent-sync-preview\n' > "$parent_sync_data/bong.db"
bong_server_restore_persistence "$parent_sync_data" "$parent_sync_stash" \
    || fail "RESTORED parent-sync fixture restore must succeed"
parent_sync_marker="$(transaction_state_dir "$parent_sync_data")/recovery-handoff"
sync() {
    if [ "${1:-}" = -f ] && [ "${3:-}" = "$parent_sync_root" ]; then
        return 1
    fi
    command sync "$@"
}
if bong_server_persistence_transaction_complete; then
    unset -f sync
    fail "stash-parent sync failure must not report transaction completion"
fi
unset -f sync
grep -Fqx 'state=RESTORED' "$parent_sync_marker" \
    || fail "stash-parent sync failure must preserve RESTORED handoff"
[ ! -e "$parent_sync_stash" ] \
    || fail "stash-parent sync fixture must exercise retry after directory removal"
bong_server_persistence_transaction_complete \
    || fail "RESTORED completion must retry the stash-parent durability sync"
[ ! -e "$parent_sync_marker" ] || fail "parent-sync retry must clear RESTORED handoff"

# V3 identity pins rely on rename preserving an inode. Before publishing READY,
# refuse a cross-device stash so mv cannot silently copy+unlink the source.
# Override the narrow device seam: this is deterministic even without another
# mount, while the ordinary three-file fixture above remains a real round-trip.
cross_device_root="$TEST_ROOT/persistence-cross-device"
cross_device_data="$cross_device_root/data"
cross_device_stash="$cross_device_root/stash"
mkdir -p "$cross_device_data"
printf 'must-not-cross-device\n' > "$cross_device_data/bong.db"
bong_server_persistence_transaction_begin "$cross_device_data" \
    || fail "cross-device fixture transaction must begin"
original_path_device="$(declare -f bong_server_path_device)"
bong_server_path_device() {
    if [ "$1" = "$cross_device_stash" ]; then
        printf '999999\n'
        return 0
    fi
    command stat -Lc '%d' -- "$1"
}
if bong_server_stash_persistence "$cross_device_data" "$cross_device_stash"; then
    eval "$original_path_device"
    fail "cross-device stash must fail before publishing READY or moving source"
fi
eval "$original_path_device"
[ "$(cat "$cross_device_data/bong.db")" = "must-not-cross-device" ] \
    || fail "cross-device preflight must leave the SQLite source untouched"
[ ! -e "$cross_device_stash/bong.db" ] \
    || fail "cross-device preflight must not move a SQLite source into stash"
[ ! -e "$cross_device_stash/stashed-files" ] \
    || fail "cross-device preflight must not publish a stash manifest"
[ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 0 ] \
    || fail "cross-device preflight must not enter READY"
bong_server_persistence_transaction_complete \
    || fail "a preflight-rejected transaction must still complete and release safely"
[ ! -e "$(transaction_state_dir "$cross_device_data")/recovery-handoff" ] \
    || fail "preflight-rejected transaction completion must clear its ACTIVE marker"

# 只有 bong.db（无 -wal/-shm）时的精确还原语义
solo_root="$TEST_ROOT/persistence-solo"
solo_data="$solo_root/data"
solo_stash="$solo_root/stash"
mkdir -p "$solo_data"
printf 'solo-original-db\n' > "$solo_data/bong.db"

test_persistence_stash "$solo_data" "$solo_stash" \
    || fail "stash must succeed when only bong.db exists (no -wal/-shm present)"
[ ! -e "$solo_data/bong.db" ] || fail "stash must remove the lone bong.db from data_dir"
[ "$(cat "$solo_stash/bong.db")" = "solo-original-db" ] \
    || fail "stash must preserve the lone bong.db content in stash_dir"
[ ! -e "$solo_stash/bong.db-wal" ] \
    || fail "stash must not fabricate a bong.db-wal in stash_dir when none ever existed in data_dir"
[ ! -e "$solo_stash/bong.db-shm" ] \
    || fail "stash must not fabricate a bong.db-shm in stash_dir when none ever existed in data_dir"

# 模拟专用 preview server 只造出了 -wal，没有触碰 bong.db 本体
printf 'preview-created-wal\n' > "$solo_data/bong.db-wal"

test_persistence_restore "$solo_data" "$solo_stash" \
    || fail "restore must succeed for the lone-bong.db stash"
[ "$(cat "$solo_data/bong.db")" = "solo-original-db" ] \
    || fail "restore must bring back the lone bong.db with its exact original content"
[ ! -e "$solo_data/bong.db-wal" ] \
    || fail "restore must delete a preview-server-created bong.db-wal that was never part of the stash, because restore is an exact-state rollback not an additive merge"
[ ! -e "$solo_data/bong.db-shm" ] \
    || fail "restore must not leave a bong.db-shm that was never part of the stash"

# 空 data_dir 的 stash 必须是成功的 no-op
empty_root="$TEST_ROOT/persistence-empty"
empty_data="$empty_root/data"
empty_stash="$empty_root/stash"
mkdir -p "$empty_data"
test_persistence_stash "$empty_data" "$empty_stash" \
    || fail "stash on an empty data_dir must succeed as a no-op instead of failing when there is nothing to move"
[ ! -e "$empty_data/bong.db" ] && [ ! -e "$empty_data/bong.db-wal" ] && [ ! -e "$empty_data/bong.db-shm" ] \
    || fail "stash on an empty data_dir must not fabricate persistence files in data_dir"
printf 'preview-created-empty-snapshot-wal\n' > "$empty_data/bong.db-wal"
test_persistence_restore "$empty_data" "$empty_stash" \
    || fail "restore of a valid empty snapshot must succeed"
[ ! -e "$empty_data/bong.db" ] && [ ! -e "$empty_data/bong.db-wal" ] && [ ! -e "$empty_data/bong.db-shm" ] \
    || fail "empty snapshot restore must remove all preview-created persistence files, including WAL"

# stash_dir 不存在时 restore 必须是成功的 no-op，且不得触碰 data_dir
missing_stash_root="$TEST_ROOT/persistence-missing-stash"
missing_stash_data="$missing_stash_root/data"
missing_stash_dir="$missing_stash_root/stash"
mkdir -p "$missing_stash_data"
printf 'untouched\n' > "$missing_stash_data/bong.db"
if test_persistence_restore "$missing_stash_data" "$missing_stash_dir"; then
    fail "restore without an active matching transaction must fail closed"
fi
[ "$(cat "$missing_stash_data/bong.db")" = "untouched" ] \
    || fail "unlocked restore rejection must leave pre-existing data_dir files untouched"

# 幂等回归：对齐 e2e-redis.sh 的真实调用模式（阶段末尾 restore 一次 +
# cleanup trap 兜底再 restore 一次），第二次调用绝不能把第一次刚还原回去
# 的文件当成"专用 server 残留"误删——否则每次成功的 e2e 跑完都会悄悄清空
# 开发者本地存档。
double_root="$TEST_ROOT/persistence-double-restore"
double_data="$double_root/data"
double_stash="$double_root/stash"
mkdir -p "$double_data"
printf 'double-original\n' > "$double_data/bong.db"
test_persistence_stash "$double_data" "$double_stash" \
    || fail "stash must succeed for the double-restore idempotency fixture"
test_persistence_restore "$double_data" "$double_stash" \
    || fail "first restore of the double-restore fixture must succeed"
if test_persistence_restore "$double_data" "$double_stash"; then
    fail "post-completion restore must not bypass the transaction gate"
fi
[ "$(cat "$double_data/bong.db")" = "double-original" ] \
    || fail "post-completion restore rejection must not delete the restored bong.db"

# 部分失败后重试回归（真实复现过的 blocker）：manifest 记录了 bong.db 被
# stash 过；把 bong.db-wal 造成一个目录，让针对它的 rm -f 中途报错，逼
# restore 在"已经正确 mv 回 bong.db"之后、"清理 bong.db-wal"之前失败退出。
# cleanup trap 会无条件再调用一次 restore 兜底（被 || true 吞掉返回值）——
# 旧实现在这第二次调用里会把 stash 里已经不在的 bong.db 误判成"从来没
# 备份过的 preview 垃圾"直接删掉，把刚刚正确还原的真实存档吞掉。新实现
# 靠清单判定"这个文件本来就该在"，第二次调用发现 data_dir 里已经有它就
# 原样保留。
partial_root="$TEST_ROOT/persistence-partial-failure"
partial_data="$partial_root/data"
partial_stash="$partial_root/stash"
mkdir -p "$partial_data"
printf 'real-developer-db\n' > "$partial_data/bong.db"

test_persistence_stash "$partial_data" "$partial_stash" \
    || fail "stash must succeed before the partial-failure fixture forces a mid-loop restore failure"

mkdir -p "$partial_data/bong.db-wal"

if bong_server_restore_persistence "$partial_data" "$partial_stash"; then
    fail "first restore must fail when a non-stashed file cannot be removed (bong.db-wal is a directory here), otherwise the mid-loop rm -f error is being silently swallowed"
fi
[ ! -e "$partial_data/bong.db" ] && [ -e "$partial_stash/bong.db" ] \
    || fail "restore preflight must reject unsafe preview WAL before moving any original DB out of stash"

rm -rf -- "$partial_data/bong.db-wal"
test_persistence_restore "$partial_data" "$partial_stash" \
    || fail "retry after unsafe preview WAL removal must restore the original DB"
[ "$(cat "$partial_data/bong.db")" = "real-developer-db" ] \
    || fail "preflight-failed restore retry must recover the exact original bong.db"

# 清单缺失时必须 fail closed：一个文件都不许删，宁可留残留交人工排查。
manifest_missing_root="$TEST_ROOT/persistence-manifest-missing"
manifest_missing_data="$manifest_missing_root/data"
manifest_missing_stash="$manifest_missing_root/stash"
mkdir -p "$manifest_missing_data"
printf 'stashed-db\n' > "$manifest_missing_data/bong.db"

test_persistence_stash "$manifest_missing_data" "$manifest_missing_stash" \
    || fail "stash must succeed for the manifest-missing fixture"
rm -f -- "$manifest_missing_stash/stashed-files"

printf 'unrelated-file-must-survive\n' > "$manifest_missing_data/unrelated.txt"

if test_persistence_restore "$manifest_missing_data" "$manifest_missing_stash"; then
    fail "restore must fail closed when the stash manifest is missing/unreadable, not silently guess which files were stashed"
fi
[ "$(cat "$manifest_missing_data/unrelated.txt")" = "unrelated-file-must-survive" ] \
    || fail "manifest-missing fail-closed restore must not touch any existing file in data_dir, expected 'unrelated.txt' untouched"
[ -e "$manifest_missing_stash/bong.db" ] \
    || fail "manifest-missing fail-closed restore must leave stash_dir contents alone for manual recovery"

# Strict parsing is defensive: malformed/duplicate manifests are not empty
# snapshots. Restore must fail before deleting preview-created data.
malformed_root="$TEST_ROOT/persistence-malformed-manifest"
malformed_data="$malformed_root/data"
malformed_stash="$malformed_root/stash"
mkdir -p "$malformed_data" "$malformed_stash"
printf 'preview-db-must-survive-invalid-manifest\n' > "$malformed_data/bong.db"
printf '%s\n%s 1:1 bong.db\n%s 1:1 bong.db\n' "$BONG_SERVER_STASH_MANIFEST_HEADER" "$(printf 'a%.0s' {1..64})" "$(printf 'a%.0s' {1..64})" > "$malformed_stash/stashed-files"
if test_persistence_restore "$malformed_data" "$malformed_stash"; then
    fail "restore must reject duplicate manifest records rather than treating them as an empty snapshot"
fi
[ "$(cat "$malformed_data/bong.db")" = "preview-db-must-survive-invalid-manifest" ] \
    || fail "invalid manifest restore must not delete any persistence file before validation completes"

# 新 transaction 必须独占创建 stash leaf。陈旧目录（无论其中是否已有
# manifest）均不得触发 move，否则旧恢复证据会被本次 transaction 覆盖。
stale_root="$TEST_ROOT/persistence-stale-stash"
stale_data="$stale_root/data"
stale_stash="$stale_root/stash"
mkdir -p "$stale_data" "$stale_stash"
printf 'developer-db-must-not-move\n' > "$stale_data/bong.db"
printf 'stale evidence\n' > "$stale_stash/stashed-files"
if test_persistence_stash "$stale_data" "$stale_stash"; then
    fail "stash must reject an already-existing stash leaf/manifest before moving developer data"
fi
[ "$(cat "$stale_data/bong.db")" = "developer-db-must-not-move" ] \
    || fail "stale stash rejection must leave original bong.db in data_dir before any move"

# 发布 manifest 失败必须发生在首个 move 之前。覆盖 sync 只在这个 unit
# fixture 注入受控发布故障，契约是原始文件仍在而不是实现的具体行号。
publish_failure_root="$TEST_ROOT/persistence-publish-failure"
publish_failure_data="$publish_failure_root/data"
publish_failure_stash="$publish_failure_root/stash"
mkdir -p "$publish_failure_data"
printf 'must-remain-before-manifest-publish\n' > "$publish_failure_data/bong.db"
sync() { return 1; }
if test_persistence_stash "$publish_failure_data" "$publish_failure_stash"; then
    unset -f sync
    fail "stash must fail when strict manifest publish cannot be durably completed"
fi
unset -f sync
[ "$(cat "$publish_failure_data/bong.db")" = "must-remain-before-manifest-publish" ] \
    || fail "manifest publish failure must cause zero database moves; original bong.db must remain"
[ ! -e "$publish_failure_stash/bong.db" ] \
    || fail "manifest publish failure must not leave a moved bong.db in stash"

# 若 move 在发布 manifest 后中途失败，已发布的 manifest 必须能让 restore
# 安全回滚；重复 restore 也不得删除已经还原的开发者快照。
move_failure_root="$TEST_ROOT/persistence-move-failure"
move_failure_data="$move_failure_root/data"
move_failure_stash="$move_failure_root/stash"
mkdir -p "$move_failure_data"
printf 'move-db\n' > "$move_failure_data/bong.db"
printf 'move-wal\n' > "$move_failure_data/bong.db-wal"
printf 'move-shm\n' > "$move_failure_data/bong.db-shm"
mv() {
    # Marker/manifest publication also uses atomic mv; fail only the actual
    # WAL persistence move so the marker is already durable.
    if [ "${2:-}" = "$move_failure_data/bong.db-wal" ]; then
        return 1
    fi
    command mv "$@"
}
if test_persistence_stash "$move_failure_data" "$move_failure_stash"; then
    unset -f mv
    fail "stash must report a mid-move failure instead of pretending the transaction completed"
fi
unset -f mv
test_persistence_restore "$move_failure_data" "$move_failure_stash" \
    || fail "published manifest must restore safely after a mid-move failure"
[ "$(cat "$move_failure_data/bong.db")" = "move-db" ] \
    || fail "mid-move rollback must recover bong.db byte-for-byte"
[ "$(cat "$move_failure_data/bong.db-wal")" = "move-wal" ] \
    || fail "mid-move rollback must preserve WAL left in data_dir"
[ "$(cat "$move_failure_data/bong.db-shm")" = "move-shm" ] \
    || fail "mid-move rollback must preserve SHM left in data_dir"
if test_persistence_restore "$move_failure_data" "$move_failure_stash"; then
    fail "post-completion restore must not bypass the transaction gate"
fi
[ "$(cat "$move_failure_data/bong.db")" = "move-db" ] \
    || fail "post-completion restore rejection must not delete the rollback-recovered bong.db"

# Digest pinning rejects preview content masquerading as a partial restore.
digest_root="$TEST_ROOT/persistence-digest-mismatch"
digest_data="$digest_root/data"
digest_stash="$digest_root/stash"
mkdir -p "$digest_data"
printf 'original-digest-pinned\n' > "$digest_data/bong.db"
test_persistence_stash "$digest_data" "$digest_stash" \
    || fail "digest mismatch fixture stash must succeed"
rm -f -- "$digest_stash/bong.db"
printf 'preview-content-with-different-digest\n' > "$digest_data/bong.db"
if test_persistence_restore "$digest_data" "$digest_stash"; then
    fail "missing stash original plus different data digest must fail closed"
fi
[ "$(cat "$digest_data/bong.db")" = "preview-content-with-different-digest" ] \
    || fail "digest mismatch must not delete or overwrite preview data"

# A true partial restore has the manifest's exact digest already in data_dir
# and must be retry-safe even after its stash copy is gone.
partial_digest_root="$TEST_ROOT/persistence-partial-digest"
partial_digest_data="$partial_digest_root/data"
partial_digest_stash="$partial_digest_root/stash"
mkdir -p "$partial_digest_data"
printf 'same-digest-partial-restore\n' > "$partial_digest_data/bong.db"
test_persistence_stash "$partial_digest_data" "$partial_digest_stash" \
    || fail "partial digest fixture stash must succeed"
mv "$partial_digest_stash/bong.db" "$partial_digest_data/bong.db"
test_persistence_restore "$partial_digest_data" "$partial_digest_stash" \
    || fail "same-digest partial restore must succeed"
[ "$(cat "$partial_digest_data/bong.db")" = "same-digest-partial-restore" ] \
    || fail "same-digest partial restore must preserve original bytes"

# Restore rejects unexpected stash evidence before deleting manifest or target
# files, leaving both developer data and operator evidence intact.
unexpected_root="$TEST_ROOT/persistence-unexpected-stash"
unexpected_data="$unexpected_root/data"
unexpected_stash="$unexpected_root/stash"
mkdir -p "$unexpected_data"
printf 'unexpected-entry-original\n' > "$unexpected_data/bong.db"
test_persistence_stash "$unexpected_data" "$unexpected_stash" \
    || fail "unexpected-entry fixture stash must succeed"
printf 'operator evidence\n' > "$unexpected_stash/not-a-db-entry"
printf 'preview-must-survive\n' > "$unexpected_data/bong.db"
if test_persistence_restore "$unexpected_data" "$unexpected_stash"; then
    fail "unexpected stash entry must fail closed before restore mutation"
fi
[ "$(cat "$unexpected_data/bong.db")" = "preview-must-survive" ] \
    || fail "unexpected stash entry failure must not mutate preview data"
[ -f "$unexpected_stash/stashed-files" ] && [ -f "$unexpected_stash/not-a-db-entry" ] \
    || fail "unexpected stash entry failure must preserve manifest and evidence"

# V3 pins the original source inode. A same-content file swapped in after
# snapshot capture must fail stash post-check and leave the durable marker.
stash_swap_root="$TEST_ROOT/persistence-stash-path-swap"
stash_swap_data="$stash_swap_root/data"
stash_swap_dir="$stash_swap_root/stash"
mkdir -p "$stash_swap_data"
printf 'stash-original\n' > "$stash_swap_data/bong.db"
original_mv_command="$(command -v mv)"
mv() {
    local source="${@: -2:1}" destination="${@: -1}"
    if [ "$source" = "$stash_swap_data/bong.db" ] && [ "$destination" = "$stash_swap_dir/bong.db" ]; then
        command mv "$@" || return 1
        printf 'stash-original\n' > "$stash_swap_dir/bong.db.attacker"
        command mv -f "$stash_swap_dir/bong.db.attacker" "$stash_swap_dir/bong.db"
        return 0
    fi
    command mv "$@"
}
if test_persistence_stash "$stash_swap_data" "$stash_swap_dir"; then
    unset -f mv
    fail "stash path swap must fail V3 post-move identity check"
fi
unset -f mv
[ -f "$stash_swap_dir/stashed-files" ] \
    || fail "stash path swap failure must retain manifest evidence"
[ -f "$(transaction_state_dir "$stash_swap_data")/recovery-handoff" ] \
    || fail "stash path swap failure must retain durable transaction marker"
bong_server_persistence_transaction_release

# Restore preflight pins the stash inode and rechecks immediately before mv.
# A same-content attacker replacement is rejected and the transaction cannot
# complete or erase its marker/manifest.
restore_swap_root="$TEST_ROOT/persistence-restore-path-swap"
restore_swap_data="$restore_swap_root/data"
restore_swap_dir="$restore_swap_root/stash"
mkdir -p "$restore_swap_data"
printf 'restore-original\n' > "$restore_swap_data/bong.db"
test_persistence_stash "$restore_swap_data" "$restore_swap_dir" \
    || fail "restore path-swap fixture stash must succeed"
printf 'preview-content\n' > "$restore_swap_data/bong.db"
mv() {
    local source="${@: -2:1}" destination="${@: -1}"
    if [ "$source" = "$restore_swap_dir/bong.db" ] && [ "$destination" = "$restore_swap_data/bong.db" ]; then
        command mv "$restore_swap_dir/bong.db" "$restore_swap_dir/bong.db.original" || return 1
        printf 'restore-original\n' > "$restore_swap_dir/bong.db"
        command mv -f "$restore_swap_dir/bong.db" "$restore_swap_data/bong.db"
        return 0
    fi
    command mv "$@"
}
if bong_server_restore_persistence "$restore_swap_data" "$restore_swap_dir"; then
    unset -f mv
    fail "restore path swap must not report success"
fi
unset -f mv
[ -f "$restore_swap_dir/stashed-files" ] \
    || fail "restore path swap failure must retain V3 manifest"
[ -f "$(transaction_state_dir "$restore_swap_data")/recovery-handoff" ] \
    || fail "restore path swap failure must retain durable marker"
if bong_server_persistence_transaction_complete; then
    fail "failed restore transaction must not complete and erase recovery marker"
fi
[ "$(cat "$restore_swap_data/bong.db")" = "restore-original" ] \
    || fail "attacker move must be detected rather than reported as a successful original restore"
bong_server_persistence_transaction_release

# A durable marker is created only after manifest publish and before moves;
# marker publication failure is therefore zero-move and stale-leaf failures do
# not gain READY recovery state.
marker_root="$TEST_ROOT/persistence-marker-order"
marker_data="$marker_root/data"
marker_stash="$marker_root/stash"
mkdir -p "$marker_data"
printf 'marker-before-move\n' > "$marker_data/bong.db"
bong_server_persistence_transaction_begin "$marker_data" \
    || fail "marker-order transaction must begin"
sync() { return 1; }
if test_persistence_stash "$marker_data" "$marker_stash"; then
    unset -f sync
    fail "marker publish failure must reject stash before any move"
fi
unset -f sync
[ "$(cat "$marker_data/bong.db")" = "marker-before-move" ] \
    || fail "marker publication failure must leave source DB unmoved"
[ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 0 ] \
    || fail "pre-manifest failure must not mark stash READY"
# The helper's pre-manifest failure cleanup releases the private holder without
# restoring the new/stale leaf; it must not leave transaction authority behind.
[ -z "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] \
    || fail "pre-manifest failure cleanup must release the transaction holder"

marker_move_root="$TEST_ROOT/persistence-marker-mid-move"
marker_move_data="$marker_move_root/data"
marker_move_stash="$marker_move_root/stash"
mkdir -p "$marker_move_data"
printf 'marker-move-db\n' > "$marker_move_data/bong.db"
printf 'marker-move-wal\n' > "$marker_move_data/bong.db-wal"
bong_server_persistence_transaction_begin "$marker_move_data" \
    || fail "mid-move marker transaction must begin"
mv() {
    if [ "${2:-}" = "$marker_move_data/bong.db-wal" ]; then return 1; fi
    command mv "$@"
}
if test_persistence_stash "$marker_move_data" "$marker_move_stash"; then
    unset -f mv
    fail "mid-move fixture must fail after durable marker publication"
fi
unset -f mv
[ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 1 ] \
    || fail "mid-move failure must leave READY set because marker already names stash path"
marker_move_state="$(transaction_state_dir "$marker_move_data")"
grep -Fq "stash_dir=$marker_move_stash" "$marker_move_state/recovery-handoff" \
    || fail "durable marker must name the stash path before first move"
test_persistence_restore "$marker_move_data" "$marker_move_stash" \
    || fail "mid-move transaction must restore from marker-backed V3 manifest"
[ ! -e "$marker_move_state/recovery-handoff" ] \
    || fail "successful marker-backed restore must clear durable handoff"

# An unconfirmed preview shutdown must never restore SQLite state. It records
# both the recoverable stash path and reason, and relinquishes the lock only.
stop_failure_root="$TEST_ROOT/persistence-stop-failure"
stop_failure_data="$stop_failure_root/data"
stop_failure_stash="$stop_failure_root/stash"
mkdir -p "$stop_failure_data"
printf 'developer-data-must-not-be-restored\n' > "$stop_failure_data/bong.db"
test_persistence_stash "$stop_failure_data" "$stop_failure_stash" \
    || fail "stop-failure fixture must create a READY transaction stash"
if bong_server_persistence_transaction_abort_unconfirmed_preview_stop \
    "$stop_failure_data" "$stop_failure_stash" \
    "preview server stop was not confirmed; restore forbidden; stash retained at $stop_failure_stash"; then
    :
else
    fail "unconfirmed preview stop must record recoverable failed handoff"
fi
[ ! -e "$stop_failure_data/bong.db" ] \
    || fail "unconfirmed stop must not restore developer SQLite data"
[ "$(cat "$stop_failure_stash/bong.db")" = "developer-data-must-not-be-restored" ] \
    || fail "unconfirmed stop must retain stash bytes unchanged"
stop_failure_marker="$(transaction_state_dir "$stop_failure_data")/recovery-handoff"
grep -Fqx 'state=FAILED' "$stop_failure_marker" \
    || fail "unconfirmed stop marker must record FAILED state"
grep -Fqx "stash_dir=$stop_failure_stash" "$stop_failure_marker" \
    || fail "unconfirmed stop marker must retain stash path"
grep -Fq 'reason=preview server stop was not confirmed; restore forbidden' "$stop_failure_marker" \
    || fail "unconfirmed stop marker must retain shutdown failure reason"
flock -n "$(transaction_state_dir "$stop_failure_data")/transaction.lock" -c true \
    || fail "unconfirmed stop must release transaction lock after durable marker"

# Stable-parent lock prevents split brain if the data directory is renamed and
# recreated. The original holder refuses to restore into the replacement and
# leaves its parent-level marker plus stash evidence for manual recovery.
rename_root="$TEST_ROOT/persistence-rename"
rename_data="$rename_root/data"
rename_stash="$rename_root/stash"
mkdir -p "$rename_data"
printf 'original-before-rename\n' > "$rename_data/bong.db"
rename_original_identity="$(bong_server_path_identity "$rename_data")"
rename_state="$(transaction_state_dir "$rename_data")"
test_persistence_stash "$rename_data" "$rename_stash" \
    || fail "rename fixture must create a marker-backed stash"
mv "$rename_data" "$rename_root/data-original"
mkdir -p "$rename_data"
printf 'replacement-must-survive\n' > "$rename_data/bong.db"
if env -u BONG_SERVER_PERSISTENCE_LOCK_FD \
    -u BONG_SERVER_PERSISTENCE_DATA_DIR \
    -u BONG_SERVER_PERSISTENCE_PARENT_DIR \
    -u BONG_SERVER_PERSISTENCE_DATA_IDENTITY \
    -u BONG_SERVER_PERSISTENCE_PARENT_IDENTITY \
    -u BONG_SERVER_PERSISTENCE_MARKER_FILE \
    -u BONG_SERVER_PERSISTENCE_STASH_READY \
    -u BONG_SERVER_PERSISTENCE_STASH_DIR \
    bash -c 'source "$1"; bong_server_persistence_transaction_begin "$2"' bash \
    "$ROOT/scripts/lib/bong-server-lifecycle.sh" "$rename_data"; then
    fail "replacement data directory must still contend on stable parent transaction lock"
fi
if bong_server_restore_persistence "$rename_data" "$rename_stash"; then
    fail "original transaction must reject restore after data directory identity changes"
fi
[ "$(cat "$rename_data/bong.db")" = "replacement-must-survive" ] \
    || fail "identity-change restore rejection must not overwrite replacement data"
[ -f "$rename_state/recovery-handoff" ] \
    || fail "identity-change rejection must retain secure runtime handoff marker"
grep -Fq "stash_dir=$rename_stash" "$rename_state/recovery-handoff" \
    || fail "runtime marker must retain original stash path"
grep -Fqx "data_identity=$rename_original_identity" "$rename_state/recovery-handoff" \
    || fail "runtime marker must retain original data directory identity"
bong_server_persistence_transaction_release

rename_link_root="$TEST_ROOT/persistence-rename-symlink"
rename_link_data="$rename_link_root/data"
rename_link_stash="$rename_link_root/stash"
mkdir -p "$rename_link_data"
printf 'original-before-symlink\n' > "$rename_link_data/bong.db"
rename_link_original_identity="$(bong_server_path_identity "$rename_link_data")"
rename_link_state="$(transaction_state_dir "$rename_link_data")"
test_persistence_stash "$rename_link_data" "$rename_link_stash" \
    || fail "symlink replacement fixture must create a marker-backed stash"
mv "$rename_link_data" "$rename_link_root/data-original"
mkdir -p "$rename_link_root/replacement"
printf 'symlink-replacement-must-survive\n' > "$rename_link_root/replacement/bong.db"
ln -s "$rename_link_root/replacement" "$rename_link_data"
if bong_server_restore_persistence "$rename_link_data" "$rename_link_stash"; then
    fail "original transaction must reject symlink data replacement"
fi
[ "$(cat "$rename_link_root/replacement/bong.db")" = "symlink-replacement-must-survive" ] \
    || fail "symlink replacement rejection must not mutate replacement data"
[ -f "$rename_link_state/recovery-handoff" ] \
    || fail "symlink identity rejection must retain secure runtime marker"
grep -Fqx "data_identity=$rename_link_original_identity" "$rename_link_state/recovery-handoff" \
    || fail "symlink rejection marker must retain original data directory identity"
bong_server_persistence_transaction_release

# Cross-run protection: a second shell cannot enter while the first owns the
# whole transaction, and an abandoned/failed transaction leaves a durable
# marker that the next shell must explicitly fail closed on.
transaction_root="$TEST_ROOT/persistence-transaction"
transaction_data="$transaction_root/data"
mkdir -p "$transaction_data"
printf 'developer-db-lock-fixture\n' > "$transaction_data/bong.db"
bong_server_persistence_transaction_begin "$transaction_data" \
    || fail "first persistence transaction must acquire its lock"
if (
    source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
    bong_server_persistence_transaction_begin "$transaction_data"
); then
    fail "second concurrent persistence transaction must be rejected without busy waiting"
fi
[ "$(cat "$transaction_data/bong.db")" = "developer-db-lock-fixture" ] \
    || fail "lock contention must not move or delete developer bong.db"
bong_server_persistence_transaction_mark_failed "fixture simulates restore failure" \
    || fail "failed restore fixture must write durable handoff marker"
bong_server_persistence_transaction_release
if bong_server_persistence_transaction_begin "$transaction_data"; then
    fail "durable failed-restore handoff marker must make the next transaction fail closed"
fi
transaction_state="$(transaction_state_dir "$transaction_data")"
[ -f "$transaction_state/recovery-handoff" ] \
    || fail "failed restore must leave a durable handoff marker with recovery evidence"
# An operator has resolved the handoff; verify normal successful completion
# removes the marker before releasing the lock.
rm -f -- "$transaction_state/recovery-handoff"
bong_server_persistence_transaction_begin "$transaction_data" \
    || fail "transaction must resume only after explicit operator handoff removal"
bong_server_persistence_transaction_complete \
    || fail "successful transaction completion must clear its durable handoff"
[ ! -e "$transaction_state/recovery-handoff" ] \
    || fail "successful restore path must remove the durable handoff marker"

restore_occurrences="$(grep -c 'bong_server_restore_persistence' "$ROOT/scripts/e2e-redis.sh" || true)"
grep -Fq 'bong_server_stash_persistence' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e-redis.sh north-rift stage must call test_persistence_stash before starting the dedicated preview server"
grep -Fq 'bong_server_finalize_preview_persistence_after_stop' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e cleanup must delegate stop-confirmed restore/abort decisions to the lifecycle helper"
grep -Fq 'bong_server_restore_persistence' "$ROOT/scripts/lib/bong-server-lifecycle.sh" \
    || fail "lifecycle helper must restore only on the explicit stop-confirmed path"
grep -Fq 'bong_server_abort_unconfirmed_preview_stop' "$ROOT/scripts/lib/bong-server-lifecycle.sh" \
    || fail "lifecycle helper must take the no-restore durable handoff path when stop_server fails"
if grep -Fq 'stop_server || true' "$ROOT/scripts/e2e-redis.sh"; then
    fail "e2e cleanup must not ignore stop_server failure before persistence restore"
fi
grep -Fq 'source "$ROOT/scripts/lib/bong-server-lifecycle.sh"' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e-redis.sh must source bong-server-lifecycle.sh to reach the stash/restore helpers"


# Managed lifecycle status is deliberately three-state. A live process whose
# metadata cannot be inspected is never evidence that its record is stale.
export BONG_SERVER_PID_FILE="$TEST_ROOT/managed.pid"
original_process_starttime="$(declare -f bong_server_process_starttime)"
original_process_executable_identity="$(declare -f bong_server_process_executable_identity)"
assert_inspection_failure_preserves_record() {
    local seam="$1" marker="$TEST_ROOT/inspection-${1}.marker" stop_status

    spawn_fixture graceful "$marker"
    bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
        || fail "$seam fixture could not create a managed record"
    case "$seam" in
        ps) ps() { return 2; } ;;
        starttime) bong_server_process_starttime() { return 2; } ;;
        executable_identity) bong_server_process_executable_identity() { return 2; } ;;
        *) fail "unknown inspection seam $seam" ;;
    esac
    if bong_server_stop_managed; then
        case "$seam" in
        ps) unset -f ps ;;
        starttime) eval "$original_process_starttime" ;;
        executable_identity) eval "$original_process_executable_identity" ;;
    esac
        fail "$seam inspection failure must fail closed"
    else
        stop_status=$?
    fi
    case "$seam" in
        ps) unset -f ps ;;
        starttime) eval "$original_process_starttime" ;;
        executable_identity) eval "$original_process_executable_identity" ;;
    esac
    [ "$stop_status" -eq 2 ] || fail "$seam inspection failure must return status 2, got $stop_status"
    [ ! -e "$marker" ] || fail "$seam inspection failure must not send TERM"
    kill -0 "$ACTIVE_PID" 2>/dev/null || fail "$seam inspection failure must preserve live fixture"
    [ -e "$BONG_SERVER_PID_FILE" ] || fail "$seam inspection failure must preserve PID record"
    kill -KILL "$ACTIVE_PID" 2>/dev/null || true
    wait "$ACTIVE_PID" 2>/dev/null || true
    ACTIVE_PID=""
    rm -f -- "$BONG_SERVER_PID_FILE"
}

assert_inspection_failure_preserves_record ps
assert_inspection_failure_preserves_record starttime
assert_inspection_failure_preserves_record executable_identity

# Clear-after-exit has an explicit tri-state: exact authority removal succeeds,
# a valid replacement is distinguishable, and uncertain reads/removals fail.
clear_tristate_record="$TEST_ROOT/clear-tristate.pid"
export BONG_SERVER_PID_FILE="$clear_tristate_record"
clear_expected_pid=41001
clear_expected_starttime=51001
clear_expected_executable=/tmp/bong-clear-expected
clear_expected_executable_identity=61:71
write_clear_record() {
    local pid="$1" starttime="$2" executable="$3" executable_identity="$4"
    printf 'pid=%s\nstarttime=%s\nexecutable=%s\nexecutable_identity=%s\n' \
        "$pid" "$starttime" "$executable" "$executable_identity" > "$clear_tristate_record"
    chmod 600 "$clear_tristate_record"
}

bong_server_clear_record_if_matches \
    "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity" \
    || fail "missing PID record must be a safe clear no-op"
write_clear_record "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity"
bong_server_clear_record_if_matches \
    "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity" \
    || fail "exact matching PID record must be removed"
[ ! -e "$clear_tristate_record" ] || fail "exact matching PID record clear must remove the pathname"

write_clear_record 41002 51002 /tmp/bong-clear-replacement 62:72
if bong_server_clear_record_if_matches \
    "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity"; then
    fail "valid replacement PID record must not report an exact clear"
else
    clear_status=$?
fi
[ "$clear_status" -eq 1 ] || fail "valid replacement PID record must return status 1, got $clear_status"
[ -e "$clear_tristate_record" ] || fail "valid replacement PID record must be preserved"

printf 'malformed\n' > "$clear_tristate_record"
chmod 600 "$clear_tristate_record"
if bong_server_clear_record_if_matches \
    "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity"; then
    fail "malformed PID record must not report a safe clear"
else
    clear_status=$?
fi
[ "$clear_status" -eq 2 ] || fail "malformed PID record clear must return status 2, got $clear_status"
[ -e "$clear_tristate_record" ] || fail "malformed PID record must be preserved"

write_clear_record "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity"
original_clear_record="$(declare -f bong_server_clear_record)"
bong_server_clear_record() { return 1; }
if bong_server_clear_record_if_matches \
    "$clear_expected_pid" "$clear_expected_starttime" \
    "$clear_expected_executable" "$clear_expected_executable_identity"; then
    eval "$original_clear_record"
    fail "PID record unlink failure must not report success"
else
    clear_status=$?
fi
eval "$original_clear_record"
[ "$clear_status" -eq 2 ] || fail "PID record unlink failure must return status 2, got $clear_status"
[ -e "$clear_tristate_record" ] || fail "PID record unlink failure must preserve the record"

# Every managed-stop branch that has observed target exit must propagate the
# clear tri-state instead of allowing teardown/relaunch on a stale authority.
original_process_is_running="$(declare -f bong_server_process_is_running)"
eval "$(declare -f bong_server_read_record | sed '1s/^bong_server_read_record /bong_server_read_record_fixture_original /')"
assert_exited_stop_cleanup_failure() {
    local mode="$1" status read_calls=0

    write_clear_record "$clear_expected_pid" "$clear_expected_starttime" \
        "$clear_expected_executable" "$clear_expected_executable_identity"
    bong_server_process_is_running() { return 1; }
    case "$mode" in
        read)
            original_read_record="$(declare -f bong_server_read_record)"
            bong_server_read_record() {
                read_calls=$((read_calls + 1))
                [ "$read_calls" -eq 1 ] && bong_server_read_record_fixture_original
            }
            ;;
        replacement)
            original_read_record="$(declare -f bong_server_read_record)"
            bong_server_read_record() {
                read_calls=$((read_calls + 1))
                if [ "$read_calls" -eq 1 ]; then
                    bong_server_read_record_fixture_original
                    return $?
                fi
                BONG_SERVER_RECORDED_PID=41002
                BONG_SERVER_RECORDED_STARTTIME=51002
                BONG_SERVER_RECORDED_EXECUTABLE=/tmp/bong-clear-replacement
                BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY=62:72
                BONG_SERVER_RECORDED_FILE_IDENTITY="$(bong_server_path_identity "$clear_tristate_record")"
            }
            ;;
        remove)
            original_clear_record="$(declare -f bong_server_clear_record)"
            bong_server_clear_record() { return 1; }
            ;;
        *) fail "unknown exited cleanup failure fixture: $mode" ;;
    esac
    if _bong_server_stop_managed; then
        status=0
    else
        status=$?
    fi
    eval "$original_process_is_running"
    case "$mode" in
        read|replacement) eval "$original_read_record" ;;
        remove) eval "$original_clear_record" ;;
    esac
    [ "$status" -eq 2 ] || fail "exited managed stop $mode cleanup failure must return 2, got $status"
    [ -e "$clear_tristate_record" ] || fail "exited managed stop $mode cleanup failure must preserve the record"
}
assert_exited_stop_cleanup_failure read
assert_exited_stop_cleanup_failure replacement
assert_exited_stop_cleanup_failure remove
unset -f bong_server_read_record_fixture_original
rm -f -- "$clear_tristate_record"

term_wait_marker="$TEST_ROOT/term-wait-inspection.marker"
spawn_fixture ignore "$term_wait_marker"
bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
    || fail "TERM-wait inspection fixture could not create a managed record"
original_wait_for_exit="$(declare -f bong_server_wait_for_exit)"
original_pidfd_signal="$(declare -f bong_server_pidfd_signal)"
bong_server_wait_for_exit() { return 2; }
bong_server_pidfd_signal() {
    if [ "${4:-}" = TERM ] && [ "${1:-}" = "$ACTIVE_PID" ]; then
        printf TERM > "$term_wait_marker"
        return 0
    fi
    return 2
}
if BONG_SERVER_STOP_GRACE_SECONDS=1 bong_server_stop_managed; then
    eval "$original_pidfd_signal"
    eval "$original_wait_for_exit"
    fail "TERM-wait inspection failure must fail closed"
else
    term_wait_status=$?
fi
eval "$original_pidfd_signal"
eval "$original_wait_for_exit"
[ "$term_wait_status" -eq 2 ] || fail "TERM-wait inspection failure must return 2"
[ -e "$term_wait_marker" ] || fail "TERM-wait inspection failure must deliver TERM through pidfd seam"
kill -0 "$ACTIVE_PID" 2>/dev/null || fail "TERM-wait inspection failure must preserve live fixture"
[ -e "$BONG_SERVER_PID_FILE" ] || fail "TERM-wait inspection failure must preserve PID record"
kill -KILL "$ACTIVE_PID" 2>/dev/null || true
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""
rm -f -- "$BONG_SERVER_PID_FILE"

# After TERM times out, the second identity inspection immediately before KILL
# is independently fail-closed. Model first identity verification as successful
# and the pre-KILL recheck as uninspectable, leaving the TERM-ignoring process
# and its record intact without escalation.
kill_identity_marker="$TEST_ROOT/kill-identity-inspection.marker"
spawn_fixture ignore "$kill_identity_marker"
bong_server_write_record "$ACTIVE_PID" "$(command -v bash)" \
    || fail "SIGKILL-identity inspection fixture could not create a managed record"
original_record_matches_process="$(declare -f bong_server_record_matches_process)"
record_match_calls=0
bong_server_record_matches_process() {
    record_match_calls=$((record_match_calls + 1))
    [ "$record_match_calls" -eq 1 ] && return 0
    return 2
}
if BONG_SERVER_STOP_GRACE_SECONDS=0 bong_server_stop_managed; then
    eval "$original_record_matches_process"
    fail "SIGKILL identity inspection failure must fail closed"
else
    kill_identity_status=$?
fi
eval "$original_record_matches_process"
[ "$kill_identity_status" -eq 2 ] || fail "SIGKILL identity inspection failure must return 2"
[ ! -e "$kill_identity_marker" ] || fail "TERM-ignoring fixture unexpectedly handled TERM"
kill -0 "$ACTIVE_PID" 2>/dev/null || fail "SIGKILL identity inspection failure must not send KILL"
[ -e "$BONG_SERVER_PID_FILE" ] || fail "SIGKILL identity inspection failure must preserve PID record"
kill -KILL "$ACTIVE_PID" 2>/dev/null || true
wait "$ACTIVE_PID" 2>/dev/null || true
ACTIVE_PID=""
rm -f -- "$BONG_SERVER_PID_FILE"

# readlink failures are also distinct from an executable mismatch: startup
# callers must learn that the process is still live but uninspectable.
readlink() {
    case "$*" in
        */proc/*) return 2 ;;
        *) command readlink "$@" ;;
    esac
}
sleep 30 &
wait_executable_pid=$!
if bong_server_wait_for_executable "$wait_executable_pid" "$(command -v bash)" 1; then
    unset -f readlink
    fail "wait_for_executable must not report a readlink failure as a mismatch"
else
    wait_executable_status=$?
fi
unset -f readlink
[ "$wait_executable_status" -eq 2 ] || fail "wait_for_executable readlink failure must return 2"
kill -KILL "$wait_executable_pid" 2>/dev/null || true
wait "$wait_executable_pid" 2>/dev/null || true

# wait_for_exit propagates a live-process inspection failure rather than
# treating it as an observed exit.
sleep 30 &
wait_inspection_pid=$!
ps() { return 2; }
if bong_server_wait_for_exit "$wait_inspection_pid" 0; then
    unset -f ps
    fail "wait_for_exit must not report an inspection failure as an exit"
else
    wait_exit_status=$?
fi
unset -f ps
[ "$wait_exit_status" -eq 2 ] || fail "wait_for_exit inspection failure must return 2"
kill -KILL "$wait_inspection_pid" 2>/dev/null || true
wait "$wait_inspection_pid" 2>/dev/null || true

# E2E's empty-PID restore gate is state-sensitive. With no READY stash it remains
# a harmless no-op; with READY bytes it must freshly observe the shared port.
extract_e2e_stop_server() {
    python3 - "$ROOT/scripts/e2e-redis.sh" <<'PY'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text()
start = text.index("stop_server() {")
end = text.index("\n}\n\ncleanup() {", start) + 2
print(text[start:end])
PY
}
eval "$(extract_e2e_stop_server)"
SERVER_PID=""
SERVER_PGID=""
SERVER_OWNER_STARTTIME=""
SERVER_OWNER_EXECUTABLE_IDENTITY=""
SERVER_AUTHORITY_UNCERTAIN=0
port_probe_calls=0
SERVER_AUTHORITY_UNCERTAIN=1
PERSISTENCE_STASH_READY=1
if stop_server; then
    fail "READY cleanup must fail closed when launch authority is uncertain"
fi
[ "$port_probe_calls" -eq 0 ] || fail "uncertain launch authority must not fall back to a port probe"
SERVER_AUTHORITY_UNCERTAIN=0
PERSISTENCE_STASH_READY=0
original_confirm_port_released="$(declare -f bong_server_confirm_port_released)"
bong_server_confirm_port_released() { port_probe_calls=$((port_probe_calls + 1)); return 1; }
stop_server || fail "empty PID without READY persistence must remain an ordinary no-op"
[ "$port_probe_calls" -eq 0 ] || fail "ordinary empty-PID cleanup must not claim fresh port evidence"
PERSISTENCE_STASH_READY=1
if stop_server; then
    fail "READY empty-PID cleanup must fail when port release cannot be confirmed"
fi
[ "$port_probe_calls" -eq 1 ] || fail "READY empty-PID cleanup must probe the shared port exactly once"
bong_server_confirm_port_released() { port_probe_calls=$((port_probe_calls + 1)); return 0; }
stop_server || fail "READY empty-PID cleanup must pass after fresh port-release evidence"
[ "$port_probe_calls" -eq 2 ] || fail "READY closed-port cleanup must obtain fresh evidence"
# A force-stopped group is cleanup evidence only, never restore authorization.
eval "$(extract_e2e_stop_server)"
SERVER_PID=424242
SERVER_PGID=424242
SERVER_OWNER_STARTTIME=1
SERVER_OWNER_EXECUTABLE_IDENTITY=1:1
SERVER_AUTHORITY_UNCERTAIN=0
PERSISTENCE_STASH_READY=1
original_group_stop="$(declare -f bong_server_stop_owned_process_group_and_release_port)"
bong_server_stop_owned_process_group_and_release_port() { return "$BONG_SERVER_STOP_FORCED"; }
if stop_server; then
    fail "force-stopped preview group must not authorize persistence restore"
fi
[ "$SERVER_PID" = 424242 ] && [ "$SERVER_PGID" = 424242 ] \
    || fail "force-stopped preview group must preserve published authority for diagnosis"
[ "$SERVER_OWNER_STARTTIME" = 1 ] \
    && [ "$SERVER_OWNER_EXECUTABLE_IDENTITY" = 1:1 ] \
    || fail "force-stopped preview group must preserve owner pins"
eval "$original_group_stop"
unset -f stop_server extract_e2e_stop_server
eval "$original_confirm_port_released"
PERSISTENCE_STASH_READY=0

# The full preview transaction must hold the production lifecycle lock from the
# pre-stash stop through restore, and competing production actions cannot enter.
preview_lock_record="$TEST_ROOT/preview-lock.pid"
export BONG_SERVER_PID_FILE="$preview_lock_record"
preview_lock_entered="$TEST_ROOT/preview-lock-entered"
preview_lock_release="$TEST_ROOT/preview-lock-release"
production_side_effect="$TEST_ROOT/production-side-effect"
preview_lock_holder_fn="$TEST_ROOT/preview-lock-holder.sh"
cat > "$preview_lock_holder_fn" <<SCRIPT
#!/usr/bin/env bash
set -euo pipefail
source "$ROOT/scripts/lib/bong-server-lifecycle.sh"
export BONG_SERVER_PID_FILE="$preview_lock_record"
hold_preview_transaction_lock() {
    : > "$preview_lock_entered"
    while [ ! -e "$preview_lock_release" ]; do sleep 0.01; done
}
bong_server_with_preview_persistence_lock hold_preview_transaction_lock
SCRIPT
chmod +x "$preview_lock_holder_fn"
"$preview_lock_holder_fn" &
preview_lock_holder=$!
for _ in $(seq 1 100); do
    [ -e "$preview_lock_entered" ] && break
    sleep 0.01
done
[ -e "$preview_lock_entered" ] || fail "preview lifecycle fixture did not acquire lock"
production_lifecycle_action() { : > "$production_side_effect"; }
BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS=0 bong_server_with_lock production_lifecycle_action \
    && fail "production lifecycle action must not enter during preview persistence interval"
[ ! -e "$production_side_effect" ] \
    || fail "contending production lifecycle action produced a side effect before preview release"
: > "$preview_lock_release"
wait "$preview_lock_holder" || fail "preview lifecycle lock holder failed"
bong_server_with_lock production_lifecycle_action \
    || fail "production lifecycle action must enter after preview releases lock"
[ -e "$production_side_effect" ] \
    || fail "production lifecycle action did not run after preview lock release"
unset -f production_lifecycle_action

# E2E's production stop helper and persistence handoff are executable without
# launching the full Redis scenario. An uncertain child enumeration must leave
# STOP_SERVER_CONFIRMED false, skip restore, and retain the durable stash.
e2e_stop_root="$TEST_ROOT/e2e-stop-helper"
e2e_stop_data="$e2e_stop_root/data"
e2e_stop_stash="$e2e_stop_root/stash"
mkdir -p "$e2e_stop_data"
printf 'e2e-developer-db\n' > "$e2e_stop_data/bong.db"
test_persistence_stash "$e2e_stop_data" "$e2e_stop_stash" \
    || fail "e2e stop helper fixture must create a READY stash"
sleep 30 &
e2e_stop_pid=$!
STOP_SERVER_CONFIRMED=0
pgrep() { return 2; }
bong_server_port_is_open() { return 1; }
if bong_server_stop_process_tree_and_release_port "$e2e_stop_pid" 25565; then
    unset -f pgrep bong_server_port_is_open
    fail "e2e stop helper must fail when child enumeration returns status 2"
fi
unset -f pgrep bong_server_port_is_open
[ "$STOP_SERVER_CONFIRMED" -eq 0 ] || fail "unconfirmed e2e stop must not set STOP_SERVER_CONFIRMED"
kill -0 "$e2e_stop_pid" 2>/dev/null || fail "failed e2e stop must not signal its live fixture"
restore_called=0
bong_server_restore_persistence() { restore_called=1; return 0; }
bong_server_finalize_preview_persistence_after_stop "$e2e_stop_data" "$e2e_stop_stash" "$STOP_SERVER_CONFIRMED" \
    || fail "unconfirmed e2e stop must write the durable abort handoff"
unset -f bong_server_restore_persistence
[ "$restore_called" -eq 0 ] || fail "unconfirmed e2e stop must not call restore"
e2e_stop_marker="$(transaction_state_dir "$e2e_stop_data")/recovery-handoff"
[ -f "$e2e_stop_marker" ] || fail "unconfirmed e2e stop must retain its abort handoff marker"
grep -Fqx 'state=FAILED' "$e2e_stop_marker" || fail "unconfirmed e2e stop marker must be FAILED"
[ "$(cat "$e2e_stop_stash/bong.db")" = "e2e-developer-db" ] \
    || fail "unconfirmed e2e stop must retain the stash unchanged"
kill -KILL "$e2e_stop_pid" 2>/dev/null || true
wait "$e2e_stop_pid" 2>/dev/null || true

# E2E stop authority is a persistent, pinned session leader. Graceful children
# must finish on TERM before the helper removes that non-reusable group owner.
# Any business child that needs KILL is a distinct forced outcome.
process_group_marker="$TEST_ROOT/process-group-descendant-term.marker"
process_group_ready="$TEST_ROOT/process-group-descendant.ready"
process_group_fixture="$TEST_ROOT/process-group-fixture.py"
cat > "$process_group_fixture" <<'PY'
import os
import signal
import sys
import time

marker, ready = sys.argv[1:]
os.setsid()
signal.signal(signal.SIGTERM, signal.SIG_IGN)
child = os.fork()
if child:
    with open(ready, "w", encoding="utf-8") as handle:
        handle.write(str(child))
    while True:
        time.sleep(1)

def terminate(*_args):
    with open(marker, "w", encoding="utf-8") as handle:
        handle.write("TERM")
    raise SystemExit(0)

signal.signal(signal.SIGTERM, terminate)
while True:
    time.sleep(1)
PY
python3 "$process_group_fixture" "$process_group_marker" "$process_group_ready" &
process_group_owner=$!
for _ in $(seq 1 100); do
    [ -e "$process_group_ready" ] && break
    sleep 0.01
done
[ -e "$process_group_ready" ] || fail "owned process-group fixture did not publish its descendant"
process_group_owner_starttime="$(bong_server_process_starttime "$process_group_owner")"
process_group_owner_identity="$(bong_server_process_executable_identity "$process_group_owner")"
process_group_pgid="$(ps -o pgid= -p "$process_group_owner" 2>/dev/null | tr -d '[:space:]')"
[ "$process_group_pgid" = "$process_group_owner" ] \
    || fail "owned fixture supervisor must be its process-group leader"
bong_server_port_is_open() { return 1; }
bong_server_stop_owned_process_group_and_release_port \
    "$process_group_owner" "$process_group_owner_starttime" \
    "$process_group_owner_identity" "$process_group_pgid" 25565 \
    || fail "owned process-group stop must terminate graceful descendants and supervisor"
unset -f bong_server_port_is_open
[ -e "$process_group_marker" ] \
    || fail "owned process-group stop did not deliver TERM to its descendant"
if bong_server_process_group_members "$process_group_pgid" >/dev/null; then
    fail "owned process-group stop left a non-zombie member alive"
fi

# The executable group helper must report status 3 when a real child ignores
# TERM and is killed. That forced result must preserve E2E authority and deny
# persistence restore through the same caller/finalizer contracts used by preview.
forced_group_marker="$TEST_ROOT/process-group-forced.marker"
forced_group_ready="$TEST_ROOT/process-group-forced.ready"
forced_group_fixture="$TEST_ROOT/process-group-forced-fixture.py"
cat > "$forced_group_fixture" <<'PY'
import os
import signal
import sys
import time

marker, ready = sys.argv[1:]
os.setsid()
signal.signal(signal.SIGTERM, signal.SIG_IGN)
child = os.fork()
if child:
    with open(ready, "w", encoding="utf-8") as handle:
        handle.write(str(child))
    while True:
        time.sleep(1)

signal.signal(signal.SIGTERM, signal.SIG_IGN)
with open(marker, "w", encoding="utf-8") as handle:
    handle.write("ready")
while True:
    time.sleep(1)
PY
python3 "$forced_group_fixture" "$forced_group_marker" "$forced_group_ready" &
forced_group_owner=$!
for _ in $(seq 1 100); do
    [ -s "$forced_group_ready" ] && [ -e "$forced_group_marker" ] && break
    sleep 0.01
done
[ -s "$forced_group_ready" ] && [ -e "$forced_group_marker" ] \
    || fail "forced process-group fixture did not publish its live descendant"
forced_group_owner_starttime="$(bong_server_process_starttime "$forced_group_owner")"
forced_group_owner_identity="$(bong_server_process_executable_identity "$forced_group_owner")"
forced_group_pgid="$(ps -o pgid= -p "$forced_group_owner" 2>/dev/null | tr -d '[:space:]')"
[ "$forced_group_pgid" = "$forced_group_owner" ] \
    || fail "forced fixture supervisor must be its process-group leader"
bong_server_port_is_open() { return 1; }
if bong_server_stop_owned_process_group_and_release_port \
    "$forced_group_owner" "$forced_group_owner_starttime" \
    "$forced_group_owner_identity" "$forced_group_pgid" 25565; then
    forced_group_status=0
else
    forced_group_status=$?
fi
unset -f bong_server_port_is_open
[ "$forced_group_status" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "TERM-ignoring process-group child must produce forced stop status"
if bong_server_process_group_members "$forced_group_pgid" >/dev/null; then
    fail "forced process-group cleanup left a non-zombie member alive"
fi

forced_stop_data="$TEST_ROOT/forced-stop-preview/data"
forced_stop_stash="$TEST_ROOT/forced-stop-preview/stash"
mkdir -p "$forced_stop_data"
printf 'developer-db\n' > "$forced_stop_data/bong.db"
bong_server_persistence_transaction_begin "$forced_stop_data" "$forced_stop_stash" \
    || fail "forced stop persistence fixture could not begin transaction"
bong_server_stash_persistence "$forced_stop_data" "$forced_stop_stash" \
    || fail "forced stop persistence fixture could not stash developer data"
SERVER_PID="$forced_group_owner"
SERVER_PGID="$forced_group_pgid"
SERVER_OWNER_STARTTIME="$forced_group_owner_starttime"
SERVER_OWNER_EXECUTABLE_IDENTITY="$forced_group_owner_identity"
SERVER_AUTHORITY_UNCERTAIN=0
STOP_SERVER_CONFIRMED=0
restore_called=0
original_group_stop="$(declare -f bong_server_stop_owned_process_group_and_release_port)"
bong_server_restore_persistence() { restore_called=1; return 0; }
bong_server_stop_owned_process_group_and_release_port() { return "$BONG_SERVER_STOP_FORCED"; }
stop_server_fixture="$TEST_ROOT/e2e-stop-server-fixture.sh"
python3 - "$ROOT/scripts/e2e-redis.sh" > "$stop_server_fixture" <<'PY_STOP_SERVER'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("stop_server() {")
end = text.index("\n}\n\ncleanup() {", start) + 2
print(text[start:end])
PY_STOP_SERVER
# shellcheck source=/dev/null
source "$stop_server_fixture"
if stop_server; then
    fail "forced process-group stop must not authorize E2E shutdown success"
else
    forced_caller_status=$?
fi
[ "$forced_caller_status" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "E2E stop caller must preserve forced process-group status"
[ "$SERVER_PID" = "$forced_group_owner" ] && [ "$SERVER_PGID" = "$forced_group_pgid" ] \
    && [ "$SERVER_OWNER_STARTTIME" = "$forced_group_owner_starttime" ] \
    && [ "$SERVER_OWNER_EXECUTABLE_IDENTITY" = "$forced_group_owner_identity" ] \
    || fail "forced E2E stop must preserve all process-group authority pins"
bong_server_finalize_preview_persistence_after_stop \
    "$forced_stop_data" "$forced_stop_stash" "$STOP_SERVER_CONFIRMED" \
    || fail "forced E2E stop must write a durable failed recovery handoff"
[ "$restore_called" -eq 0 ] \
    || fail "forced E2E stop must not restore persistence"
forced_stop_handoff="$(transaction_state_dir "$forced_stop_data")/recovery-handoff"
grep -Fqx 'state=FAILED' "$forced_stop_handoff" \
    || fail "forced E2E stop must retain a FAILED recovery handoff"
[ "$(cat "$forced_stop_stash/bong.db")" = "developer-db" ] \
    || fail "forced E2E stop must leave the developer database in its stash"
unset -f bong_server_restore_persistence \
    bong_server_stop_owned_process_group_and_release_port stop_server
eval "$original_group_stop"

# A dead/reused or uninspectable owner invalidates the numeric PGID before any
# enumeration or signal, preventing a same-number foreign group from being killed.
original_pinned_process_status="$(declare -f bong_server_pinned_process_status)"
original_process_group_members="$(declare -f bong_server_process_group_members)"
original_pidfd_signal="$(declare -f bong_server_pidfd_signal)"
for owner_status in 1 2; do
    group_enumerations=0
    group_signals=0
    bong_server_pinned_process_status() { return "$owner_status"; }
    bong_server_process_group_members() { group_enumerations=$((group_enumerations + 1)); printf '999999\n'; }
    bong_server_pidfd_signal() { group_signals=$((group_signals + 1)); return 0; }
    if bong_server_stop_owned_process_group_and_release_port \
        424242 1 1:1 424242 25565; then
        fail "owner status $owner_status must invalidate process-group authority"
    fi
    [ "$group_enumerations" -eq 0 ] \
        || fail "invalid owner status $owner_status must refuse group enumeration"
    [ "$group_signals" -eq 0 ] \
        || fail "invalid owner status $owner_status must refuse all signals"
done
unset -f bong_server_pinned_process_status bong_server_process_group_members bong_server_pidfd_signal
eval "$original_pinned_process_status"
eval "$original_process_group_members"
eval "$original_pidfd_signal"

# Readiness evidence is a private single-line PID assertion. Missing is status 1;
# malformed or symlinked evidence is status 2.
readiness_dir="$TEST_ROOT/readiness"
mkdir "$readiness_dir"
chmod 700 "$readiness_dir"
readiness_path="$readiness_dir/server.ready"
if bong_server_read_ready_pid "$readiness_path" >/dev/null; then
    fail "missing readiness path must not report success"
else
    readiness_status=$?
fi
[ "$readiness_status" -eq 1 ] || fail "missing readiness path must return status 1"
printf 'pid=4242\n' > "$readiness_path"
chmod 600 "$readiness_path"
[ "$(bong_server_read_ready_pid "$readiness_path")" = 4242 ] \
    || fail "valid readiness evidence must return its PID"
printf 'pid=4242\nextra=1\n' > "$readiness_path"
if bong_server_read_ready_pid "$readiness_path" >/dev/null; then
    fail "multi-line readiness evidence must fail closed"
else
    readiness_status=$?
fi
[ "$readiness_status" -eq 2 ] || fail "multi-line readiness evidence must return status 2"
rm -f "$readiness_path"
ln -s /dev/null "$readiness_path"
if bong_server_read_ready_pid "$readiness_path" >/dev/null; then
    fail "readiness symlink must fail closed"
else
    readiness_status=$?
fi
[ "$readiness_status" -eq 2 ] || fail "readiness symlink must return status 2"
rm -f "$readiness_path"

# Startup authority is transactional: the supervisor alone reads the control
# pipe, rolls back every uncommitted process-group peer on EOF, and retains its
# pinned owner identity after commit until owner-bound teardown. The focused
# executable protocol suite covers invalid bytes, exact ACK, fd0 isolation,
# stopped/no-ACK, malformed/EOF ACK, and post-ACK identity loss.
"$ROOT/scripts/test-listener-owner.sh"
"$ROOT/scripts/test-supervisor-protocol.sh"

# The moved S3 signal-boundary contract lives as a checked-in Python suite. The
# lifecycle entrypoint is where the real CI path reaches it: smoke-test-e2e.sh ->
# this script -> the delegated Python suite. The suite is CARGO_TARGET_DIR safe
# (the copy-failure fixture pins its own environment); wiring it here means the
# rollback / signal / target-resolution / artifact-cleanup contracts can no
# longer regress while the normal CI entrypoint stays green.
python3 "$ROOT/scripts/tests/s3_lifecycle_signal_boundary_contract_test.py"
if [ "${GITHUB_ACTIONS:-}" = true ]; then
    # CI 必须显式启用真实的 tmux 关停顺序契约：默认 CI 运行不得静默跳过它。
    # 若 workflow 忘记设置 opt-in，这里直接 fail 而不是打印 SKIP，保住覆盖。
    [ "${BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST:-0}" = 1 ] \
        || fail "GitHub Actions CI must enable BONG_RUN_TMUX_SHUTDOWN_ORDER_TEST=1 for the real tmux shutdown-order contract"
    "$ROOT/scripts/test-tmux-shutdown-order.sh"
else
    printf 'SKIP: tmux shutdown-order signal test is quarantined to opted-in GitHub Actions e2e\n'
fi
supervisor="$ROOT/scripts/lib/bong-process-group-supervisor.py"
supervisor_fixture_dir="$TEST_ROOT/supervisor-fixture"
supervisor_fixture_bin="$supervisor_fixture_dir/bin"
supervisor_fixture_marker="$supervisor_fixture_dir/cargo.marker"
supervisor_fixture_term_marker="$supervisor_fixture_dir/term.marker"
supervisor_fixture_descendant="$supervisor_fixture_dir/descendant.pid"
supervisor_fixture_cargo="$supervisor_fixture_bin/cargo"
supervisor_fixture_build_token="$supervisor_fixture_dir/build-token.sh"
supervisor_fixture_build_token_args="$supervisor_fixture_dir/build-token.args"
mkdir -p "$supervisor_fixture_bin" "$supervisor_fixture_dir/server"
cat > "$supervisor_fixture_cargo" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
: "${SUPERVISOR_FIXTURE_MARKER:?}"
: "${SUPERVISOR_FIXTURE_TERM_MARKER:?}"
: "${SUPERVISOR_FIXTURE_DESCENDANT:?}"
: "${CARGO_TARGET_DIR:?}"
if IFS= read -r -t 0.2 _; then
    printf 'control-stdin-leaked\n' > "$SUPERVISOR_FIXTURE_MARKER"
    exit 41
fi
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/bong-server" <<'SERVER'
#!/usr/bin/env bash
set -euo pipefail
: "${SUPERVISOR_FIXTURE_MARKER:?}"
: "${SUPERVISOR_FIXTURE_TERM_MARKER:?}"
: "${SUPERVISOR_FIXTURE_DESCENDANT:?}"
trap 'kill "$(cat "$SUPERVISOR_FIXTURE_DESCENDANT")" 2>/dev/null || true; exit 0' TERM
(
    trap '' TERM
    while :; do sleep 1; done
) &
printf '%s\n' "$!" > "$SUPERVISOR_FIXTURE_DESCENDANT"
printf 'started\n' > "$SUPERVISOR_FIXTURE_MARKER"
while :; do sleep 1; done
SERVER
chmod +x "$CARGO_TARGET_DIR/release/bong-server"
SCRIPT
chmod +x "$supervisor_fixture_cargo"
cat > "$supervisor_fixture_build_token" <<'SCRIPT'
#!/usr/bin/env bash
set -euo pipefail
: "${SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS:?}"
printf '%s\n' "$@" > "$SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS"
[ "$#" -ge 1 ] && [ "$1" = cargo ] || exit 42
shift
exec cargo "$@"
SCRIPT
chmod +x "$supervisor_fixture_build_token"

start_supervisor_fixture() {
    local control_fd ready_fd ready_line

    rm -f -- "$supervisor_fixture_build_token_args"
    coproc LIFECYCLE_SUPERVISOR_FIXTURE {
        exec env \
            PATH="$supervisor_fixture_bin:$PATH" \
            CARGO_TARGET_DIR="$supervisor_fixture_dir/server/target" \
            SUPERVISOR_FIXTURE_MARKER="$supervisor_fixture_marker" \
            SUPERVISOR_FIXTURE_TERM_MARKER="$supervisor_fixture_term_marker" \
            SUPERVISOR_FIXTURE_DESCENDANT="$supervisor_fixture_descendant" \
            SUPERVISOR_FIXTURE_BUILD_TOKEN_ARGS="$supervisor_fixture_build_token_args" \
            python3 "$supervisor" "$supervisor_fixture_dir/server" "$supervisor_fixture_build_token" \
            2>"$supervisor_fixture_dir/supervisor.log"
    }
    SUPERVISOR_FIXTURE_PID="$LIFECYCLE_SUPERVISOR_FIXTURE_PID"
    ready_fd="${LIFECYCLE_SUPERVISOR_FIXTURE[0]}"
    control_fd="${LIFECYCLE_SUPERVISOR_FIXTURE[1]}"
    SUPERVISOR_FIXTURE_READY_FD="$ready_fd"
    SUPERVISOR_FIXTURE_CONTROL_FD="$control_fd"
    IFS= read -r -t 5 -u "$ready_fd" ready_line \
        || fail "startup supervisor fixture did not publish READY"
    [[ "$ready_line" = 'READY pid='[0-9]* ]] \
        || fail "startup supervisor fixture published malformed READY"
    SUPERVISOR_FIXTURE_OWNER_PID="${ready_line#READY pid=}"
    for _ in $(seq 1 100); do
        [ -s "$supervisor_fixture_descendant" ] \
            && [ -s "$supervisor_fixture_build_token_args" ] \
            && break
        sleep 0.01
    done
    [ -s "$supervisor_fixture_descendant" ] \
        || fail "startup supervisor fixture did not publish its descendant"
    [ -s "$supervisor_fixture_build_token_args" ] \
        || fail "startup supervisor build-token fixture did not publish argv"
    [ "$(tr '\n' ' ' < "$supervisor_fixture_build_token_args")" = "cargo build --release " ] \
        || fail "startup supervisor must route exact cargo build --release argv through build-token"
}

read_supervisor_ack() {
    local acknowledgement=""
    IFS= read -r -t 5 -u "$SUPERVISOR_FIXTURE_READY_FD" acknowledgement \
        || return 1
    [ "$acknowledgement" = COMMITTED ]
}

assert_supervisor_rollback() {
    local owner="$1" pgid="$2" descendant="$3"
    eval "exec ${SUPERVISOR_FIXTURE_CONTROL_FD}>&-" 2>/dev/null || true
    eval "exec ${SUPERVISOR_FIXTURE_READY_FD}<&-" 2>/dev/null || true
    wait "$owner" 2>/dev/null && fail "uncommitted supervisor must report a nonzero exit"
    for _ in $(seq 1 100); do
        if ! kill -0 "$descendant" 2>/dev/null; then
            break
        fi
        descendant_state="$(ps -o stat= -p "$descendant" 2>/dev/null | tr -d '[:space:]')"
        [[ "$descendant_state" = Z* ]] && break
        sleep 0.01
    done
    if kill -0 "$descendant" 2>/dev/null; then
        descendant_state="$(ps -o stat= -p "$descendant" 2>/dev/null | tr -d '[:space:]')"
        [[ "$descendant_state" = Z* ]] \
            || fail "uncommitted supervisor left a live group peer after rollback"
    fi
    if bong_server_process_group_members "$pgid" >/dev/null; then
        fail "uncommitted supervisor left a non-zombie group member alive"
    fi
}

start_supervisor_fixture
rollback_owner="$SUPERVISOR_FIXTURE_OWNER_PID"
rollback_pgid="$(ps -o pgid= -p "$rollback_owner" 2>/dev/null | tr -d '[:space:]')"
[ "$rollback_pgid" = "$rollback_owner" ] \
    || fail "startup rollback supervisor must own its process group"
rollback_descendant="$(cat "$supervisor_fixture_descendant")"
assert_supervisor_rollback "$rollback_owner" "$rollback_pgid" "$rollback_descendant"
[ ! -f "$supervisor_fixture_term_marker" ] \
    || fail "TERM-ignoring rollback descendant unexpectedly handled TERM"
[ "$(cat "$supervisor_fixture_marker")" = started ] \
    || fail "cargo child inherited and consumed the supervisor control pipe"

rm -f -- "$supervisor_fixture_marker" "$supervisor_fixture_term_marker" \
    "$supervisor_fixture_descendant"
start_supervisor_fixture
committed_owner="$SUPERVISOR_FIXTURE_OWNER_PID"
committed_pgid="$(ps -o pgid= -p "$committed_owner" 2>/dev/null | tr -d '[:space:]')"
committed_starttime="$(bong_server_process_starttime "$committed_owner")"
committed_identity="$(bong_server_process_executable_identity "$committed_owner")"
printf C >&"$SUPERVISOR_FIXTURE_CONTROL_FD" \
    || fail "startup supervisor fixture could not commit authority"
read_supervisor_ack || fail "startup supervisor fixture did not flush exact COMMITTED ACK"
exec {SUPERVISOR_FIXTURE_CONTROL_FD}>&-
exec {SUPERVISOR_FIXTURE_READY_FD}<&-
sleep 0.1
kill -0 "$committed_owner" 2>/dev/null \
    || fail "committed startup supervisor relinquished its pinned owner identity"
bong_server_port_is_open() { return 1; }
if bong_server_stop_owned_process_group_and_release_port \
    "$committed_owner" "$committed_starttime" "$committed_identity" \
    "$committed_pgid" 25565; then
    committed_stop_status=0
else
    committed_stop_status=$?
fi
[ "$committed_stop_status" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "committed startup fixture must report forced cleanup when its TERM-ignoring descendant is killed"
unset -f bong_server_port_is_open
[ "$(cat "$supervisor_fixture_marker")" = started ] \
    || fail "committed cargo child inherited and consumed the supervisor control pipe"

# The executable supervisor protocol has one reader and exact acknowledgement;
# static pins also prevent future e2e changes from publishing pre-ACK authority.
grep -Fq 'stdin=subprocess.DEVNULL' "$ROOT/scripts/lib/bong-process-group-supervisor.py" \
    || fail "cargo must retain /dev/null stdin before and after commit"
grep -Fq 'close_fds=True' "$ROOT/scripts/lib/bong-process-group-supervisor.py" \
    || fail "cargo must not inherit protocol descriptors"
grep -Fq 'sys.stdout.buffer.write(b"COMMITTED\n")' "$ROOT/scripts/lib/bong-process-group-supervisor.py" \
    || fail "supervisor must emit exact COMMITTED acknowledgement"

# Production startup must never HUP an unverified pane. Before identity pinning,
# cleanup preserves tmux; after pinning it can only destroy the session after the
# pinned TERM/wait/KILL helper confirms the exact process is gone.
start_cleanup_fixture="$TEST_ROOT/start-cleanup-fixture.sh"
python3 - "$ROOT/scripts/start.sh" > "$start_cleanup_fixture" <<'PY'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("cleanup_pinned_server_or_preserve_tmux() {")
end = text.index("\n}\nfor _ in", start) + 2
print(text[start:end])
PY
# shellcheck source=/dev/null
source "$start_cleanup_fixture"
server_ready_path="$TEST_ROOT/start-cleanup.ready"
SESSION=bong-cleanup-fixture
server_pid=424242
server_starttime=1
server_executable_identity=1:1
server_executable=""
server_identity_pinned=0
tmux_kill_calls=0
pinned_stop_calls=0
tmux() { [ "${1:-}" = kill-session ] && tmux_kill_calls=$((tmux_kill_calls + 1)); }
bong_server_rollback_pinned_managed_process() { pinned_stop_calls=$((pinned_stop_calls + 1)); return 0; }
if cleanup_pinned_server_or_preserve_tmux "identity pin failed"; then
    fail "unverified production startup cleanup must fail closed"
fi
[ "$pinned_stop_calls" -eq 0 ] \
    || fail "unverified production startup cleanup must not signal a guessed PID"
[ "$tmux_kill_calls" -eq 0 ] \
    || fail "unverified production startup cleanup must preserve tmux instead of HUP"
server_identity_pinned=1
cleanup_pinned_server_or_preserve_tmux "readiness failed" \
    || fail "verified production startup cleanup should accept a safe pinned stop"
[ "$pinned_stop_calls" -eq 1 ] \
    || fail "verified production startup cleanup must use pinned TERM/wait/KILL"
[ "$tmux_kill_calls" -eq 1 ] \
    || fail "verified production startup cleanup may destroy tmux only after graceful stop"
bong_server_rollback_pinned_managed_process() { pinned_stop_calls=$((pinned_stop_calls + 1)); return "$BONG_SERVER_STOP_FORCED"; }
if cleanup_pinned_server_or_preserve_tmux "forced shutdown"; then
    fail "force-stopped startup cleanup must not authorize tmux teardown"
fi
[ "$tmux_kill_calls" -eq 1 ] \
    || fail "force-stopped startup cleanup must preserve tmux"
bong_server_rollback_pinned_managed_process() { pinned_stop_calls=$((pinned_stop_calls + 1)); return 2; }
if cleanup_pinned_server_or_preserve_tmux "inspection failed"; then
    fail "uninspectable pinned startup cleanup must fail closed"
fi
[ "$tmux_kill_calls" -eq 1 ] \
    || fail "uninspectable pinned startup cleanup must preserve tmux"
unset -f cleanup_pinned_server_or_preserve_tmux tmux bong_server_rollback_pinned_managed_process

# The three production entrypoints must execute the same tri-state contract, not
# merely mention it in static text. Start/reload may replace after status 3 while
# preserving 1/2; stop completes non-server teardown and returns status 3 so an
# operator can observe that AppExit/Last was not proven.
source "$ROOT/scripts/dev-reload.sh"
production_managed_status=0
bong_server_stop_managed() { return "$production_managed_status"; }
for production_managed_status in 0 "$BONG_SERVER_STOP_FORCED"; do
    stop_managed_server_before_reload \
        || fail "dev-reload must continue after safe stop status $production_managed_status"
done
for production_managed_status in 1 2; do
    if stop_managed_server_before_reload 2> "$TEST_ROOT/dev-reload-stop-$production_managed_status.log"; then
        fail "dev-reload must fail closed on managed stop status $production_managed_status"
    else
        production_result=$?
    fi
    [ "$production_result" -eq "$production_managed_status" ] \
        || fail "dev-reload must preserve managed stop status $production_managed_status"
done

start_stop_fixture="$TEST_ROOT/start-stop-fixture.sh"
python3 - "$ROOT/scripts/start.sh" > "$start_stop_fixture" <<'PY_START_STOP'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("stop_managed_server_before_start() {")
end = text.index("\n}\n\nstart_bong_stack()", start) + 2
print(text[start:end])
PY_START_STOP
# shellcheck source=/dev/null
source "$start_stop_fixture"
for production_managed_status in 0 "$BONG_SERVER_STOP_FORCED"; do
    stop_managed_server_before_start \
        || fail "start.sh must continue after safe stop status $production_managed_status"
done
for production_managed_status in 1 2; do
    if stop_managed_server_before_start 2> "$TEST_ROOT/start-stop-$production_managed_status.log"; then
        fail "start.sh must fail closed on managed stop status $production_managed_status"
    else
        production_result=$?
    fi
    [ "$production_result" -eq "$production_managed_status" ] \
        || fail "start.sh must preserve managed stop status $production_managed_status"
done
unset -f stop_managed_server_before_start

# shellcheck source=/dev/null
source "$ROOT/scripts/stop.sh"
original_production_stop_managed="$(declare -f bong_server_stop_managed)"
original_production_tmux_scan="$(declare -f bong_server_tmux_has_unmanaged_server)"
production_managed_status=0
stop_teardown_log="$TEST_ROOT/production-stop-teardown.log"
: > "$stop_teardown_log"
bong_server_stop_managed() { return "$production_managed_status"; }
bong_server_tmux_has_unmanaged_server() { return 1; }
tmux() { printf 'tmux\n' >> "$stop_teardown_log"; return 0; }
pkill() { printf 'pkill\n' >> "$stop_teardown_log"; return 0; }
redis-cli() { printf 'redis\n' >> "$stop_teardown_log"; return 0; }
for production_managed_status in 0 "$BONG_SERVER_STOP_FORCED"; do
    : > "$stop_teardown_log"
    if stop_bong_stack > /dev/null 2> "$TEST_ROOT/production-stop-$production_managed_status.log"; then
        production_result=0
    else
        production_result=$?
    fi
    [ "$production_result" -eq "$production_managed_status" ] \
        || fail "stop.sh must return managed stop status $production_managed_status after teardown"
    [ "$(wc -l < "$stop_teardown_log")" -eq 3 ] \
        || fail "stop.sh must complete tmux/agent/Redis teardown after safe status $production_managed_status"
done
for production_managed_status in 1 2; do
    : > "$stop_teardown_log"
    if stop_bong_stack > /dev/null 2> "$TEST_ROOT/production-stop-$production_managed_status.log"; then
        fail "stop.sh must fail closed on managed stop status $production_managed_status"
    else
        production_result=$?
    fi
    [ "$production_result" -eq "$production_managed_status" ] \
        || fail "stop.sh must preserve unsafe managed stop status $production_managed_status"
    [ ! -s "$stop_teardown_log" ] \
        || fail "stop.sh must not teardown tmux/agent/Redis after unsafe status $production_managed_status"
done
production_managed_status="$BONG_SERVER_STOP_FORCED"
: > "$stop_teardown_log"
if run_stop_bong_stack > /dev/null 2> "$TEST_ROOT/production-stop-wrapper.log"; then
    fail "stop.sh wrapper must expose forced shutdown as non-zero"
else
    production_result=$?
fi
[ "$production_result" -eq "$BONG_SERVER_STOP_FORCED" ] \
    || fail "stop.sh wrapper must preserve forced status $BONG_SERVER_STOP_FORCED"
grep -Fq 'Done (managed bong-server required forced shutdown)' "$TEST_ROOT/production-stop-wrapper.log" \
    || fail "stop.sh wrapper must report the forced completion outcome"
unset -f bong_server_stop_managed bong_server_tmux_has_unmanaged_server tmux pkill redis-cli \
    stop_bong_stack run_stop_bong_stack stop_managed_server_before_reload
eval "$original_production_stop_managed"
eval "$original_production_tmux_scan"

# The production startup bind gate must reject a reachable listener owned by a
# foreign process; only the exact pinned server may authorize PID publication.
start_port_gate_fixture="$TEST_ROOT/start-port-gate-fixture.sh"
python3 - "$ROOT/scripts/start.sh" > "$start_port_gate_fixture" <<'PY_START_PORT'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("port_ready=0")
end = text.index("\nif [ \"$port_ready\" -ne 1 ]; then", start)
print(text[start:end])
PY_START_PORT
server_pid=424242
server_starttime=1
server_executable_identity=1:1
listener_owner_marker="$TEST_ROOT/start-port-owner.calls"
port_probe_marker="$TEST_ROOT/start-port-probe.calls"
: > "$listener_owner_marker"
: > "$port_probe_marker"
sleep() { :; }
bong_server_pinned_process_owns_ipv4_listener() {
    printf x >> "$listener_owner_marker"
    return 1
}
bong_server_port_is_open() { printf x >> "$port_probe_marker"; return 0; }
bong_server_pinned_process_status() { return 0; }
# shellcheck source=/dev/null
source "$start_port_gate_fixture"
[ "$port_ready" -eq 0 ] \
    || fail "reachable foreign listener must not satisfy production startup bind gate"
[ "$(wc -c < "$listener_owner_marker")" -eq 500 ] \
    || fail "production startup must perform exact listener ownership checks"
[ "$(wc -c < "$port_probe_marker")" -eq 0 ] \
    || fail "foreign ownership failure must short-circuit the generic connect probe"

: > "$listener_owner_marker"
: > "$port_probe_marker"
bong_server_pinned_process_owns_ipv4_listener() {
    printf x >> "$listener_owner_marker"
    return 2
}
# shellcheck source=/dev/null
source "$start_port_gate_fixture"
[ "$port_ready" -eq 0 ] \
    || fail "uninspectable listener ownership must not satisfy production startup"
[ "$listener_inspection_failed" -eq 1 ] \
    || fail "uninspectable first ownership check must remain distinguishable from no owner"
[ "$(wc -c < "$listener_owner_marker")" -eq 1 ] \
    || fail "uninspectable first ownership check must fail closed immediately"
[ "$(wc -c < "$port_probe_marker")" -eq 0 ] \
    || fail "uninspectable ownership must not reach the generic connect probe"

: > "$listener_owner_marker"
: > "$port_probe_marker"
bong_server_pinned_process_owns_ipv4_listener() {
    local calls
    printf x >> "$listener_owner_marker"
    calls="$(wc -c < "$listener_owner_marker")"
    [ "$calls" -eq 1 ] && return 0
    return 2
}
# shellcheck source=/dev/null
source "$start_port_gate_fixture"
[ "$port_ready" -eq 0 ] \
    || fail "uninspectable post-connect ownership recheck must not publish authority"
[ "$listener_inspection_failed" -eq 1 ] \
    || fail "uninspectable ownership recheck must fail closed"
[ "$(wc -c < "$listener_owner_marker")" -eq 2 ] \
    || fail "production startup must recheck ownership after connect"
[ "$(wc -c < "$port_probe_marker")" -eq 1 ] \
    || fail "post-connect inspection fixture must probe the owned port exactly once"

: > "$listener_owner_marker"
: > "$port_probe_marker"
bong_server_pinned_process_owns_ipv4_listener() {
    local calls
    printf x >> "$listener_owner_marker"
    calls="$(wc -c < "$listener_owner_marker")"
    [ "$calls" -eq 2 ] && return 1
    return 0
}
# shellcheck source=/dev/null
source "$start_port_gate_fixture"
[ "$port_ready" -eq 1 ] \
    || fail "transient post-connect ownership loss must retry until a stable owner is proven"
[ "$listener_inspection_failed" -eq 0 ] \
    || fail "reliable ownership loss must not be misclassified as inspection failure"
[ "$(wc -c < "$listener_owner_marker")" -eq 4 ] \
    || fail "stable startup authority requires ownership-connect-ownership after retry"
[ "$(wc -c < "$port_probe_marker")" -eq 2 ] \
    || fail "ownership race retry must perform a fresh connect probe"

: > "$listener_owner_marker"
: > "$port_probe_marker"
bong_server_pinned_process_owns_ipv4_listener() {
    printf x >> "$listener_owner_marker"
    return 0
}
# shellcheck source=/dev/null
source "$start_port_gate_fixture"
[ "$port_ready" -eq 1 ] \
    || fail "stable exact listener ownership must satisfy production startup"
[ "$listener_inspection_failed" -eq 0 ] \
    || fail "stable listener ownership must not report inspection failure"
[ "$(wc -c < "$listener_owner_marker")" -eq 2 ] \
    || fail "successful startup must bracket connect with two ownership checks"
[ "$(wc -c < "$port_probe_marker")" -eq 1 ] \
    || fail "successful startup must connect exactly once between ownership checks"
unset -f sleep bong_server_pinned_process_owns_ipv4_listener \
    bong_server_port_is_open bong_server_pinned_process_status

# The persistence-sensitive preview bind gate has the same three-state contract:
# reliable absence may retry, inspection failure must stop, and connect is always
# bracketed by exact process-group ownership checks.
preview_port_gate_fixture="$TEST_ROOT/preview-port-gate-fixture.sh"
python3 - "$ROOT/scripts/e2e-redis.sh" > "$preview_port_gate_fixture" <<'PY_PREVIEW_PORT'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("  NORTH_RIFT_PORT_READY=0")
end = text.index("\n  if [ \"$NORTH_RIFT_PORT_READY\" -ne 1 ]; then", start)
print(text[start:end])
PY_PREVIEW_PORT
SERVER_PID=424242
SERVER_OWNER_STARTTIME=1
SERVER_OWNER_EXECUTABLE_IDENTITY=1:1
SERVER_PGID=424242
preview_listener_marker="$TEST_ROOT/preview-port-owner.calls"
preview_probe_marker="$TEST_ROOT/preview-port-probe.calls"
: > "$preview_listener_marker"
: > "$preview_probe_marker"
sleep() { :; }
bong_server_owned_process_group_owns_ipv4_listener() {
    printf x >> "$preview_listener_marker"
    return 2
}
port_open() { printf x >> "$preview_probe_marker"; return 0; }
bong_server_pinned_process_group_status() { return 0; }
# shellcheck source=/dev/null
source "$preview_port_gate_fixture"
[ "$NORTH_RIFT_PORT_READY" -eq 0 ] \
    || fail "uninspectable preview listener ownership must not satisfy readiness"
[ "$NORTH_RIFT_LISTENER_INSPECTION_FAILED" -eq 1 ] \
    || fail "preview listener inspection failure must remain distinguishable"
[ "$(wc -c < "$preview_listener_marker")" -eq 1 ] \
    || fail "preview listener inspection failure must stop immediately"
[ "$(wc -c < "$preview_probe_marker")" -eq 0 ] \
    || fail "uninspectable preview ownership must not reach connect"

: > "$preview_listener_marker"
: > "$preview_probe_marker"
bong_server_owned_process_group_owns_ipv4_listener() {
    local calls
    printf x >> "$preview_listener_marker"
    calls="$(wc -c < "$preview_listener_marker")"
    [ "$calls" -eq 1 ] && return 0
    return 2
}
# shellcheck source=/dev/null
source "$preview_port_gate_fixture"
[ "$NORTH_RIFT_PORT_READY" -eq 0 ] \
    || fail "uninspectable preview ownership recheck must not satisfy readiness"
[ "$NORTH_RIFT_LISTENER_INSPECTION_FAILED" -eq 1 ] \
    || fail "preview ownership recheck inspection failure must fail closed"
[ "$(wc -c < "$preview_listener_marker")" -eq 2 ] \
    || fail "preview connect must be followed by an ownership recheck"
[ "$(wc -c < "$preview_probe_marker")" -eq 1 ] \
    || fail "preview recheck fixture must connect exactly once"

: > "$preview_listener_marker"
: > "$preview_probe_marker"
bong_server_owned_process_group_owns_ipv4_listener() {
    printf x >> "$preview_listener_marker"
    return 0
}
# shellcheck source=/dev/null
source "$preview_port_gate_fixture"
[ "$NORTH_RIFT_PORT_READY" -eq 1 ] \
    || fail "stable exact process-group listener ownership must satisfy preview readiness"
[ "$NORTH_RIFT_LISTENER_INSPECTION_FAILED" -eq 0 ] \
    || fail "stable preview ownership must not report inspection failure"
[ "$(wc -c < "$preview_listener_marker")" -eq 2 ] \
    || fail "successful preview readiness must bracket connect with ownership checks"
[ "$(wc -c < "$preview_probe_marker")" -eq 1 ] \
    || fail "successful preview readiness must connect exactly once"
unset -f sleep bong_server_owned_process_group_owns_ipv4_listener \
    port_open bong_server_pinned_process_group_status

grep -Fq 'app.add_systems(PostStartup, server_readiness::publish_if_requested_from_env);' "$ROOT/server/src/main.rs" \
    || fail "production readiness must run after all Startup systems complete"
if python3 - "$ROOT/server/src/main.rs" <<'PY'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
start = text.index("fn run_server() {")
end = text.index("\n}", start)
raise SystemExit("server_readiness::publish_if_requested_from_env" in text[start:end])
PY
then
    :
else
    fail "run_server must not publish readiness before the Bevy startup schedules"
fi
grep -Fq "export BONG_SERVER_READY_PATH='\$server_ready_path'" "$ROOT/scripts/start.sh" \
    || fail "start.sh must pass a private readiness path to the production server"
grep -Fq 'ready_pid="$(bong_server_read_ready_pid "$server_ready_path")"' "$ROOT/scripts/start.sh" \
    || fail "start.sh must validate readiness evidence before publishing PID authority"
grep -Fq 'bong_server_pinned_process_owns_ipv4_listener \' "$ROOT/scripts/start.sh" \
    || fail "start.sh must prove the exact pinned server owns port 25565"
grep -Fq 'bong_server_port_is_open 25565' "$ROOT/scripts/start.sh" \
    || fail "start.sh must verify the owned Valence listener accepts connections"
grep -Fq 'bong_server_stop_owned_process_group_and_release_port' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e stop must bind group teardown to pinned supervisor identity"
grep -Fq 'bong-process-group-supervisor.py' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e server launch must use the persistent process-group supervisor"
! grep -Fq 'setsid --fork' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e launch must not recover authority through a replaceable pathname"
grep -Fq 'bong_server_rollback_pinned_managed_process \' "$ROOT/scripts/start.sh" \
    || fail "production startup cleanup must delegate to the pinned TERM/wait/KILL helper"
grep -Fq 'cleanup_pinned_server_or_preserve_tmux \' "$ROOT/scripts/start.sh" \
    || fail "readiness and record failure paths must use safe pinned cleanup"
grep -Fq 'preserving tmux for diagnosis' "$ROOT/scripts/start.sh" \
    || fail "uninspectable startup failures must preserve tmux instead of sending HUP"
grep -Fq 'coproc BONG_SERVER_SUPERVISOR' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e launch must hold an uncommitted supervisor control pipe"
grep -Fq 'printf C >&"$control_fd"' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e launch must commit supervisor authority only after identity pinning"
grep -Fq '[ "$committed_line" = COMMITTED ]' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e launch must require an exact COMMITTED acknowledgement"
grep -Fq 'bong_server_pinned_process_group_status' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e launch must re-pin owner identity after COMMITTED"
grep -Fq 'if command != b"C":' "$ROOT/scripts/lib/bong-process-group-supervisor.py" \
    || fail "supervisor must roll back when startup authority is not committed"
grep -Fq 'sys.stdout.buffer.write(b"COMMITTED\n")' "$ROOT/scripts/lib/bong-process-group-supervisor.py" \
    || fail "supervisor must publish the COMMITTED acknowledgement"

"$ROOT/scripts/test-process-group-snapshot.sh" \
    || fail "process-group snapshot regression must pass before lifecycle suite succeeds"

printf 'PASS: server lifecycle ownership, pidfd signaling, process groups, readiness, locks, and persistence handoff are fail-closed\n'
