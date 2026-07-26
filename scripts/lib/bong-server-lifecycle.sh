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
# 搬完之后写一份清单 <stash_dir>/stashed-files，逐行记录本次真正搬走的
# 文件名（"清单存在 ⇒ 搬运已完成"）——bong_server_restore_persistence 靠
# 这份清单判定"这个后缀本来就该恢复"还是"这个后缀本来就不存在、纯属
# 专用 server 造的垃圾"，而不是靠"stash 里还有没有这个文件"反推（那样
# 会把"已经被上一次调用还原走"和"从来没备份过"这两种相反状态混为一谈）。
# 幂等：源文件已不在 data_dir 且清单已存在时（例如已经 stash 过一次）视为
# 成功 no-op，不覆盖已有清单。
bong_server_stash_persistence() {
    local data_dir="${1:-}"
    local stash_dir="${2:-}"
    local manifest_file suffix source_file
    local -a moved=()

    [ -n "$data_dir" ] || return 1
    [ -n "$stash_dir" ] || return 1
    [ -d "$data_dir" ] || return 1

    mkdir -p -- "$stash_dir" || return 1

    for suffix in "" "-wal" "-shm"; do
        source_file="$data_dir/bong.db$suffix"
        if [ -e "$source_file" ]; then
            mv -- "$source_file" "$stash_dir/bong.db$suffix" || return 1
            moved+=("bong.db$suffix")
        fi
    done

    manifest_file="$stash_dir/stashed-files"
    if [ ! -e "$manifest_file" ]; then
        : > "$manifest_file" || return 1
        if [ "${#moved[@]}" -gt 0 ]; then
            printf '%s\n' "${moved[@]}" >> "$manifest_file" || return 1
        fi
    fi

    return 0
}

# bong_server_stash_persistence 的逆操作：依 <stash_dir>/stashed-files 清单
# 把 bong.db{,-wal,-shm} 精确还原回 data_dir——清单里没记录的后缀是专用
# server 自己造的垃圾，rm -f 掉；清单里记录过的后缀，stash 里还有就 mv -f
# 拿回去，stash 里已经没有但 data_dir 里已经存在，说明是上一次（可能中途
# 失败）的 restore 调用已经正确搬回去了，原样保留、绝不删除；两边都没有
# 则是异常状态（例如上一次中途失败），直接报错，绝不静默当"没备份过"处理。
#
# 幂等 / 部分失败安全：即使某次 restore 调用在处理到一半时失败（例如某个
# 非清单内的后缀因为被外部造成目录而 rm -f 报错），已经成功还原的文件也
# 不会在下一次重试调用（如 cleanup trap 的无条件二次调用）里被误判成
# "没备份过"而删掉——因为判定始终以清单为准，不看 stash_dir 里还剩什么。
# stash_dir 存在但清单缺失/不可读时 fail closed：一个文件都不许碰，直接
# 返回非零，把异常留给人工排查而不是猜测。全部处理完成后才 rm -rf
# stash_dir；stash_dir 本不存在时视为成功 no-op。
bong_server_restore_persistence() {
    local data_dir="${1:-}"
    local stash_dir="${2:-}"
    local manifest_file suffix filename stash_file data_file

    [ -n "$data_dir" ] || return 1
    [ -n "$stash_dir" ] || return 1

    [ -d "$stash_dir" ] || return 0

    manifest_file="$stash_dir/stashed-files"
    if [ ! -f "$manifest_file" ] || [ ! -r "$manifest_file" ]; then
        echo "FAIL: stash manifest missing or unreadable at $manifest_file; refusing to touch $data_dir without knowing which files were actually stashed (fail closed, no files removed)" >&2
        return 1
    fi

    mkdir -p -- "$data_dir" || return 1

    for suffix in "" "-wal" "-shm"; do
        filename="bong.db$suffix"
        stash_file="$stash_dir/$filename"
        data_file="$data_dir/$filename"
        if grep -Fxq -- "$filename" "$manifest_file"; then
            if [ -e "$stash_file" ]; then
                mv -f -- "$stash_file" "$data_file" || return 1
            elif [ -e "$data_file" ]; then
                : # 已被上一次（可能中途失败的）restore 调用还原过，原样保留
            else
                echo "FAIL: $filename was recorded as stashed but is missing from both $stash_dir and $data_dir; a previous restore likely failed partway through, refusing to proceed silently" >&2
                return 1
            fi
        else
            rm -f -- "$data_file" || return 1
        fi
    done

    rm -rf -- "$stash_dir" 2>/dev/null || true
    return 0
}
