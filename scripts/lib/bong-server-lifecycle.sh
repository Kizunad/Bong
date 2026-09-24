#!/usr/bin/env bash

# Capture the library directory while this file is sourced. Function-time
# BASH_SOURCE may point at a test/caller frame after declare/eval replacement.
BONG_SERVER_LIFECYCLE_LIBRARY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$BONG_SERVER_LIFECYCLE_LIBRARY_DIR/bong-cargo-target.sh"

# Environment values are not lock authority. Sourcing this library resets the
# private reentrancy state; only bong_server_with_lock may populate it after a
# verified flock succeeds in this shell process.
unset BONG_SERVER_LIFECYCLE_LOCK_DEPTH BONG_SERVER_LIFECYCLE_LOCK_FD BONG_SERVER_LIFECYCLE_LOCK_PATH BONG_SERVER_LIFECYCLE_LOCK_IDENTITY
BONG_SERVER_LIFECYCLE_LOCK_OWNER_BASHPID=""

BONG_SERVER_STOP_FORCED=3

# Linux kill(2) reserves 0 and negative values for process-group or broad
# delivery. PID 1 is never a Bong-owned child. Every signal authority and the
# process identity readers feeding it must reject these values before touching
# /proc, pgrep, ps, kill, or pidfd.
bong_server_validate_signal_id() {
    local value="${1:-}"

    [[ "$value" =~ ^[0-9]+$ ]] && [ "$value" -gt 1 ]
}

bong_server_validate_real_directory() {
    local directory="${1:-}"
    [ -n "$directory" ] && [ -d "$directory" ] && [ ! -L "$directory" ]
}

bong_server_validate_secure_directory() {
    local directory="${1:-}"
    local expected_mode="${2:-700}"
    local owner mode

    [ -n "$directory" ] && [ -d "$directory" ] && [ ! -L "$directory" ] || return 1
    owner="$(stat -Lc '%u' -- "$directory" 2>/dev/null)" || return 1
    mode="$(stat -Lc '%a' -- "$directory" 2>/dev/null)" || return 1
    [ "$owner" = "$UID" ] && [ "$mode" = "$expected_mode" ]
}

bong_server_runtime_dir() {
    local parent candidate

    if [ -n "${XDG_RUNTIME_DIR:-}" ] && bong_server_validate_secure_directory "$XDG_RUNTIME_DIR" 700; then
        parent="$XDG_RUNTIME_DIR"
        candidate="$parent/bong"
    else
        parent="/tmp"
        candidate="/tmp/bong-${UID}"
    fi
    if [ ! -e "$candidate" ] && [ ! -L "$candidate" ]; then
        (umask 077 && mkdir -- "$candidate") || return 1
    fi
    if ! bong_server_validate_secure_directory "$candidate" 700; then
        echo "FAIL: insecure bong runtime directory $candidate (requires non-symlink directory owned by uid $UID with mode 0700)" >&2
        return 1
    fi
    printf '%s\n' "$candidate"
}

bong_server_pid_file() {
    if [ -n "${BONG_SERVER_PID_FILE:-}" ]; then
        printf '%s\n' "$BONG_SERVER_PID_FILE"
    else
        printf '%s/bong-server.pid\n' "$(bong_server_runtime_dir)" || return 1
    fi
}

bong_server_validate_pid_record_parent() {
    local record="${1:-}" directory canonical lexical

    [ -n "$record" ] || return 1
    directory="$(dirname -- "$record")"
    canonical="$(realpath -e -- "$directory" 2>/dev/null)" || return 1
    lexical="$(realpath -ms -- "$directory" 2>/dev/null)" || return 1
    [ "$canonical" = "$lexical" ] || return 1
    bong_server_validate_secure_directory "$canonical" 700
}

# Test seams deliberately run after open / before the final unlink validation.
# Production keeps them no-op; lifecycle tests replace them to prove that a
# pathname replacement cannot turn a checked record into a signal authority.
bong_server_after_record_open() {
    :
}

bong_server_before_record_clear_remove() {
    :
}

bong_server_validate_lock_file() {
    local lock="${1:-}"
    local owner mode links

    [ -f "$lock" ] && [ ! -L "$lock" ] || return 1
    owner="$(stat -Lc '%u' -- "$lock" 2>/dev/null)" || return 1
    mode="$(stat -Lc '%a' -- "$lock" 2>/dev/null)" || return 1
    links="$(stat -Lc '%h' -- "$lock" 2>/dev/null)" || return 1
    [ "$owner" = "$UID" ] && [ "$mode" = "600" ] && [ "$links" = "1" ]
}

# PID records and advisory locks have the same trust boundary: they must be a
# private, single-link regular file owned by this invocation's uid.
bong_server_validate_pid_record_file() {
    bong_server_validate_lock_file "${1:-}"
}

bong_server_validate_fd_secure_regular_file() {
    local fd="${1:-}"
    local owner mode links kind

    [[ "$fd" =~ ^[0-9]+$ ]] || return 1
    kind="$(stat -Lc '%F' -- "/proc/self/fd/$fd" 2>/dev/null)" || return 1
    owner="$(stat -Lc '%u' -- "/proc/self/fd/$fd" 2>/dev/null)" || return 1
    mode="$(stat -Lc '%a' -- "/proc/self/fd/$fd" 2>/dev/null)" || return 1
    links="$(stat -Lc '%h' -- "/proc/self/fd/$fd" 2>/dev/null)" || return 1
    { [ "$kind" = "regular file" ] || [ "$kind" = "regular empty file" ]; } || return 1
    [ "$owner" = "$UID" ] \
        && [ "$mode" = "600" ] && [ "$links" = "1" ]
}

bong_server_path_identity() {
    stat -Lc '%d:%i' -- "${1:-}" 2>/dev/null
}

bong_server_fd_matches_path() {
    local fd="${1:-}" path="${2:-}" fd_identity path_identity
    [[ "$fd" =~ ^[0-9]+$ ]] || return 1
    fd_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || return 1
    path_identity="$(bong_server_path_identity "$path")" || return 1
    [ "$fd_identity" = "$path_identity" ]
}

