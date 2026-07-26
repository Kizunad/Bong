#!/usr/bin/env bash

bong_server_pid_file() {
    printf '%s\n' "${BONG_SERVER_PID_FILE:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/bong-server-${UID}.pid}"
}

bong_server_with_lock() {
    local record lock directory fd status

    if [ "${BONG_SERVER_LIFECYCLE_LOCK_DEPTH:-0}" -gt 0 ]; then
        "$@"
        return $?
    fi

    record="$(bong_server_pid_file)"
    directory="$(dirname -- "$record")"
    mkdir -p -- "$directory" || return 1
    lock="${record}.lock"
    exec {fd}>"$lock" || return 1
    flock -x "$fd" || {
        exec {fd}>&-
        return 1
    }
    BONG_SERVER_LIFECYCLE_LOCK_DEPTH=1 "$@"
    status=$?
    flock -u "$fd"
    exec {fd}>&-
    return "$status"
}

bong_server_process_is_running() {
    local pid="${1:-}"
    local state

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    state="$(ps -o stat= -p "$pid" 2>/dev/null)" || return 1
    [[ "$state" != Z* ]]
}

bong_server_process_starttime() {
    local pid="${1:-}"
    local stat rest
    local -a fields

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    IFS= read -r stat < "/proc/$pid/stat" || return 1
    rest="${stat#*) }"
    [ "$rest" != "$stat" ] || return 1
    read -r -a fields <<< "$rest"
    [ "${#fields[@]}" -ge 20 ] || return 1
    printf '%s\n' "${fields[19]}"
}

bong_server_process_executable() {
    local pid="${1:-}"

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    readlink -f -- "/proc/$pid/exe" 2>/dev/null
}

bong_server_process_executable_identity() {
    local pid="${1:-}"

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    stat -Lc '%d:%i' -- "/proc/$pid/exe" 2>/dev/null
}

bong_server_resolve_executable() {
    local working_directory="${1:-}"
    local executable_path="${2:-}"

    [ -d "$working_directory" ] || return 1
    [ -n "$executable_path" ] || return 1
    (
        cd "$working_directory" || return 1
        readlink -f -- "$executable_path"
    )
}

bong_server_read_record() {
    local record line key value
    local pid="" starttime="" executable="" executable_identity=""
    local count=0

    record="$(bong_server_pid_file)"
    [ -f "$record" ] || return 1
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            pid=*) key=pid; value="${line#pid=}" ;;
            starttime=*) key=starttime; value="${line#starttime=}" ;;
            executable=*) key=executable; value="${line#executable=}" ;;
            executable_identity=*) key=executable_identity; value="${line#executable_identity=}" ;;
            *) return 1 ;;
        esac
        case "$key" in
            pid) [ -z "$pid" ] || return 1; pid="$value" ;;
            starttime) [ -z "$starttime" ] || return 1; starttime="$value" ;;
            executable) [ -z "$executable" ] || return 1; executable="$value" ;;
            executable_identity) [ -z "$executable_identity" ] || return 1; executable_identity="$value" ;;
        esac
        count=$((count + 1))
    done < "$record"

    [ "$count" -eq 4 ] || return 1
    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [[ "$starttime" =~ ^[0-9]+$ ]] || return 1
    [ -n "$executable" ] || return 1
    [[ "$executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 1
    BONG_SERVER_RECORDED_PID="$pid"
    BONG_SERVER_RECORDED_STARTTIME="$starttime"
    BONG_SERVER_RECORDED_EXECUTABLE="$executable"
    BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY="$executable_identity"
}

bong_server_record_matches_process() {
    local actual_starttime actual_executable_identity

    bong_server_process_is_running "$BONG_SERVER_RECORDED_PID" || return 1
    actual_starttime="$(bong_server_process_starttime "$BONG_SERVER_RECORDED_PID")" || return 1
    [ "$actual_starttime" = "$BONG_SERVER_RECORDED_STARTTIME" ] || return 1
    actual_executable_identity="$(bong_server_process_executable_identity "$BONG_SERVER_RECORDED_PID")" || return 1
    [ "$actual_executable_identity" = "$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY" ]
}

bong_server_clear_record() {
    local record

    record="$(bong_server_pid_file)"
    rm -f -- "$record"
}

bong_server_clear_record_if_matches() {
    local expected_pid="${1:-}"
    local expected_starttime="${2:-}"
    local expected_executable="${3:-}"
    local expected_executable_identity="${4:-}"

    if ! bong_server_read_record; then
        return 0
    fi
    if [ "$BONG_SERVER_RECORDED_PID" = "$expected_pid" ] \
        && [ "$BONG_SERVER_RECORDED_STARTTIME" = "$expected_starttime" ] \
        && [ "$BONG_SERVER_RECORDED_EXECUTABLE" = "$expected_executable" ] \
        && [ "$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY" = "$expected_executable_identity" ]; then
        bong_server_clear_record
    fi
}

_bong_server_write_record() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local record directory temporary starttime executable executable_identity

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [ -n "$expected_executable" ] || return 1
    bong_server_process_is_running "$pid" || return 1
    starttime="$(bong_server_process_starttime "$pid")" || return 1
    executable="$(bong_server_process_executable "$pid")" || return 1
    executable_identity="$(bong_server_process_executable_identity "$pid")" || return 1
    expected_executable="$(readlink -f -- "$expected_executable")" || return 1
    [ "$executable" = "$expected_executable" ] || return 1

    record="$(bong_server_pid_file)"
    directory="$(dirname -- "$record")"
    mkdir -p -- "$directory" || return 1
    temporary="$(mktemp "$directory/.bong-server.pid.XXXXXX")" || return 1
    if ! printf 'pid=%s\nstarttime=%s\nexecutable=%s\nexecutable_identity=%s\n' \
        "$pid" "$starttime" "$executable" "$executable_identity" > "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    mv -f -- "$temporary" "$record"
}

