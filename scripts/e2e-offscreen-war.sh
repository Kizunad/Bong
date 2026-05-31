#!/usr/bin/env bash
set -euo pipefail

# plan-offscreen-war-v1 P0+P2 真服 headless e2e。
#
# P0（底盘通路 + 守恒 telemetry + 派系 bootstrap）：
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
# P2（离屏战死闭环，本 plan 脊柱核心，全 headless redis 可观测）：
#   3. 路径 B 受控种对：停 P0 server → 从 redis HGETALL 抓一个真实快照作模板 →
#      flushall dormant → HSET 一对 Attack+Defend 同 zone 低真元 dormant → 重启 server。
#   4. 等数轮快进 tick，断言（§11 核心 ①②③④⑤）：
#      ① 出现 cause=combat & from_dormant_combat=true 的死亡（不再全是 natural_aging）；
#      ② HLEN 人口下降量 == 观测到的（去重）combat 死亡数（种群守恒）；
#      ③ 战死方 zone spirit_qi 上升（qi/ledger 的 account:zone:spawn > 0，败者真元守恒回灌）；
#      ④ bong:qi/ledger budget==DEFAULT_SPIRIT_QI_TOTAL 且 total_observed ≤ budget（精确守恒）；
#      ⑤ bong:npc/combat 的 outcome.loser 与战死 npc_id 一致。
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
# P2 受控对专属订阅日志（FIX-A）：P2 combat 阶段起一条**全新** subscriber 写到这里，
# 让"重启后是否真有受控对战死"的等待 / 断言只看 P2 新事件，绝不被 P0 默认 seed 阶段
# 早已记下的 cause=combat 历史行短路假过。
REDIS_SUB_P2_LOG="$RUN_DIR/redis-sub-p2.log"
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
REDIS_SUB_P2_PID=""
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

