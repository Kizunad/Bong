#!/usr/bin/env bash
set -euo pipefail

# plan-offscreen-war-v1 P0 轻量 smoke：e2e-offscreen-war.sh 的子集。
#
# 只验最小可观测面：起服后
#   1. bong:qi/ledger HASH key 存在（守恒 telemetry 通路活着）。
#   2. HGETALL bong:npc/dormant 里 seeded dormant 的 faction 字段非 None。
#
# 不测 spawn roundtrip / 精确守恒数值（那些归 e2e-offscreen-war.sh）。
# fork 自 scripts/e2e-redis.sh 的 redis fallback + cargo run + cleanup trap。

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"
TASK_ID="offscreen-war-p0"
SCRIPT_TAG="smoke-offscreen-war"
RUN_LABEL="${RUN_LABEL:-default}"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$-${RUN_LABEL}"
RUN_DIR="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-run-${RUN_ID}"
LOG_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}.log"
ERROR_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-error.log"

REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
DEFAULT_REDIS_URL="redis://127.0.0.1:6379"
NODE_BIN="$ROOT/agent/node_modules/.bin"
RUST_PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"

REDIS_LOG="$RUN_DIR/redis.log"
SERVER_LOG="$RUN_DIR/server.log"
OBSERVE_LOG="$RUN_DIR/observe.log"

SIM_SEED="${BONG_SIM_SEED:-20260531}"
DORMANT_TICK_INTERVAL="${BONG_DORMANT_TICK_INTERVAL:-20}"

PASS=0
FAIL=0
CURRENT_STAGE="init"
REDIS_PID=""
SERVER_PID=""
REDIS_PROVIDER=""
REDIS_SERVER_BIN=""
DOCKER_CONTAINER_NAME="bong-${TASK_ID}-smoke-redis-${RUN_ID}"
DOCKER_REDIS_STARTED=0

mkdir -p "$EVIDENCE_DIR" "$RUN_DIR"
touch "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1

pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }

finalize_failure() {
  local stage_name="$1"; local message="$2"
  FAIL=$((FAIL + 1))
  printf "task=%s\nscript=%s\nstatus=FAILED\nstage=%s\nmessage=%s\nrun_id=%s\n" \
    "$TASK_ID" "$SCRIPT_TAG" "$stage_name" "$message" "$RUN_ID" >"$ERROR_FILE"
  echo "[$TASK_ID][FAIL][$stage_name] $message"
  echo "[evidence] run_dir: $RUN_DIR"
  exit 1
}

wait_for_pattern() {
  local file="$1"; local pattern="$2"; local timeout_secs="$3"; local elapsed=0
  while [ "$elapsed" -lt "$timeout_secs" ]; do
    if [ -f "$file" ] && grep -Eq "$pattern" "$file"; then return 0; fi
    sleep 1; elapsed=$((elapsed + 1))
  done
  return 1
}

redis_node() {
  ( cd "$ROOT/agent/packages/tiandao"; PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module )
}

probe_redis() {
  redis_node <<'NODE' >/dev/null 2>&1
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { lazyConnect: true, maxRetriesPerRequest: 1, enableOfflineQueue: false });
try { await client.connect(); const pong = await client.ping(); await client.quit(); process.exit(pong === "PONG" ? 0 : 1); }
catch { try { client.disconnect(); } catch {} process.exit(1); }
NODE
}

seed_one_dormant() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const snap = {"char_id":"dormant:smoke:1","archetype":"rogue","dimension":"overworld","zone_name":"spawn","position":[16.3,64.0,4.4],"schedule_seed":11220119909606146934,"cultivation":{"realm":"Awaken","qi_current":5.0,"qi_max":10.0,"qi_max_frozen":null,"last_qi_zero_at":null,"pending_material_bonus":0.0,"composure":1.0,"composure_recover_rate":0.001},"meridian_system":{"regular":[{"id":"Lung","opened":true,"open_progress":0.0,"flow_rate":1.0,"flow_capacity":10.0,"rate_tier":0,"capacity_tier":0,"throughput_current":0.0,"integrity":1.0,"cracks":[],"opened_at":0}],"extraordinary":[]},"meridian_severed":{"severed_meridians":[],"severed_at":{},"dead_meridians":[]},"contamination":{"entries":[]},"lifespan":{"age_ticks":0.0,"max_age_ticks":110000.0},"shared_lifespan":{"born_at_tick":0,"years_lived":0.0,"cap_by_realm":120,"offline_pause_tick":null},"lifespan_extension_ledger":{"accumulated_years":0.0,"enlightenment_used":false},"death_registry":{"char_id":"dormant:smoke:1","death_count":0,"last_death_tick":null,"prev_death_tick":null,"last_death_zone":null},"life_record":{"character_id":"dormant:smoke:1","created_at":0,"biography":[],"insights_taken":[],"death_insights":[],"skill_milestones":[],"void_actions":[],"legacy_inheritor":null,"legacy_items":[],"legacy_letterbox":null,"spirit_root_first":null},"faction":{"faction_id":"attack","rank":"disciple","reputation":{"loyalty":0.5}},"patrol":{"home_zone":"spawn","anchor_index":0,"current_target":[32.0,64.0,32.0]},"loot_table":{"archetype":"rogue","entries":[{"template_id":"item.bone_coin","chance":0.5,"min_stack":3,"max_stack":12}]},"intent":{"cultivate":{"zone":"spawn"}},"dormant_since_tick":0,"last_dormant_tick_processed":0,"initial_qi":5.0,"qi_ledger_net":0.0};
await client.del("bong:npc/dormant");
await client.hset("bong:npc/dormant", snap.char_id, JSON.stringify(snap));
await client.quit();
process.exit(0);
NODE
}

