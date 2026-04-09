#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"

SMOKE_LOG="$EVIDENCE_DIR/task-24-final-matrix.log"
ERROR_LOG="$EVIDENCE_DIR/task-24-final-matrix-error.log"

RUN_LABEL="${RUN_LABEL:-default}"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$-${RUN_LABEL}"
RUN_DIR="$EVIDENCE_DIR/task-24-run-$RUN_ID"

SCHEMA_STAGE_LOG="$RUN_DIR/schema.log"
AGENT_CHECK_LOG="$RUN_DIR/agent-check.log"
AGENT_TEST_LOG="$RUN_DIR/agent-test.log"
AGENT_START_MOCK_LOG="$RUN_DIR/agent-start-mock.log"
SERVER_FMT_LOG="$RUN_DIR/server-fmt.log"
SERVER_CLIPPY_LOG="$RUN_DIR/server-clippy.log"
SERVER_TEST_LOG="$RUN_DIR/server-test.log"
SERVER_PROOF_LOG="$RUN_DIR/server-proof.log"
CLIENT_TEST_LOG="$RUN_DIR/client-test.log"
CLIENT_BUILD_LOG="$RUN_DIR/client-build.log"
FULLSTACK_REDIS_LOG="$RUN_DIR/fullstack-redis.log"
FULLSTACK_SERVER_LOG="$RUN_DIR/fullstack-server.log"
FULLSTACK_REDIS_SUB_LOG="$RUN_DIR/fullstack-redis-sub.log"
FULLSTACK_TIANDAO_LOG="$RUN_DIR/fullstack-tiandao.log"

PASS=0
FAIL=0
CURRENT_STAGE="init"

REDIS_PID=""
SERVER_PID=""
REDIS_SUB_PID=""

pass() {
  echo "  ✓ $1"
  PASS=$((PASS + 1))
}

fail() {
  echo "  ✗ $1"
  FAIL=$((FAIL + 1))
}

