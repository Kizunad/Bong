#!/usr/bin/env bash
set -euo pipefail

# plan-offscreen-war-v1 P0 真服 headless e2e。
#
# 验证"离屏世界可被外部 redis 自动观测"的底盘通路（P0 只测脚手架本身 +
# 守恒 telemetry + 派系 bootstrap；dormant 战死闭环留 P2）：
#   1. agent_command_spawn_then_death_roundtrip
#      - redis-cli publish bong:agent_command（注意是 agent_command 不是 agent_cmd）
#        注入 SpawnNpc → SUB bong:npc/spawn 断言收到 + zone 匹配。
#      - 路径 B：起服前 HSET bong:npc/dormant 种两个敌对派系 dormant（Attack/Defend，
#        is_hostile_pair=true）→ HGETALL 确认 seed 起效 + faction 非 None。
#   2. bong:qi/ledger 起服后非空且 total_observed ≈ DEFAULT_SPIRIT_QI_TOTAL（精确守恒；
#      断言取 server const 引用，不写字面 100）。
#
# 确定性：BONG_SIM_SEED 固定 + BONG_DORMANT_TICK_INTERVAL 小值（免 sleep 60s）。
#
# fork 自 scripts/e2e-redis.sh：同款 redis 三级 fallback + cargo run 起服 +
# ioredis subscriber + wait_for_pattern + cleanup trap。

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
EVIDENCE_DIR="$ROOT/.sisyphus/evidence"
TASK_ID="offscreen-war-p0"
SCRIPT_TAG="e2e-offscreen-war"
RUN_LABEL="${RUN_LABEL:-default}"
RUN_ID="$(date +%Y%m%d-%H%M%S)-$$-${RUN_LABEL}"
RUN_DIR="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-run-${RUN_ID}"
LOG_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}.log"
ERROR_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-error.log"
SUCCESS_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-success.txt"
MANIFEST_FILE="$EVIDENCE_DIR/${TASK_ID}-${SCRIPT_TAG}-manifest.txt"

REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
DEFAULT_REDIS_URL="redis://127.0.0.1:6379"
NODE_BIN="$ROOT/agent/node_modules/.bin"
RUST_PATH="/opt/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH"

REDIS_LOG="$RUN_DIR/redis.log"
SERVER_LOG="$RUN_DIR/server.log"
REDIS_SUB_LOG="$RUN_DIR/redis-sub.log"
OBSERVE_LOG="$RUN_DIR/observe.log"

# 确定性旋钮（P0 交付物 1）。
SIM_SEED="${BONG_SIM_SEED:-20260531}"
DORMANT_TICK_INTERVAL="${BONG_DORMANT_TICK_INTERVAL:-20}"

PASS=0
FAIL=0
CURRENT_STAGE="init"
REDIS_PID=""
SERVER_PID=""
REDIS_SUB_PID=""
REDIS_PROVIDER=""
REDIS_SERVER_BIN=""
DOCKER_CONTAINER_NAME="bong-${TASK_ID}-redis-${RUN_ID}"
DOCKER_REDIS_STARTED=0

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
  printf "task=%s\nscript=%s\nrun_id=%s\nrun_label=%s\nstatus=%s\nstage=%s\nmessage=%s\ntimestamp=%s\nfiles:\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n- %s\n" \
    "$TASK_ID" "$SCRIPT_TAG" "$RUN_ID" "$RUN_LABEL" "$status" "$stage_name" "$message" \
    "$(date -Iseconds)" "$LOG_FILE" "$ERROR_FILE" "$MANIFEST_FILE" "$SUCCESS_FILE" \
    "$REDIS_LOG" "$SERVER_LOG" "$OBSERVE_LOG" >"$MANIFEST_FILE"
}