_bong_server_lifecycle_lock_is_held_here() {
    local fd="${BONG_SERVER_LIFECYCLE_LOCK_FD:-}" path="${BONG_SERVER_LIFECYCLE_LOCK_PATH:-}"
    local expected_identity="${BONG_SERVER_LIFECYCLE_LOCK_IDENTITY:-}" actual_identity

    [ "${BONG_SERVER_LIFECYCLE_LOCK_OWNER_BASHPID:-}" = "$BASHPID" ] || return 1
    [[ "$fd" =~ ^[0-9]+$ ]] && [ -n "$path" ] && [[ "$expected_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 1
    bong_server_validate_lock_file "$path" || return 1
    bong_server_validate_fd_secure_regular_file "$fd" || return 1
    bong_server_fd_matches_path "$fd" "$path" || return 1
    actual_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || return 1
    [ "$actual_identity" = "$expected_identity" ]
}

bong_server_with_lock() {
    local record lock directory fd status timeout lock_identity

    if _bong_server_lifecycle_lock_is_held_here; then
        "$@"
        return $?
    fi
    timeout="${BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS:-10}"
    [[ "$timeout" =~ ^[0-9]+([.][0-9]+)?$ ]] || {
        echo "FAIL: BONG_SERVER_LIFECYCLE_LOCK_TIMEOUT_SECONDS must be a non-negative number: $timeout" >&2
        return 1
    }

    record="$(bong_server_pid_file)" || return 1
    directory="$(dirname -- "$record")"
    # Both default and explicit records live in a private, real directory. Do
    # not create override parents: callers must establish that trust boundary.
    bong_server_validate_pid_record_parent "$record" || {
        echo "FAIL: refusing lifecycle lock outside a secure real directory: $directory" >&2
        return 1
    }
    lock="${record}.lock"
    if [ ! -e "$lock" ] && [ ! -L "$lock" ]; then
        (umask 077 && : >> "$lock") || return 1
    fi
    if ! bong_server_validate_lock_file "$lock"; then
        echo "FAIL: insecure lifecycle lock $lock (requires regular non-symlink file owned by uid $UID with mode 0600)" >&2
        return 1
    fi
    # Append mode intentionally never truncates an existing path. Validate both
    # before and after opening so a substituted symlink/non-regular lock fails.
    exec {fd}>>"$lock" || return 1
    if ! bong_server_validate_lock_file "$lock" || ! bong_server_fd_matches_path "$fd" "$lock"; then
        exec {fd}>&-
        echo "FAIL: lifecycle lock changed or was substituted while opening: $lock" >&2
        return 1
    fi
    flock -x -w "$timeout" "$fd" || {
        echo "FAIL: timed out after ${timeout}s waiting for lifecycle lock $lock" >&2
        exec {fd}>&-
        return 1
    }
    lock_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        flock -u "$fd" || true
        exec {fd}>&-
        return 1
    }
    BONG_SERVER_LIFECYCLE_LOCK_FD="$fd"
    BONG_SERVER_LIFECYCLE_LOCK_PATH="$lock"
    BONG_SERVER_LIFECYCLE_LOCK_IDENTITY="$lock_identity"
    BONG_SERVER_LIFECYCLE_LOCK_OWNER_BASHPID="$BASHPID"
    "$@"
    status=$?
    unset BONG_SERVER_LIFECYCLE_LOCK_FD BONG_SERVER_LIFECYCLE_LOCK_PATH BONG_SERVER_LIFECYCLE_LOCK_IDENTITY BONG_SERVER_LIFECYCLE_LOCK_OWNER_BASHPID
    flock -u "$fd"
    exec {fd}>&-
    return "$status"
}

bong_server_process_is_running() {
    local pid="${1:-}"
    local state

    bong_server_validate_signal_id "$pid" || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    state="$(ps -o stat= -p "$pid" 2>/dev/null)" || {
        kill -0 "$pid" 2>/dev/null || return 1
        return 2
    }
    [[ "$state" != Z* ]]
}

# Convert a failed /proc inspection into the lifecycle tri-state. A process that
# disappeared while its metadata was read is ordinary absence; a still-live
# process whose metadata could not be inspected is an unsafe failure.
bong_server_process_inspection_failed() {
    local pid="${1:-}" status

    bong_server_process_is_running "$pid"
    status=$?
    [ "$status" -eq 1 ] && return 1
    return 2
}

# Keep stat parsing separate so callers and tests can verify that a `) ` inside
# comm cannot be mistaken for the final comm delimiter.
bong_server_parse_stat_starttime_and_group() {
    local stat="${1:-}" rest
    local -a fields

    # `##` takes the longest match, therefore stripping through the last `) `.
    rest="${stat##*) }"
    [ "$rest" != "$stat" ] || return 1
    read -r -a fields <<< "$rest"
    [ "${#fields[@]}" -ge 20 ] || return 1
    printf '%s %s\n' "${fields[19]}" "${fields[2]}"
}

# Reads the two identity fields from one /proc stat snapshot. The pgrp pin is
# needed for process-group teardown: a numeric PID alone can be reused outside
# the group between enumeration and signal delivery.
bong_server_process_starttime_and_group() {
    local pid="${1:-}" stat

    bong_server_validate_signal_id "$pid" || return 1
    { IFS= read -r stat < "/proc/$pid/stat"; } 2>/dev/null || return 1
    bong_server_parse_stat_starttime_and_group "$stat"
}

bong_server_process_starttime() {
    local snapshot

    snapshot="$(bong_server_process_starttime_and_group "${1:-}")" || return 1
    printf '%s\n' "${snapshot%% *}"
}

bong_server_process_executable() {
    local pid="${1:-}"

    bong_server_validate_signal_id "$pid" || return 1
    readlink -f -- "/proc/$pid/exe" 2>/dev/null
}

bong_server_process_executable_identity() {
    local pid="${1:-}"

    bong_server_validate_signal_id "$pid" || return 1
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
    local record line key value fd record_identity opened_identity
    local pid="" starttime="" executable="" executable_identity=""
    local count=0

    record="$(bong_server_pid_file)" || return 1
    bong_server_validate_pid_record_parent "$record" || return 1
    bong_server_validate_pid_record_file "$record" || return 1
    record_identity="$(bong_server_path_identity "$record")" || return 1
    exec {fd}<"$record" || return 1
    bong_server_after_record_open "$record" "$fd"
    # Revalidate the pathname after open. The pre-open check only rejects an
    # initial bad path; this check rejects a symlink/file swap performed while
    # open(2) followed the pathname.
    if ! bong_server_validate_pid_record_file "$record" \
        || ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$record"; then
        exec {fd}<&-
        return 1
    fi
    opened_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        exec {fd}<&-
        return 1
    }
    [ "$opened_identity" = "$record_identity" ] || {
        exec {fd}<&-
        return 1
    }
    while IFS= read -r -u "$fd" line || [ -n "$line" ]; do
        case "$line" in
            pid=*) key=pid; value="${line#pid=}" ;;
            starttime=*) key=starttime; value="${line#starttime=}" ;;
            executable=*) key=executable; value="${line#executable=}" ;;
            executable_identity=*) key=executable_identity; value="${line#executable_identity=}" ;;
            *) exec {fd}<&-; return 1 ;;
        esac
        case "$key" in
            pid) [ -z "$pid" ] || { exec {fd}<&-; return 1; }; pid="$value" ;;
            starttime) [ -z "$starttime" ] || { exec {fd}<&-; return 1; }; starttime="$value" ;;
            executable) [ -z "$executable" ] || { exec {fd}<&-; return 1; }; executable="$value" ;;
            executable_identity) [ -z "$executable_identity" ] || { exec {fd}<&-; return 1; }; executable_identity="$value" ;;
        esac
        count=$((count + 1))
    done
    if ! bong_server_validate_pid_record_file "$record" \
        || ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$record"; then
        exec {fd}<&-
        return 1
    fi
    opened_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        exec {fd}<&-
        return 1
    }
    exec {fd}<&-

    [ "$count" -eq 4 ] || return 1
    bong_server_validate_signal_id "$pid" || return 1
    [[ "$starttime" =~ ^[0-9]+$ ]] || return 1
    [ -n "$executable" ] || return 1
    [[ "$executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 1
    BONG_SERVER_RECORDED_PID="$pid"
    BONG_SERVER_RECORDED_STARTTIME="$starttime"
    BONG_SERVER_RECORDED_EXECUTABLE="$executable"
    BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY="$executable_identity"
    BONG_SERVER_RECORDED_FILE_IDENTITY="$opened_identity"
}

bong_server_record_matches_process() {
    local actual_starttime actual_executable_identity status

    if bong_server_process_is_running "$BONG_SERVER_RECORDED_PID"; then
        status=0
    else
        status=$?
    fi
    [ "$status" -eq 0 ] || return "$status"
    actual_starttime="$(bong_server_process_starttime "$BONG_SERVER_RECORDED_PID")" || {
        bong_server_process_inspection_failed "$BONG_SERVER_RECORDED_PID"
        return $?
    }
    [ "$actual_starttime" = "$BONG_SERVER_RECORDED_STARTTIME" ] || return 1
    actual_executable_identity="$(bong_server_process_executable_identity "$BONG_SERVER_RECORDED_PID")" || {
        bong_server_process_inspection_failed "$BONG_SERVER_RECORDED_PID"
        return $?
    }
    [ "$actual_executable_identity" = "$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY" ]
}

bong_server_pidfd_signal() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}" signal_name="${4:-}"
    local expected_pgrp="${5:-}"
    local -a pidfd_args

    bong_server_validate_signal_id "$pid" || return 2
    [[ "$expected_starttime" =~ ^[0-9]+$ ]] || return 2
    [[ "$expected_executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
    [ -z "$expected_pgrp" ] || bong_server_validate_signal_id "$expected_pgrp" || return 2
    case "$signal_name" in TERM|KILL) ;; *) return 2 ;; esac
    [ -n "${BONG_SERVER_LIFECYCLE_LIBRARY_DIR:-}" ] || return 2
    pidfd_args=("$pid" "$expected_starttime" "$expected_executable_identity" "$signal_name")
    [ -z "$expected_pgrp" ] || pidfd_args+=("$expected_pgrp")
    python3 "$BONG_SERVER_LIFECYCLE_LIBRARY_DIR/bong-pidfd-signal.py" "${pidfd_args[@]}"
}

# 0 means the exact pinned process owns a matching IPv4 LISTEN socket; 1 means
# reliable absence/mismatch, and 2 means inspection was unavailable or malformed.
bong_server_pinned_process_owns_ipv4_listener() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}"
    local port="${4:-}" expected_pgrp="${5:-}"
    local -a owner_args

    bong_server_validate_signal_id "$pid" || return 2
    [[ "$expected_starttime" =~ ^[0-9]+$ ]] || return 2
    [[ "$expected_executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
    [[ "$port" =~ ^[0-9]+$ ]] && [ "$port" -ge 1 ] && [ "$port" -le 65535 ] || return 2
    [ -z "$expected_pgrp" ] || bong_server_validate_signal_id "$expected_pgrp" || return 2
    [ -n "${BONG_SERVER_LIFECYCLE_LIBRARY_DIR:-}" ] || return 2
    owner_args=("$pid" "$expected_starttime" "$expected_executable_identity" "$port")
    [ -z "$expected_pgrp" ] || owner_args+=("$expected_pgrp")
    python3 "$BONG_SERVER_LIFECYCLE_LIBRARY_DIR/bong-listener-owner.py" "${owner_args[@]}"
}

# 0 = the pinned process still matches; 1 = it is reliably gone/reused;
# 2 = a live process could not be inspected. Callers must fail closed on 2.
bong_server_pinned_process_status() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}"
    local actual_starttime actual_executable_identity status

    bong_server_validate_signal_id "$pid" || return 2
    [[ "$expected_starttime" =~ ^[0-9]+$ ]] || return 2
    [[ "$expected_executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
    if bong_server_process_is_running "$pid"; then
        status=0
    else
        status=$?
    fi
    [ "$status" -eq 0 ] || return "$status"
    actual_starttime="$(bong_server_process_starttime "$pid")" || {
        bong_server_process_inspection_failed "$pid"
        return $?
    }
    [ "$actual_starttime" = "$expected_starttime" ] || return 1
    actual_executable_identity="$(bong_server_process_executable_identity "$pid")" || {
        bong_server_process_inspection_failed "$pid"
        return $?
    }
    [ "$actual_executable_identity" = "$expected_executable_identity" ]
}

# Like bong_server_pinned_process_status, but also pins the /proc stat process
# group from one snapshot. 0 means every authority field still matches; 1 means
# gone/reused/mismatched; 2 means inspection failed and callers must fail closed.
bong_server_pinned_process_group_status() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}"
    local expected_pgid="${4:-}" snapshot actual_starttime actual_pgid actual_executable_identity status

    bong_server_validate_signal_id "$pid" || return 2
    [[ "$expected_starttime" =~ ^[0-9]+$ ]] || return 2
    [[ "$expected_executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
    bong_server_validate_signal_id "$expected_pgid" || return 2
    if bong_server_process_is_running "$pid"; then
        status=0
    else
        status=$?
    fi
    [ "$status" -eq 0 ] || return "$status"
    snapshot="$(bong_server_process_starttime_and_group "$pid")" || {
        bong_server_process_inspection_failed "$pid"
        return $?
    }
    read -r actual_starttime actual_pgid <<< "$snapshot"
    [ "$actual_starttime" = "$expected_starttime" ] || return 1
    [ "$actual_pgid" = "$expected_pgid" ] || return 1
    actual_executable_identity="$(bong_server_process_executable_identity "$pid")" || {
        bong_server_process_inspection_failed "$pid"
        return $?
    }
    [ "$actual_executable_identity" = "$expected_executable_identity" ]
}

bong_server_wait_for_pinned_process_exit() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}"
    local grace_seconds="${4:-}" deadline status

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 2
    deadline=$((SECONDS + grace_seconds))
    while :; do
        if bong_server_pinned_process_status \
            "$pid" "$expected_starttime" "$expected_executable_identity"; then
            status=0
        else
            status=$?
        fi
        case "$status" in
            1) return 0 ;;
            2) return 2 ;;
        esac
        [ "$SECONDS" -lt "$deadline" ] || return 1
        sleep 0.05
    done
}

# Stop a process whose immutable identity has already been pinned. Status 0 means
# TERM was delivered and the exact process exited within its grace period. Status
# 3 means identity-safe KILL cleanup completed, so AppExit/Last is not proven; 1
# means it remained alive after KILL, and 2 means identity or signaling uncertainty.
bong_server_stop_pinned_process() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable_identity="${3:-}"
    local grace_seconds="${4:-10}" kill_grace_seconds="${5:-2}" status

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 2
    [[ "$kill_grace_seconds" =~ ^[0-9]+$ ]] || return 2
    if bong_server_pidfd_signal \
        "$pid" "$expected_starttime" "$expected_executable_identity" TERM; then
        status=0
    else
        status=$?
    fi
    case "$status" in
        1) return 1 ;;
        2) return 2 ;;
    esac

    if bong_server_wait_for_pinned_process_exit \
        "$pid" "$expected_starttime" "$expected_executable_identity" "$grace_seconds"; then
        return 0
    else
        status=$?
    fi
    case "$status" in
        2) return 2 ;;
        1) ;;
        *) return 2 ;;
    esac

    if bong_server_pidfd_signal \
        "$pid" "$expected_starttime" "$expected_executable_identity" KILL; then
        status=0
    else
        status=$?
    fi
    case "$status" in
        1)
            if bong_server_pinned_process_status \
                "$pid" "$expected_starttime" "$expected_executable_identity"; then
                return 2
            else
                status=$?
            fi
            case "$status" in
                1) return "$BONG_SERVER_STOP_FORCED" ;;
                *) return 2 ;;
            esac
            ;;
        2) return 2 ;;
    esac
    if bong_server_wait_for_pinned_process_exit \
        "$pid" "$expected_starttime" "$expected_executable_identity" "$kill_grace_seconds"; then
        return "$BONG_SERVER_STOP_FORCED"
    else
        status=$?
    fi
    return "$status"
}

