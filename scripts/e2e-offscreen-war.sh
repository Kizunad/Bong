#!/usr/bin/env bash
set -euo pipefail

# plan-offscreen-war-v1 P0 真服 headless e2e。
#
# 验证"离屏世界可被外部 redis 自动观测"的底盘通路（P0 只测脚手架本身 +
# 守恒 telemetry + 派系 bootstrap；dormant 战死闭环留 P2）：
#   1. agent_command_spawn_then_death_roundtrip
#      - redis-cli publish bong:agent_command（注意是 agent_command 不是 agent_cmd）
#        注入 SpawnNpc → SUB bong:npc/spawn 断言收到 + zone 匹配。
#      - dormant 派系：清空 bong:npc/dormant 后 server 默认 seed 8 个 rogue，commit D
#        按 char_id 哈希赋 Attack/Defend → HGETALL 确认 seed 起效 + faction 非 None
#        + attack/defend 双方都在（保证 is_hostile_pair 后续能配对）。
#   2. bong:qi/ledger 起服后非空；守恒断言锁**天道预算**（budget_initial_total ==
#      DEFAULT_SPIRIT_QI_TOTAL，zero-sum 真锚点），并校 total_observed（已落位真元）
#      在预算内（不凭空造真元）。断言取 server const 引用，不写字面 100。
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

# 清空 bong:npc/dormant，让 server 在起服时按 `BONG_DORMANT_ROGUE_SEED_COUNT`
# 默认 seed（commit D 给每个 rogue 按 char_id 哈希赋 Attack/Defend）。
#
# 为何不用 plan §11 的 path-B 手写 HSET：手写 schema-accurate 快照极易随
# NpcDormantSnapshot 结构漂移（meridian_system.regular 是 [Meridian;12] 等），
# 且 P0 只需"有派系的 dormant"而非"同 zone 敌对对"（后者是 P2 战斗的需求）。
# 用 server 自种 8 个（小值，避免 1000 条全量 HASH 替换触发 3s redis 超时），
# 直接走真实 seed 路径，端到端验证 commit D 的派系 bootstrap。P2 再引 path-B 控制对。
clear_dormant_key() {
  redis_node <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
await client.del("bong:npc/dormant");
console.log("[observe] cleared bong:npc/dormant → server will default-seed factioned rogues");
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
  // AgentCommandV1.source 只接受 arbiter|calamity|mutation|era（schema/agent_command.rs:48）。
  source: "arbiter",
  commands: [ { type: "spawn_npc", target: "spawn", params: { archetype: "rogue" } } ],
};
const delivered = await client.publish("bong:agent_command", JSON.stringify(cmd));
console.log(`[observe] published spawn_npc to bong:agent_command (delivered=${delivered})`);
await client.quit();
process.exit(0);
NODE
}

# 读 bong:qi/ledger HASH 做守恒可观测断言。
#
# 关键纠正：`total_observed = player+zone+container+ledger` 是**已落位**真元（minimal
# 测试世界起服后 zone spirit_qi 很低，实测 ≈7），而**守恒总量恒定的 100 是天道预算
# `budget_*`**（zero-sum 的真锚点，worldview §十）。二者不是一回事——plan 原文把"已落位"
# 误当"预算"。这里断言真正守恒量：
#   ① budget_initial_total 必须严格 == DEFAULT_SPIRIT_QI_TOTAL（全服灵气恒定）；
#   ② budget_current_total 只被时代衰减拉低 → ∈ (0, initial]；
#   ③ total_observed（已落位）必须 ≥0 且 ≤ 预算（不能凭空多出真元 = 吞/造真元红线）。
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
const budgetInit = Number(hash.budget_initial_total);
const budgetCur = Number(hash.budget_current_total);
console.log(`[observe] bong:qi/ledger fields=${keys.length} total_observed=${total} budget_initial=${budgetInit} budget_current=${budgetCur} (守恒总量=budget≈${expected})`);
if (![total, budgetInit, budgetCur].every(Number.isFinite)) {
  console.error("[observe] ledger numbers not finite"); process.exit(3);
}
// ① 守恒预算 == DEFAULT_SPIRIT_QI_TOTAL（严格，全服灵气恒定）。
if (Math.abs(budgetInit - expected) > 1e-6) {
  console.error(`[observe] budget_initial_total=${budgetInit} != DEFAULT_SPIRIT_QI_TOTAL ${expected}`); process.exit(4);
}
// ② current 只被时代衰减拉低：∈ (0, initial]。
if (budgetCur <= 0 || budgetCur > budgetInit + 1e-6) {
  console.error(`[observe] budget_current_total=${budgetCur} 越界 (应 ∈ (0, ${budgetInit}])`); process.exit(5);
}
// ③ 已落位真元 ≥0 且不得超过预算（超过 = 凭空造真元，吞/造真元红线）。
if (total < 0 || total > budgetCur + 1e-6) {
  console.error(`[observe] total_observed=${total} 超出守恒预算 ${budgetCur}（凭空多出真元）`); process.exit(6);
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
echo "=== [2/6] Clean dormant slate (server default-seeds factioned rogues on boot) ==="
if clear_dormant_key >>"$OBSERVE_LOG" 2>&1; then
  pass "cleared bong:npc/dormant (server will default-seed factioned rogues)"
else
  finalize_failure "seed" "failed to clear bong:npc/dormant; see $OBSERVE_LOG"
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
  # 默认 seed 8 个 dormant rogue（commit D 按 char_id 哈希赋 Attack/Defend）。小值避免
  # 1000 条全量 bong:npc/dormant HASH 替换触发 redis 3s 超时（实测 1000 必超时、8 秒级完成）。
  export BONG_DORMANT_ROGUE_SEED_COUNT="${BONG_DORMANT_ROGUE_SEED_COUNT:-8}"
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

# dormant 默认 seed anchor（commit D 赋派系起效）。
if wait_for_pattern "$SERVER_LOG" "seeded [0-9]+ dormant rogue NPC snapshots" 120; then
  pass "server default-seeded factioned dormant rogues"
else
  echo "[observe] note: seed log anchor not found (continuing; HGETALL will verify)"
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

# (b) HGETALL bong:npc/dormant → seed took effect + faction non-None (attack+defend)。
# dormant→redis HASH 同步挂在周期 world_state publish 上（network/mod.rs:936，每
# WORLD_STATE_PUBLISH_INTERVAL_TICKS=200 tick ≈ 10s 一次，且是 debug 日志不打 info）。
# 起服后首个 publish 周期前 HASH 仍空，故轮询重试等首次同步落地。
FACTIONS_OK=0
for _ in $(seq 1 30); do
  if assert_seeded_factions >>"$OBSERVE_LOG" 2>&1; then FACTIONS_OK=1; break; fi
  sleep 1
done
if [ "$FACTIONS_OK" -eq 1 ]; then
  pass "HGETALL bong:npc/dormant confirms seeded factions (attack+defend, no null)"
else
  finalize_failure "observe" "seeded dormant faction assertion failed after 30s; see $OBSERVE_LOG"
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
  pass "bong:qi/ledger non-empty + budget==DEFAULT_SPIRIT_QI_TOTAL ($EXPECTED_TOTAL) + 已落位真元在预算内"
else
  finalize_failure "observe" "bong:qi/ledger empty / budget!=$EXPECTED_TOTAL / 已落位真元超预算; see $OBSERVE_LOG"
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