bong_server_write_record() {
    bong_server_with_lock _bong_server_write_record "$@"
}

bong_server_wait_for_executable() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local attempts="${3:-500}"
    local actual_executable attempt

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [ -n "$expected_executable" ] || return 1
    [[ "$attempts" =~ ^[0-9]+$ ]] || return 1
    expected_executable="$(readlink -f -- "$expected_executable")" || return 1
    for ((attempt = 0; attempt < attempts; attempt++)); do
        bong_server_process_is_running "$pid" || return 1
        actual_executable="$(bong_server_process_executable "$pid")" || actual_executable=""
        if [ "$actual_executable" = "$expected_executable" ]; then
            return 0
        fi
        sleep 0.01
    done
    return 1
}

bong_server_wait_for_exit() {
    local pid="${1:-}"
    local grace_seconds="${2:-}"
    local deadline

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 1
    deadline=$((SECONDS + grace_seconds))
    while bong_server_process_is_running "$pid"; do
        [ "$SECONDS" -lt "$deadline" ] || return 1
        sleep 0.05
    done
}

bong_server_process_tree_has_server() {
    local pid="${1:-}"
    local child executable executable_name command_line

    bong_server_process_is_running "$pid" || return 1
    executable="$(bong_server_process_executable "$pid")" || executable=""
    executable_name="$(basename -- "$executable")"
    command_line="$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null || true)"
    if [ "$executable_name" = bong-server ] \
        || [[ "$command_line" == *bong-server* ]]; then
        return 0
    fi

    while IFS= read -r child; do
        bong_server_process_tree_has_server "$child" && return 0
    done < <(pgrep -P "$pid" 2>/dev/null || true)
    return 1
}

bong_server_tmux_has_unmanaged_server() {
    local session="${1:-}"
    local pane_pid

    [ -n "$session" ] || return 1
    while IFS= read -r pane_pid; do
        bong_server_process_tree_has_server "$pane_pid" && return 0
    done < <(tmux list-panes -a -t "$session" -F '#{pane_pid}' 2>/dev/null)
    return 1
}

