#!/usr/bin/env bash
# dev-reload.sh — one-command regen + validate + rebuild + restart
# Usage: bash scripts/dev-reload.sh [--skip-regen] [--skip-validate] [--console]
#
# --console (worldgen-v4 P1 §8.1 #7, dev-only): after the raster regen+validate,
# launch the FastAPI 3D-preview console_server in the background (logs to
# /tmp/bong-console.log, http://127.0.0.1:8765). The console is fully decoupled
# from the cargo build/restart steps below — it just reads the same rasters. The
# vite + three.js viewer is started separately:
#   cd worldgen/console && npm install && npm run dev
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/bong-server-lifecycle.sh"
background_job_is_running() {
    local pid="${1:-}"
    local running_pid

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    while IFS= read -r running_pid; do
        [ "$running_pid" = "$pid" ] && return 0
    done < <(jobs -pr)
    return 1
}

detach_background_job() {
    local pid="${1:-}"

    if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
        echo "FAIL: invalid background job pid: ${pid:-<empty>}" >&2
        return 1
    fi
    if ! background_job_is_running "$pid"; then
        echo "FAIL: background job $pid is not running in this shell" >&2
        return 1
    fi

    if ! disown "$pid" 2>/dev/null; then
        echo "FAIL: background job $pid could not be detached" >&2
        return 1
    fi
}

background_process_is_running() {
    local pid="${1:-}"
    local state

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    kill -0 "$pid" 2>/dev/null || return 1
    state="$(ps -o stat= -p "$pid" 2>/dev/null)" || return 1
    [[ "$state" != Z* ]]
}