wait_for_pattern() {
  local file="$1"
  local pattern="$2"
  local timeout_secs="$3"

  local elapsed=0
  while [ "$elapsed" -lt "$timeout_secs" ]; do
    if [ -f "$file" ] && grep -Eq "$pattern" "$file"; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

wait_for_redis_ping() {
  local timeout_secs="$1"

  local elapsed=0
  while [ "$elapsed" -lt "$timeout_secs" ]; do
    if redis-cli ping >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  return 1
}

dump_failure_context() {
  {
    echo "===== task-24 failure context ====="
    echo "stage: $CURRENT_STAGE"
    echo "run_label: $RUN_LABEL"
    echo "run_id: $RUN_ID"
    echo "run_dir: $RUN_DIR"
    echo "timestamp: $(date -Iseconds)"
    echo ""
    echo "----- schema.log (last 120 lines) -----"
    tail -n 120 "$SCHEMA_STAGE_LOG" 2>/dev/null || true
    echo ""
    echo "----- agent-check.log (last 120 lines) -----"
    tail -n 120 "$AGENT_CHECK_LOG" 2>/dev/null || true
    echo ""
    echo "----- agent-test.log (last 120 lines) -----"
    tail -n 120 "$AGENT_TEST_LOG" 2>/dev/null || true
    echo ""
    echo "----- agent-start-mock.log (last 120 lines) -----"
    tail -n 120 "$AGENT_START_MOCK_LOG" 2>/dev/null || true
    echo ""
    echo "----- server-fmt.log (last 120 lines) -----"
    tail -n 120 "$SERVER_FMT_LOG" 2>/dev/null || true
    echo ""
    echo "----- server-clippy.log (last 120 lines) -----"
    tail -n 120 "$SERVER_CLIPPY_LOG" 2>/dev/null || true
    echo ""
    echo "----- server-test.log (last 120 lines) -----"
    tail -n 120 "$SERVER_TEST_LOG" 2>/dev/null || true
    echo ""
    echo "----- server-proof.log (last 160 lines) -----"
    tail -n 160 "$SERVER_PROOF_LOG" 2>/dev/null || true
    echo ""
    echo "----- client-test.log (last 120 lines) -----"
    tail -n 120 "$CLIENT_TEST_LOG" 2>/dev/null || true
    echo ""
    echo "----- client-build.log (last 120 lines) -----"
    tail -n 120 "$CLIENT_BUILD_LOG" 2>/dev/null || true
    echo ""
    echo "----- fullstack-server.log (last 160 lines) -----"
    tail -n 160 "$FULLSTACK_SERVER_LOG" 2>/dev/null || true
    echo ""
    echo "----- fullstack-tiandao.log (last 160 lines) -----"
    tail -n 160 "$FULLSTACK_TIANDAO_LOG" 2>/dev/null || true
    echo ""
    echo "----- fullstack-redis-sub.log (last 160 lines) -----"
    tail -n 160 "$FULLSTACK_REDIS_SUB_LOG" 2>/dev/null || true
    echo ""
  } >>"$ERROR_LOG"
}

cleanup() {
  local exit_code=$?

  if [ -n "$REDIS_SUB_PID" ] && kill -0 "$REDIS_SUB_PID" 2>/dev/null; then
    kill "$REDIS_SUB_PID" 2>/dev/null || true
    wait "$REDIS_SUB_PID" 2>/dev/null || true
  fi

  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi

  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
    redis-cli shutdown nosave >/dev/null 2>&1 || true
    wait "$REDIS_PID" 2>/dev/null || true
  fi

  bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true

  if [ "$exit_code" -ne 0 ] || [ "$FAIL" -ne 0 ]; then
    dump_failure_context
  fi
}

mkdir -p "$EVIDENCE_DIR" "$RUN_DIR"
: >"$SCHEMA_STAGE_LOG"
: >"$AGENT_CHECK_LOG"
: >"$AGENT_TEST_LOG"
: >"$AGENT_START_MOCK_LOG"
: >"$SERVER_FMT_LOG"
: >"$SERVER_CLIPPY_LOG"
: >"$SERVER_TEST_LOG"
: >"$SERVER_PROOF_LOG"
: >"$CLIENT_TEST_LOG"
: >"$CLIENT_BUILD_LOG"
: >"$FULLSTACK_REDIS_LOG"
: >"$FULLSTACK_SERVER_LOG"
: >"$FULLSTACK_REDIS_SUB_LOG"
: >"$FULLSTACK_TIANDAO_LOG"
echo "[task-24] no failures recorded for run $RUN_ID" >"$ERROR_LOG"

exec > >(tee "$SMOKE_LOG") 2>&1
trap cleanup EXIT

echo "=== Task 24 Final Fullstack Matrix (M1-M3) ==="
echo "run_label: $RUN_LABEL"
echo "run_id: $RUN_ID"
echo "run_dir: $RUN_DIR"
echo "smoke_log: $SMOKE_LOG"
echo "error_log: $ERROR_LOG"

echo ""
CURRENT_STAGE="pre-cleanup"
echo "=== [0/11] Pre-cleanup ==="
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup completed"

echo ""
CURRENT_STAGE="schema"
echo "=== [1/11] schema -> check/test/generate ==="
if (cd "$ROOT/agent/packages/schema" && npm run check) >"$SCHEMA_STAGE_LOG" 2>&1; then
  pass "schema check"
else
  fail "schema check"
fi
if (cd "$ROOT/agent/packages/schema" && npm test) >>"$SCHEMA_STAGE_LOG" 2>&1; then
  pass "schema test"
else
  fail "schema test"
fi
if (cd "$ROOT/agent/packages/schema" && npm run generate) >>"$SCHEMA_STAGE_LOG" 2>&1; then
  pass "schema generate"
else
  fail "schema generate"
fi

echo ""
CURRENT_STAGE="tiandao"
echo "=== [2/11] tiandao -> check/test/start:mock (+chat drain proof) ==="
if (cd "$ROOT/agent/packages/tiandao" && npm run check) >"$AGENT_CHECK_LOG" 2>&1; then
  pass "tiandao check"
else
  fail "tiandao check"
fi
if (cd "$ROOT/agent/packages/tiandao" && npm test) >"$AGENT_TEST_LOG" 2>&1; then
  pass "tiandao test"
else
  fail "tiandao test"
fi
if (cd "$ROOT/agent/packages/tiandao" && timeout 120s npm run start:mock) >"$AGENT_START_MOCK_LOG" 2>&1; then
  pass "tiandao start:mock"
else
  fail "tiandao start:mock"
fi
if grep -Eq 'tests/chat-drain\.test\.ts' "$AGENT_TEST_LOG"; then
  pass "chat drain proof"
else
  fail "chat drain proof"
fi

echo ""
CURRENT_STAGE="server"
echo "=== [3/11] server -> fmt/clippy/test + persistence/progression/payload proofs ==="
if (
  export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo fmt --check
) >"$SERVER_FMT_LOG" 2>&1; then
  pass "server fmt"
else
  fail "server fmt"
fi
if (
  export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo clippy --all-targets -- -D warnings
) >"$SERVER_CLIPPY_LOG" 2>&1; then
  pass "server clippy"
else
  fail "server clippy"
fi
if (
  export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo test
) >"$SERVER_TEST_LOG" 2>&1; then
  pass "server cargo test"
else
  fail "server cargo test"
fi

if (
  export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo test save_and_load_roundtrip_by_uuid -- --nocapture
  cargo test player::progression:: -- --nocapture
  cargo test payload_builder_zone_info_happy_path -- --nocapture
  cargo test payload_builder_event_alert_happy_path -- --nocapture
  cargo test payload_builder_player_state_happy_path -- --nocapture
  cargo test missing_target_route_player_state_does_not_broadcast_to_all_clients -- --nocapture
  cargo test player_state_periodic_emission_happens_without_component_change -- --nocapture
) >"$SERVER_PROOF_LOG" 2>&1; then
  pass "server proof tests"
else
  fail "server proof tests"
fi

if grep -Eq 'save_and_load_roundtrip_by_uuid' "$SERVER_PROOF_LOG"; then
  pass "persistence proof marker"
else
  fail "persistence proof marker"
fi
if grep -Eq 'player::progression::|progression_changes_are_visible_through_existing_projection_seam' "$SERVER_PROOF_LOG"; then
  pass "progression proof marker"
else
  fail "progression proof marker"
fi
if grep -Eq 'payload_builder_zone_info_happy_path' "$SERVER_PROOF_LOG"; then
  pass "zone_info proof marker"
else
  fail "zone_info proof marker"
fi
if grep -Eq 'payload_builder_event_alert_happy_path' "$SERVER_PROOF_LOG"; then
  pass "event_alert proof marker"
else
  fail "event_alert proof marker"
fi
if grep -Eq 'payload_builder_player_state_happy_path' "$SERVER_PROOF_LOG"; then
  pass "player_state proof marker"
else
  fail "player_state proof marker"
fi

echo ""
CURRENT_STAGE="client"
echo "=== [4/11] client -> test/build + typed payload anchors ==="
if (cd "$ROOT/client" && ./gradlew --no-daemon test) >"$CLIENT_TEST_LOG" 2>&1; then
  pass "client gradle test"
else
  fail "client gradle test"
fi
if (cd "$ROOT/client" && ./gradlew --no-daemon build) >"$CLIENT_BUILD_LOG" 2>&1; then
  pass "client gradle build"
else
  fail "client gradle build"
fi

FIXTURE_XML="$ROOT/client/build/test-results/test/TEST-com.bong.client.BongNetworkHandlerPayloadFixtureTest.xml"
NARRATION_XML="$ROOT/client/build/test-results/test/TEST-com.bong.client.NarrationStateTest.xml"

if [ -f "$FIXTURE_XML" ] && grep -Eq 'sharedNarrationFixtureParsesSuccessfully\(\)' "$FIXTURE_XML"; then
  pass "client sharedNarration fixture anchor"
else
  fail "client sharedNarration fixture anchor"
fi
if [ -f "$FIXTURE_XML" ] && grep -Eq 'sharedZoneInfoFixtureParsesSuccessfully\(\)' "$FIXTURE_XML"; then
  pass "client sharedZoneInfo fixture anchor"
else
  fail "client sharedZoneInfo fixture anchor"
fi
if [ -f "$FIXTURE_XML" ] && grep -Eq 'sharedEventAlertFixtureParsesSuccessfully\(\)' "$FIXTURE_XML"; then
  pass "client sharedEventAlert fixture anchor"
else
  fail "client sharedEventAlert fixture anchor"
fi
if [ -f "$FIXTURE_XML" ] && grep -Eq 'sharedPlayerStateFixtureParsesSuccessfully\(\)' "$FIXTURE_XML"; then
  pass "client sharedPlayerState fixture anchor"
else
  fail "client sharedPlayerState fixture anchor"
fi
if [ -f "$NARRATION_XML" ] && grep -Eq 'typedNarrationPayloadRoutesIntoNarrationState\(\)' "$NARRATION_XML"; then
  pass "client typed narration anchor"
else
  fail "client typed narration anchor"
fi

echo ""
CURRENT_STAGE="fullstack"
echo "=== [5/11] fullstack runtime -> redis + server + deterministic tiandao mock ==="
redis-server --save "" --appendonly no --loglevel warning >"$FULLSTACK_REDIS_LOG" 2>&1 &
REDIS_PID="$!"

if wait_for_redis_ping 20; then
  pass "redis ping"
else
  fail "redis ping"
fi

(
  export PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  cd "$ROOT/server"
  cargo run >"$FULLSTACK_SERVER_LOG" 2>&1
) &
SERVER_PID="$!"

if wait_for_pattern "$FULLSTACK_SERVER_LOG" "\[bong\]\[world\] creating overworld test area" 120; then
  pass "server world bootstrap"
else
  fail "server world bootstrap"
fi
if wait_for_pattern "$FULLSTACK_SERVER_LOG" "\[bong\]\[redis\] subscribed to bong:agent_command and bong:agent_narrate" 120; then
  pass "server redis bridge subscribed"
else
  fail "server redis bridge subscribed"
fi

timeout 180s redis-cli --raw SUBSCRIBE bong:world_state bong:agent_command bong:agent_narrate >"$FULLSTACK_REDIS_SUB_LOG" 2>&1 &
REDIS_SUB_PID="$!"
sleep 1

if wait_for_pattern "$FULLSTACK_REDIS_SUB_LOG" '"tick"[[:space:]]*:' 45; then
  pass "world_state proof"
else
  fail "world_state proof"
fi

(
  cd "$ROOT/agent/packages/tiandao"
  npx tsx <<'TS'
import { RedisIpc } from "./src/redis-ipc.ts";
import { createMockClient } from "./src/llm.ts";
import { runMockTickForTest } from "./src/main.ts";

const redis = new RedisIpc({ url: "redis://127.0.0.1:6379" });
await redis.connect();

const waitForState = async (timeoutMs: number) => {
  const latest = redis.getLatestState();
  if (latest) return latest;
  return await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`timed out waiting for world_state after ${timeoutMs}ms`)), timeoutMs);
    redis.onWorldState((state) => {
      clearTimeout(timer);
      resolve(state);
    });
  });
};