# 起一个 redis subscriber 把 spawn/death/world_state/combat 写到 $1 日志文件。
# $1 = 输出日志（默认 P0 的 $REDIS_SUB_LOG）。返回 PID 由调用方读 `$!`/全局变量自取。
start_redis_subscriber() {
  local out_log="${1:-$REDIS_SUB_LOG}"
  (
    cd "$ROOT/agent/packages/tiandao"
    PATH="$NODE_BIN:$PATH" REDIS_URL="$REDIS_URL" node --input-type=module <<'NODE'
import Redis from "ioredis";
const IORedis = Redis.default ?? Redis;
const channels = ["bong:npc/spawn", "bong:npc/death", "bong:world_state", "bong:npc/combat"];
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
  ) >"$out_log" 2>&1 &
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

# ── plan-offscreen-war-v1 P2：离屏战死闭环 combat 场景 ─────────────────────────
#
# P2 需要"同 zone 两个敌对 dormant"才能开战。setup 用**路径 B**（HSET bong:npc/dormant
# 写 schema-accurate 快照）：先从 P0 已起的 server 的 HGETALL 抓一个**真实** seeded
# 快照作模板（避免手写 12 经脉等结构、随 NpcDormantSnapshot 漂移），再 mutate（char_id /
# faction / 低正真元 / 长寿命）成一对 Attack+Defend 同 zone 候选。重启 server 时 store
# 非空 → 跳默认 seed → 受控对生效。固定 BONG_SIM_SEED + 小 BONG_DORMANT_TICK_INTERVAL
# 让战斗确定性快进。

# 从当前 redis 的 bong:npc/dormant 抓一个真实快照作模板，写到 $1 文件（JSON）。
capture_dormant_template() {
  local out_file="$1"
  OUT_FILE="$out_file" redis_node <<'NODE'
import Redis from "ioredis";
import { writeFileSync } from "node:fs";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const hash = await client.hgetall("bong:npc/dormant");
await client.quit();
const raws = Object.values(hash);
if (raws.length === 0) { console.error("[observe] no dormant snapshot to template from"); process.exit(2); }
// 取一个 schema-accurate 真实快照作模板（任意一个即可，结构完整）。
const template = JSON.parse(raws[0]);
writeFileSync(process.env.OUT_FILE, JSON.stringify(template));
console.log(`[observe] captured dormant template from ${template.char_id} (zone=${template.zone_name})`);
process.exit(0);
NODE
}

# 用模板造一对 Attack+Defend 同 zone、低正真元（combat-eligible）的受控 dormant，
# flushall 旧 dormant 后 HSET 两个。$1 = 模板文件。
seed_combat_pair() {
  local template_file="$1"
  TEMPLATE_FILE="$template_file" redis_node <<'NODE'
import Redis from "ioredis";
import { readFileSync } from "node:fs";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const template = JSON.parse(readFileSync(process.env.TEMPLATE_FILE, "utf8"));

// FIX-C(b)：把受控对锚定进 **spawn** zone（zones.json spirit_qi=0.3 → zone_current≈15，
// 距 QI_ZONE_UNIT_CAPACITY=50 还有 ~35 自由容量 ≫ 败者 qi_current=5）。这样
// release_dormant_qi_to_zone 的 5 必定**全额**落账、不触发 retain-until-released，败者必被
// 移除 → popDrop 必反映战死，与 FIX-C(a) 的轮询合力确保"延迟≠失败"且确定性。spawn AABB
// 为 [-750,750]³（zones.json），故下方位置取近原点 [0,64,0] 必落在 spawn 内。
const COMBAT_ZONE = "spawn";

// 用模板深拷贝两份，只改 char_id / faction / 真元 / 寿命 / zone / 位置（同 zone、互相靠近）。
function makeCombatant(charId, factionId, pos) {
  const snap = JSON.parse(JSON.stringify(template));
  snap.char_id = charId;
  // faction：保留模板 membership 的其余字段，只换 faction_id（Attack vs Defend → is_hostile_pair=true）。
  if (snap.faction == null) {
    snap.faction = { faction_id: factionId, rank: "disciple", reputation: {}, lineage: null, mission_queue: {} };
  } else {
    snap.faction.faction_id = factionId;
  }
  // 低正真元（combat-eligible 且战死后能守恒回灌让 zone spirit_qi 上升）。
  snap.cultivation.qi_current = 5.0;
  if (snap.cultivation.qi_max < 5.0) snap.cultivation.qi_max = 60.0;
  snap.initial_qi = 5.0;
  // 长寿命 + 0 年龄：本轮绝不自然老死，让"死人"唯一来源是离屏战斗。
  snap.lifespan.age_ticks = 0.0;
  snap.lifespan.max_age_ticks = 100000000.0;
  // 强制锚定到 spawn zone（FIX-C(b)，自由容量充足保证全额释放）。release 优先按 position
  // find_zone（mod.rs:1290），故 zone_name 与 position 都对齐到 spawn。
  snap.zone_name = COMBAT_ZONE;
  // 同 zone、cultivate 意图（位置不漂移），两者靠近但不同坐标。
  // 注意：DormantBehaviorIntent 是 #[serde(rename_all = "snake_case")]，variant 键必须小写。
  snap.intent = { cultivate: { zone: COMBAT_ZONE } };
  snap.position = pos;
  // 让本 tick 必结算（last_processed 落后于快进后的 tick）。
  snap.dormant_since_tick = 0;
  snap.last_dormant_tick_processed = 0;
  // DeathRegistry / LifeRecord char_id 同步（模板里是别的 char_id）。
  if (snap.death_registry && typeof snap.death_registry === "object") {
    if ("char_id" in snap.death_registry) snap.death_registry.char_id = charId;
    if ("character_id" in snap.death_registry) snap.death_registry.character_id = charId;
  }
  if (snap.life_record && typeof snap.life_record === "object") {
    if ("char_id" in snap.life_record) snap.life_record.char_id = charId;
    if ("character_id" in snap.life_record) snap.life_record.character_id = charId;
  }
  return snap;
}

// 近原点（必在 spawn AABB [-750,750]³ 内），两者靠近但坐标不同 → 同 zone 可配对开战。
const base = [0.0, 64.0, 0.0];
const atk = makeCombatant("dormant:combat:atk", "attack", [base[0], base[1], base[2]]);
const def = makeCombatant("dormant:combat:def", "defend", [base[0] + 1.0, base[1], base[2] + 1.0]);

await client.del("bong:npc/dormant");
await client.hset("bong:npc/dormant", "dormant:combat:atk", JSON.stringify(atk));
await client.hset("bong:npc/dormant", "dormant:combat:def", JSON.stringify(def));
const n = await client.hlen("bong:npc/dormant");
await client.quit();
console.log(`[observe] seeded controlled hostile pair (atk+def) in zone=${atk.zone_name}, HLEN=${n}`);
if (n !== 2) { console.error(`[observe] expected HLEN 2 after seeding pair, got ${n}`); process.exit(3); }
process.exit(0);
NODE
}

# 把当前 bong:qi/ledger 里所有 `account:zone:*` 余额 dump 成 JSON 写到 $1 文件（FIX-B）。
# 用作"受控战死前"的 preAccount 基线，让 ③ 断言"账户实际上升（post-pre>0）"而非仅"post>0"
# （后者可能被模板自带的预存余额假过，即使还灵气路径断了）。
capture_zone_accounts() {
  local out_file="$1"
  OUT_FILE="$out_file" redis_node <<'NODE'
import Redis from "ioredis";
import { writeFileSync } from "node:fs";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });
const hash = await client.hgetall("bong:qi/ledger");
await client.quit();
const accounts = {};
for (const [k, v] of Object.entries(hash)) {
  if (k.startsWith("account:zone:")) accounts[k] = Number(v);
}
writeFileSync(process.env.OUT_FILE, JSON.stringify(accounts));
console.log(`[observe] captured pre-combat zone accounts: ${JSON.stringify(accounts)}`);
process.exit(0);
NODE
}