bong_server_remove_secure_file_if_identity() {
    local record="${1:-}" expected_identity="${2:-}" current_identity fd

    [[ "$expected_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 1
    bong_server_validate_pid_record_parent "$record" || return 1
    bong_server_validate_pid_record_file "$record" || return 1
    exec {fd}<"$record" || return 1
    bong_server_before_record_clear_remove "$record" "$fd"
    if ! bong_server_validate_pid_record_parent "$record" \
        || ! bong_server_validate_pid_record_file "$record" \
        || ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$record"; then
        exec {fd}<&-
        return 1
    fi
    current_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        exec {fd}<&-
        return 1
    }
    [ "$current_identity" = "$expected_identity" ] || {
        exec {fd}<&-
        return 1
    }
    # The private parent is rechecked immediately before unlink; an actor who
    # cannot mutate that directory cannot exchange this checked pathname after
    # the final identity comparison.
    if ! bong_server_validate_pid_record_parent "$record" \
        || ! bong_server_validate_pid_record_file "$record" \
        || ! bong_server_fd_matches_path "$fd" "$record"; then
        exec {fd}<&-
        return 1
    fi
    exec {fd}<&-
    rm -f -- "$record"
}

bong_server_clear_record() {
    local expected_identity="${1:-}" record

    record="$(bong_server_pid_file)" || return 1
    bong_server_remove_secure_file_if_identity "$record" "$expected_identity"
}

bong_server_clear_record_if_matches() {
    local expected_pid="${1:-}"
    local expected_starttime="${2:-}"
    local expected_executable="${3:-}"
    local expected_executable_identity="${4:-}"
    local record

    record="$(bong_server_pid_file)" || return 2
    bong_server_validate_pid_record_parent "$record" || return 2
    if [ ! -e "$record" ] && [ ! -L "$record" ]; then
        return 0
    fi
    bong_server_read_record || return 2
    if [ "$BONG_SERVER_RECORDED_PID" != "$expected_pid" ] \
        || [ "$BONG_SERVER_RECORDED_STARTTIME" != "$expected_starttime" ] \
        || [ "$BONG_SERVER_RECORDED_EXECUTABLE" != "$expected_executable" ] \
        || [ "$BONG_SERVER_RECORDED_EXECUTABLE_IDENTITY" != "$expected_executable_identity" ]; then
        return 1
    fi
    bong_server_clear_record "$BONG_SERVER_RECORDED_FILE_IDENTITY" || return 2
}

# An observed process exit is not a completed lifecycle operation until the
# matching authority record is gone. A valid replacement (1) and an uncertain
# read/remove failure (2) both preserve the pathname and fail the outer stop
# closed so callers cannot continue into tmux teardown or relaunch.
_bong_server_finish_managed_record_cleanup() {
    local status

    bong_server_clear_record_if_matches "$@"
    status=$?
    case "$status" in
        0) return 0 ;;
        1)
            echo "FAIL: managed bong-server PID record was replaced after the process exited; preserving the replacement" >&2
            return 2
            ;;
        *)
            echo "FAIL: could not safely remove the exited managed bong-server PID record; preserving it for diagnosis" >&2
            return 2
            ;;
    esac
}

# Roll back only the process identity captured by the caller that launched it.
# The lifecycle record is cleanup authority, not signal authority here: another
# launcher may already have published a successor by the time this rollback runs.
# In that case status 1 from clear_record_if_matches means "replacement preserved"
# and is a successful cleanup outcome for the old launch.
_bong_server_rollback_pinned_managed_process() {
    local pid="${1:-}" expected_starttime="${2:-}" expected_executable="${3:-}"
    local expected_executable_identity="${4:-}" operation="${5:-preview rollback}"
    local stop_status clear_status forced_stop=0

    if bong_server_stop_pinned_process \
        "$pid" "$expected_starttime" "$expected_executable_identity" 10 2; then
        stop_status=0
    else
        stop_status=$?
    fi
    case "$stop_status" in
        0) ;;
        1)
            # Status 1 also covers a pinned process that survived the final KILL
            # wait. Distinguish that from ordinary absence/reuse before clearing
            # any matching authority record.
            if bong_server_pinned_process_status \
                "$pid" "$expected_starttime" "$expected_executable_identity"; then
                echo "FAIL: pinned bong-server remained alive during $operation; preserving its record" >&2
                return 1
            else
                stop_status=$?
            fi
            [ "$stop_status" -eq 1 ] || {
                echo "FAIL: pinned bong-server identity became uncertain during $operation; preserving its record" >&2
                return 2
            }
            ;;
        "$BONG_SERVER_STOP_FORCED") forced_stop=1 ;;
        *)
            echo "FAIL: could not safely stop the pinned bong-server identity during $operation (status=$stop_status); preserving its record" >&2
            return "$stop_status"
            ;;
    esac

    if bong_server_clear_record_if_matches \
        "$pid" "$expected_starttime" "$expected_executable" "$expected_executable_identity"; then
        clear_status=0
    else
        clear_status=$?
    fi
    case "$clear_status" in
        0) ;;
        1)
            echo "INFO: pinned bong-server record was replaced before $operation; preserving the successor record" >&2
            ;;
        *)
            echo "FAIL: could not safely clear the pinned bong-server record during $operation (status=$clear_status); preserving it for diagnosis" >&2
            return "$clear_status"
            ;;
    esac

    if [ "$forced_stop" -eq 1 ]; then
        echo "WARN: pinned bong-server required identity-safe SIGKILL during $operation; the exact process is gone" >&2
    fi
    return 0
}

bong_server_rollback_pinned_managed_process() {
    bong_server_with_lock _bong_server_rollback_pinned_managed_process "$@"
}

bong_server_read_ready_pid() {
    local ready_path="${1:-}" directory fd line extra ready_pid

    [ -n "$ready_path" ] || return 2
    if [ ! -e "$ready_path" ] && [ ! -L "$ready_path" ]; then
        return 1
    fi
    directory="$(dirname -- "$ready_path")"
    bong_server_validate_secure_directory "$directory" 700 || return 2
    if ! { exec {fd}<"$ready_path"; } 2>/dev/null; then
        return 2
    fi
    if ! bong_server_validate_lock_file "$ready_path" \
        || ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$ready_path"; then
        exec {fd}<&-
        return 2
    fi
    IFS= read -r line <&"$fd" || {
        exec {fd}<&-
        return 2
    }
    if IFS= read -r extra <&"$fd"; then
        exec {fd}<&-
        return 2
    fi
    exec {fd}<&-
    [[ "$line" =~ ^pid=([0-9]+)$ ]] || return 2
    ready_pid="${BASH_REMATCH[1]}"
    bong_server_validate_signal_id "$ready_pid" || return 2
    printf '%s\n' "$ready_pid"
}

_bong_server_write_record() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local record directory temporary starttime executable executable_identity fd published_identity

    bong_server_validate_signal_id "$pid" || return 1
    [ -n "$expected_executable" ] || return 1
    bong_server_process_is_running "$pid" || return 1
    starttime="$(bong_server_process_starttime "$pid")" || return 1
    executable="$(bong_server_process_executable "$pid")" || return 1
    executable_identity="$(bong_server_process_executable_identity "$pid")" || return 1
    expected_executable="$(readlink -f -- "$expected_executable")" || return 1
    [ "$executable" = "$expected_executable" ] || return 1

    record="$(bong_server_pid_file)" || return 1
    directory="$(dirname -- "$record")"
    bong_server_validate_pid_record_parent "$record" || {
        echo "FAIL: refusing PID record publish outside a secure real directory: $directory" >&2
        return 1
    }
    temporary="$(umask 077 && mktemp "$directory/.bong-server.pid.XXXXXX")" || return 1
    chmod 600 -- "$temporary" || { rm -f -- "$temporary"; return 1; }
    if ! printf 'pid=%s\nstarttime=%s\nexecutable=%s\nexecutable_identity=%s\n' \
        "$pid" "$starttime" "$executable" "$executable_identity" > "$temporary"; then
        rm -f -- "$temporary"
        return 1
    fi
    bong_server_validate_pid_record_file "$temporary" || { rm -f -- "$temporary"; return 1; }
    mv -f -- "$temporary" "$record" || return 1
    exec {fd}<"$record" || return 1
    if ! bong_server_validate_pid_record_parent "$record" \
        || ! bong_server_validate_pid_record_file "$record" \
        || ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$record"; then
        exec {fd}<&-
        echo "FAIL: PID record publish was substituted or has unsafe metadata: $record" >&2
        return 1
    fi
    published_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        exec {fd}<&-
        return 1
    }
    exec {fd}<&-
    BONG_SERVER_RECORDED_FILE_IDENTITY="$published_identity"
}

bong_server_write_record() {
    bong_server_with_lock _bong_server_write_record "$@"
}

bong_server_kill_tree() {
    local pid="${1:-}" children child status

    bong_server_validate_signal_id "$pid" || return 1
    # Command substitution drops trailing newlines, so append a private status
    # delimiter. This preserves a status-0 blank child line as malformed input
    # while still allowing pgrep's status-1 "no children" result to be empty.
    children="$(
        pgrep -P "$pid" 2>/dev/null
        printf '\034%s' "$?"
    )"
    status="${children##*$'\034'}"
    children="${children%$'\034'*}"
    case "$status" in
        0) [ -n "$children" ] || {
            echo "FAIL: pgrep returned an empty child pid for $pid" >&2
            return 1
        } ;;
        1) [ -z "$children" ] || {
            echo "FAIL: pgrep returned child output with no-child status for pid $pid" >&2
            return 1
        } ;;
        *)
            echo "FAIL: could not enumerate children of pid $pid (pgrep status $status)" >&2
            return 1
            ;;
    esac
    if [ -n "$children" ]; then
        while IFS= read -r child || [ -n "$child" ]; do
            [ -n "$child" ] || { echo "FAIL: pgrep returned an empty child pid for $pid" >&2; return 1; }
            bong_server_kill_tree "$child" || return 1
        done < <(printf '%s' "$children")
    fi
    kill "$pid" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.2
    done
    kill -9 "$pid" 2>/dev/null || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        kill -0 "$pid" 2>/dev/null || return 0
        sleep 0.2
    done
    echo "FAIL: pid $pid survived SIGKILL" >&2
    return 1
}

bong_server_port_is_open() {
    local port="${1:-}"

    [[ "$port" =~ ^[0-9]+$ ]] || return 1
    python3 - "$port" <<'PY'
import socket
import sys

try:
    socket.create_connection(("127.0.0.1", int(sys.argv[1])), timeout=1.0).close()
except OSError:
    sys.exit(1)
PY
}