const worldState = await waitForState(45_000);
const llmResponse = JSON.stringify({
  commands: [
    {
      type: "spawn_event",
      target: "spawn",
      params: {
        event: "thunder_tribulation",
        intensity: 0.7,
        duration_ticks: 120,
      },
    },
    {
      type: "modify_zone",
      target: "spawn",
      params: {
        spirit_qi_delta: -0.05,
        danger_level_delta: 1,
      },
    },
    {
      type: "npc_behavior",
      target: "global",
      params: {
        flee_threshold: 0.65,
      },
    },
  ],
  narrations: [
    {
      scope: "broadcast",
      style: "system_warning",
      text: "天道测试叙事：雷劫将至。",
    },
  ],
  reasoning: "task-24 fullstack smoke deterministic mock",
});

const summary = await runMockTickForTest({
  llmClient: createMockClient(llmResponse),
  state: worldState,
  sink: redis,
  now: () => 1_000_000,
  model: "task-24-smoke-mock",
});

console.log("[smoke][tiandao] deterministic mock tick summary", summary);
await redis.disconnect();
TS
) >"$FULLSTACK_TIANDAO_LOG" 2>&1

if grep -Eq '\[tiandao\]\[arbiter\] merged commands:' "$FULLSTACK_TIANDAO_LOG"; then
  pass "merged command proof"