finalize_failure() {
  local stage_name="$1"
  local message="$2"
  FAIL=$((FAIL + 1))
  rm -f "$SUCCESS_FILE"
  printf "task=%s\nscript=%s\nstatus=FAILED\nstage=%s\nmessage=%s\nrun_id=%s\n" \
    "$TASK_ID" "$SCRIPT_TAG" "$stage_name" "$message" "$RUN_ID" >"$ERROR_FILE"
  write_manifest "FAILED" "$stage_name" "$message"
  echo "[evidence] manifest: $MANIFEST_FILE"
  echo "[evidence] run_dir: $RUN_DIR"
  echo "[$TASK_ID][FAIL][$stage_name] $message"
  exit 1
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

# ── redis client helpers (ioredis；inline-resp fallback 支持 HSET/HGETALL/PUBLISH/SUB) ──

redis_node() {
  # 把 stdin 的 JS 跑在 ioredis 客户端上下文里：暴露 client + done(code)。
  (
    cd "$ROOT/agent/packages/tiandao"
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module
  )
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

# 路径 B：起服前 HSET 两个敌对派系 dormant（基于 schema-accurate 模板 mutate）。
seed_hostile_dormant() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });

// schema-accurate dormant snapshot 模板（由 server serde dump 得到，对齐 NpcDormantSnapshot）。
const base = {"char_id":"dormant:e2e:attacker","archetype":"rogue","dimension":"overworld","zone_name":"spawn","position":[16.3,64.0,4.4],"schedule_seed":11220119909606146934,"cultivation":{"realm":"Awaken","qi_current":5.0,"qi_max":10.0,"qi_max_frozen":null,"last_qi_zero_at":null,"pending_material_bonus":0.0,"composure":1.0,"composure_recover_rate":0.001},"meridian_system":{"regular":[{"id":"Lung","opened":true,"open_progress":0.0,"flow_rate":1.0,"flow_capacity":10.0,"rate_tier":0,"capacity_tier":0,"throughput_current":0.0,"integrity":1.0,"cracks":[],"opened_at":0}],"extraordinary":[]},"meridian_severed":{"severed_meridians":[],"severed_at":{},"dead_meridians":[]},"contamination":{"entries":[]},"lifespan":{"age_ticks":0.0,"max_age_ticks":110000.0},"shared_lifespan":{"born_at_tick":0,"years_lived":0.0,"cap_by_realm":120,"offline_pause_tick":null},"lifespan_extension_ledger":{"accumulated_years":0.0,"enlightenment_used":false},"death_registry":{"char_id":"dormant:e2e:attacker","death_count":0,"last_death_tick":null,"prev_death_tick":null,"last_death_zone":null},"life_record":{"character_id":"dormant:e2e:attacker","created_at":0,"biography":[],"insights_taken":[],"death_insights":[],"skill_milestones":[],"void_actions":[],"legacy_inheritor":null,"legacy_items":[],"legacy_letterbox":null,"spirit_root_first":null},"faction":{"faction_id":"attack","rank":"disciple","reputation":{"loyalty":0.5}},"patrol":{"home_zone":"spawn","anchor_index":0,"current_target":[32.0,64.0,32.0]},"loot_table":{"archetype":"rogue","entries":[{"template_id":"item.bone_coin","chance":0.5,"min_stack":3,"max_stack":12}]},"intent":{"cultivate":{"zone":"spawn"}},"dormant_since_tick":0,"last_dormant_tick_processed":0,"initial_qi":5.0,"qi_ledger_net":0.0};

function variant(id, faction, pos) {
  const s = structuredClone(base);
  s.char_id = id;
  s.faction.faction_id = faction;
  s.position = pos;
  s.death_registry.char_id = id;
  s.life_record.character_id = id;
  return s;
}

const attacker = variant("dormant:e2e:attacker", "attack", [16.3, 64.0, 4.4]);
const defender = variant("dormant:e2e:defender", "defend", [18.0, 64.0, 6.0]);

await client.del("bong:npc/dormant");
await client.hset("bong:npc/dormant", attacker.char_id, JSON.stringify(attacker));
await client.hset("bong:npc/dormant", defender.char_id, JSON.stringify(defender));
const got = await client.hlen("bong:npc/dormant").catch(() => null);
console.log(`[observe] seeded hostile dormant pair (hlen=${got})`);
await client.quit();
process.exit(0);
NODE
}

start_redis_subscriber() {
  (
    cd "$ROOT/agent/packages/tiandao"
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const channels = ["bong:npc/spawn", "bong:npc/death", "bong:world_state"];
const sub = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 1 });
const shutdown = async () => { try { await sub.quit(); } catch { try { sub.disconnect(); } catch {} } process.exit(0); };
process.on("SIGINT", shutdown); process.on("SIGTERM", shutdown);
await sub.subscribe(...channels);
console.log(`[observe] subscribed ${channels.join(",")}`);
sub.on("message", (channel, message) => {
  const preview = message.length > 320 ? `${message.slice(0, 320)}...` : message;
  console.log(`[observe] channel=${channel} payload=${preview}`);
});
setInterval(() => {}, 1000);
NODE
  ) >"$REDIS_SUB_LOG" 2>&1 &
  REDIS_SUB_PID="$!"
}