# E2E uses this exact helper before any persistence restore. A failed descendant
# enumeration means teardown is unconfirmed even if the parent later exits.
bong_server_stop_process_tree_and_release_port() {
    local pid="${1:-}" port="${2:-25565}" tree_stopped=1 port_released=0

    bong_server_validate_signal_id "$pid" || return 1
    [[ "$port" =~ ^[0-9]+$ ]] || return 1
    if kill -0 "$pid" 2>/dev/null; then
        if bong_server_kill_tree "$pid"; then
            if ! wait "$pid" 2>/dev/null; then
                kill -0 "$pid" 2>/dev/null && tree_stopped=0
            fi
        else
            tree_stopped=0
        fi
    fi
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        if ! bong_server_port_is_open "$port"; then
            port_released=1
            break
        fi
        sleep 0.2
    done
    [ "$tree_stopped" -eq 1 ] && [ "$port_released" -eq 1 ]
}

# Emits `pid starttime executable_identity` captured while the PID is still in
# the requested process group. A later signal must use these pins, never inspect
# the PID for the first time after this snapshot: that would accept a recycled,
# foreign process under the old numeric PID.
bong_server_process_group_members() {
    local pgid="${1:-}" pid member_pgid state found=0 process_table
    local snapshot starttime snapshot_pgrp executable_identity status

    bong_server_validate_signal_id "$pgid" || return 2
    process_table="$(ps -e -o pid=,pgid=,stat= 2>/dev/null)" || return 2
    while read -r pid member_pgid state; do
        [ -z "$pid$member_pgid$state" ] && continue
        [ -n "$pid" ] && [ -n "$member_pgid" ] && [ -n "$state" ] || {
            echo "FAIL: ps returned malformed process-group membership" >&2
            return 2
        }
        [ "$member_pgid" = "$pgid" ] || continue
        [[ "$state" = Z* ]] && continue
        snapshot="$(bong_server_process_starttime_and_group "$pid")" || {
            bong_server_process_inspection_failed "$pid"
            status=$?
            [ "$status" -eq 1 ] && continue
            return 2
        }
        read -r starttime snapshot_pgrp <<< "$snapshot"
        [[ "$starttime" =~ ^[0-9]+$ ]] && [ "$snapshot_pgrp" = "$pgid" ] || continue
        executable_identity="$(bong_server_process_executable_identity "$pid")" || {
            bong_server_process_inspection_failed "$pid"
            status=$?
            [ "$status" -eq 1 ] && continue
            return 2
        }
        [[ "$executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
        printf '%s %s %s\n' "$pid" "$starttime" "$executable_identity"
        found=1
    done <<< "$process_table"
    [ "$found" -eq 1 ]
}

# Verify that at least one exact, frozen member of an owned process group holds
# the requested IPv4 listener. The persistent owner is revalidated before the
# scan and before accepting a member, so a recycled numeric PGID is never trust.
bong_server_owned_process_group_owns_ipv4_listener() {
    local owner_pid="${1:-}" owner_starttime="${2:-}" owner_executable_identity="${3:-}"
    local pgid="${4:-}" port="${5:-}" members status pid starttime executable_identity

    bong_server_pinned_process_group_status \
        "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$pgid" || return $?
    members="$(bong_server_process_group_members "$pgid")"
    status=$?
    case "$status" in
        0) ;;
        1) return 1 ;;
        *) return 2 ;;
    esac
    while read -r pid starttime executable_identity; do
        [ -n "$pid$starttime$executable_identity" ] || continue
        [ "$pid" = "$owner_pid" ] && continue
        bong_server_pinned_process_group_status \
            "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$pgid" || return 2
        if bong_server_pinned_process_owns_ipv4_listener \
            "$pid" "$starttime" "$executable_identity" "$port" "$pgid"; then
            return 0
        else
            status=$?
        fi
        [ "$status" -eq 1 ] || return 2
    done <<< "$members"
    return 1
}

# E2E servers run below a persistent session leader whose PID/starttime/executable
# are pinned before use. The leader cannot be replaced while any old group member
# remains, so checking it immediately before every scan closes numeric-PGID reuse.
bong_server_stop_owned_process_group_and_release_port() {
    local owner_pid="${1:-}" owner_starttime="${2:-}" owner_executable_identity="${3:-}"
    local pgid="${4:-}" port="${5:-25565}"
    local grace_seconds="${6:-10}" kill_grace_seconds="${7:-2}"
    local current_pgid members status pid starttime executable_identity
    local forced_stop=0 forced_children=0

    bong_server_validate_signal_id "$owner_pid" || return 2
    [[ "$owner_starttime" =~ ^[0-9]+$ ]] || return 2
    [[ "$owner_executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
    [ "$pgid" = "$owner_pid" ] || {
        echo "FAIL: preview process-group owner must also be group leader" >&2
        return 2
    }
    [[ "$port" =~ ^[0-9]+$ ]] || return 2
    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 2
    [[ "$kill_grace_seconds" =~ ^[0-9]+$ ]] || return 2
    current_pgid="$(ps -o pgid= -p "$BASHPID" 2>/dev/null)" || return 2
    current_pgid="${current_pgid//[[:space:]]/}"
    [ "$pgid" != "$current_pgid" ] || {
        echo "FAIL: refusing to stop the caller's own process group $pgid" >&2
        return 2
    }

    bong_server_pinned_process_status \
        "$owner_pid" "$owner_starttime" "$owner_executable_identity" || {
        status=$?
        echo "FAIL: preview process-group owner identity is absent or uncertain" >&2
        [ "$status" -eq 2 ] && return 2
        return 1
    }
    members="$(bong_server_process_group_members "$pgid")"
    status=$?
    case "$status" in
        0) ;;
        1) members="" ;;
        *) echo "FAIL: could not enumerate owned preview process group $pgid" >&2; return 2 ;;
    esac
    while read -r pid starttime executable_identity; do
        [ -n "$pid$starttime$executable_identity" ] || continue
        bong_server_validate_signal_id "$pid" && [[ "$starttime" =~ ^[0-9]+$ ]] \
            && [[ "$executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
        [ "$pid" = "$owner_pid" ] && continue
        bong_server_pinned_process_status \
            "$owner_pid" "$owner_starttime" "$owner_executable_identity" || return 2
        bong_server_pidfd_signal "$pid" "$starttime" "$executable_identity" TERM "$pgid"
        status=$?
        case "$status" in 0|1) ;; *) return 2 ;; esac
    done <<< "$members"

    if bong_server_wait_for_owned_process_group_children \
        "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$pgid" "$grace_seconds"; then
        status=0
    else
        status=$?
    fi
    if [ "$status" -ne 0 ]; then
        [ "$status" -eq 1 ] || return "$status"
        bong_server_pinned_process_status \
            "$owner_pid" "$owner_starttime" "$owner_executable_identity" || return 2
        members="$(bong_server_process_group_members "$pgid")"
        status=$?
        case "$status" in
            0) ;;
            1) members="" ;;
            *) return 2 ;;
        esac
        forced_children=0
        while read -r pid starttime executable_identity; do
            [ -n "$pid$starttime$executable_identity" ] || continue
            bong_server_validate_signal_id "$pid" && [[ "$starttime" =~ ^[0-9]+$ ]] \
                && [[ "$executable_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 2
            [ "$pid" = "$owner_pid" ] && continue
            bong_server_pinned_process_status \
                "$owner_pid" "$owner_starttime" "$owner_executable_identity" || return 2
            bong_server_pidfd_signal "$pid" "$starttime" "$executable_identity" KILL "$pgid"
            status=$?
            case "$status" in
                0) forced_children=1 ;;
                1) ;;
                *) return 2 ;;
            esac
        done <<< "$members"
        bong_server_wait_for_owned_process_group_children \
            "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$pgid" "$kill_grace_seconds" || return $?
        if [ "$forced_children" -eq 1 ]; then
            forced_stop=1
        fi
    fi

    bong_server_pidfd_signal \
        "$owner_pid" "$owner_starttime" "$owner_executable_identity" KILL
    status=$?
    case "$status" in
        0|1) ;;
        *) echo "FAIL: could not safely stop preview process-group owner" >&2; return 2 ;;
    esac
    bong_server_wait_for_pinned_process_exit \
        "$owner_pid" "$owner_starttime" "$owner_executable_identity" "$kill_grace_seconds" || return $?
    wait "$owner_pid" 2>/dev/null || true
    bong_server_confirm_port_released "$port" || return $?
    [ "$forced_stop" -eq 0 ] || return "$BONG_SERVER_STOP_FORCED"
}

bong_server_wait_for_owned_process_group_children() {
    local owner_pid="${1:-}" owner_starttime="${2:-}" owner_executable_identity="${3:-}"
    local pgid="${4:-}" grace_seconds="${5:-}" deadline members status pid found_child

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 2
    deadline=$((SECONDS + grace_seconds))
    while :; do
        bong_server_pinned_process_status \
            "$owner_pid" "$owner_starttime" "$owner_executable_identity" || return 2
        members="$(bong_server_process_group_members "$pgid")"
        status=$?
        case "$status" in
            0)
                found_child=0
                while read -r pid _; do
                    [ -n "$pid" ] || continue
                    if [ "$pid" != "$owner_pid" ]; then
                        found_child=1
                        break
                    fi
                done <<< "$members"
                [ "$found_child" -eq 0 ] && return 0
                ;;
            1) return 2 ;;
            *) return 2 ;;
        esac
        [ "$SECONDS" -lt "$deadline" ] || return 1
        sleep 0.05
    done
}

bong_server_confirm_port_released() {
    local port="${1:-25565}"

    [[ "$port" =~ ^[0-9]+$ ]] || return 1
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        if ! bong_server_port_is_open "$port"; then
            return 0
        fi
        sleep 0.2
    done
    return 1
}

# A preview persistence transaction must exclude every production start/reload
# path for its full stash -> preview -> stop -> restore interval. The only legal
# order is lifecycle -> persistence; the e2e caller enters this wrapper before
# opening the persistence transaction and never acquires lifecycle from inside it.
bong_server_with_preview_persistence_lock() {
    bong_server_with_lock "$@"
}

bong_server_wait_for_executable() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local attempts="${3:-500}"
    local actual_executable attempt status

    bong_server_validate_signal_id "$pid" || return 1
    [ -n "$expected_executable" ] || return 1
    [[ "$attempts" =~ ^[0-9]+$ ]] || return 1
    expected_executable="$(readlink -f -- "$expected_executable")" || return 1
    for ((attempt = 0; attempt < attempts; attempt++)); do
        if bong_server_process_is_running "$pid"; then
            status=0
        else
            status=$?
        fi
        [ "$status" -eq 0 ] || return "$status"
        actual_executable="$(bong_server_process_executable "$pid")" || {
            bong_server_process_inspection_failed "$pid"
            return $?
        }
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
    local deadline status

    bong_server_validate_signal_id "$pid" || return 1
    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || return 1
    deadline=$((SECONDS + grace_seconds))
    while :; do
        if bong_server_process_is_running "$pid"; then
            status=0
        else
            status=$?
        fi
        case "$status" in
            1) return 0 ;;
            2) return 2 ;;
        esac
        [ "$SECONDS" -lt "$deadline" ] || return 1
        sleep 0.05
    done
}