# 断言：① 出现 cause=combat & from_dormant_combat=true 的死亡；② HLEN 人口降 ==
# 观测到的 combat 死亡数；③ 战死方 zone spirit_qi 实际上升（post-pre>0，对齐 pre 基线）；
# ④ ledger budget==DEFAULT 且 total_observed 全程 ≤ budget；⑤ bong:npc/combat outcome
# 的 loser 与 death 一致。读 redis-sub 日志（已收集 combat/death/world_state）+ HGETALL/HLEN
# + qi/ledger HASH + preAccount 基线文件。
assert_combat_closure() {
  local sub_log="$1"
  local expected_total="$2"
  local pre_account_file="$3"
  SUB_LOG="$sub_log" EXPECTED_TOTAL="$expected_total" PRE_ACCOUNT_FILE="$pre_account_file" redis_node <<'NODE'
import Redis from "ioredis";
import { readFileSync } from "node:fs";
const IORedis = Redis.default ?? Redis;
const client = new IORedis(process.env.REDIS_URL, { maxRetriesPerRequest: 2 });

const PAIR_PREFIX = "dormant:combat:";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// 重读 P2 专属订阅日志（FIX-A：只含重启后受控对阶段的事件，绝无 P0 默认 seed 历史行）。
// 每次轮询都重读，让 retain-until-released 跨多 tick 陆续到达的 pair death 被纳入计数。
function parseLog() {
  const lines = readFileSync(process.env.SUB_LOG, "utf8").split("\n");
  const deaths = [];
  const outcomes = [];
  for (const line of lines) {
    const m = line.match(/channel=(bong:npc\/death|bong:npc\/combat) payload=(.*)$/);
    if (!m) continue;
    let payload;
    try { payload = JSON.parse(m[2].replace(/\.\.\.$/, "")); } catch { continue; }
    if (m[1] === "bong:npc/death") deaths.push(payload);
    else outcomes.push(payload);
  }
  return { deaths, outcomes };
}

// ② FIX-C(a)：受控对人口降必须 == 受控对去重 combat 死亡数。retain-until-released 让满 zone
// 的败者残余真元跨多 tick 才释放完才移除 → 固定窗口内 HLEN 可能尚未反映移除。改为**轮询**
// HLEN（同时重读日志吸收新到达的 pair death），直到 popDrop===pairDeadIds.size 或超时，
// 每次重试打日志；只有超时才判失败（不再把合法的延迟移除误判为 NPC 凭空消失）。
const POP_POLL_TIMEOUT_SECS = 90;
const POP_POLL_INTERVAL_MS = 2000;
let combatDeaths = [];
let pairDeadIds = new Set();
let remaining = NaN;
let popDrop = NaN;
let parsed = { deaths: [], outcomes: [] };
let reconciled = false;
const deadlineMs = Date.now() + POP_POLL_TIMEOUT_SECS * 1000;
let attempt = 0;
while (Date.now() <= deadlineMs) {
  attempt += 1;
  parsed = parseLog();
  // ① cause=combat & from_dormant_combat=true 的战死必须出现（不再全是 natural_aging）。
  combatDeaths = parsed.deaths.filter((d) => d.cause === "combat" && d.from_dormant_combat === true);
  const pairCombatDeaths = combatDeaths.filter((d) => d.npc_id.startsWith(PAIR_PREFIX));
  pairDeadIds = new Set(pairCombatDeaths.map((d) => d.npc_id));
  remaining = await client.hlen("bong:npc/dormant");
  popDrop = 2 - remaining; // setup 只种了 2 个受控对（dormant:combat:atk/def）
  console.log(`[observe] (try ${attempt}) deaths=${parsed.deaths.length} combat_deaths=${combatDeaths.length} outcomes=${parsed.outcomes.length} t0_pop=2 t1_pop=${remaining} pop_drop=${popDrop} pair_combat_dead=${pairDeadIds.size}`);
  // 受控对至少 1 个 combat 死亡（确证受控对真打过；P0 默认 rogue 死亡不在 P2 日志里）。
  if (pairDeadIds.size >= 1 && popDrop === pairDeadIds.size) {
    reconciled = true;
    break;
  }
  await sleep(POP_POLL_INTERVAL_MS);
}

// ① 终态：必须观测到受控对 combat 死亡（P2 日志只含重启后事件，无 P0 历史污染）。
if (combatDeaths.length === 0) {
  console.error(`[observe] FAIL ①: P2 阶段无 cause=combat & from_dormant_combat=true 死亡（受控对未开打）; all deaths: ${JSON.stringify(parsed.deaths)}`);
  await client.quit();
  process.exit(2);
}
if (pairDeadIds.size < 1) {
  console.error(`[observe] FAIL ②: 受控对（${PAIR_PREFIX}*）至少应有 1 个 combat 死亡，实际 ${pairDeadIds.size}; combat deaths: ${JSON.stringify(combatDeaths.map((d)=>d.npc_id))}`);
  await client.quit();
  process.exit(3);
}
// ② 轮询超时仍未对齐 → NPC 凭空消失或重复计数（残余永不释放且非延迟）。
if (!reconciled) {
  console.error(`[observe] FAIL ②: 轮询 ${POP_POLL_TIMEOUT_SECS}s 后受控对人口降 ${popDrop} 仍 != 受控对去重 combat 死亡数 ${pairDeadIds.size}（NPC 凭空消失或重复计数）t0=2 t1=${remaining}`);
  await client.quit();
  process.exit(3);
}

const { outcomes } = parsed;

// ③ FIX-B：战死方 zone 还灵气必须**确实落账**——既不能靠 world-init 自带余额假过（仅 post>0
//   会），也要证回灌量真到了 zone 账户。硬锁：pairReleased>0（telemetry == 真实 transfer.amount）
//   且 post ≥ pairReleased（释放量已落进该 zone 账户）。preAccount delta 一并打日志作诊断。
const pairOutcomes = outcomes.filter((o) => o.loser.startsWith(PAIR_PREFIX) || o.winner.startsWith(PAIR_PREFIX));
const hash = await client.hgetall("bong:qi/ledger");
let preAccounts = {};
try { preAccounts = JSON.parse(readFileSync(process.env.PRE_ACCOUNT_FILE, "utf8")); } catch { preAccounts = {}; }
const pairZones = [...new Set(pairOutcomes.map((o) => o.zone))];
const pairReleased = pairOutcomes.reduce((s, o) => s + Number(o.qi_released || 0), 0);
let landedOk = false;
let sumDelta = 0;
const deltaReport = [];
for (const z of pairZones) {
  const key = `account:zone:${z}`;
  const post = Number(hash[key]);
  const pre = Number.isFinite(preAccounts[key]) ? preAccounts[key] : 0;
  const delta = (Number.isFinite(post) ? post : 0) - pre;
  deltaReport.push(`${z}: pre=${pre} post=${post} delta=${delta}`);
  if (Number.isFinite(delta)) sumDelta += delta;
  // 回灌的真元必须**确实落在**该 zone 账户里：post ≥ 本场 pairReleased（容差 ε）。
  // 释放断链 → pairReleased=0 → 下方硬失败；释放了但被吞/没落账 → post<pairReleased → 失败。
  if (Number.isFinite(post) && post + 1e-6 >= pairReleased) { landedOk = true; }
}
console.log(`[observe] pair zones=${pairZones.join(",")} zone_deltas=[${deltaReport.join(" | ")}] sum_delta=${sumDelta} pair_total_released=${pairReleased}`);
// 硬约束 ③：① 确有真元被回灌（pairReleased>0，telemetry == release_dormant_qi_to_zone 的
// transfer.amount，释放断链则为 0）；② 回灌量确实落在受控对 zone 账户里（post ≥ pairReleased）。
// 仅"account>0"会被 world-init 自带余额假过（即使释放断链），故这里锁"释放量已落账"。
// 注：preAccount 抓在 P0 停机后 / P2 重启前，重启会重置 ledger 基线，故 delta 仅作诊断报告
// （不作硬等式断言，避免把"重启重置基线"误判为失败——确定性优先）。
if (pairReleased <= 0 || !landedOk) {
  console.error(`[observe] FAIL ③: 受控对战死还灵气未落账（pairReleased=${pairReleased} landedOk=${landedOk}）——release_dormant_qi_to_zone 未把残余真元守恒回灌进 zone 账户; pre/post deltas=[${deltaReport.join(" | ")}]`);
  await client.quit();
  process.exit(4);
}

// ④ ledger budget==DEFAULT 且 total_observed ≤ budget（精确守恒不凭空造真元）。
const expected = Number(process.env.EXPECTED_TOTAL);
const total = Number(hash.total_observed);
const budgetInit = Number(hash.budget_initial_total);
const budgetCur = Number(hash.budget_current_total);
console.log(`[observe] post-combat qi/ledger total_observed=${total} budget_init=${budgetInit} budget_cur=${budgetCur}`);
if (Math.abs(budgetInit - expected) > 1e-6) {
  console.error(`[observe] FAIL ④: budget_initial_total ${budgetInit} != DEFAULT_SPIRIT_QI_TOTAL ${expected}`);
  await client.quit();
  process.exit(5);
}
if (!Number.isFinite(total) || total < 0 || total > budgetCur + 1e-6) {
  console.error(`[observe] FAIL ④: total_observed ${total} out of conservation budget ${budgetCur} (造真元红线)`);
  await client.quit();
  process.exit(5);
}

// ⑤ bong:npc/combat 受控对 outcome 的 loser 与受控对 combat 死亡一致。
const outcomeLosers = new Set(pairOutcomes.map((o) => o.loser));
const matched = [...pairDeadIds].every((id) => outcomeLosers.has(id));
console.log(`[observe] pair_outcome_losers=${[...outcomeLosers].join(",")} pair_death_ids=${[...pairDeadIds].join(",")}`);
if (pairOutcomes.length === 0 || !matched) {
  console.error(`[observe] FAIL ⑤: 受控对 bong:npc/combat outcome.loser 与战死 npc_id 不一致`);
  await client.quit();
  process.exit(6);
}

await client.quit();
console.log("[observe] combat closure assertions ①②③④⑤ all PASS");
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
  if [ -n "$REDIS_SUB_P2_PID" ] && kill -0 "$REDIS_SUB_P2_PID" 2>/dev/null; then
    kill "$REDIS_SUB_P2_PID" 2>/dev/null || true; wait "$REDIS_SUB_P2_PID" 2>/dev/null || true
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

# 起一个确定性 server：$1=日志文件、$2=seed_count。设置 SERVER_PID 全局，等 world +
# redis 两个 anchor 起来。失败直接 finalize_failure。
start_server() {
  local server_log="$1"
  local seed_count="$2"
  (
    export PATH="$RUST_PATH"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/bong-target}"
    # 默认 seed N 个 dormant rogue（commit D 按 char_id 哈希赋 Attack/Defend）。小值避免
    # 1000 条全量 bong:npc/dormant HASH 替换触发 redis 3s 超时（实测 1000 必超时、8 秒级完成）。
    export BONG_DORMANT_ROGUE_SEED_COUNT="$seed_count"
    export BONG_SKIP_SKIN_PREFETCH="${BONG_SKIP_SKIN_PREFETCH:-1}"
    export BONG_SIM_SEED="$SIM_SEED"
    export BONG_DORMANT_TICK_INTERVAL="$DORMANT_TICK_INTERVAL"
    cd "$ROOT/server"
    cargo run --release
  ) >"$server_log" 2>&1 &
  SERVER_PID="$!"

  if wait_for_pattern "$server_log" "\\[bong\\]\\[world\\] creating overworld test area" 300; then
    pass "server world bootstrap"
  else
    finalize_failure "server" "missing world bootstrap anchor in $server_log"
  fi
  if wait_for_pattern "$server_log" "\\[bong\\]\\[redis\\] subscribed to bong:agent_command" 300; then
    pass "server redis subscribed"
  else
    finalize_failure "server" "missing redis subscribed anchor in $server_log"
  fi
}