_bong_server_stop_managed() {
    local grace_seconds="${BONG_SERVER_STOP_GRACE_SECONDS:-10}"
    local record pid starttime executable executable_identity

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || {
        echo "FAIL: BONG_SERVER_STOP_GRACE_SECONDS must be a non-negative integer: $grace_seconds" >&2
        return 1
    }
    record="$(bong_server_pid_file)"
    if [ ! -e "$record" ]; then
        return 0
    fi
    if ! bong_server_read_record; then
        echo "FAIL: managed bong-server record is malformed; refusing to destroy an unverified session" >&2
        return 2
    fi

    pid="$BONG_SERVER_RECORDED_PID"
    starttime="$BONG_SERVER_RECORDED_STARTTIME"
    executable="$BONG_SERVER_RECORDED_EXECUTABLE"
    executable_identity="$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY"
    if ! bong_server_process_is_running "$pid"; then
        bong_server_clear_record_if_matches "$pid" "$starttime" "$executable" "$executable_identity"
        return 0
    fi
    if ! bong_server_record_matches_process; then
        echo "FAIL: managed bong-server record no longer identifies pid $pid; refusing to signal an unverified process" >&2
        return 2
    fi

    kill -TERM "$pid" 2>/dev/null || true
    if bong_server_wait_for_exit "$pid" "$grace_seconds"; then
        bong_server_clear_record_if_matches "$pid" "$starttime" "$executable" "$executable_identity"
        return 0
    fi
    if ! bong_server_process_is_running "$pid"; then
        bong_server_clear_record_if_matches "$pid" "$starttime" "$executable" "$executable_identity"
        return 0
    fi
    if ! bong_server_record_matches_process; then
        echo "FAIL: managed bong-server identity changed while waiting; refusing SIGKILL" >&2
        return 2
    fi

    kill -KILL "$pid" 2>/dev/null || true
    if ! bong_server_wait_for_exit "$pid" 2; then
        echo "FAIL: managed bong-server pid $pid did not exit after SIGKILL" >&2
        return 1
    fi
    bong_server_clear_record_if_matches "$pid" "$starttime" "$executable" "$executable_identity"
}

bong_server_stop_managed() {
    bong_server_with_lock _bong_server_stop_managed
}

# 把 <data_dir>/bong.db{,-wal,-shm} 中存在的文件挪到 <stash_dir>/，用于在
# 另起一台共用 cwd/持久化路径的专用 server（如 north-rift preview）之前，
# 把开发者本地真实存档暂时移出去，避免被专用 server 的 hydrate/flush 读写。
# 幂等：源文件已不在 data_dir（例如已经 stash 过一次）时视为成功 no-op。
bong_server_stash_persistence() {
    local data_dir="${1:-}"
    local stash_dir="${2:-}"
    local suffix source_file

    [ -n "$data_dir" ] || return 1
    [ -n "$stash_dir" ] || return 1
    [ -d "$data_dir" ] || return 1

    mkdir -p -- "$stash_dir" || return 1

    for suffix in "" "-wal" "-shm"; do
        source_file="$data_dir/bong.db$suffix"
        if [ -e "$source_file" ]; then
            mv -- "$source_file" "$stash_dir/bong.db$suffix" || return 1
        fi
    done

    return 0
}

# bong_server_stash_persistence 的逆操作：把 stash_dir 里的 bong.db{,-wal,-shm}
# 精确还原回 data_dir——stash 里没有的那个后缀会把 data_dir 里对应文件删掉
# （清掉专用 server 自己新造的残留），保证还原成 stash 那一刻的精确状态。
# 幂等：stash_dir 不存在时视为成功 no-op；一次成功还原后会清空并删除
# stash_dir 本身，使重复调用（例如"阶段末尾 + cleanup trap 兜底"两次调用）
# 天然落入这个 no-op 分支，不会在第二次调用时把刚还原回去的文件当"专用
# server 残留"误删。
bong_server_restore_persistence() {
    local data_dir="${1:-}"
    local stash_dir="${2:-}"
    local suffix stash_file data_file

    [ -n "$data_dir" ] || return 1
    [ -n "$stash_dir" ] || return 1

    [ -d "$stash_dir" ] || return 0

    mkdir -p -- "$data_dir" || return 1

    for suffix in "" "-wal" "-shm"; do
        stash_file="$stash_dir/bong.db$suffix"
        data_file="$data_dir/bong.db$suffix"
        if [ -e "$stash_file" ]; then
            mv -f -- "$stash_file" "$data_file" || return 1
        else
            rm -f -- "$data_file" || return 1
        fi
    done

    rm -rf -- "$stash_dir" 2>/dev/null || true
    return 0
}