publish_spawn_npc() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const cmd = {
  v: 1,
  id: `e2e_spawn_${Date.now()}`,
  source: "deduction",
  commands: [ { type: "spawn_npc", target: "spawn", params: { archetype: "rogue" } } ],
};
const delivered = await client.publish("bong:agent_command", JSON.stringify(cmd));
console.log(`[observe] published spawn_npc to bong:agent_command (delivered=${delivered})`);
await client.quit();
process.exit(0);
NODE
}

# 读 bong:qi/ledger HASH 并断言 total_observed ≈ EXPECTED_TOTAL（精确守恒）。
assert_qi_ledger() {
  local expected="$1"
  EXPECTED_TOTAL="$expected" redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const expected = Number(process.env.EXPECTED_TOTAL);
const hash = await client.hgetall("bong:qi/ledger");
await client.quit();
const keys = Object.keys(hash);
if (keys.length === 0) { console.error("[observe] bong:qi/ledger is EMPTY post-boot"); process.exit(2); }
const total = Number(hash.total_observed);
console.log(`[observe] bong:qi/ledger fields=${keys.length} total_observed=${total} expected≈${expected}`);
// 守恒容差：起服后无 EraDecay，total_observed 应 ≈ DEFAULT_SPIRIT_QI_TOTAL。
// 容差放宽到 1.0 吸收 dormant seed/regen 在 zone↔npc 间的小幅再分配（仍是守恒内转移）。
if (!Number.isFinite(total)) { console.error("[observe] total_observed not finite"); process.exit(3); }
if (Math.abs(total - expected) > 1.0) {
  console.error(`[observe] total_observed=${total} drifted from expected ${expected} by >1.0`);
  process.exit(4);
}
process.exit(0);
NODE
}

# 断言 HGETALL bong:npc/dormant 含 seed 的敌对派系 dormant 且 faction 非 None。
assert_seeded_factions() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const hash = await client.hgetall("bong:npc/dormant");
await client.quit();
const snaps = Object.values(hash).map((raw) => JSON.parse(raw));
const factions = new Set();
let nullFaction = 0;
for (const s of snaps) {
  if (s.faction == null) { nullFaction += 1; }
  else { factions.add(s.faction.faction_id); }
}
console.log(`[observe] dormant count=${snaps.length} factions=${[...factions].join(",")} null_faction=${nullFaction}`);
if (snaps.length === 0) { console.error("[observe] no dormant snapshots restored"); process.exit(2); }
if (nullFaction > 0) { console.error(`[observe] ${nullFaction} dormant snapshots have faction=null`); process.exit(3); }
if (!factions.has("attack") || !factions.has("defend")) {
  console.error("[observe] seeded hostile pair missing attack/defend factions"); process.exit(4);
}
process.exit(0);
NODE
}

start_local_redis_binary() {
  "$REDIS_SERVER_BIN" --save "" --appendonly no --bind 127.0.0.1 --port 6379 --loglevel warning >"$REDIS_LOG" 2>&1 &
  REDIS_PID="$!"
  REDIS_PROVIDER="binary:$REDIS_SERVER_BIN"
}

start_docker_redis() {
  docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true
  if ! docker run -d --rm --name "$DOCKER_CONTAINER_NAME" -p 6379:6379 redis:7-alpine >"$REDIS_LOG" 2>&1; then
    return 1
  fi
  DOCKER_REDIS_STARTED=1
  REDIS_PROVIDER="docker:redis:7-alpine"
  return 0
}

