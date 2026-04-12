#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"
TASK_ID="task-13"
SCRIPT_TAG="smoke-test-e2e"
RUN_LABEL="${RUN_LABEL:-default}"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$-${RUN_LABEL}"
RUN_DIR="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-run-${RUN_ID}"
LOG_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}.log"
ERROR_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-error.log"
SUCCESS_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-success.txt"
MANIFEST_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-manifest.txt"

NODE_BIN="$ROOT/agent/node_modules/.bin"
RUST_PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"

SCHEMA_LOG="$RUN_DIR/schema.log"
AGENT_LOG="$RUN_DIR/agent.log"
SERVER_LOG="$RUN_DIR/server.log"
E2E_LOG="$RUN_DIR/e2e.log"

PASS=0
FAIL=0
CURRENT_STAGE="init"

mkdir -p "$EVIDENCE_DIR" "$RUN_DIR"
touch "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1

pass() {
  echo "  ✓ $1"
  PASS=$((PASS + 1))
}

write_manifest() {
  local status="$1"
  local stage_name="$2"
  local message="$3"
  printf "task=%s\nscript=%s\nrun_id=%s\nrun_label=%s\nstatus=%s\nstage=%s\nmessage=%s\ntimestamp=%s\nfiles:\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n" \
    "$TASK_ID" \
    "$SCRIPT_TAG" \
    "$RUN_ID" \
    "$RUN_LABEL" \
    "$status" \
    "$stage_name" \
    "$message" \
    "$(date -Iseconds)" \
    "$LOG_FILE" \
    "$ERROR_FILE" \
    "$MANIFEST_FILE" \
    "$SUCCESS_FILE" \
    "$SCHEMA_LOG" \
    "$AGENT_LOG" \
    "$SERVER_LOG" \
    "$E2E_LOG" >"$MANIFEST_FILE"
}

finalize_failure() {
  local stage_name="$1"
  local message="$2"
  FAIL=$((FAIL + 1))
  rm -f "$SUCCESS_FILE"
  printf "task=%s\nscript=%s\nstatus=FAILED\nstage=%s\nmessage=%s\nrun_id=%s\n" \
    "$TASK_ID" \
    "$SCRIPT_TAG" \
    "$stage_name" \
    "$message" \
    "$RUN_ID" >"$ERROR_FILE"
  write_manifest "FAILED" "$stage_name" "$message"
  echo "[evidence] manifest: $MANIFEST_FILE"
  echo "[evidence] run_dir: $RUN_DIR"
  echo "[$TASK_ID][FAIL][$stage_name] $message"
  exit 1
}

echo "===== $TASK_ID $SCRIPT_TAG ====="
echo "run_label: $RUN_LABEL"
echo "run_id: $RUN_ID"
echo "run_dir: $RUN_DIR"
echo "log_file: $LOG_FILE"

echo ""
CURRENT_STAGE="pre-cleanup"
echo "=== [$TASK_ID][$SCRIPT_TAG][0/5] Pre-cleanup ==="
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup complete"

echo ""
CURRENT_STAGE="schema"
echo "=== [$TASK_ID][$SCRIPT_TAG][1/5] Schema staged smoke ==="
if (
  cd "$ROOT/agent/packages/schema" && \
    PATH="$NODE_BIN:$PATH" npm run check && \
    PATH="$NODE_BIN:$PATH" npm test && \
    PATH="$NODE_BIN:$PATH" npm run generate
) >"$SCHEMA_LOG" 2>&1; then
  pass "schema check"
  pass "schema test"
  pass "schema generate"
else
  finalize_failure "schema" "schema staged smoke failed; see $SCHEMA_LOG"
fi

echo ""
CURRENT_STAGE="agent"
echo "=== [$TASK_ID][$SCRIPT_TAG][2/5] Agent staged smoke ==="
if (
  cd "$ROOT/agent/packages/tiandao" && \
    PATH="$NODE_BIN:$PATH" npm run check && \
    PATH="$NODE_BIN:$PATH" npm test
) >"$AGENT_LOG" 2>&1; then
  pass "tiandao check"
  pass "tiandao test"
else
  finalize_failure "agent" "tiandao staged smoke failed; see $AGENT_LOG"
fi

echo ""
CURRENT_STAGE="server"
echo "=== [$TASK_ID][$SCRIPT_TAG][3/5] Server staged smoke ==="
if (
  export PATH="$RUST_PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo test
) >"$SERVER_LOG" 2>&1; then
  pass "server cargo test"
else
  finalize_failure "server" "server staged smoke failed; see $SERVER_LOG"
fi

echo ""
CURRENT_STAGE="e2e"
echo "=== [$TASK_ID][$SCRIPT_TAG][4/5] Redis e2e closure ==="
if bash "$ROOT/scripts/e2e-redis.sh" >"$E2E_LOG" 2>&1; then
  pass "e2e redis harness"
else
  finalize_failure "e2e" "e2e redis harness failed; see $E2E_LOG"
fi

CURRENT_STAGE="summary"
echo ""
echo "=== [$TASK_ID][$SCRIPT_TAG] Evidence paths ==="
echo "  log: $LOG_FILE"
echo "  error: $ERROR_FILE"
echo "  manifest: $MANIFEST_FILE"
echo "  run_dir: $RUN_DIR"
echo "  schema: $SCHEMA_LOG"
echo "  agent: $AGENT_LOG"
echo "  server: $SERVER_LOG"
echo "  e2e: $E2E_LOG"

echo ""
echo "=== [$TASK_ID][$SCRIPT_TAG] Result ==="
echo "Result: $PASS passed, $FAIL failed"

if [ "$FAIL" -eq 0 ]; then
  printf "task=%s\nstatus=PASS\nrun_id=%s\nmessage=all-stages-passed\n" "$TASK_ID" "$RUN_ID" >"$SUCCESS_FILE"
  write_manifest "PASS" "complete" "all-stages-passed"
  echo "ALL PASS"
  exit 0
fi

finalize_failure "$CURRENT_STAGE" "unexpected failure state"