bong_server_process_tree_has_server() {
    local pid="${1:-}" child executable executable_name command_line children status child_status

    # 0 = found; 1 = reliably absent; 2 = a live process could not be inspected.
    # Recheck existence after every failed inspection: a process which naturally
    # exited during inspection is safely absent, not an infrastructure error.
    bong_server_process_is_running "$pid"
    status=$?
    [ "$status" -eq 0 ] || return "$status"
    executable="$(bong_server_process_executable "$pid")" || {
        bong_server_process_is_running "$pid"; status=$?
        [ "$status" -eq 1 ] && return 1
        return 2
    }
    executable_name="$(basename -- "$executable")" || {
        bong_server_process_is_running "$pid"; status=$?
        [ "$status" -eq 1 ] && return 1
        return 2
    }
    command_line="$(tr '\0' ' ' < "/proc/$pid/cmdline")" || {
        bong_server_process_is_running "$pid"; status=$?
        [ "$status" -eq 1 ] && return 1
        return 2
    }
    if [ "$executable_name" = bong-server ] \
        || [[ "$command_line" == *bong-server* ]]; then
        return 0
    fi

    children="$(pgrep -P "$pid" 2>/dev/null)"
    status=$?
    case "$status" in
        0) ;;
        1) return 1 ;;
        *)
            bong_server_process_is_running "$pid"
            status=$?
            [ "$status" -eq 1 ] && return 1
            return 2
            ;;
    esac
    while IFS= read -r child; do
        [ -n "$child" ] || {
            bong_server_process_is_running "$pid"
            status=$?
            [ "$status" -eq 1 ] && return 1
            return 2
        }
        bong_server_process_tree_has_server "$child"
        child_status=$?
        case "$child_status" in
            0) return 0 ;;
            1) ;;
            *) return 2 ;;
        esac
    done <<< "$children"
    return 1
}

# Returns 0 when target session owns an unrecorded bong-server, 1 when the
# session was enumerated successfully and contains none, 2 when tmux cannot be
# queried. Callers must treat 2 as unsafe: it is not evidence that teardown is
# harmless. `-s` scopes all windows to this session only (never `-a`).
bong_server_tmux_has_unmanaged_server() {
    local session="${1:-}" sessions panes pane_pid status found=0 tmux_error

    [ -n "$session" ] || return 2
    sessions="$(LC_ALL=C tmux list-sessions -F '#{session_name}' 2>&1)"
    status=$?
    if [ "$status" -ne 0 ]; then
        # tmux uses status 1 for both expected no-server/no-socket absence and
        # operational faults. Only its explicit absence diagnostics are safe.
        if [ "$status" -eq 1 ] && [[ "$sessions" =~ (no[[:space:]]server[[:space:]]running|no[[:space:]]server[[:space:]]on|no[[:space:]]socket) ]]; then
            return 1
        fi
        echo "FAIL: could not enumerate tmux sessions; refusing unsafe teardown: $sessions" >&2
        return 2
    fi
    while IFS= read -r pane_pid; do
        [ "$pane_pid" = "$session" ] && { found=1; break; }
    done <<< "$sessions"
    [ "$found" -eq 1 ] || return 1
    panes="$(LC_ALL=C tmux list-panes -s -t "$session" -F '#{pane_pid}' 2>&1)"
    status=$?
    if [ "$status" -ne 0 ]; then
        if [ "$status" -eq 1 ] && [[ "$panes" =~ (can.t[[:space:]]find[[:space:]]session|no[[:space:]]server[[:space:]]running|no[[:space:]]server[[:space:]]on|no[[:space:]]socket) ]]; then
            return 1
        fi
        echo "FAIL: could not enumerate tmux session '$session' panes; refusing unsafe teardown: $panes" >&2
        return 2
    fi
    while IFS= read -r pane_pid; do
        [ -n "$pane_pid" ] || { echo "FAIL: tmux returned an uninspectable pane pid" >&2; return 2; }
        bong_server_process_tree_has_server "$pane_pid"
        status=$?
        case "$status" in
            0) return 0 ;;
            1) ;;
            *) echo "FAIL: could not inspect tmux pane process tree; refusing unsafe teardown" >&2; return 2 ;;
        esac
    done <<< "$panes"
    return 1
}

_bong_server_stop_managed() {
    local grace_seconds="${BONG_SERVER_STOP_GRACE_SECONDS:-10}"
    local record pid starttime executable executable_identity status

    [[ "$grace_seconds" =~ ^[0-9]+$ ]] || {
        echo "FAIL: BONG_SERVER_STOP_GRACE_SECONDS must be a non-negative integer: $grace_seconds" >&2
        return 1
    }
    record="$(bong_server_pid_file)"
    if [ ! -e "$record" ] && [ ! -L "$record" ]; then
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
    if bong_server_process_is_running "$pid"; then
        status=0
    else
        status=$?
    fi
    case "$status" in
        1)
            _bong_server_finish_managed_record_cleanup "$pid" "$starttime" "$executable" "$executable_identity"
            return $?
            ;;
        2)
            echo "FAIL: could not inspect managed bong-server pid $pid; preserving record and refusing signals" >&2
            return 2
            ;;
    esac
    bong_server_record_matches_process
    status=$?
    case "$status" in
        0) ;;
        1)
            echo "FAIL: managed bong-server record no longer identifies pid $pid; refusing to signal an unverified process" >&2
            return 2
            ;;
        *)
            echo "FAIL: could not verify managed bong-server identity for pid $pid; preserving record and refusing signals" >&2
            return 2
            ;;
    esac

    bong_server_pidfd_signal "$pid" "$starttime" "$executable_identity" TERM
    status=$?
    case "$status" in
        0) ;;
        1)
            _bong_server_finish_managed_record_cleanup "$pid" "$starttime" "$executable" "$executable_identity"
            return $?
            ;;
        *)
            echo "FAIL: could not deliver identity-safe SIGTERM to managed bong-server pid $pid; preserving record" >&2
            return 2
            ;;
    esac
    bong_server_wait_for_exit "$pid" "$grace_seconds"
    status=$?
    case "$status" in
        0)
            _bong_server_finish_managed_record_cleanup "$pid" "$starttime" "$executable" "$executable_identity"
            return $?
            ;;
        2)
            echo "FAIL: could not inspect managed bong-server pid $pid while waiting after TERM; preserving record and refusing SIGKILL" >&2
            return 2
            ;;
    esac
    if bong_server_process_is_running "$pid"; then
        status=0
    else
        status=$?
    fi
    case "$status" in
        1)
            _bong_server_finish_managed_record_cleanup "$pid" "$starttime" "$executable" "$executable_identity"
            return $?
            ;;
        2)
            echo "FAIL: could not inspect managed bong-server pid $pid before SIGKILL; preserving record" >&2
            return 2
            ;;
    esac
    bong_server_record_matches_process
    status=$?
    case "$status" in
        0) ;;
        1)
            echo "FAIL: managed bong-server identity changed while waiting; refusing SIGKILL" >&2
            return 2
            ;;
        *)
            echo "FAIL: could not verify managed bong-server identity before SIGKILL; preserving record" >&2
            return 2
            ;;
    esac

    bong_server_pidfd_signal "$pid" "$starttime" "$executable_identity" KILL
    status=$?
    case "$status" in
        0) ;;
        1)
            if _bong_server_finish_managed_record_cleanup \
                "$pid" "$starttime" "$executable" "$executable_identity"; then
                return "$BONG_SERVER_STOP_FORCED"
            else
                status=$?
            fi
            return "$status"
            ;;
        *)
            echo "FAIL: could not deliver identity-safe SIGKILL to managed bong-server pid $pid; preserving record" >&2
            return 2
            ;;
    esac
    bong_server_wait_for_exit "$pid" 2
    status=$?
    if [ "$status" -ne 0 ]; then
        if [ "$status" -eq 2 ]; then
            echo "FAIL: could not inspect managed bong-server pid $pid after SIGKILL; preserving record" >&2
            return 2
        fi
        echo "FAIL: managed bong-server pid $pid did not exit after SIGKILL" >&2
        return 1
    fi
    if _bong_server_finish_managed_record_cleanup \
        "$pid" "$starttime" "$executable" "$executable_identity"; then
        return "$BONG_SERVER_STOP_FORCED"
    else
        status=$?
    fi
    return "$status"
}

bong_server_stop_managed() {
    bong_server_with_lock _bong_server_stop_managed
}

# Restart/start callers may proceed after status 3 only because that status proves
# the exact managed process is gone and its authority record was safely removed.
# AppExit/Last is still unproven, so make the degraded shutdown visible instead of
# flattening it into either ordinary success or an ownership failure.
bong_server_stop_managed_for_replacement() {
    local operation="${1:-managed server replacement}" status

    if bong_server_stop_managed; then
        return 0
    else
        status=$?
    fi
    case "$status" in
        "$BONG_SERVER_STOP_FORCED")
            echo "WARN: managed bong-server required identity-safe SIGKILL; AppExit/Last persistence is unconfirmed; continuing $operation because the exact process is gone and its authority record was safely cleared" >&2
            return 0
            ;;
        *)
            echo "FAIL: managed bong-server stop did not complete safely (status=$status); refusing $operation" >&2
            return "$status"
            ;;
    esac
}

# 这些文件属于同一个 SQLite 持久化快照。预览 e2e 必须先把开发者快照
# 原子地发布清单，再移动文件；绝不能从一个已有 stash 目录恢复或继续写入。
# 清单的 header 使合法空快照和截断/损坏的空文件可区分。
BONG_SERVER_STASH_MANIFEST_HEADER="BONG_SERVER_PERSISTENCE_STASH_V3"

_bong_server_persistence_filename_is_valid() {
    case "${1:-}" in
        bong.db|bong.db-wal|bong.db-shm) return 0 ;;
        *) return 1 ;;
    esac
}

_bong_server_sha256_file() {
    local file="${1:-}" digest
    [ -f "$file" ] && [ ! -L "$file" ] || return 1
    digest="$(sha256sum -- "$file" 2>/dev/null)" || return 1
    digest="${digest%% *}"
    [[ "$digest" =~ ^[0-9a-f]{64}$ ]] || return 1
    printf '%s\n' "$digest"
}

_bong_server_file_matches_digest() {
    local file="${1:-}" expected="${2:-}" actual
    [[ "$expected" =~ ^[0-9a-f]{64}$ ]] || return 1
    actual="$(_bong_server_sha256_file "$file")" || return 1
    [ "$actual" = "$expected" ]
}

_bong_server_regular_file_identity() {
    local file="${1:-}"
    [ -f "$file" ] && [ ! -L "$file" ] || return 1
    stat -Lc '%d:%i' -- "$file" 2>/dev/null
}