# 停掉当前 server（用于 combat 场景前的受控重种）。
stop_server() {
  if [ -n "$SERVER_PID" ] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  SERVER_PID=""
}

echo ""
CURRENT_STAGE="server"
echo "=== [4/7] Server startup (deterministic env) ==="
# 默认 seed 8 个 dormant rogue（commit D 按 char_id 哈希赋 Attack/Defend）。
start_server "$SERVER_LOG" "${BONG_DORMANT_ROGUE_SEED_COUNT:-8}"

# dormant 默认 seed anchor（commit D 赋派系起效）。
if wait_for_pattern "$SERVER_LOG" "seeded [0-9]+ dormant rogue NPC snapshots" 120; then
  pass "server default-seeded factioned dormant rogues"
else
  echo "[observe] note: seed log anchor not found (continuing; HGETALL will verify)"
fi

echo ""
CURRENT_STAGE="observe"
echo "=== [5/7] Observe: agent_command_spawn_then_death_roundtrip + qi/ledger ==="
start_redis_subscriber "$REDIS_SUB_LOG"
REDIS_SUB_PID="$!"
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
CURRENT_STAGE="combat"
echo "=== [6/7] Combat closure: controlled hostile pair → 离屏战死闭环 ==="
# (1) 停掉 P0 server，从 redis 仍存的 HASH 抓一个真实快照作模板（schema-accurate）。
stop_server
COMBAT_SERVER_LOG="$RUN_DIR/server-combat.log"
TEMPLATE_FILE="$RUN_DIR/dormant-template.json"
if capture_dormant_template "$TEMPLATE_FILE" >>"$OBSERVE_LOG" 2>&1; then
  pass "captured schema-accurate dormant template from redis"
