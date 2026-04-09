#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"
LOG_FILE="$EVIDENCE_DIR/task-26-smoke.log"
ERROR_FILE="$EVIDENCE_DIR/task-26-smoke-error.txt"
SUCCESS_FILE="$EVIDENCE_DIR/task-26-smoke-success.txt"
MANIFEST_FILE="$EVIDENCE_DIR/task-26-smoke-manifest.txt"
SERVER_BOOT_LOG="$EVIDENCE_DIR/task-26-server-start.log"
TIANDAO_BOOT_LOG="$EVIDENCE_DIR/task-26-tiandao-start.log"
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"

mkdir -p "$EVIDENCE_DIR"
touch "$LOG_FILE"

exec > >(tee -a "$LOG_FILE") 2>&1

RUN_TS="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

write_manifest() {
  local status="$1"
  local stage_name="$2"
  local message="$3"

  printf "task=26\nrun=%s\nstatus=%s\nstage=%s\nmessage=%s\nfiles:\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n" \
    "$RUN_TS" \
    "$status" \
    "$stage_name" \
    "$message" \
    "$LOG_FILE" \
    "$ERROR_FILE" \
    "$SUCCESS_FILE" \
    "$MANIFEST_FILE" \
    "$SERVER_BOOT_LOG" \
    "$TIANDAO_BOOT_LOG" > "$MANIFEST_FILE"

  echo "[evidence] manifest written: $MANIFEST_FILE"
  echo "[evidence] files:"
  echo "  - $LOG_FILE"
  echo "  - $ERROR_FILE"
  echo "  - $SUCCESS_FILE"
  echo "  - $MANIFEST_FILE"
  echo "  - $SERVER_BOOT_LOG"
  echo "  - $TIANDAO_BOOT_LOG"
}

stage() {
  local index="$1"
  local name="$2"
  echo ""
  echo "=== [Task-26][${index}] ${name} ==="
}

fail_stage() {
  local stage_name="$1"
  local message="$2"

  rm -f "$SUCCESS_FILE"
  printf "task=26\nstatus=FAILED\nstage=%s\nmessage=%s\nlog=%s\n" \
    "$stage_name" \
    "$message" \
    "$LOG_FILE" > "$ERROR_FILE"

  write_manifest "FAILED" "$stage_name" "$message"

  echo "[Task-26][FAIL][${stage_name}] ${message}"
  exit 1
}

run_or_fail() {
  local stage_name="$1"
  local description="$2"
  shift 2

  echo "[run][${stage_name}] ${description}"
  if ! "$@"; then
    fail_stage "$stage_name" "${description}"
  fi
}

require_anchor() {
  local stage_name="$1"
  local log_path="$2"
  local pattern="$3"
  local label="$4"

  if ! grep -qE "$pattern" "$log_path"; then
    fail_stage "$stage_name" "missing startup anchor: $label"
  fi

  echo "[anchor][${stage_name}] ${label}"
}

echo "===== [Task-26] run start: ${RUN_TS} ====="

stage "0/10" "Worktree context"
if [[ ! -d "$ROOT/server" || ! -d "$ROOT/agent" || ! -d "$ROOT/client" ]]; then
  fail_stage "context" "missing one of required directories: server/ agent/ client/"
fi
if [[ ! -d "$ROOT/.git" && ! -f "$ROOT/.git" ]]; then
  fail_stage "context" "current path is not a git worktree"
fi
echo "[context] root=${ROOT}"

stage "1/10" "Redis environment (existing service, no Docker)"
if ! command -v redis-cli >/dev/null 2>&1; then
  fail_stage "redis" "redis-cli not found; install Redis client/server in environment"
fi

redis_ping_output="$(timeout 3s redis-cli -u "$REDIS_URL" PING 2>&1 || true)"
echo "[redis] ping(${REDIS_URL}) => ${redis_ping_output}"
if [[ "$redis_ping_output" != "PONG" ]]; then
  fail_stage "redis" "cannot reach Redis at ${REDIS_URL}; expected PONG"
fi

stage "2/10" "Contract anchors"
if ! grep -q '"name": "@bong/schema"' "$ROOT/agent/packages/schema/package.json"; then
  fail_stage "contract" "@bong/schema package marker not found"
fi
if ! grep -q 'new Identifier("bong", "server_data")' "$ROOT/client/src/main/java/com/bong/client/network/BongNetworkHandler.java"; then
  fail_stage "contract" "client bong:server_data channel anchor not found"
fi
echo "[contract] @bong/schema and bong:server_data anchors verified"

stage "3/10" "Schema smoke (@bong/schema source-of-truth)"
run_or_fail "schema" "npm run build" bash -lc "cd '$ROOT/agent/packages/schema' && npm run build"
run_or_fail "schema" "npm test -- tests/schema.test.ts" bash -lc "cd '$ROOT/agent/packages/schema' && npm test -- tests/schema.test.ts"