# Kept as a narrow seam so lifecycle tests can prove cross-device refusal even
# on hosts without a second mount. Production reads stat(2)'s device number.
bong_server_path_device() {
    local path="${1:-}" device
    device="$(stat -Lc '%d' -- "$path" 2>/dev/null)" || return 1
    [[ "$device" =~ ^[0-9]+$ ]] || return 1
    printf '%s\n' "$device"
}

_bong_server_persistence_preflight_stash_devices() {
    local data_dir="${1:-}" stash_dir="${2:-}" entry filename source_file stash_device source_device
    shift 2 || return 1

    stash_device="$(bong_server_path_device "$stash_dir")" || {
        echo "FAIL: cannot determine stash directory device: $stash_dir" >&2
        return 1
    }
    for entry in "$@"; do
        filename="${entry##* }"
        _bong_server_persistence_filename_is_valid "$filename" || return 1
        source_file="$data_dir/$filename"
        source_device="$(bong_server_path_device "$source_file")" || {
            echo "FAIL: cannot determine persistence source device: $source_file" >&2
            return 1
        }
        if [ "$source_device" != "$stash_device" ]; then
            echo "FAIL: persistence stash crosses devices ($source_file device $source_device, $stash_dir device $stash_device); refusing before READY or moves" >&2
            return 1
        fi
    done
}

_bong_server_file_matches_snapshot() {
    local file="${1:-}" expected_digest="${2:-}" expected_identity="${3:-}" identity
    [[ "$expected_digest" =~ ^[0-9a-f]{64}$ ]] || return 1
    [[ "$expected_identity" =~ ^[0-9]+:[0-9]+$ ]] || return 1
    identity="$(_bong_server_regular_file_identity "$file")" || return 1
    [ "$identity" = "$expected_identity" ] || return 1
    _bong_server_file_matches_digest "$file" "$expected_digest"
}

_bong_server_publish_stash_manifest() {
    local stash_dir="${1:-}"
    shift || return 1
    local temporary manifest_file entry filename digest identity

    [ -d "$stash_dir" ] && [ ! -L "$stash_dir" ] || return 1
    manifest_file="$stash_dir/stashed-files"
    [ ! -e "$manifest_file" ] && [ ! -L "$manifest_file" ] || {
        echo "FAIL: stash manifest already exists at $manifest_file; refusing to overwrite recoverable evidence" >&2
        return 1
    }
    temporary="$(mktemp "$stash_dir/.stashed-files.XXXXXX")" || return 1
    {
        printf '%s\n' "$BONG_SERVER_STASH_MANIFEST_HEADER"
        for entry in "$@"; do
            digest="${entry%% *}"
            entry="${entry#* }"
            identity="${entry%% *}"
            filename="${entry#* }"
            [ "$entry" = "$identity $filename" ] \
                && _bong_server_persistence_filename_is_valid "$filename" \
                && [[ "$digest" =~ ^[0-9a-f]{64}$ ]] \
                && [[ "$identity" =~ ^[0-9]+:[0-9]+$ ]] || exit 1
            printf '%s %s %s\n' "$digest" "$identity" "$filename"
        done
    } > "$temporary" || { rm -f -- "$temporary"; return 1; }
    sync -f -- "$temporary" || { rm -f -- "$temporary"; return 1; }
    mv -- "$temporary" "$manifest_file" || { rm -f -- "$temporary"; return 1; }
    sync -f -- "$stash_dir" || return 1
}

# Strict V3 parser: global indexed arrays pair filenames with expected snapshots.
_bong_server_read_stash_manifest() {
    local manifest_file="${1:-}" line digest identity filename last_had_newline=1
    local -A seen=()

    BONG_SERVER_STASHED_FILES=()
    BONG_SERVER_STASHED_DIGESTS=()
    BONG_SERVER_STASHED_IDENTITIES=()
    [ -f "$manifest_file" ] && [ ! -L "$manifest_file" ] && [ -r "$manifest_file" ] || return 1
    exec {BONG_SERVER_MANIFEST_FD}<"$manifest_file" || return 1
    if ! IFS= read -r line <&"$BONG_SERVER_MANIFEST_FD"; then
        exec {BONG_SERVER_MANIFEST_FD}<&-; return 1
    fi
    [ "$line" = "$BONG_SERVER_STASH_MANIFEST_HEADER" ] || { exec {BONG_SERVER_MANIFEST_FD}<&-; return 1; }
    while IFS= read -r line <&"$BONG_SERVER_MANIFEST_FD"; do
        [[ "$line" =~ ^([0-9a-f]{64})\ ([0-9]+:[0-9]+)\ (bong\.db|bong\.db-wal|bong\.db-shm)$ ]] || { exec {BONG_SERVER_MANIFEST_FD}<&-; return 1; }
        digest="${BASH_REMATCH[1]}"; identity="${BASH_REMATCH[2]}"; filename="${BASH_REMATCH[3]}"
        [ -z "${seen[$filename]+x}" ] || { exec {BONG_SERVER_MANIFEST_FD}<&-; return 1; }
        seen["$filename"]=1
        BONG_SERVER_STASHED_FILES+=("$filename")
        BONG_SERVER_STASHED_DIGESTS+=("$digest")
        BONG_SERVER_STASHED_IDENTITIES+=("$identity")
    done
    [ -z "$line" ] || last_had_newline=0
    exec {BONG_SERVER_MANIFEST_FD}<&-
    [ "$last_had_newline" -eq 1 ]
}

_bong_server_manifest_digest_for_file() {
    local wanted="${1:-}" index
    for index in "${!BONG_SERVER_STASHED_FILES[@]}"; do
        [ "${BONG_SERVER_STASHED_FILES[$index]}" = "$wanted" ] && {
            printf '%s\n' "${BONG_SERVER_STASHED_DIGESTS[$index]}"
            return 0
        }
    done
    return 1
}


_bong_server_manifest_identity_for_file() {
    local wanted="${1:-}" index
    for index in "${!BONG_SERVER_STASHED_FILES[@]}"; do
        [ "${BONG_SERVER_STASHED_FILES[$index]}" = "$wanted" ] && {
            printf '%s\n' "${BONG_SERVER_STASHED_IDENTITIES[$index]}"
            return 0
        }
    done
    return 1
}

_bong_server_persistence_transaction_identity_ok() {
    local current_data current_parent
    [ -n "${BONG_SERVER_PERSISTENCE_DATA_DIR:-}" ] || return 1
    bong_server_validate_real_directory "$BONG_SERVER_PERSISTENCE_DATA_DIR" || return 1
    bong_server_validate_real_directory "$BONG_SERVER_PERSISTENCE_PARENT_DIR" || return 1
    current_data="$(bong_server_path_identity "$BONG_SERVER_PERSISTENCE_DATA_DIR")" || return 1
    current_parent="$(bong_server_path_identity "$BONG_SERVER_PERSISTENCE_PARENT_DIR")" || return 1
    [ "$current_data" = "$BONG_SERVER_PERSISTENCE_DATA_IDENTITY" ] \
        && [ "$current_parent" = "$BONG_SERVER_PERSISTENCE_PARENT_IDENTITY" ]
}

bong_server_persistence_transaction_state_dir() {
    local data_dir="${1:-}" canonical_data_dir runtime_root state_root key state_dir

    bong_server_validate_real_directory "$data_dir" || return 1
    canonical_data_dir="$(readlink -f -- "$data_dir")" || return 1
    runtime_root="$(bong_server_runtime_dir)" || return 1
    state_root="$runtime_root/persistence-transactions"
    if [ ! -e "$state_root" ] && [ ! -L "$state_root" ]; then
        (umask 077 && mkdir -- "$state_root") || return 1
    fi
    bong_server_validate_secure_directory "$state_root" 700 || {
        echo "FAIL: insecure persistence transaction state root $state_root" >&2
        return 1
    }
    key="$(printf '%s' "$canonical_data_dir" | sha256sum)" || return 1
    key="${key%% *}"
    [[ "$key" =~ ^[0-9a-f]{64}$ ]] || return 1
    state_dir="$state_root/$key"
    if [ ! -e "$state_dir" ] && [ ! -L "$state_dir" ]; then
        (umask 077 && mkdir -- "$state_dir") || return 1
    fi
    bong_server_validate_secure_directory "$state_dir" 700 || {
        echo "FAIL: insecure persistence transaction state directory $state_dir" >&2
        return 1
    }
    printf '%s\n' "$state_dir"
}