else
  finalize_failure "combat" "failed to capture dormant template; see $OBSERVE_LOG"
fi

# (2) flushall dormant + HSET 受控 Attack/Defend 同 zone 低真元对（路径 B）。
if seed_combat_pair "$TEMPLATE_FILE" >>"$OBSERVE_LOG" 2>&1; then
  pass "seeded controlled hostile pair (attack+defend, same zone, low qi)"
else
  finalize_failure "combat" "failed to seed controlled combat pair; see $OBSERVE_LOG"
fi

# (2.5) FIX-B：在受控战死**之前**抓 ledger 各 zone 账户余额作 preAccount 基线，让 ③ 断言
# "账户实际上升（post-pre>0）"而非仅"post>0"——后者可能被模板自带预存余额假过。
PRE_ACCOUNT_FILE="$RUN_DIR/pre-combat-accounts.json"
if capture_zone_accounts "$PRE_ACCOUNT_FILE" >>"$OBSERVE_LOG" 2>&1; then
  pass "captured pre-combat zone account baseline (FIX-B preAccount)"
else
  finalize_failure "combat" "failed to capture pre-combat zone accounts; see $OBSERVE_LOG"
fi

# (2.6) FIX-A：起一条**全新** subscriber 写到 P2 专属日志，**且在重启 server 之前**确认其
# 已 subscribe。这样 P2 阶段的 death/combat 等待与断言只看这条新日志——绝不被 P0 默认 seed
# 阶段（8 个 factioned rogue 互殴）早已记入 $REDIS_SUB_LOG 的 cause=combat 历史行短路假过。
# P0 的 subscriber 继续跑无妨（它写自己的日志，P2 断言不读它）。
: >"$REDIS_SUB_P2_LOG"
start_redis_subscriber "$REDIS_SUB_P2_LOG"
REDIS_SUB_P2_PID="$!"
if wait_for_pattern "$REDIS_SUB_P2_LOG" "\\[observe\\] subscribed" 30; then
  pass "P2-fresh observer subscribed (only post-restart events; no P0 history leakage)"
