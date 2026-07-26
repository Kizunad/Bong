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

bong_server_stash_persistence "$data_dir" "$stash_dir" \
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

bong_server_restore_persistence "$data_dir" "$stash_dir" \
    || fail "restore must succeed after a stash + preview-server-write cycle"
[ "$(cat "$data_dir/bong.db")" = "original-db" ] \
    || fail "restore must overwrite the preview-created bong.db with the pre-stash original because restore is authoritative, not a merge"
[ "$(cat "$data_dir/bong.db-wal")" = "original-wal" ] \
    || fail "restore must bring back bong.db-wal exactly as it was before stash"
[ "$(cat "$data_dir/bong.db-shm")" = "original-shm" ] \
    || fail "restore must bring back bong.db-shm exactly as it was before stash"
[ ! -d "$stash_dir" ] \
    || fail "restore must remove the now-empty stash_dir so a repeated cleanup-trap restore call falls into the safe no-op branch instead of re-deleting what it just restored"

# 只有 bong.db（无 -wal/-shm）时的精确还原语义
solo_root="$TEST_ROOT/persistence-solo"
solo_data="$solo_root/data"
solo_stash="$solo_root/stash"
mkdir -p "$solo_data"
printf 'solo-original-db\n' > "$solo_data/bong.db"

bong_server_stash_persistence "$solo_data" "$solo_stash" \
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

bong_server_restore_persistence "$solo_data" "$solo_stash" \
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
bong_server_stash_persistence "$empty_data" "$empty_stash" \
    || fail "stash on an empty data_dir must succeed as a no-op instead of failing when there is nothing to move"
[ -z "$(ls -A "$empty_data" 2>/dev/null)" ] \
    || fail "stash on an empty data_dir must not fabricate any file in data_dir"

# stash_dir 不存在时 restore 必须是成功的 no-op，且不得触碰 data_dir
missing_stash_root="$TEST_ROOT/persistence-missing-stash"
missing_stash_data="$missing_stash_root/data"
missing_stash_dir="$missing_stash_root/stash"
mkdir -p "$missing_stash_data"
printf 'untouched\n' > "$missing_stash_data/bong.db"
bong_server_restore_persistence "$missing_stash_data" "$missing_stash_dir" \
    || fail "restore must succeed as a no-op when stash_dir does not exist"
[ "$(cat "$missing_stash_data/bong.db")" = "untouched" ] \
    || fail "restore no-op (missing stash_dir) must leave pre-existing data_dir files untouched"

# 幂等回归：对齐 e2e-redis.sh 的真实调用模式（阶段末尾 restore 一次 +
# cleanup trap 兜底再 restore 一次），第二次调用绝不能把第一次刚还原回去
# 的文件当成"专用 server 残留"误删——否则每次成功的 e2e 跑完都会悄悄清空
# 开发者本地存档。
double_root="$TEST_ROOT/persistence-double-restore"
double_data="$double_root/data"
double_stash="$double_root/stash"
mkdir -p "$double_data"
printf 'double-original\n' > "$double_data/bong.db"
bong_server_stash_persistence "$double_data" "$double_stash" \
    || fail "stash must succeed for the double-restore idempotency fixture"
bong_server_restore_persistence "$double_data" "$double_stash" \
    || fail "first restore of the double-restore fixture must succeed"
bong_server_restore_persistence "$double_data" "$double_stash" \
    || fail "second (idempotent) restore call must not error"
[ "$(cat "$double_data/bong.db")" = "double-original" ] \
    || fail "second (idempotent) restore call must not delete the file the first restore just brought back, expected 'double-original' still present"

restore_occurrences="$(grep -c 'bong_server_restore_persistence' "$ROOT/scripts/e2e-redis.sh" || true)"
grep -Fq 'bong_server_stash_persistence' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e-redis.sh north-rift stage must call bong_server_stash_persistence before starting the dedicated preview server"
[ "${restore_occurrences:-0}" -ge 2 ] \
    || fail "e2e-redis.sh must call bong_server_restore_persistence both at the end of the north-rift stage and inside the cleanup trap, found ${restore_occurrences:-0}"
grep -Fq 'source "$ROOT/scripts/lib/bong-server-lifecycle.sh"' "$ROOT/scripts/e2e-redis.sh" \
    || fail "e2e-redis.sh must source bong-server-lifecycle.sh to reach the stash/restore helpers"

echo "PASS: managed PID lifecycle validates identity, TERM/KILL, stale records, and no name-based server kills"