bong_server_persistence_transaction_begin() {
    local data_dir="${1:-}" parent lock_file marker_file temporary fd data_identity parent_identity state_dir canonical_data_dir
    bong_server_validate_real_directory "$data_dir" || {
        echo "FAIL: persistence data directory must be a real non-symlink directory" >&2; return 1; }
    [ -z "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] || { echo "FAIL: persistence transaction is already held by this shell" >&2; return 1; }
    parent="$(dirname -- "$data_dir")"
    bong_server_validate_real_directory "$parent" || { echo "FAIL: persistence parent must be a real non-symlink directory" >&2; return 1; }
    canonical_data_dir="$(readlink -f -- "$data_dir")" || return 1
    data_identity="$(bong_server_path_identity "$data_dir")" || return 1
    parent_identity="$(bong_server_path_identity "$parent")" || return 1
    state_dir="$(bong_server_persistence_transaction_state_dir "$data_dir")" || return 1
    lock_file="$state_dir/transaction.lock"
    marker_file="$state_dir/recovery-handoff"
    if [ ! -e "$lock_file" ] && [ ! -L "$lock_file" ]; then (umask 077 && : >> "$lock_file") || return 1; fi
    if ! bong_server_validate_lock_file "$lock_file"; then echo "FAIL: insecure persistence transaction lock $lock_file" >&2; return 1; fi
    exec {fd}>>"$lock_file" || return 1
    if ! bong_server_validate_fd_secure_regular_file "$fd" || ! bong_server_fd_matches_path "$fd" "$lock_file"; then
        exec {fd}>&-; echo "FAIL: persistence transaction lock changed while opening" >&2; return 1
    fi
    flock -xn "$fd" || { echo "FAIL: persistence transaction lock is held for $data_dir; refusing to interleave database stash/restore" >&2; exec {fd}>&-; return 1; }
    if [ -e "$marker_file" ] || [ -L "$marker_file" ]; then
        echo "FAIL: persistence recovery handoff exists at $marker_file; refusing to overwrite an unrecovered stash" >&2
        flock -u "$fd"; exec {fd}>&-; return 1
    fi
    temporary="$(umask 077 && mktemp "$state_dir/.recovery-handoff.XXXXXX")" || { flock -u "$fd"; exec {fd}>&-; return 1; }
    chmod 600 -- "$temporary" || { rm -f -- "$temporary"; flock -u "$fd"; exec {fd}>&-; return 1; }
    if ! printf 'version=3\nstate=ACTIVE\npid=%s\ndata_dir=%s\ndata_identity=%s\nparent_identity=%s\nstate_dir=%s\nstarted=%s\n' \
        "$$" "$canonical_data_dir" "$data_identity" "$parent_identity" "$state_dir" "$(date -Iseconds)" > "$temporary" \
        || ! sync -f -- "$temporary" || ! mv -- "$temporary" "$marker_file" || ! sync -f -- "$state_dir"; then
        rm -f -- "$temporary"; flock -u "$fd"; exec {fd}>&-; return 1
    fi
    bong_server_validate_pid_record_file "$marker_file" || { flock -u "$fd"; exec {fd}>&-; return 1; }
    BONG_SERVER_PERSISTENCE_MARKER_IDENTITY="$(bong_server_path_identity "$marker_file")" || { flock -u "$fd"; exec {fd}>&-; return 1; }
    BONG_SERVER_PERSISTENCE_LOCK_FD="$fd"
    BONG_SERVER_PERSISTENCE_DATA_DIR="$data_dir"
    BONG_SERVER_PERSISTENCE_PARENT_DIR="$parent"
    BONG_SERVER_PERSISTENCE_STATE_DIR="$state_dir"
    BONG_SERVER_PERSISTENCE_DATA_IDENTITY="$data_identity"
    BONG_SERVER_PERSISTENCE_PARENT_IDENTITY="$parent_identity"
    BONG_SERVER_PERSISTENCE_MARKER_FILE="$marker_file"
    BONG_SERVER_PERSISTENCE_STASH_READY=0
    BONG_SERVER_PERSISTENCE_STASH_DIR=""
    BONG_SERVER_PERSISTENCE_RESTORE_DURABLE=0
}

_bong_server_persistence_transaction_write_marker() {
    local state="${1:-}" detail_key="${2:-}" detail_value="${3:-}" temporary
    [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] && [ -n "$state" ] || return 1
    _bong_server_persistence_transaction_identity_ok || return 1
    temporary="$(umask 077 && mktemp "${BONG_SERVER_PERSISTENCE_STATE_DIR}/.recovery-handoff.XXXXXX")" || return 1
    chmod 600 -- "$temporary" || { rm -f -- "$temporary"; return 1; }
    if ! printf 'version=3\nstate=%s\npid=%s\ndata_dir=%s\ndata_identity=%s\nparent_identity=%s\nstate_dir=%s\nstash_dir=%s\ntimestamp=%s\n%s=%s\n' \
        "$state" "$$" "$(readlink -f -- "$BONG_SERVER_PERSISTENCE_DATA_DIR")" "$BONG_SERVER_PERSISTENCE_DATA_IDENTITY" "$BONG_SERVER_PERSISTENCE_PARENT_IDENTITY" \
        "$BONG_SERVER_PERSISTENCE_STATE_DIR" "${BONG_SERVER_PERSISTENCE_STASH_DIR:-}" "$(date -Iseconds)" "$detail_key" "$detail_value" > "$temporary" \
        || ! sync -f -- "$temporary" || ! mv -f -- "$temporary" "$BONG_SERVER_PERSISTENCE_MARKER_FILE" || ! sync -f -- "$BONG_SERVER_PERSISTENCE_STATE_DIR"; then
        rm -f -- "$temporary"; return 1
    fi
    bong_server_validate_pid_record_file "$BONG_SERVER_PERSISTENCE_MARKER_FILE" || return 1
    BONG_SERVER_PERSISTENCE_MARKER_IDENTITY="$(bong_server_path_identity "$BONG_SERVER_PERSISTENCE_MARKER_FILE")"
}

_bong_server_persistence_transaction_marker_has_state() {
    local expected_state="${1:-}" line seen=0 fd marker_identity

    [ -n "$expected_state" ] && [ -n "${BONG_SERVER_PERSISTENCE_MARKER_FILE:-}" ] || return 1
    bong_server_validate_pid_record_file "$BONG_SERVER_PERSISTENCE_MARKER_FILE" || return 1
    exec {fd}<"$BONG_SERVER_PERSISTENCE_MARKER_FILE" || return 1
    if ! bong_server_validate_fd_secure_regular_file "$fd" \
        || ! bong_server_fd_matches_path "$fd" "$BONG_SERVER_PERSISTENCE_MARKER_FILE"; then
        exec {fd}<&-
        return 1
    fi
    marker_identity="$(bong_server_path_identity "/proc/self/fd/$fd")" || {
        exec {fd}<&-
        return 1
    }
    [ "$marker_identity" = "$BONG_SERVER_PERSISTENCE_MARKER_IDENTITY" ] || {
        exec {fd}<&-
        return 1
    }
    while IFS= read -r -u "$fd" line || [ -n "$line" ]; do
        case "$line" in
            state=*)
                [ "$seen" -eq 0 ] || { exec {fd}<&-; return 1; }
                [ "${line#state=}" = "$expected_state" ] || { exec {fd}<&-; return 1; }
                seen=1
                ;;
        esac
    done
    exec {fd}<&-
    [ "$seen" -eq 1 ]
}

bong_server_persistence_transaction_set_stash() {
    local stash_dir="${1:-}"
    [ -n "$stash_dir" ] && [ -d "$stash_dir" ] || return 1
    BONG_SERVER_PERSISTENCE_STASH_DIR="$stash_dir"
    _bong_server_persistence_transaction_write_marker "STASHED" "stash_dir" "$stash_dir" || return 1
    BONG_SERVER_PERSISTENCE_STASH_READY=1
}

bong_server_persistence_transaction_mark_failed() {
    local reason="${1:-unspecified failure}"
    _bong_server_persistence_transaction_identity_ok || return 1
    _bong_server_persistence_transaction_write_marker "FAILED" "reason" "$reason"
}

# A preview that did not prove shutdown/port release may still own SQLite files.
# Deliberately do not call restore here: retain a FAILED durable handoff with
# the stash path, then release only the advisory transaction lock.
bong_server_persistence_transaction_abort_unconfirmed_preview_stop() {
    local data_dir="${1:-}" stash_dir="${2:-}" reason="${3:-preview server stop was not confirmed}"
    [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] \
        && [ "$BONG_SERVER_PERSISTENCE_DATA_DIR" = "$data_dir" ] \
        && [ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 1 ] \
        && [ "${BONG_SERVER_PERSISTENCE_STASH_DIR:-}" = "$stash_dir" ] || return 1
    bong_server_persistence_transaction_mark_failed "$reason" || return 1
    bong_server_persistence_transaction_release
}

bong_server_abort_unconfirmed_preview_stop() {
    local data_dir="${1:-}" stash_dir="${2:-}" reason="${3:-preview server stop was not confirmed}"

    bong_server_persistence_transaction_abort_unconfirmed_preview_stop \
        "$data_dir" "$stash_dir" "$reason"
}

bong_server_finalize_preview_persistence_after_stop() {
    local data_dir="${1:-}" stash_dir="${2:-}" stop_confirmed="${3:-0}"

    [ -n "$data_dir" ] && [ -n "$stash_dir" ] || return 1
    case "$stop_confirmed" in
        1)
            if bong_server_restore_persistence "$data_dir" "$stash_dir"; then
                if bong_server_persistence_transaction_complete; then
                    return 0
                fi
                if [ "${BONG_SERVER_PERSISTENCE_RESTORE_DURABLE:-0}" -eq 1 ] \
                    && _bong_server_persistence_transaction_marker_has_state "RESTORED"; then
                    echo "FAIL: restored persistence is durable but handoff cleanup is incomplete; preserving RESTORED for retry" >&2
                    return 1
                fi
                bong_server_persistence_transaction_mark_failed "restore completed but transaction handoff cleanup failed" || true
                bong_server_persistence_transaction_release
                return 1
            fi
            bong_server_persistence_transaction_mark_failed "cleanup restore failed; inspect $stash_dir" || true
            bong_server_persistence_transaction_release
            return 1
            ;;
        0)
            # A failed process-tree inspection is indistinguishable from an
            # unconfirmed stop: restoring SQLite here could race a live server.
            bong_server_abort_unconfirmed_preview_stop \
                "$data_dir" "$stash_dir" \
                "preview server stop was not confirmed; restore forbidden; stash retained at $stash_dir"
            ;;
        *) return 1 ;;
    esac
}

bong_server_persistence_transaction_release() {
    local fd="${BONG_SERVER_PERSISTENCE_LOCK_FD:-}"
    [ -n "$fd" ] || return 0
    flock -u "$fd" || true; exec {fd}>&-
    unset BONG_SERVER_PERSISTENCE_LOCK_FD BONG_SERVER_PERSISTENCE_DATA_DIR BONG_SERVER_PERSISTENCE_PARENT_DIR BONG_SERVER_PERSISTENCE_STATE_DIR BONG_SERVER_PERSISTENCE_DATA_IDENTITY BONG_SERVER_PERSISTENCE_PARENT_IDENTITY BONG_SERVER_PERSISTENCE_MARKER_FILE BONG_SERVER_PERSISTENCE_MARKER_IDENTITY BONG_SERVER_PERSISTENCE_STASH_READY BONG_SERVER_PERSISTENCE_STASH_DIR BONG_SERVER_PERSISTENCE_RESTORE_DURABLE
}

_bong_server_persistence_cleanup_restored_stash() {
    local stash_dir="${BONG_SERVER_PERSISTENCE_STASH_DIR:-}" stash_parent manifest_file entries entry find_status

    [ -n "$stash_dir" ] || return 0
    stash_parent="$(dirname -- "$stash_dir")"
    bong_server_validate_real_directory "$stash_parent" || return 1
    if [ ! -e "$stash_dir" ] && [ ! -L "$stash_dir" ]; then
        sync -f -- "$stash_parent" || {
            echo "FAIL: could not durably sync persistence stash directory removal" >&2
            return 1
        }
        return 0
    fi
    [ -d "$stash_dir" ] && [ ! -L "$stash_dir" ] || {
        echo "FAIL: restored persistence stash path is not a real directory: $stash_dir" >&2
        return 1
    }
    entries="$(find "$stash_dir" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null)"
    find_status=$?
    [ "$find_status" -eq 0 ] || {
        echo "FAIL: cannot enumerate restored persistence stash directory" >&2
        return 1
    }
    while IFS= read -r entry; do
        [ -z "$entry" ] && continue
        [ "$entry" = stashed-files ] || {
            echo "FAIL: unexpected restored persistence stash entry $entry; preserving RESTORED handoff" >&2
            return 1
        }
    done <<< "$entries"
    manifest_file="$stash_dir/stashed-files"
    if [ -e "$manifest_file" ] || [ -L "$manifest_file" ]; then
        [ -f "$manifest_file" ] && [ ! -L "$manifest_file" ] || {
            echo "FAIL: restored persistence manifest is not a regular file; preserving RESTORED handoff" >&2
            return 1
        }
        rm -f -- "$manifest_file" || return 1
    fi
    sync -f -- "$stash_dir" || {
        echo "FAIL: could not durably sync restored stash evidence cleanup" >&2
        return 1
    }
    rmdir -- "$stash_dir" || {
        echo "FAIL: stash directory unexpectedly nonempty after validated restore" >&2
        return 1
    }
    sync -f -- "$stash_parent" || {
        echo "FAIL: could not durably sync persistence stash directory removal" >&2
        return 1
    }
}