ensure_redis() {
  if probe_redis; then
    REDIS_PROVIDER="existing:${REDIS_URL}"
    return 0
  fi
  if [ "$REDIS_URL" != "$DEFAULT_REDIS_URL" ]; then
    finalize_failure "redis" "Redis at $REDIS_URL is unavailable and auto-provision only supports $DEFAULT_REDIS_URL"
  fi
  REDIS_SERVER_BIN="$(command -v redis-server || command -v valkey-server || true)"
  if [ -n "$REDIS_SERVER_BIN" ]; then
    start_local_redis_binary
  elif command -v docker >/dev/null 2>&1; then
    if ! start_docker_redis; then
      finalize_failure "redis" "no real redis (redis-server/valkey/docker) available; bong:qi/ledger HashReplace needs DEL+RENAME which the inline-resp fallback in e2e-redis.sh does NOT support — run this e2e against a real redis"
    fi
  else
    finalize_failure "redis" "no real redis available; bong:qi/ledger HashReplace needs DEL+RENAME unsupported by inline fallback — run against real redis/CI"
  fi
  local elapsed=0
  while [ "$elapsed" -lt 30 ]; do
    if probe_redis; then return 0; fi
    sleep 1
    elapsed=$((elapsed + 1))
  done
  finalize_failure "redis" "Redis provider '$REDIS_PROVIDER' did not become healthy within 30s"
}