resolve_executable_path() {
    local workdir="${1:-}"
    local executable="${2:-}"
    local search_path="${3-$PATH}"
    local candidate

    [ -n "$workdir" ] && [ -n "$executable" ] || return 1
    (
        cd -- "$workdir" 2>/dev/null || exit 1
        if [[ "$executable" == */* ]]; then
            candidate="$executable"
        else
            candidate="$(PATH="$search_path" type -P -- "$executable")" || exit 1
        fi
        [ -x "$candidate" ] || exit 1
        readlink -f -- "$candidate"
    )
}

wait_for_process_executable() {
    local pid="${1:-}"
    local expected_executable="${2:-}"
    local expected_starttime="${3:-}"
    local actual_executable
    local actual_starttime
    local attempt

    [[ "$pid" =~ ^[0-9]+$ ]] || return 1
    [ -n "$expected_executable" ] || return 1
    # Bong's supported dev/CI hosts are Linux. `/proc/$pid/exe` is the exec
    # acknowledgement: the wrapper PID cannot pass until it has become the
    # requested server image. Test/wrapper callers may pass their final stable
    # image to launch_bong_server; the production call uses the server binary.
    for ((attempt = 0; attempt < 500; attempt++)); do
        background_process_is_running "$pid" || return 1
        actual_executable="$(readlink -f -- "/proc/$pid/exe" 2>/dev/null)" \
            || actual_executable=""
        if [ "$actual_executable" = "$expected_executable" ]; then
            if [ -n "$expected_starttime" ]; then
                # PID-reuse guard: a recycled pid running the same binary must
                # not be accepted as the server. An unreadable stat or a start
                # time that differs from the handoff moment reads as
                # NOT-OUR-PROCESS and fails closed.
                actual_starttime="$(bong_server_process_starttime "$pid")" \
                    || return 1
                [ "$actual_starttime" = "$expected_starttime" ] || return 1
            fi
            return 0
        fi
        sleep 0.01 || return 1
    done
    return 1
}

terminate_background_process() {
    local pid="${1:-}"
    local attempt

    [[ "$pid" =~ ^[0-9]+$ ]] || return 0
    if background_process_is_running "$pid"; then
        kill "$pid" 2>/dev/null || true
        for ((attempt = 0; attempt < 300; attempt++)); do
            background_process_is_running "$pid" || break
            sleep 0.01 || break
        done
    fi
    if background_process_is_running "$pid"; then
        kill -KILL "$pid" 2>/dev/null || true
        for ((attempt = 0; attempt < 300; attempt++)); do
            background_process_is_running "$pid" || break
            sleep 0.01 || break
        done
    fi
    wait "$pid" 2>/dev/null || true
}

rollback_launched_server() {
    local pid="${1:-}"
    local expected_starttime="${2:-}"
    local operation="${3:-failed bong-server launch}"
    local status starttime executable executable_identity

    [[ "$expected_starttime" =~ ^[0-9]+$ ]] || return 2
    if bong_server_process_is_running "$pid"; then
        status=0
    else
        status=$?
    fi
    case "$status" in
        1)
            wait "$pid" 2>/dev/null || true
            return 0
            ;;
        2)
            echo "FAIL: could not inspect failed bong-server launch pid $pid; preserving it for diagnosis" >&2
            return 2
            ;;
    esac
    starttime="$(bong_server_process_starttime "$pid")" || return 2
    # Rollback may only signal the process this launch actually started. If the
    # pid was recycled before rollback, the current occupant has a different
    # start time and must be left alone - the launched process is already gone.
    if [ "$starttime" != "$expected_starttime" ]; then
        echo "INFO: failed bong-server launch pid $pid is no longer the launched process (recycled); nothing to roll back" >&2
        return 0
    fi
    executable="$(bong_server_process_executable "$pid")" || return 2
    executable_identity="$(bong_server_process_executable_identity "$pid")" || return 2
    bong_server_rollback_pinned_managed_process \
        "$pid" "$starttime" "$executable" "$executable_identity" "$operation"
}

launch_detached_job() {
    local pid
    local expected_executable="${2:-}"
    local inspect_status
    local identity_dir identity_pipe identity_fd
    local reported_pid reported_starttime

    DETACHED_PID=""
    DETACHED_STARTTIME=""
    if [ "$#" -eq 0 ]; then
        echo "FAIL: no background command provided" >&2
        return 1
    fi

    identity_dir="$(mktemp -d "${TMPDIR:-/tmp}/bong-launch-identity.XXXXXX")" || {
        echo "FAIL: could not create background identity channel" >&2
        return 1
    }
    identity_pipe="$identity_dir/identity"
    if ! mkfifo -m 600 "$identity_pipe"; then
        echo "FAIL: could not create background identity channel" >&2
        rmdir "$identity_dir" 2>/dev/null || true
        return 1
    fi
    if ! exec {identity_fd}<>"$identity_pipe"; then
        echo "FAIL: could not open background identity channel" >&2
        rm -f "$identity_pipe"
        rmdir "$identity_dir" 2>/dev/null || true
        return 1
    fi
    if ! rm -f "$identity_pipe" || ! rmdir "$identity_dir"; then
        echo "FAIL: could not unlink background identity channel" >&2
        exec {identity_fd}>&-
        return 1
    fi

    (
        local child_pid child_starttime

        trap '' HUP
        trap - EXIT
        child_pid="$BASHPID"
        child_starttime="$(bong_server_process_starttime "$child_pid")" || exit 125
        printf '%s %s\n' "$child_pid" "$child_starttime" >&"$identity_fd" || exit 125
        exec {identity_fd}>&-
        exec < /dev/null
        "$@"
    ) &
    pid=$!
    # The child reports its own identity while it is necessarily still alive.
    # The parent never reads /proc through a pid that may already be recycled;
    # exec preserves the reported pid and start time through image transition.
    if ! IFS=' ' read -r -t 5 -u "$identity_fd" reported_pid reported_starttime; then
        echo "FAIL: background job $pid did not report its fork-time identity" >&2
        exec {identity_fd}>&-
        if background_job_is_running "$pid"; then
            kill "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
        return 1
    fi
    exec {identity_fd}>&-
    if [ "$reported_pid" != "$pid" ] || [[ ! "$reported_starttime" =~ ^[0-9]+$ ]]; then
        echo "FAIL: background job $pid reported an invalid fork-time identity" >&2
        if background_job_is_running "$pid"; then
            kill "$pid" 2>/dev/null || true
        fi
        wait "$pid" 2>/dev/null || true
        return 1
    fi
    DETACHED_STARTTIME="$reported_starttime"
    if ! detach_background_job "$pid"; then
        # Tri-state probe (bong_server_process_is_running: 0=live, 1=confirmed
        # absent/zombie, 2=UNKNOWN inspection). Only confirmed death may be
        # reported as a launch failure - a failed inspection means kill -0
        # succeeded, the wrapper may still be live and never detached.
        if bong_server_process_is_running "$pid"; then
            inspect_status=0
        else
            inspect_status=$?
        fi
        if [ "$inspect_status" -ne 1 ]; then
            if [ "$inspect_status" -eq 2 ]; then
                echo "FAIL: could not inspect background job $pid after detach failure; failing closed" >&2
            else
                echo "FAIL: background job $pid is still running after detach failure; failing closed" >&2
            fi
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
            DETACHED_PID=""
            DETACHED_STARTTIME=""
            return 1
        fi
        # The wrapper died inside the detach window, so there is no live
        # process that can be handed to the exec poller. Refuse the handoff
        # outright: publishing a dead pid would let a recycled occupant be
        # mistaken for this launch. The exec-identity diagnostic is still
        # reported so the failure reads the same regardless of where in the
        # detach window the wrapper died.
        if [ -n "$expected_executable" ]; then
            echo "FAIL: bong server process $pid did not exec expected executable $expected_executable" >&2
        fi
        echo "FAIL: background job $pid died before detach completed; refusing to publish its pid" >&2
        wait "$pid" 2>/dev/null || true
        DETACHED_PID=""
        DETACHED_STARTTIME=""
        return 1
    fi
    # Post-detach tri-state probe: only a confirmed-live process may be
    # published. A dead wrapper's pid could be recycled and then accepted by
    # the exec poller as the server; an uninspectable one cannot be verified.
    if ! bong_server_process_is_running "$pid"; then
        inspect_status=$?
        if [ "$inspect_status" -eq 2 ]; then
            echo "FAIL: could not inspect background job $pid after detach; failing closed" >&2
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
            DETACHED_PID=""
            DETACHED_STARTTIME=""
            return 1
        fi
        if [ -n "$expected_executable" ]; then
            echo "FAIL: bong server process $pid did not exec expected executable $expected_executable" >&2
        else
            echo "FAIL: background job $pid died after detach; refusing to publish its pid" >&2
        fi
        wait "$pid" 2>/dev/null || true
        DETACHED_PID=""
        DETACHED_STARTTIME=""
        return 1
    fi
    DETACHED_PID="$pid"
}

run_bong_server() {
    local workdir="${BONG_SERVER_WORKDIR:-server}"
    local executable="${BONG_SERVER_EXECUTABLE:-./target/debug/bong-server}"
    local log_path="${BONG_SERVER_LOG:-/tmp/bong-server.log}"
    local -a env_args=()

    if declare -p ENV_ARGS >/dev/null 2>&1; then
        env_args=("${ENV_ARGS[@]}")
    fi

    if ! cd -- "$workdir"; then
        echo "FAIL: could not enter bong server workdir: $workdir" >&2
        return 1
    fi
    exec env "${env_args[@]}" "$executable" > "$log_path" 2>&1
}

launch_bong_server() {
    local startup_grace="${BONG_SERVER_STARTUP_GRACE_SECONDS:-2}"
    local workdir="${BONG_SERVER_WORKDIR:-server}"
    local executable="${BONG_SERVER_EXECUTABLE:-./target/debug/bong-server}"
    local expected_executable="${1:-}"
    local resolved_executable
    local resolved_expected_executable
    local launched_pid
    local effective_path="$PATH"
    local env_arg

    SERVER_PID=""
    DETACHED_PID=""
    DETACHED_STARTTIME=""
    if [[ ! "$startup_grace" =~ ^([0-9]+([.][0-9]*)?|[.][0-9]+)$ ]]; then
        echo "FAIL: BONG_SERVER_STARTUP_GRACE_SECONDS must be a non-negative number: $startup_grace" >&2
        return 1
    fi
    if ! (cd -- "$workdir") 2>/dev/null; then
        echo "FAIL: could not enter bong server workdir: $workdir" >&2
        return 1
    fi
    if declare -p ENV_ARGS >/dev/null 2>&1; then
        for env_arg in "${ENV_ARGS[@]}"; do
            if [[ "$env_arg" == PATH=* ]]; then
                effective_path="${env_arg#PATH=}"
            fi
        done
    fi
    if ! resolved_executable="$(
        resolve_executable_path "$workdir" "$executable" "$effective_path"
    )"; then
        echo "FAIL: bong server executable is not executable: $executable" >&2
        return 1
    fi
    if [ -n "$expected_executable" ]; then
        if ! resolved_expected_executable="$(
            resolve_executable_path "$workdir" "$expected_executable" "$effective_path"
        )"; then
            echo "FAIL: expected bong server executable is not executable: $expected_executable" >&2
            return 1
        fi
    else
        resolved_expected_executable="$resolved_executable"
    fi
    launch_detached_job run_bong_server "$resolved_expected_executable" || return 1
    launched_pid="$DETACHED_PID"
    # The start time was pinned at fork inside launch_detached_job, before any
    # detach or death window, so it can never bind to a recycled pid. The exec
    # acknowledgement compares the current occupant against that pin: a
    # recycled pid running the same binary is NOT-OUR-PROCESS and fails closed.
    if ! wait_for_process_executable "$launched_pid" "$resolved_expected_executable" "$DETACHED_STARTTIME"; then
        echo "FAIL: bong server process $launched_pid did not exec expected executable $resolved_expected_executable" >&2
        rollback_launched_server "$launched_pid" "$DETACHED_STARTTIME" "final executable verification" || return $?
        SERVER_PID=""
        DETACHED_PID=""
        DETACHED_STARTTIME=""
        return 1
    fi
    SERVER_PID="$launched_pid"
    # Zero grace must not fork an external sleep: a fork failure at exactly
    # this point would misreport an otherwise healthy launch. Branch on the
    # numeric value, not the textual form - every representation the format
    # check accepts (0, 0.0, 00, .0, 0.00, ...) denotes zero and must skip
    # the fork.
    if [[ ! "$startup_grace" =~ ^0*([.][0]*)?$ ]] && ! sleep "$startup_grace"; then
        echo "FAIL: bong server startup grace wait failed: $startup_grace" >&2
        rollback_launched_server "$SERVER_PID" "$DETACHED_STARTTIME" "startup grace wait" || return $?
        SERVER_PID=""
        DETACHED_PID=""
        DETACHED_STARTTIME=""
        return 1
    fi
    if ! bong_server_write_record "$SERVER_PID" "$resolved_expected_executable"; then
        echo "FAIL: could not record managed bong server pid $SERVER_PID" >&2
        rollback_launched_server "$SERVER_PID" "$DETACHED_STARTTIME" "managed PID record publish" || return $?
        SERVER_PID=""
        DETACHED_PID=""
        DETACHED_STARTTIME=""
        return 1
    fi
}

stop_managed_server_before_reload() {
    local status

    if bong_server_stop_managed_for_replacement "server rebuild and relaunch"; then
        return 0
    else
        status=$?
    fi
    echo "FAIL: previous server could not be stopped safely; refusing rebuild and relaunch" >&2
    return "$status"
}

# Tests source this file to exercise the exact production detach and stop helpers.
if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
    return 0
fi

set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)" || {
    echo "FAIL: dev-reload.sh 必须在 git worktree 内运行" >&2
    exit 1
}
if [ -z "$ROOT" ]; then
    echo "FAIL: git rev-parse 返回了空仓库根目录" >&2
    exit 1
fi
cd "$ROOT"

SKIP_REGEN=false
SKIP_VALIDATE=false
LAUNCH_CONSOLE=false
for arg in "$@"; do
    case "$arg" in
        --skip-regen)    SKIP_REGEN=true ;;
        --skip-validate) SKIP_VALIDATE=true ;;
        --console)       LAUNCH_CONSOLE=true ;;
    esac
done

RASTER_DIR="worldgen/generated/terrain-gen/rasters"
WORLDGEN_RASTER_DIR="generated/terrain-gen/rasters"
MANIFEST="$RASTER_DIR/manifest.json"

# plan-tsy-worldgen-v1 §6.1 — TSY 双 manifest 改造
TSY_BLUEPRINT="server/zones.tsy.json"
WORLDGEN_TSY_OUTPUT_DIR="generated/terrain-gen-tsy"
TSY_RASTER_DIR="worldgen/$WORLDGEN_TSY_OUTPUT_DIR/rasters"
WORLDGEN_TSY_RASTER_DIR="$WORLDGEN_TSY_OUTPUT_DIR/rasters"
TSY_MANIFEST="$TSY_RASTER_DIR/manifest.json"

# --- Step 1: Regenerate rasters (overworld + optional TSY) ---
if [ "$SKIP_REGEN" = false ]; then
    if [ -f "$TSY_BLUEPRINT" ]; then
        echo "==> [1/4] Regenerating terrain rasters (overworld + tsy)..."
        (cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster \
             --tsy-blueprint "../$TSY_BLUEPRINT" \
             --tsy-output-dir "$WORLDGEN_TSY_OUTPUT_DIR") || {
            echo "FAIL: terrain generation failed"; exit 1
        }
    else
        echo "==> [1/4] Regenerating terrain rasters (overworld only — no $TSY_BLUEPRINT)..."
        (cd worldgen && .venv/bin/python -m scripts.terrain_gen --backend raster) || {
            echo "FAIL: terrain generation failed"; exit 1
        }
    fi
    echo "    OK"
else
    echo "==> [1/4] Skipping raster regeneration (--skip-regen)"
fi

# --- Step 2: Validate raster data (overworld + optional TSY) ---
if [ "$SKIP_VALIDATE" = false ]; then
    echo "==> [2/4] Validating raster data..."
    (cd worldgen && .venv/bin/python -c "
from scripts.terrain_gen.harness.raster_check import validate_rasters
import sys
ok, msg = validate_rasters('$WORLDGEN_RASTER_DIR')
print('[overworld]')
print(msg)
ok_all = ok
import os.path
if os.path.isdir('$WORLDGEN_TSY_RASTER_DIR'):
    ok2, msg2 = validate_rasters('$WORLDGEN_TSY_RASTER_DIR')
    print('[tsy]')
    print(msg2)
    ok_all = ok_all and ok2
sys.exit(0 if ok_all else 1)
") || { echo "FAIL: raster validation failed"; exit 1; }
    echo "    OK"
else
    echo "==> [2/4] Skipping validation (--skip-validate)"
fi

# --- Optional: launch dev-only 3D preview console (worldgen-v4 P1 §8.1 #7) ---
# Decoupled from build/restart — backgrounded so the 4-step flow continues.
if [ "$LAUNCH_CONSOLE" = true ]; then
    echo "==> [console] Launching worldgen-v4 3D preview console (dev-only)..."
    pkill -f 'scripts.terrain_gen.console_server' 2>/dev/null || true
    (cd worldgen && .venv/bin/python -m scripts.terrain_gen.console_server \
         --rasters "$WORLDGEN_RASTER_DIR" \
         > /tmp/bong-console.log 2>&1 &)
    disown
    echo "    console -> http://127.0.0.1:8765 (log: /tmp/bong-console.log)"
    echo "    viewer  -> cd worldgen/console && npm install && npm run dev"
fi

restart_bong_server() {
# --- Step 3: Stop the previous server before rebuilding ---
# Cargo may atomically replace the executable. Stop while its exact PID/starttime
# record is still valid so a normal SIGTERM reaches the AppExit bridge.
echo "==> [3/5] Stopping previous server..."
stop_managed_server_before_reload || return $?

# --- Step 4: Rebuild server ---
echo "==> [4/5] Building server..."
(cd server && "$ROOT/scripts/build-token.sh" cargo build 2>&1) || { echo "FAIL: cargo build failed"; exit 1; }
echo "    OK"

# --- Step 5: Launch server ---
echo "==> [5/5] Starting server..."
MANIFEST_ABS="$(pwd)/$MANIFEST"
TSY_MANIFEST_ABS="$(pwd)/$TSY_MANIFEST"
ENV_ARGS=("BONG_TERRAIN_RASTER_PATH=$MANIFEST_ABS")
if [ -f "$TSY_MANIFEST_ABS" ]; then
    ENV_ARGS+=("BONG_TSY_RASTER_PATH=$TSY_MANIFEST_ABS")
fi
launch_bong_server

if grep -q "loaded.*terrain tiles" /tmp/bong-server.log 2>/dev/null; then
    TILES=$(grep -o 'loaded [0-9]* terrain' /tmp/bong-server.log | grep -o '[0-9]*')
    echo "    Server running — $TILES tiles loaded"
else
    echo "    Server starting... check /tmp/bong-server.log"
fi

echo "==> Done. Connect to localhost:25565"
}

bong_server_with_lock restart_bong_server