bong_server_persistence_transaction_complete() {
    [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] || return 1
    _bong_server_persistence_transaction_identity_ok || return 1
    [ "${BONG_SERVER_PERSISTENCE_RESTORE_DURABLE:-0}" -eq 1 ] || {
        if [ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 0 ] \
            && [ -z "${BONG_SERVER_PERSISTENCE_STASH_DIR:-}" ]; then
            _bong_server_persistence_transaction_write_marker "RESTORED" "restored" "no-stash" || return 1
            BONG_SERVER_PERSISTENCE_RESTORE_DURABLE=1
        else
            echo "FAIL: persistence transaction has no durable RESTORED state; refusing to clear recovery marker" >&2
            return 1
        fi
    }
    _bong_server_persistence_transaction_marker_has_state "RESTORED" || {
        echo "FAIL: persistence recovery marker is not the verified RESTORED handoff; refusing completion" >&2
        return 1
    }
    _bong_server_persistence_cleanup_restored_stash || return 1
    bong_server_remove_secure_file_if_identity "$BONG_SERVER_PERSISTENCE_MARKER_FILE" "$BONG_SERVER_PERSISTENCE_MARKER_IDENTITY" || return 1
    sync -f -- "$BONG_SERVER_PERSISTENCE_STATE_DIR" || return 1
    bong_server_persistence_transaction_release
}

# A leaf is transaction-exclusive. V3 manifest + durable stash marker are both
# completed before the first mv. Every move validates the original snapshot.
bong_server_stash_persistence() {
    local data_dir="${1:-}" stash_dir="${2:-}" suffix filename source_file digest identity
    local -a snapshot=()
    [ -n "$data_dir" ] && [ -n "$stash_dir" ] || return 1
    [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] && [ "$BONG_SERVER_PERSISTENCE_DATA_DIR" = "$data_dir" ] || {
        echo "FAIL: persistence stash requires the active matching transaction" >&2; return 1; }
    _bong_server_persistence_transaction_identity_ok || { echo "FAIL: persistence transaction path identity changed; refusing stash" >&2; return 1; }
    bong_server_validate_real_directory "$data_dir" || return 1
    mkdir -p -- "$(dirname -- "$stash_dir")" || return 1
    if ! mkdir -- "$stash_dir"; then
        echo "FAIL: stash directory already exists or cannot be exclusively created: $stash_dir; refusing to move persistence files" >&2; return 1
    fi
    for suffix in "" "-wal" "-shm"; do
        filename="bong.db$suffix"; source_file="$data_dir/$filename"
        if [ -e "$source_file" ] || [ -L "$source_file" ]; then
            digest="$(_bong_server_sha256_file "$source_file")" || { echo "FAIL: persistence source is not a regular digestible file: $source_file" >&2; return 1; }
            identity="$(_bong_server_regular_file_identity "$source_file")" || { echo "FAIL: persistence source identity cannot be pinned: $source_file" >&2; return 1; }
            snapshot+=("$digest $identity $filename")
        fi
    done
    _bong_server_persistence_preflight_stash_devices "$data_dir" "$stash_dir" "${snapshot[@]}" || return 1
    _bong_server_publish_stash_manifest "$stash_dir" "${snapshot[@]}" || return 1
    _bong_server_read_stash_manifest "$stash_dir/stashed-files" || { echo "FAIL: newly published V3 stash manifest did not validate; refusing moves" >&2; return 1; }
    if [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ]; then
        [ "$BONG_SERVER_PERSISTENCE_DATA_DIR" = "$data_dir" ] || { echo "FAIL: transaction data directory does not match stash source" >&2; return 1; }
        bong_server_persistence_transaction_set_stash "$stash_dir" || { echo "FAIL: cannot durably record stash path before move; refusing moves" >&2; return 1; }
    fi
    for entry in "${snapshot[@]}"; do
        digest="${entry%% *}"; entry="${entry#* }"; identity="${entry%% *}"; filename="${entry#* }"; source_file="$data_dir/$filename"
        _bong_server_file_matches_snapshot "$source_file" "$digest" "$identity" || { echo "FAIL: persistence source changed before move: $source_file" >&2; return 1; }
        mv -- "$source_file" "$stash_dir/$filename" || return 1
        _bong_server_file_matches_snapshot "$stash_dir/$filename" "$digest" "$identity" || { echo "FAIL: moved stash file snapshot mismatch: $filename" >&2; return 1; }
    done
    sync -f -- "$data_dir" || {
        echo "FAIL: could not durably sync persistence data directory after stash" >&2
        return 1
    }
    sync -f -- "$stash_dir" || {
        echo "FAIL: could not durably sync persistence stash directory after stash" >&2
        return 1
    }
}

bong_server_restore_persistence() {
    local data_dir="${1:-}" stash_dir="${2:-}" manifest_file suffix filename stash_file data_file expected expected_identity entry entries find_status
    local -A source_location=() pinned_identity=() data_candidate_identity=() data_candidate_digest=()
    [ -n "$data_dir" ] && [ -n "$stash_dir" ] || return 1
    [ -n "${BONG_SERVER_PERSISTENCE_LOCK_FD:-}" ] && [ "$BONG_SERVER_PERSISTENCE_DATA_DIR" = "$data_dir" ] \
        && [ "${BONG_SERVER_PERSISTENCE_STASH_READY:-0}" -eq 1 ] && [ "${BONG_SERVER_PERSISTENCE_STASH_DIR:-}" = "$stash_dir" ] || {
        echo "FAIL: persistence restore requires its active READY transaction and matching stash path" >&2; return 1; }
    _bong_server_persistence_transaction_identity_ok || { echo "FAIL: persistence transaction path identity changed; refusing restore" >&2; return 1; }
    if [ "${BONG_SERVER_PERSISTENCE_RESTORE_DURABLE:-0}" -eq 1 ]; then
        _bong_server_persistence_transaction_marker_has_state "RESTORED" || {
            echo "FAIL: in-memory restored state does not match the durable recovery handoff" >&2
            return 1
        }
        return 0
    fi
    [ -d "$stash_dir" ] && [ ! -L "$stash_dir" ] || return 1
    manifest_file="$stash_dir/stashed-files"
    _bong_server_read_stash_manifest "$manifest_file" || { echo "FAIL: stash manifest is missing, malformed, or unreadable at $manifest_file; refusing to touch $data_dir (fail closed)" >&2; return 1; }
    entries="$(find "$stash_dir" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null)"; find_status=$?
    [ "$find_status" -eq 0 ] || { echo "FAIL: cannot enumerate stash directory; refusing restore" >&2; return 1; }
    while IFS= read -r entry; do
        [ -z "$entry" ] && continue; [ "$entry" = stashed-files ] && continue
        _bong_server_manifest_digest_for_file "$entry" >/dev/null || { echo "FAIL: unexpected stash entry $entry; refusing restore" >&2; return 1; }
    done <<< "$entries"
    # Preflight every leaf before mutating any one. Pin the candidate pathname's
    # inode as well as the immutable V3 source inode to detect swaps at mv time.
    for suffix in "" "-wal" "-shm"; do
        filename="bong.db$suffix"; data_file="$data_dir/$filename"; stash_file="$stash_dir/$filename"
        if [ -e "$data_file" ] || [ -L "$data_file" ]; then
            data_candidate_identity["$filename"]="$(_bong_server_regular_file_identity "$data_file")" || { echo "FAIL: persistence target is not a regular file: $data_file" >&2; return 1; }
            data_candidate_digest["$filename"]="$(_bong_server_sha256_file "$data_file")" || { echo "FAIL: cannot pin persistence target digest: $data_file" >&2; return 1; }
        fi
        if expected="$(_bong_server_manifest_digest_for_file "$filename")"; then
            expected_identity="$(_bong_server_manifest_identity_for_file "$filename")" || return 1
            if [ -e "$stash_file" ] || [ -L "$stash_file" ]; then
                _bong_server_file_matches_snapshot "$stash_file" "$expected" "$expected_identity" || { echo "FAIL: stash snapshot mismatch for $filename" >&2; return 1; }
                source_location["$filename"]="stash"; pinned_identity["$filename"]="$expected_identity"
            elif [ -e "$data_file" ] || [ -L "$data_file" ]; then
                _bong_server_file_matches_snapshot "$data_file" "$expected" "$expected_identity" || { echo "FAIL: partial restore snapshot mismatch for $filename" >&2; return 1; }
                source_location["$filename"]="data"; pinned_identity["$filename"]="$expected_identity"
            else
                echo "FAIL: $filename is recorded but absent from stash and data" >&2; return 1
            fi
        fi
    done
    for suffix in "" "-wal" "-shm"; do
        filename="bong.db$suffix"; stash_file="$stash_dir/$filename"; data_file="$data_dir/$filename"
        if expected="$(_bong_server_manifest_digest_for_file "$filename")"; then
            expected_identity="${pinned_identity[$filename]}"
            if [ "${source_location[$filename]}" = stash ]; then
                _bong_server_file_matches_snapshot "$stash_file" "$expected" "$expected_identity" || { echo "FAIL: stash source changed after preflight: $filename" >&2; return 1; }
                if [ -n "${data_candidate_identity[$filename]+x}" ]; then
                    _bong_server_file_matches_snapshot "$data_file" "${data_candidate_digest[$filename]}" "${data_candidate_identity[$filename]}" || { echo "FAIL: persistence target changed after preflight: $filename" >&2; return 1; }
                elif [ -e "$data_file" ] || [ -L "$data_file" ]; then
                    echo "FAIL: persistence target appeared after preflight: $filename" >&2; return 1
                fi
                mv -f -- "$stash_file" "$data_file" || return 1
                _bong_server_file_matches_snapshot "$data_file" "$expected" "$expected_identity" || { echo "FAIL: restored snapshot mismatch after move: $filename" >&2; return 1; }
            else
                _bong_server_file_matches_snapshot "$data_file" "$expected" "$expected_identity" || { echo "FAIL: partial restore source changed after preflight: $filename" >&2; return 1; }
            fi
        else
            if [ -n "${data_candidate_identity[$filename]+x}" ]; then
                _bong_server_file_matches_snapshot "$data_file" "${data_candidate_digest[$filename]}" "${data_candidate_identity[$filename]}" || { echo "FAIL: persistence target changed before cleanup: $data_file" >&2; return 1; }
                rm -f -- "$data_file" || return 1
            elif [ -e "$data_file" ] || [ -L "$data_file" ]; then
                echo "FAIL: persistence target appeared after preflight: $data_file" >&2; return 1
            fi
        fi
    done
    sync -f -- "$data_dir" || {
        echo "FAIL: could not durably sync restored persistence data directory" >&2
        return 1
    }
    sync -f -- "$stash_dir" || {
        echo "FAIL: could not durably sync persistence stash directory after restore moves" >&2
        return 1
    }
    _bong_server_persistence_transaction_write_marker "RESTORED" "restored" "durable" || return 1
    BONG_SERVER_PERSISTENCE_RESTORE_DURABLE=1
}