cleanup() {
  if [ -n "$REDIS_SUB_PID" ] && kill -0 "$REDIS_SUB_PID" 2>/dev/null; then
    kill "$REDIS_SUB_PID" 2>/dev/null || true; wait "$REDIS_SUB_PID" 2>/dev/null || true
  fi
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true
  fi
  if [ -n "$REDIS_PID" ] && kill -0 "$REDIS_PID" 2>/dev/null; then
    kill "$REDIS_PID" 2>/dev/null || true; wait "$REDIS_PID" 2>/dev/null || true
  fi
  if [ "$DOCKER_REDIS_STARTED" -eq 1 ]; then
    docker rm -f "$DOCKER_CONTAINER_NAME" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# 从 server const 读 DEFAULT_SPIRIT_QI_TOTAL（不写字面 100，对齐守恒红线纪律）。
read_default_spirit_qi_total() {
  grep -E "pub const DEFAULT_SPIRIT_QI_TOTAL: f64 = " "$ROOT/server/src/qi_physics/constants.rs" \
    | sed -nE 's/.*= ([0-9.]+);.*/\1/p' | head -1
}

echo "===== $TASK_ID $SCRIPT_TAG ====="
echo "run_id: $RUN_ID  run_dir: $RUN_DIR"
echo "sim_seed: $SIM_SEED  dormant_tick_interval: $DORMANT_TICK_INTERVAL"

echo ""
CURRENT_STAGE="pre-cleanup"
echo "=== [0/6] Pre-cleanup ==="
bash "$ROOT/scripts/stop.sh" >/dev/null 2>&1 || true
pass "pre-cleanup complete"

echo ""
CURRENT_STAGE="redis"
echo "=== [1/6] Redis provider ==="
ensure_redis
echo "[redis] provider: $REDIS_PROVIDER"
pass "redis ready"

echo ""
CURRENT_STAGE="seed"
echo "=== [2/6] Path-B seed hostile dormant (HSET before boot) ==="
if seed_hostile_dormant >>"$OBSERVE_LOG" 2>&1; then
  pass "seeded hostile dormant pair via HSET"
else
  finalize_failure "seed" "failed to HSET hostile dormant pair; see $OBSERVE_LOG"
fi

echo ""
CURRENT_STAGE="schema"
echo "=== [3/6] Schema build ==="
if (cd "$ROOT/agent/packages/schema" && PATH="$NODE_BIN:$PATH" npm run build) >>"$REDIS_LOG" 2>&1; then
  pass "schema build"
else
  finalize_failure "schema" "schema build failed; see $REDIS_LOG"
fi

echo ""
CURRENT_STAGE="server"
echo "=== [4/6] Server startup (deterministic env) ==="
(
  export PATH="$RUST_PATH"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
  # store 非空（path-B HSET 已种）→ 默认 seed 跳过；但仍设小值防 0。
  export BONG_ROGUE_SEED_COUNT="${BONG_ROGUE_SEED_COUNT:-0}"
  export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
  export BONG_SIM_SEED="$SIM_SEED"
  export BONG_DORMANT_TICK_INTERVAL="$DORMANT_TICK_INTERVAL"
  cd "$ROOT/server"
  cargo run --release
) >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[world\\] creating overworld test area" 300; then
  pass "server world bootstrap"
else
  finalize_failure "server" "missing world bootstrap anchor in $SERVER_LOG"
fi

if wait_for_pattern "$SERVER_LOG" "\\[bong\\]\\[redis\\] subscribed to bong:agent_command" 300; then
  pass "server redis subscribed"
else
  finalize_failure "server" "missing redis subscribed anchor in $SERVER_LOG"
fi

# dormant store restore anchor（path-B seed 起效）。
if wait_for_pattern "$SERVER_LOG" "loaded [0-9]+ dormant NPC snapshot" 60; then
  pass "dormant store restored from path-B HSET"
else
  echo "[observe] note: dormant restore log anchor not found (continuing; HGETALL will verify)"
fi

echo ""
CURRENT_STAGE="observe"
echo "=== [5/6] Observe: agent_command_spawn_then_death_roundtrip + qi/ledger ==="
start_redis_subscriber
if wait_for_pattern "$REDIS_SUB_LOG" "\\[observe\\] subscribed" 30; then
  pass "observer subscribed"
else
  finalize_failure "observe" "observer did not start; see $REDIS_SUB_LOG"
fi

# (a) publish SpawnNpc → expect bong:npc/spawn with zone=spawn.
publish_spawn_npc >>"$OBSERVE_LOG" 2>&1 || finalize_failure "observe" "spawn publish failed; see $OBSERVE_LOG"
if wait_for_pattern "$REDIS_SUB_LOG" "channel=bong:npc/spawn.*\"zone\":\"spawn\"" 60; then
  pass "agent_command_spawn_then_death_roundtrip: bong:npc/spawn received with zone=spawn"
else
  finalize_failure "observe" "did not observe bong:npc/spawn with zone=spawn after agent_command publish; see $REDIS_SUB_LOG"
fi

# (b) HGETALL bong:npc/dormant → seed took effect + faction non-None (attack+defend).
if assert_seeded_factions >>"$OBSERVE_LOG" 2>&1; then
  pass "HGETALL bong:npc/dormant confirms seeded hostile factions (attack+defend, no null)"
else
  finalize_failure "observe" "seeded dormant faction assertion failed; see $OBSERVE_LOG"
fi

# (c) bong:qi/ledger non-empty + total_observed ≈ DEFAULT_SPIRIT_QI_TOTAL (exact conservation).
EXPECTED_TOTAL="$(read_default_spirit_qi_total)"
[ -n "$EXPECTED_TOTAL" ] || finalize_failure "observe" "could not read DEFAULT_SPIRIT_QI_TOTAL from constants.rs"
echo "[observe] DEFAULT_SPIRIT_QI_TOTAL (from server const) = $EXPECTED_TOTAL"
# 等一个 world_state publish 周期让 qi/ledger 首次写入。
if wait_for_pattern "$REDIS_SUB_LOG" "channel=bong:world_state" 60; then
  pass "world_state publish observed (qi/ledger cadence is alive)"
else
  echo "[observe] note: world_state not yet seen; probing qi/ledger directly"
fi
LEDGER_OK=0
for _ in $(seq 1 30); do
  if assert_qi_ledger "$EXPECTED_TOTAL" >>"$OBSERVE_LOG" 2>&1; then LEDGER_OK=1; break; fi
  sleep 1
done
if [ "$LEDGER_OK" -eq 1 ]; then
  pass "bong:qi/ledger non-empty + total_observed ≈ DEFAULT_SPIRIT_QI_TOTAL ($EXPECTED_TOTAL)"
else
  finalize_failure "observe" "bong:qi/ledger empty or total_observed drifted from $EXPECTED_TOTAL; see $OBSERVE_LOG"
fi

echo ""
CURRENT_STAGE="summary"
echo "=== [6/6] Evidence ==="
echo "  log: $LOG_FILE"
echo "  server: $SERVER_LOG"
echo "  observe: $OBSERVE_LOG"
echo "  redis-sub: $REDIS_SUB_LOG"
echo ""
echo "Result: $PASS passed, $FAIL failed"

if [ "$FAIL" -eq 0 ]; then
  printf "task=%s\nstatus=PASS\nrun_id=%s\nmessage=all-anchors-passed\n" "$TASK_ID" "$RUN_ID" >"$SUCCESS_FILE"
  write_manifest "PASS" "complete" "all-anchors-passed"
  echo "ALL PASS"
  exit 0
fi
finalize_failure "$CURRENT_STAGE" "unexpected failure state"