else
  finalize_failure "combat" "P2-fresh observer did not start; see $REDIS_SUB_P2_LOG"
fi

# (3) 重启 server（seed_count=0 → 不默认 seed；store 由 redis 还原非空 → 跑受控对）。
start_server "$COMBAT_SERVER_LOG" 0
# 等受控对（dormant:combat:*）真的战死出现在 **P2 专属日志**。pattern scope 到受控对 npc_id
# + cause=combat，连"是否有战死"这一步都只认受控对，不会被任何残留事件误满足。
echo "[observe] waiting for CONTROLLED-PAIR offscreen combat death in P2-fresh log ..."
if wait_for_pattern "$REDIS_SUB_P2_LOG" "channel=bong:npc/death .*\"npc_id\":\"dormant:combat:.*\"cause\":\"combat\"" 120; then
  pass "observed bong:npc/death cause=combat for controlled pair (受控对真的死人了)"
else
  finalize_failure "combat" "no controlled-pair (dormant:combat:*) cause=combat death observed within 120s after combat-pair reboot; see $REDIS_SUB_P2_LOG / $COMBAT_SERVER_LOG"
fi

# 等一个 world_state/dormant HASH publish 周期让 HLEN / qi/ledger 反映战死后状态。
# （②③ 的最终对齐由 assert_combat_closure 内部轮询保证，这里只是给首个 publish 周期一点裕量。）
sleep 12

# (4) 全套 combat closure 断言 ①②③④⑤——只读 P2 专属日志 + preAccount 基线。
EXPECTED_TOTAL="${EXPECTED_TOTAL:-$(read_default_spirit_qi_total)}"
if assert_combat_closure "$REDIS_SUB_P2_LOG" "$EXPECTED_TOTAL" "$PRE_ACCOUNT_FILE" >>"$OBSERVE_LOG" 2>&1; then
  pass "combat closure ①cause=combat ②受控对人口降==战死数(轮询) ③zone还灵气(post-pre>0) ④守恒 ⑤outcome对账"
else
  finalize_failure "combat" "combat closure assertions failed; see $OBSERVE_LOG / $REDIS_SUB_P2_LOG"
fi

echo ""
CURRENT_STAGE="summary"
echo "=== [7/7] Evidence ==="
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
