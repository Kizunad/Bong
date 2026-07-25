#!/usr/bin/env bash

bong_server_pid_file() {
    printf '%s\n' "${BONG_SERVER_PID_FILE:-${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/bong-server-${UID}.pid}"
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
    local pid="" starttime="" executable=""
    local count=0

    record="$(bong_server_pid_file)"
    [ -f "$record" ] || return 1
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            pid=*) key=pid; value="${line#pid=}" ;;
            starttime=*) key=starttime; value="${line#starttime=}" ;;
            executable=*) key=executable; value="${line#executable=}" ;;
            *) return 1 ;;
        esac
        case "$key" in
            pid) [ -z "$pid" ] || return 1; pid="$value" ;;
            starttime) [ -z "$starttime" ] || return 1; starttime="$value" ;;
            executable) [ -z "$executable" ] || return 1; executable="$value" ;;
        esac
        count=$((count + 1))
    done < "$record"

    [ "$count" -eq 3 ] || return 1
    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [[ "$starttime" =~ ^[0-9]+$ ]] || return 1
    [ -n "$executable" ] || return 1
    BONG_SERVER_RECORDED_PID="$pid"
    BONG_SERVER_RECORDED_STARTTIME="$starttime"
    BONG_SERVER_RECORDED_EXECUTABLE="$executable"
}

bong_server_record_matches_process() {
    local actual_starttime actual_executable

    bong_server_process_is_running "$BONG_SERVER_RECORDED_PID" || return 1
    actual_starttime="$(bong_server_process_starttime "$BONG_SERVER_RECORDED_PID")" || return 1
    actual_executable="$(bong_server_process_executable "$BONG_SERVER_RECORDED_PID")" || return 1
    [ "$actual_starttime" = "$BONG_SERVER_RECORDED_STARTTIME" ] \
        && [ "$actual_executable" = "$BONG_SERVER_RECORDED_EXECUTABLE" ]
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

    if ! bong_server_read_record; then
        return 0
    fi
    if [ "$BONG_SERVER_RECORDED_PID" = "$expected_pid" ] \
        && [ "$BONG_SERVER_RECORDED_STARTTIME" = "$expected_starttime" ] \
        && [ "$BONG_SERVER_RECORDED_EXECUTABLE" = "$expected_executable" ]; then
        bong_server_clear_record
    fi
}

bong_server_write_record() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local record directory temporary starttime executable

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [ -n "$expected_executable" ] || return 1
    bong_server_process_is_running "$pid" || return 1
    starttime="$(bong_server_process_starttime "$pid")" || return 1
    executable="$(bong_server_process_executable "$pid")" || return 1
    expected_executable="$(readlink -f -- "$expected_executable")" || return 1
    [ "$executable" = "$expected_executable" ] || return 1

    record="$(bong_server_pid_file)"
    directory="$(dirname -- "$record")"
    mkdir -p -- "$directory" || return 1
    temporary="$(mktemp "$directory/.bong-server.pid.XXXXXX")" || return 1
    if ! printf 'pid=%s\nstarttime=%s\nexecutable=%s\n' "$pid" "$starttime" "$executable" > "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    mv -f -- "$temporary" "$record"
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

bong_server_stop_managed() {
    local grace_seconds="${BONG_SERVER_STOP_GRACE_SECONDS:-10}"
    local pid starttime executable

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || {
        echo "FAIL: BONG_SERVER_STOP_GRACE_SECONDS must be a non-negative integer: $grace_seconds" >&2
        return 1
    }
    if ! bong_server_read_record; then
        bong_server_clear_record
        return 0
    fi
    if ! bong_server_record_matches_process; then
        bong_server_clear_record_if_matches \
            "$BONG_SERVER_RECORDED_PID" "$BONG_SERVER_RECORDED_STARTTIME" "$BONG_SERVER_RECORDED_EXECUTABLE"
        return 0
    fi

    pid="$BONG_SERVER_RECORDED_PID"
    starttime="$BONG_SERVER_RECORDED_STARTTIME"
    executable="$BONG_SERVER_RECORDED_EXECUTABLE"
    kill -TERM "$pid" 2>/dev/null || true
    if bong_server_wait_for_exit "$pid" "$grace_seconds"; then
        bong_server_clear_record_if_matches "$pid" "$starttime" "$executable"
        return 0
    fi
    if ! bong_server_read_record || ! bong_server_record_matches_process; then
        bong_server_clear_record_if_matches "$pid" "$starttime" "$executable"
        return 0
    fi
    if [ "$BONG_SERVER_RECORDED_PID" != "$pid" ] \
        || [ "$BONG_SERVER_RECORDED_STARTTIME" != "$starttime" ] \
        || [ "$BONG_SERVER_RECORDED_EXECUTABLE" != "$executable" ]; then
        return 0
    fi

    kill -KILL "$pid" 2>/dev/null || true
    if ! bong_server_wait_for_exit "$pid" 2; then
        echo "FAIL: managed bong-server pid $pid did not exit after SIGKILL" >&2
        return 1
    fi
    bong_server_clear_record_if_matches "$pid" "$starttime" "$executable"
}