stage "4/10" "Server entrypoint startup"
run_or_fail "server-start" "cargo build" bash -lc "cd '$ROOT/server' && cargo build"
server_start_exit=0
echo "[run][server-start] timeout 20s cargo run"
timeout 20s bash -lc "cd '$ROOT/server' && cargo run" > "$SERVER_BOOT_LOG" 2>&1 || server_start_exit=$?
echo "[server-start] exit=${server_start_exit}, log=${SERVER_BOOT_LOG}"
if [[ "$server_start_exit" -ne 0 && "$server_start_exit" -ne 124 ]]; then
  fail_stage "server-start" "cargo run failed before startup anchors"
fi
require_anchor "server-start" "$SERVER_BOOT_LOG" "\\[bong\\]\\[bridge\\] tokio runtime started" "bridge runtime started"
require_anchor "server-start" "$SERVER_BOOT_LOG" "creating overworld" "world creation"
require_anchor "server-start" "$SERVER_BOOT_LOG" "\\[bong\\]\\[player\\] registering player init/cleanup systems" "player systems registered"
require_anchor "server-start" "$SERVER_BOOT_LOG" "\\[bong\\]\\[redis\\] connecting to" "redis bridge connection"

stage "5/10" "Tiandao entrypoint startup"
run_or_fail "tiandao-start" "npm run build" bash -lc "cd '$ROOT/agent/packages/tiandao' && npm run build"
tiandao_start_exit=0
echo "[run][tiandao-start] timeout 40s npm run start:mock"
timeout 40s bash -lc "cd '$ROOT/agent/packages/tiandao' && npm run start:mock" > "$TIANDAO_BOOT_LOG" 2>&1 || tiandao_start_exit=$?
echo "[tiandao-start] exit=${tiandao_start_exit}, log=${TIANDAO_BOOT_LOG}"
if [[ "$tiandao_start_exit" -ne 0 && "$tiandao_start_exit" -ne 124 ]]; then
  fail_stage "tiandao-start" "npm run start:mock failed before startup anchors"
fi
require_anchor "tiandao-start" "$TIANDAO_BOOT_LOG" "\\[tiandao\\] mode: mock" "mock mode selected"
require_anchor "tiandao-start" "$TIANDAO_BOOT_LOG" "\\[tiandao\\] === tick start ===" "tick start"
require_anchor "tiandao-start" "$TIANDAO_BOOT_LOG" "\\[tiandao\\] === tick end ===" "tick end"

stage "6/10" "Tiandao smoke tests (world_state -> command/narration path)"
run_or_fail "tiandao" "npm run test -- runtime redis-ipc main-loop" bash -lc "cd '$ROOT/agent/packages/tiandao' && npm run test -- runtime redis-ipc main-loop"

stage "7/10" "Server smoke tests (world_state + command executor + scoped narration)"
run_or_fail "server" "cargo test world_state_tests::uses_real_player_names_and_positions" bash -lc "cd '$ROOT/server' && cargo test world_state_tests::uses_real_player_names_and_positions -- --nocapture"
run_or_fail "server" "cargo test command_executor_tests::applies_modify_zone" bash -lc "cd '$ROOT/server' && cargo test command_executor_tests::applies_modify_zone -- --nocapture"
run_or_fail "server" "cargo test narration_tests::player_scope_matches_username_and_offline_id" bash -lc "cd '$ROOT/server' && cargo test narration_tests::player_scope_matches_username_and_offline_id -- --nocapture"

stage "8/10" "Client smoke tests (typed payload parsing)"
run_or_fail "client" "./gradlew test --tests *BongNetworkHandlerTest --tests *NarrationPayloadParserTest" bash -lc "cd '$ROOT/client' && ./gradlew test --tests '*BongNetworkHandlerTest' --tests '*NarrationPayloadParserTest'"

stage "9/10" "Cross-layer closure proof"
echo "[closure] world_state publication -> server world_state_tests::uses_real_player_names_and_positions"
echo "[closure] tiandao startup entrypoint -> npm run start:mock with tick anchors"
echo "[closure] agent command/narration path -> tiandao runtime + redis-ipc + main-loop tests"
echo "[closure] server startup entrypoint -> timeout cargo run with bridge/world/player/redis anchors"
echo "[closure] server execution path -> command_executor_tests::applies_modify_zone + narration_tests::player_scope_matches_username_and_offline_id"
echo "[closure] client payload parsing -> BongNetworkHandlerTest + NarrationPayloadParserTest"

stage "10/10" "Evidence manifest"
printf "task=26\nstatus=PASS\nrun=%s\nmessage=all-stages-passed\nlog=%s\nmanifest=%s\n" \
  "$RUN_TS" \
  "$LOG_FILE" \
  "$MANIFEST_FILE" > "$SUCCESS_FILE"
write_manifest "PASS" "complete" "all-stages-passed"

echo "[Task-26] smoke harness PASS"