else
  fail "merged command proof"
fi
if grep -Eq 'tests/chat-drain\.test\.ts' "$AGENT_TEST_LOG"; then
  pass "chat drain proof (from tiandao tests)"
else
  fail "chat drain proof (from tiandao tests)"
fi
if wait_for_pattern "$FULLSTACK_SERVER_LOG" '\[bong\]\[network\] sent bong:server_data narration payload: 1 narrations,' 30; then
  pass "typed narration proof"
else
  fail "typed narration proof"
fi

echo ""
CURRENT_STAGE="summary"
echo "=== [6/11] Evidence paths (task-24 run layer) ==="
echo "  run_dir: $RUN_DIR"
echo "  schema: $SCHEMA_STAGE_LOG"
echo "  agent-check: $AGENT_CHECK_LOG"
echo "  agent-test: $AGENT_TEST_LOG"
echo "  agent-start-mock: $AGENT_START_MOCK_LOG"
echo "  server-fmt: $SERVER_FMT_LOG"
echo "  server-clippy: $SERVER_CLIPPY_LOG"
echo "  server-test: $SERVER_TEST_LOG"
echo "  server-proof: $SERVER_PROOF_LOG"
echo "  client-test: $CLIENT_TEST_LOG"
echo "  client-build: $CLIENT_BUILD_LOG"
echo "  fullstack-redis: $FULLSTACK_REDIS_LOG"
echo "  fullstack-server: $FULLSTACK_SERVER_LOG"
echo "  fullstack-redis-sub: $FULLSTACK_REDIS_SUB_LOG"
echo "  fullstack-tiandao: $FULLSTACK_TIANDAO_LOG"

echo ""
echo "=== [7/11] Consolidated smoke evidence files ==="
echo "  matrix stdout log: $SMOKE_LOG"
echo "  matrix failure log: $ERROR_LOG"

echo ""
echo "=== [8/11] Result ==="
echo "Result: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] && echo "ALL PASS" || echo "SOME FAILURES"

echo ""
echo "=== [9/11] Exit ==="
exit "$FAIL"