assert_ledger_and_faction() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const ledger = await client.hgetall("bong:qi/ledger");
const dormant = await client.hgetall("bong:npc/dormant");
await client.quit();
if (Object.keys(ledger).length === 0) { console.error("[smoke] bong:qi/ledger key absent/empty"); process.exit(2); }
const snaps = Object.values(dormant).map((r) => JSON.parse(r));
if (snaps.length === 0) { console.error("[smoke] no dormant snapshots"); process.exit(3); }
const allFactioned = snaps.every((s) => s.faction != null && typeof s.faction.faction_id === "string");
if (!allFactioned) { console.error("[smoke] some dormant snapshot has faction=null"); process.exit(4); }
console.log(`[smoke] ledger_fields=${Object.keys(ledger).length} dormant=${snaps.length} all_factioned=true`);
process.exit(0);
NODE
}

start_local_redis_binary() {
  "$REDIS_SERVER_BIN" --save "" --appendonly no --bind 127.0.0.1 --port 6379 --loglevel warning >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"; REDIS_PROVIDER="binary:$REDIS_SERVER_BIN"
}

start_docker_redis() {
  docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true
  docker run -d --rm --name "$DOCKER_CONTAINER_NAME" -p 6379:6379 redis:7-alpine >"$REDIS_LOG" 2>&1 || return 1
  DOCKER_REDIS_STARTED=1; REDIS_PROVIDER="docker:redis:7-alpine"; return 0
}

ensure_redis() {
  if probe_redis; then REDIS_PROVIDER="existing:${REDIS_URL}"; return 0; fi
  if [ "$REDIS_URL" != "$DEFAULT_REDIS_URL" ]; then
    finalize_failure "redis" "Redis at $REDIS_URL unavailable; auto-provision only supports $DEFAULT_REDIS_URL"
  fi
  REDIS_SERVER_BIN="$(command -v redis-server || command -v valkey-server || true)"
  if [ -n "$REDIS_SERVER_BIN" ]; then start_local_redis_binary
  elif command -v docker >/dev/null 2>&1; then
    start_docker_redis || finalize_failure "redis" "no real redis available; bong:qi/ledger needs DEL+RENAME unsupported by inline fallback"
  else
    finalize_failure "redis" "no real redis available; bong:qi/ledger needs DEL+RENAME unsupported by inline fallback"
  fi
  local elapsed=0
  while [ "$elapsed" -lt 30 ]; do
    if probe_redis; then return 0; fi
    sleep 1; elapsed=$((elapsed + 1))
  done
  finalize_failure "redis" "Redis provider '$REDIS_PROVIDER' not healthy within 30s"
}

cleanup() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi
  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then kill "$REDIS_PID" 2>/dev/null || true; wait "$REDIS_PID" 2>/dev/null || true; fi
  if [ "$DOCKER_REDIS_STARTED" -eq 1 ]; then docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true; fi
}
trap cleanup EXIT

echo "===== $TASK_ID $SCRIPT_TAG ====="
echo "run_id: $RUN_ID  run_dir: $RUN_DIR"

CURRENT_STAGE="pre-cleanup"
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup complete"

CURRENT_STAGE="redis"
ensure_redis
echo "[redis] provider: $REDIS_PROVIDER"
pass "redis ready"

CURRENT_STAGE="seed"
seed_one_dormant >>"$OBSERVE_LOG" 2>&1 || finalize_failure "seed" "HSET dormant failed; see $OBSERVE_LOG"
pass "seeded one factioned dormant via HSET"

CURRENT_STAGE="server"
(
  export PATH="$RUST_PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
  export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
  export BONG_SIM_SEED="$SIM_SEED"
  export BONG_DORMANT_TICK_INTERVAL="$DORMANT_TICK_INTERVAL"
  cd "$ROOT/server"
  cargo run --release
) >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[redis\\] subscribed to bong:agent_command" 300; then
  pass "server up + redis subscribed"
else
  finalize_failure "server" "server did not reach redis-subscribed anchor; see $SERVER_LOG"
fi

CURRENT_STAGE="observe"
# 等首个 world_state 周期写 bong:qi/ledger。
wait_for_pattern "$SERVER_LOG" "syncing .* dormant NPC snapshots to Redis HASH|\\[bong\\]\\[network\\]" 60 || true
OK=0
for _ in $(seq 1 30); do
  if assert_ledger_and_faction >>"$OBSERVE_LOG" 2>&1; then OK=1; break; fi
  sleep 1
done
if [ "$OK" -eq 1 ]; then
  pass "bong:qi/ledger present + seeded dormant faction non-None"
else
  finalize_failure "observe" "ledger key missing or dormant faction null; see $OBSERVE_LOG"
fi

echo ""
echo "Result: $PASS passed, $FAIL failed"
echo "  run_dir: $RUN_DIR"
[ "$FAIL" -eq 0 ] && { echo "ALL PASS"; exit 0; }
finalize_failure "$CURRENT_STAGE" "unexpected failure state"
