//! NPC dormant data plane.
//!
//! v1 keeps a deliberately small two-state model: live ECS entities stay
//! hydrated, far NPCs move into this resource and are advanced in batches.

/// plan-offscreen-war-v1 P1：离屏 dormant 战斗的纯逻辑核心（配对 + 胜负 roll）。
///
/// 全部纯函数（只接 `&` 入参、返回 owned 数据），零 store mutation / 零 ledger /
/// 零真元流动——把"谁打谁、谁赢"从结算解耦出来，可被饱和单测完全锁住。真正的
/// 战死结算（release qi / emit death / 人口回写）在 P2 接进 `dormant_global_tick_system`。
pub mod combat;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, DVec3, Event, EventWriter, Res, ResMut, Resource, Startup, Update,
};

use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, next_realm, try_breakthrough, BreakthroughError, BreakthroughSuccess,
    RollSource, XorshiftRoll, MIN_ZONE_QI_TO_BREAKTHROUGH, MIN_ZONE_QI_TO_GUYUAN,
};
use crate::cultivation::components::{
    Contamination, Cultivation, MeridianId, MeridianSystem, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::npc::faction::{
    FactionId, FactionMembership, FactionRank, FactionStore, MissionQueue, Reputation,
};
use crate::npc::lifecycle::{NpcArchetype, NpcDeathNotice, NpcDeathReason, NpcLifespan};
use crate::npc::loot::default_loot_for_archetype;
use crate::npc::loot::NpcLootTable;
use crate::npc::movement::GameTick;
use crate::npc::schedule::schedule_seed_from_char_id;
use crate::npc::spawn::{classify_zones_by_qi, initial_age_for_index};
use crate::qi_physics::{
    constants::{QI_EPSILON, QI_ZONE_UNIT_CAPACITY},
    qi_release_to_zone, regen_from_zone, QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::schema::cultivation::realm_to_string;
use crate::social::components::CharId;
use crate::world::dimension::DimensionKind;
use crate::world::zone::ZoneRegistry;

pub const NPC_DORMANT_REDIS_KEY: &str = "bong:npc/dormant";
const REDIS_URL_ENV_KEY: &str = "REDIS_URL";
const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379";
pub const HYDRATE_RADIUS_BLOCKS: f64 = 64.0;
pub const DEHYDRATE_RADIUS_BLOCKS: f64 = 256.0;
pub const DORMANT_ZONE_ABSORPTION_RADIUS_BLOCKS: f64 = 64.0;
pub const DORMANT_LIFECYCLE_TICK_INTERVAL: u32 = 20 * 60;

/// plan-offscreen-war-v1 P0：覆盖 `DORMANT_LIFECYCLE_TICK_INTERVAL` 的离屏快进 env。
///
/// **dev/test-only 节流旋钮**——只改 dormant batch tick 的间隔（让真服 e2e 能把
/// 一轮 60s 离屏 tick 压到秒级），**绝不**绕过 worldview 修炼规则或 qi_physics
/// 守恒律。值落非法（非数字 / 0）时回退默认 1200。
pub const DORMANT_TICK_INTERVAL_ENV: &str = "BONG_DORMANT_TICK_INTERVAL";

/// plan-offscreen-war-v1 P0：离屏战争 RNG 种子 env，用于 P1/P2 让战死结果可复现。
///
/// **dev/test-only 随机种子旋钮**——只决定 `AbstractCombatSeed` 与未来 dormant 战斗
/// RNG 的初值，**不**改变守恒：真元流动仍走 `release_dormant_qi_to_zone` →
/// `ledger.transfer(ReleaseToZone)`。env 未设时保持现状默认（种子 0）。
pub const SIM_SEED_ENV: &str = "BONG_SIM_SEED";

/// 纯解析：`BONG_DORMANT_TICK_INTERVAL` 原始值 → tick 间隔。
/// 合法（可解析 u32 且 > 0）才覆盖，否则回退 `default`（零 / 负 / 垃圾值 graceful fallback）。
fn parse_dormant_tick_interval(raw: Option<&str>, default: u32) -> u32 {
    raw.and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

/// 纯解析：`BONG_SIM_SEED` 原始值 → u64 种子。可解析才采用，否则回退默认种子 0。
fn parse_sim_seed(raw: Option<&str>) -> u64 {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0)
}

/// 读取 `BONG_DORMANT_TICK_INTERVAL`：合法（可解析 u32 且 > 0）才覆盖，否则回退默认。
fn dormant_tick_interval_from_env(default: u32) -> u32 {
    parse_dormant_tick_interval(
        std::env::var(DORMANT_TICK_INTERVAL_ENV).ok().as_deref(),
        default,
    )
}

/// 读取 `BONG_SIM_SEED`：可解析 u64 才采用，否则回退默认种子 0（= 现有行为）。
pub fn sim_seed_from_env() -> u64 {
    parse_sim_seed(std::env::var(SIM_SEED_ENV).ok().as_deref())
}

#[derive(Clone, Debug, Resource)]
pub struct NpcVirtualizationConfig {
    pub hydrate_radius_blocks: f64,
    pub dehydrate_radius_blocks: f64,
    pub transition_interval_ticks: u32,
    pub dormant_tick_interval_ticks: u32,
    pub dormant_aging_rate_multiplier: f64,
    pub max_hydrated_count: usize,
    pub max_dormant_count: usize,
    /// Test and batch-run escape hatch. Runtime keeps no-player worlds hydrated
    /// until seed paths can create dormant NPCs directly.
    pub dehydrate_without_players: bool,
    /// plan-offscreen-war-v1 P0：离屏战争 RNG 种子。dormant 战斗 roll 读此值，
    /// 由 `BONG_SIM_SEED` 注入（默认 0 = 现有行为）。**只影响随机种子，不绕守恒。**
    /// P2 起被 `run_dormant_combat_phase` → `roll_dormant_combat_death` 真实消费。
    pub sim_seed: u64,
    /// plan-offscreen-war-v1 P1：每 zone 每轮离屏战斗对数上限。
    ///
    /// `collect_zone_combat_pairs`（`dormant/combat.rs`）按此值截断每个 zone 配出的
    /// 敌对对数量，防 5000 dormant 规模下一个高密度 zone 在单 tick 内引爆过多战斗。
    /// 候选集先按战力 cap 到 `2 * max_combats_per_zone`，再两两配对，再 cap 到本值。
    /// 默认 3。**纯节流上限，不绕守恒**：P2 结算仍逐败者走 `release_dormant_qi_to_zone`。
    pub max_combats_per_zone: u32,
}

impl Default for NpcVirtualizationConfig {
    fn default() -> Self {
        Self {
            hydrate_radius_blocks: HYDRATE_RADIUS_BLOCKS,
            dehydrate_radius_blocks: DEHYDRATE_RADIUS_BLOCKS,
            transition_interval_ticks: 20,
            dormant_tick_interval_ticks: DORMANT_LIFECYCLE_TICK_INTERVAL,
            dormant_aging_rate_multiplier: 0.3,
            max_hydrated_count: 200,
            max_dormant_count: 5000,
            dehydrate_without_players: false,
            sim_seed: 0,
            max_combats_per_zone: 3,
        }
    }
}

impl NpcVirtualizationConfig {
    /// 运行时配置：以默认值为底，套用 `BONG_DORMANT_TICK_INTERVAL` /
    /// `BONG_SIM_SEED` 两个 dev/test env 覆盖。两个 env 都只动节流间隔与随机种子，
    /// 不触碰 worldview 修炼规则或 qi_physics 守恒律。
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            dormant_tick_interval_ticks: dormant_tick_interval_from_env(
                default.dormant_tick_interval_ticks,
            ),
            sim_seed: sim_seed_from_env(),
            ..default
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct DormantRoguePopulationSeedConfig {
    pub target_count: u32,
    pub resource_fraction: f32,
    pub resource_spirit_qi_threshold: f64,
    pub max_initial_age_ratio: f64,
}

impl Default for DormantRoguePopulationSeedConfig {
    fn default() -> Self {
        let target_count = std::env::var("BONG_DORMANT_ROGUE_SEED_COUNT")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(1000);
        Self {
            target_count,
            resource_fraction: 0.8,
            resource_spirit_qi_threshold: 0.4,
            max_initial_age_ratio: 0.8,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantPatrolSnapshot {
    pub home_zone: String,
    pub anchor_index: usize,
    pub current_target: [f64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantGuardianRelicSnapshot {
    pub relic_id: String,
    pub alarm_center: [f64; 3],
    pub alarm_radius: f64,
    pub trial_template_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_offered_tick: Option<u32>,
    pub offer_cooldown_ticks: u32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DormantZhinianPhase {
    Masquerade,
    Aggressive,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantFuyaAuraSnapshot {
    pub radius_blocks: f32,
    pub drain_boost_multiplier: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DormantDaoxiangOriginSnapshot {
    pub from_family: String,
    pub from_corpse_death_cause: String,
    pub activated_at_tick: u64,
    pub inherited_drops: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantTsyHostileSnapshot {
    pub family_id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zhinian_phase: Option<DormantZhinianPhase>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub zhinian_phase_entered_at_tick: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fuya_aura: Option<DormantFuyaAuraSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub daoxiang_origin: Option<DormantDaoxiangOriginSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DormantBehaviorIntent {
    Wander { drift_radius: f64 },
    PatrolToward { target: [f64; 3] },
    FleeFrom { source: [f64; 3], until_tick: u64 },
    Cultivate { zone: String },
    Retire { destination: [f64; 3] },
}

impl DormantBehaviorIntent {
    pub fn for_archetype(archetype: NpcArchetype, patrol: Option<&DormantPatrolSnapshot>) -> Self {
        match archetype {
            NpcArchetype::Rogue | NpcArchetype::Disciple => patrol
                .map(|patrol| Self::Cultivate {
                    zone: patrol.home_zone.clone(),
                })
                .unwrap_or(Self::Wander {
                    drift_radius: 120.0,
                }),
            NpcArchetype::Beast | NpcArchetype::GuardianRelic => patrol
                .map(|patrol| Self::PatrolToward {
                    target: patrol.current_target,
                })
                .unwrap_or(Self::Wander { drift_radius: 80.0 }),
            _ => Self::Wander {
                drift_radius: 120.0,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcDormantSnapshot {
    pub char_id: CharId,
    pub archetype: NpcArchetype,
    pub dimension: DimensionKind,
    pub zone_name: String,
    pub position: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub schedule_seed: Option<u64>,
    pub cultivation: Cultivation,
    pub meridian_system: MeridianSystem,
    pub meridian_severed: MeridianSeveredPermanent,
    pub contamination: Contamination,
    pub lifespan: NpcLifespan,
    pub shared_lifespan: LifespanComponent,
    pub lifespan_extension_ledger: LifespanExtensionLedger,
    pub death_registry: DeathRegistry,
    pub life_record: LifeRecord,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction: Option<FactionMembership>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub patrol: Option<DormantPatrolSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub loot_table: Option<NpcLootTable>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub guardian_relic: Option<DormantGuardianRelicSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tsy_hostile: Option<DormantTsyHostileSnapshot>,
    pub intent: DormantBehaviorIntent,
    pub dormant_since_tick: u64,
    pub last_dormant_tick_processed: u64,
    pub initial_qi: f64,
    pub qi_ledger_net: f64,
}

impl NpcDormantSnapshot {
    pub fn position_vec(&self) -> DVec3 {
        dvec3_from_array(self.position)
    }

    pub fn set_position_vec(&mut self, pos: DVec3) {
        self.position = vec3_to_array(pos);
    }

    pub fn realm_label(&self) -> String {
        realm_to_string(self.cultivation.realm).to_string()
    }

    pub fn faction_id_label(&self) -> Option<crate::npc::faction::FactionId> {
        self.faction
            .as_ref()
            .map(|membership| membership.faction_id)
    }
}

#[derive(Clone, Debug, Default, Resource, Serialize, Deserialize)]
pub struct NpcDormantStore {
    pub snapshots: HashMap<CharId, NpcDormantSnapshot>,
    pub by_archetype: HashMap<NpcArchetype, Vec<CharId>>,
    pub by_zone: HashMap<String, Vec<CharId>>,
    #[serde(skip, default)]
    restore_failed: bool,
    /// Persistence dirty flag. Set by every mutator that changes a snapshot
    /// (seed, dormant aging tick, death/release, hydrate/dehydrate). The Redis
    /// publish path (`network::publish_world_state_to_redis`) only re-serializes
    /// and re-pushes the whole hash when this is set, then clears it via
    /// [`Self::take_dirty`]. dormant changes are sparse (aging is a 60 s batch),
    /// so a clean publish cycle skips the full serde + hash replace entirely.
    /// NOT set by the Redis restore path: a snapshot just loaded from Redis is
    /// already persisted and must not trigger an immediate write-back.
    #[serde(skip, default)]
    dirty: bool,
}

impl NpcDormantStore {
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn mark_restore_failed(&mut self) {
        self.restore_failed = true;
    }

    pub fn restore_failed(&self) -> bool {
        self.restore_failed
    }

    /// Mark the store as needing a Redis hash write on the next publish cycle.
    /// Every code path that mutates a snapshot must call this so persistence
    /// never silently drops a change.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Read-only dirty accessor. The production publish path consumes the flag
    /// via [`Self::take_dirty`] (read-and-clear in one step), so this is only
    /// used by tests to assert the gate's state without mutating it. Kept as a
    /// stable accessor alongside [`Self::restore_failed`]; `allow(dead_code)`
    /// because non-test builds have no caller.
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Read-and-clear the dirty flag in one step. Returns the value the flag
    /// had on entry; leaves the flag `false`. The publish path uses this so a
    /// successful (or attempted) write resets the gate and a subsequent clean
    /// cycle is skipped.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn insert(&mut self, snapshot: NpcDormantSnapshot) -> Option<NpcDormantSnapshot> {
        let previous = self.snapshots.insert(snapshot.char_id.clone(), snapshot);
        self.rebuild_indexes();
        self.dirty = true;
        previous
    }

    pub fn remove(&mut self, char_id: &str) -> Option<NpcDormantSnapshot> {
        let removed = self.snapshots.remove(char_id);
        if removed.is_some() {
            self.rebuild_indexes();
            self.dirty = true;
        }
        removed
    }

    pub fn contains(&self, char_id: &str) -> bool {
        self.snapshots.contains_key(char_id)
    }

    #[cfg(test)]
    pub fn ids_by_archetype(&self, archetype: NpcArchetype) -> &[CharId] {
        self.by_archetype
            .get(&archetype)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub fn ids_by_zone(&self, zone_name: &str) -> &[CharId] {
        self.by_zone
            .get(zone_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// plan-offscreen-war-v1 P1：production-visible 读取某 zone 的 dormant char_ids。
    ///
    /// `ids_by_zone`（上）是 `#[cfg(test)]` only，离屏战斗配对（`combat.rs`
    /// `collect_zone_combat_pairs`）需要在非 test build 里枚举一个 zone 的候选集，
    /// 故新增本只读访问器。返回 `by_zone` 索引里已排序（`rebuild_indexes` 对每个
    /// zone 的 id 列表做了 `sort`）的 slice，空 zone 返回空 slice，不分配。
    pub fn char_ids_in_zone(&self, zone_name: &str) -> &[CharId] {
        self.by_zone
            .get(zone_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn sorted_snapshots(&self) -> Vec<&NpcDormantSnapshot> {
        let mut values = self.snapshots.values().collect::<Vec<_>>();
        values.sort_by(|left, right| left.char_id.cmp(&right.char_id));
        values
    }

    pub fn rebuild_indexes(&mut self) {
        self.by_archetype.clear();
        self.by_zone.clear();
        for snapshot in self.snapshots.values() {
            self.by_archetype
                .entry(snapshot.archetype)
                .or_default()
                .push(snapshot.char_id.clone());
            self.by_zone
                .entry(snapshot.zone_name.clone())
                .or_default()
                .push(snapshot.char_id.clone());
        }
        for ids in self.by_archetype.values_mut() {
            ids.sort();
        }
        for ids in self.by_zone.values_mut() {
            ids.sort();
        }
    }

    pub fn to_redis_hash_payloads(&self) -> Result<Vec<(String, String)>, serde_json::Error> {
        self.sorted_snapshots()
            .into_iter()
            .map(|snapshot| {
                serde_json::to_string(snapshot).map(|payload| (snapshot.char_id.clone(), payload))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Event, PartialEq, Eq)]
pub struct DormantSeveredAt {
    pub char_id: CharId,
    pub meridian_id: crate::cultivation::components::MeridianId,
}

/// plan-offscreen-war-v1 P2：一场离屏 dormant 派系互殴战死的内部战果 event。
///
/// 由 `dormant_global_tick_system` 的 combat phase 在败者结算后 emit；
/// `network::npc_event_bridge::publish_dormant_combat_events` 消费它发 `bong:npc/combat`
/// telemetry。**这是纯观测**——真元守恒回灌已由结算里的 `release_dormant_qi_to_zone` →
/// `ledger.transfer(ReleaseToZone)` 真实完成，本 event 不携带也不触发任何真元流动
/// （区别于 `abstract_combat_system` 那种「emit QiTransfer 却无人 apply」的吞真元红线）。
///
/// `qi_released` 是本场实际守恒回灌给 zone 的量（== release 的 `transfer.amount`）；zone 满
/// 时可能 0（败者满真元留账户、下轮 retain-until-released 重试），此时本 event 仍 emit
/// （战斗已发生、败者已 roll 出），但 `qi_released == 0.0` 且败者本轮不从 store 移除。
#[derive(Clone, Debug, Event, PartialEq)]
pub struct DormantCombatOutcome {
    pub winner: CharId,
    pub loser: CharId,
    pub zone: String,
    pub qi_released: f64,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering dormant NPC store and batch tick");
    app.init_resource::<NpcDormantStore>()
        .insert_resource(NpcVirtualizationConfig::from_env())
        .insert_resource(DormantRoguePopulationSeedConfig::default())
        .add_event::<DormantSeveredAt>()
        .add_event::<DormantCombatOutcome>()
        .add_systems(Startup, load_dormant_store_from_redis_system)
        .add_systems(
            Update,
            (
                seed_initial_dormant_population_on_startup,
                dormant_global_tick_system,
            ),
        );
}

fn load_dormant_store_from_redis_system(mut store: ResMut<NpcDormantStore>) {
    if !store.is_empty() {
        return;
    }
    match load_dormant_snapshots_from_redis(&mut store) {
        Ok(0) => {}
        Ok(count) => {
            tracing::info!("[bong][npc] loaded {count} dormant NPC snapshot(s) from Redis HASH")
        }
        Err(error) => {
            tracing::warn!("[bong][npc] failed dormant Redis HASH restore: {error}");
            store.mark_restore_failed();
        }
    }
}

fn load_dormant_snapshots_from_redis(store: &mut NpcDormantStore) -> Result<usize, String> {
    let client = redis::Client::open(dormant_redis_url_from_env()).map_err(|error| {
        format!("failed to open Redis client for {NPC_DORMANT_REDIS_KEY}: {error}")
    })?;
    let mut connection = client
        .get_connection()
        .map_err(|error| format!("failed to connect Redis for {NPC_DORMANT_REDIS_KEY}: {error}"))?;
    // One-time startup janitor: sweep any `{key}:tmp*` blobs left behind by an
    // earlier session whose hash-replace timed out (the bug this plan fixes
    // could leak hundreds of MB of nonce-suffixed temp keys). Best-effort — a
    // failed sweep must never block dormant restore.
    purge_leaked_dormant_temp_keys(&mut connection);
    let entries: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(NPC_DORMANT_REDIS_KEY)
        .query(&mut connection)
        .map_err(|error| format!("failed to HGETALL {NPC_DORMANT_REDIS_KEY}: {error}"))?;
    load_dormant_snapshots_from_hash_entries(store, entries)
}

/// SCAN glob that matches every temporary key the hash-replace dance can
/// create for the dormant store. Both the current deterministic `{key}:tmp`
/// and any legacy `{key}:tmp:<nonce>` survivors are covered by the trailing `*`.
fn dormant_tmp_scan_pattern() -> String {
    format!("{NPC_DORMANT_REDIS_KEY}:tmp*")
}

/// Given the raw keys returned by a SCAN, keep only the dormant temp keys that
/// are safe to delete. The live hash `bong:npc/dormant` itself shares the
/// `{key}` prefix but is NOT a temp key (it lacks the `:tmp` segment), so it
/// must be excluded — deleting it would wipe the persisted snapshots. Any key
/// that genuinely starts with `{key}:tmp` (the deterministic temp key or a
/// legacy nonce-suffixed leak) is purgeable.
fn tmp_keys_to_purge(scanned: &[String]) -> Vec<String> {
    let tmp_prefix = format!("{NPC_DORMANT_REDIS_KEY}:tmp");
    scanned
        .iter()
        .filter(|key| key.starts_with(&tmp_prefix))
        .cloned()
        .collect()
}

/// Best-effort sweep of leaked dormant temp keys on a blocking connection.
/// Never returns an error: persistence restore must proceed even if the
/// janitor cannot run (e.g. SCAN unsupported by a proxy).
fn purge_leaked_dormant_temp_keys(connection: &mut redis::Connection) {
    let pattern = dormant_tmp_scan_pattern();
    // `Cmd::iter` takes `self` by value, so the builder must be owned (not the
    // `&mut Cmd` the chained `.arg(..)` calls return) before iterating.
    let mut scan_cmd = redis::cmd("SCAN");
    scan_cmd
        .cursor_arg(0)
        .arg("MATCH")
        .arg(pattern.as_str())
        .arg("COUNT")
        .arg(512);
    let scanned: Vec<String> = match scan_cmd.iter::<String>(connection) {
        Ok(iter) => iter.collect(),
        Err(error) => {
            tracing::warn!(
                "[bong][npc] dormant temp-key janitor SCAN failed (skipping cleanup): {error}"
            );
            return;
        }
    };
    let purgeable = tmp_keys_to_purge(&scanned);
    if purgeable.is_empty() {
        return;
    }
    let mut del = redis::cmd("DEL");
    for key in &purgeable {
        del.arg(key.as_str());
    }
    match del.query::<i64>(connection) {
        Ok(deleted) => tracing::info!(
            "[bong][npc] dormant temp-key janitor purged {deleted} leaked `{NPC_DORMANT_REDIS_KEY}:tmp*` key(s)"
        ),
        Err(error) => tracing::warn!(
            "[bong][npc] dormant temp-key janitor DEL failed (left {} key(s)): {error}",
            purgeable.len()
        ),
    }
}

fn load_dormant_snapshots_from_hash_entries(
    store: &mut NpcDormantStore,
    entries: HashMap<String, String>,
) -> Result<usize, String> {
    if entries.is_empty() {
        return Ok(0);
    }
    let mut skipped = 0usize;
    for (char_id, payload) in entries {
        match serde_json::from_str::<NpcDormantSnapshot>(&payload) {
            Ok(snapshot) => {
                store.snapshots.insert(snapshot.char_id.clone(), snapshot);
            }
            Err(error) => {
                skipped += 1;
                tracing::warn!("[bong][npc] skipped invalid dormant snapshot `{char_id}`: {error}");
            }
        }
    }
    store.rebuild_indexes();
    if skipped > 0 && store.is_empty() {
        return Err(format!(
            "all {skipped} dormant Redis snapshot entries were invalid"
        ));
    }
    Ok(store.len())
}

fn dormant_redis_url_from_env() -> String {
    std::env::var(REDIS_URL_ENV_KEY)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string())
}

pub fn current_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default()
}

pub fn should_run_interval(tick: u64, interval: u32) -> bool {
    let interval = interval.max(1) as u64;
    tick == 0 || tick.is_multiple_of(interval)
}

pub fn vec3_to_array(pos: DVec3) -> [f64; 3] {
    [pos.x, pos.y, pos.z]
}

pub fn dvec3_from_array(pos: [f64; 3]) -> DVec3 {
    DVec3::new(pos[0], pos[1], pos[2])
}

pub fn planar_distance(left: DVec3, right: DVec3) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    (dx * dx + dz * dz).sqrt()
}

#[allow(clippy::too_many_arguments)]
fn dormant_global_tick_system(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    faction_store: Option<Res<FactionStore>>,
    mut store: ResMut<NpcDormantStore>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut death_notices: EventWriter<NpcDeathNotice>,
    mut combat_outcomes: EventWriter<DormantCombatOutcome>,
) {
    let tick = current_tick(game_tick.as_deref());
    if !should_run_interval(tick, config.dormant_tick_interval_ticks) {
        return;
    }
    let mut ids = store.snapshots.keys().cloned().collect::<Vec<_>>();
    ids.sort();

    let mut expired = Vec::new();
    let mut indexes_dirty = false;
    // Whether this tick actually advanced any snapshot (position / aging / regen
    // / breakthrough) or removed an expired one. Drives the persistence dirty
    // flag so a tick that touched nothing (all `elapsed_ticks == 0`) does not
    // schedule a redundant full hash write.
    let mut mutated_any = false;
    for char_id in ids {
        let Some(snapshot) = store.snapshots.get_mut(&char_id) else {
            continue;
        };
        let elapsed_ticks = tick.saturating_sub(snapshot.last_dormant_tick_processed);
        snapshot.last_dormant_tick_processed = tick;
        if elapsed_ticks == 0 {
            continue;
        }
        mutated_any = true;
        advance_dormant_position(snapshot, elapsed_ticks, tick);
        if let Some(zones) = zones.as_deref() {
            indexes_dirty |= refresh_snapshot_zone_name(snapshot, zones);
        }
        snapshot.lifespan.age_ticks +=
            elapsed_ticks as f64 * config.dormant_aging_rate_multiplier.max(0.0);

        if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
            apply_dormant_regen(snapshot, zones, ledger);
        }
        let zone_qi = zones
            .as_deref()
            .and_then(|zones| dormant_zone_qi(snapshot, zones));
        let _ = advance_dormant_breakthrough(snapshot, zone_qi, tick);

        if snapshot.lifespan.is_expired() {
            if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
                release_dormant_qi_to_zone(snapshot, zones, ledger);
            }
            if snapshot.cultivation.qi_current > QI_EPSILON {
                tracing::warn!(
                    "[bong][npc] retained expired dormant NPC `{}` until {:.6} qi releases",
                    snapshot.char_id,
                    snapshot.cultivation.qi_current
                );
                continue;
            }
            death_notices.send(dormant_natural_death_notice(snapshot));
            expired.push(char_id);
        }
    }

    let mut removed_expired = !expired.is_empty();
    for char_id in expired {
        store.snapshots.remove(&char_id);
    }
    // Rebuild the spatial indexes BEFORE the combat phase so `by_zone` reflects
    // this tick's post-movement / post-natural-death population — the combat
    // phase pairs strictly within a zone via `char_ids_in_zone`.
    if removed_expired || indexes_dirty {
        store.rebuild_indexes();
    }

    // plan-offscreen-war-v1 P2：离屏派系互殴 combat phase。**同一个 system、同一个 tick
    // interval、同一份 store/ledger 可变借用**——绝不另起第二个 timer（§10.1 #3：第二个
    // timer 会与本系统抢 store/ledger 可变借用）。借用安全走 collect-then-index
    // （`collect_zone_combat_pairs` 先返回 owned `Vec<(CharId,CharId)>`，再逐 id 索引结算），
    // 规避 per-char_id 单可变借用与两两对战冲突。faction_store 是只读 `Res`。
    if let (Some(faction_store), Some(zones), Some(ledger)) = (
        faction_store.as_deref(),
        zones.as_deref_mut(),
        ledger.as_deref_mut(),
    ) {
        let combat_removed = run_dormant_combat_phase(
            &mut store,
            faction_store,
            &config,
            tick,
            zones,
            ledger,
            &mut death_notices,
            &mut combat_outcomes,
        );
        if combat_removed {
            removed_expired = true;
            store.rebuild_indexes();
        }
    }

    // Any advanced or removed snapshot changed persisted state; schedule a
    // Redis write on the next publish cycle.
    if mutated_any || removed_expired {
        store.mark_dirty();
    }
}

/// plan-offscreen-war-v1 P2：离屏派系互殴战死结算（脊柱核心）。
///
/// 接 P1 纯逻辑：先 `collect_zone_combat_pairs`（只读、owned id 对）→ 逐对 `roll_*` 出败者
/// → 守恒结算败者真元 → emit death + outcome → 人口回写。**守恒唯一流动点**：败者残余真元
/// 走 `release_dormant_qi_to_zone` → `ledger.transfer(ReleaseToZone)` 真实回灌 zone
/// （§10.1 #5 ②）。胜者真元不变（dormant 简化，未流动即未失衡，§10.1 #5 ③）。
///
/// **防吞真元（retain-until-released）**：若 release 后败者仍有 `qi_current > QI_EPSILON`
/// （zone 已满到 `QI_ZONE_UNIT_CAPACITY`，overflow 留在败者账户），本轮**不** `store.remove`
/// 败者，留到后续轮次继续释放——复用自然老死路径的同款防吞真元模式
/// （`docs/CLAUDE.md §四`「离屏战斗吞真元」红线）。真元全部释放（`<= QI_EPSILON`）的败者才
/// 移除。同 zone 多败者**顺序** release（先 release 抬高 zone_qi，后者读更高基线，order-
/// independent、不溢出，与 `release.rs accumulate_zone_release` 同源）。
///
/// 返回是否有败者被移除（调用方据此 rebuild 索引 + mark dirty）。借用安全：本函数独占
/// `&mut store` / `&mut ledger`，配对用 immutable snapshot 读，结算用 char_id 索引 get_mut，
/// 任一时刻只持一个 snapshot 的可变借用。
#[allow(clippy::too_many_arguments)]
fn run_dormant_combat_phase(
    store: &mut NpcDormantStore,
    faction_store: &FactionStore,
    config: &NpcVirtualizationConfig,
    tick: u64,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    death_notices: &mut EventWriter<NpcDeathNotice>,
    combat_outcomes: &mut EventWriter<DormantCombatOutcome>,
) -> bool {
    // ① 配对：immutable 只读 → owned id 对（§10.1 #3 collect-then-index）。
    let pairs = combat::collect_zone_combat_pairs(store, faction_store, config);
    if pairs.is_empty() {
        return false;
    }

    let mut removed_any = false;
    for (a_id, b_id) in pairs {
        // 防御：上一对的结算可能已移除本对成员（理论上 collect 保证每个 NPC 一轮至多一次，
        // 但 retain-until-released 让 store 在结算中变动，索引取不到就跳过，绝不 panic）。
        let (Some(a), Some(b)) = (store.snapshots.get(&a_id), store.snapshots.get(&b_id)) else {
            continue;
        };

        // ② roll 败者（纯函数，只读双方快照，确定性 RNG 用 config.sim_seed）。
        let Some(loser_id) = combat::roll_dormant_combat_death(a, b, tick, config.sim_seed) else {
            // 非法自我对战（a_id == b_id）：collect 已规范化升序不会产生，仍兜底跳过。
            continue;
        };
        let winner_id = if loser_id == a_id {
            b_id.clone()
        } else {
            a_id.clone()
        };

        // ③ 败者守恒结算：唯一真元流动点。
        let Some(loser) = store.snapshots.get_mut(&loser_id) else {
            continue;
        };
        // 战死方所在 zone（release 内部也会重定位，这里取 snapshot.zone_name 作 telemetry）。
        let zone_name = loser.zone_name.clone();
        // 胜者真元不变（dormant 简化，§10.1 #5 ③）——不读不写胜者，少一次 ledger 操作。
        let released = release_dormant_qi_to_zone(loser, zones, ledger)
            .map(|transfer| transfer.amount)
            .unwrap_or(0.0);

        // ④ 战死 death notice（reason=Combat + from_dormant_combat=true + pos）。
        death_notices.send(dormant_combat_death_notice(loser));

        // ⑤ 战果 telemetry（纯观测，不携带真元流动）。
        combat_outcomes.send(DormantCombatOutcome {
            winner: winner_id,
            loser: loser_id.clone(),
            zone: zone_name,
            qi_released: released,
        });

        // ⑥ 人口回写 + 防吞真元：真元全释放才移除；zone 满残余 overflow > ε 则本轮保留、下轮重试。
        // 先把后续唯一需要的标量（残余真元）从 `&mut loser` 借用里拷出。
        let residual = loser.cultivation.qi_current;
        // 随即显式终结 `&mut loser` 借用：NLL 本会在这里结束，但显式 `let _ = loser;`（`drop`
        // 对引用是 no-op，会触发 clippy::dropping_references）让下方 retain/remove 决策对未来
        // 编辑（如在此块内再次借用 `store`）防御性免疫——后续只用早已 owned 的标量
        // （`loser_id` / `zone_name` / `released` / `residual`），行为不变。
        let _ = loser;
        if residual > QI_EPSILON {
            tracing::warn!(
                "[bong][npc] retained combat-dead dormant NPC `{}` until {:.6} residual qi releases (zone full)",
                loser_id,
                residual
            );
        } else {
            store.snapshots.remove(&loser_id);
            removed_any = true;
        }
    }

    removed_any
}

fn seed_initial_dormant_population_on_startup(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    seed_config: Res<DormantRoguePopulationSeedConfig>,
    mut store: ResMut<NpcDormantStore>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut seeded: valence::prelude::Local<bool>,
) {
    if *seeded || seed_config.target_count == 0 {
        return;
    }
    if store.restore_failed() {
        *seeded = true;
        tracing::warn!("[bong][npc] skipped dormant seed population because Redis restore failed");
        return;
    }
    if !store.is_empty() {
        *seeded = true;
        return;
    }
    let Some(zone_registry) = zone_registry.as_deref() else {
        return;
    };
    if zone_registry.zones.is_empty() {
        return;
    }

    let capacity = config.max_dormant_count.saturating_sub(store.len());
    let target_count = seed_config.target_count.min(capacity as u32);
    if target_count == 0 {
        *seeded = true;
        return;
    }

    let (resource_zones, background_zones) = classify_zones_by_qi(
        &zone_registry.zones,
        seed_config.resource_spirit_qi_threshold,
    );
    let resource_target =
        ((target_count as f32) * seed_config.resource_fraction.clamp(0.0, 1.0)).round() as u32;
    let tick = current_tick(game_tick.as_deref());

    // Per-zone running counter: each zone gets a *dense* low-discrepancy
    // sequence index so its dormant snapshots tile evenly across the zone AABB
    // instead of piling onto shared patrol anchors (the old ±2 block jitter).
    let mut zone_local_counts: HashMap<String, u32> = HashMap::new();
    for index in 0..target_count {
        let zone_candidates = if index < resource_target && !resource_zones.is_empty() {
            &resource_zones
        } else if !background_zones.is_empty() {
            &background_zones
        } else {
            &resource_zones
        };
        if zone_candidates.is_empty() {
            break;
        }

        let zone = zone_candidates[(index as usize) % zone_candidates.len()];
        let zone_local_index = {
            let counter = zone_local_counts.entry(zone.name.clone()).or_insert(0);
            let current = *counter;
            *counter += 1;
            current
        };
        let snapshot = dormant_rogue_seed_snapshot(
            zone,
            index,
            zone_local_index,
            tick,
            seed_config.max_initial_age_ratio,
        );
        store.snapshots.insert(snapshot.char_id.clone(), snapshot);
    }
    store.rebuild_indexes();
    // Freshly seeded population must be persisted on the next publish cycle.
    store.mark_dirty();
    *seeded = true;
    tracing::info!(
        "[bong][npc] seeded {} dormant rogue NPC snapshots",
        store.len()
    );
}

/// Plastic-number (R2) low-discrepancy sequence constants — Roberts 2018.
/// `α_x = 1/g`, `α_z = 1/g²` where g ≈ 1.32472 is the plastic number (the
/// unique real root of x³ = x + 1). Stepping `frac(0.5 + αₙ·n)` over n yields
/// near-uniform 2D coverage for *any* point count, so a zone's dormant
/// snapshots stay spread out no matter how many seed into it.
const DORMANT_SCATTER_ALPHA_X: f64 = 0.754_877_666_246_692_8;
const DORMANT_SCATTER_ALPHA_Z: f64 = 0.569_840_290_998_053_2;

/// Deterministically scatter a dormant rogue across `zone`'s XZ footprint using
/// its per-zone sequence index. Y matches the hydrated `PoissonSpawnSampler`
/// path (`(min.y + max.y) / 2`) — both feed the same `spawn_rogue_npc_at` on
/// hydrate, where gravity grounds the entity, so only the XZ spread is visible.
fn dormant_seed_scatter_position(zone: &crate::world::zone::Zone, zone_local_index: u32) -> DVec3 {
    let (min, max) = zone.bounds;
    let n = (zone_local_index as f64) + 1.0;
    let fx = (0.5 + DORMANT_SCATTER_ALPHA_X * n).fract();
    let fz = (0.5 + DORMANT_SCATTER_ALPHA_Z * n).fract();
    let raw = DVec3::new(
        min.x + fx * (max.x - min.x),
        (min.y + max.y) * 0.5,
        min.z + fz * (max.z - min.z),
    );
    zone.clamp_position(raw)
}

/// plan-offscreen-war-v1 P0 #1：给 seeded dormant rogue 按 char_id 哈希分派系。
///
/// `is_hostile_pair` 当前只认 Attack↔Defend（Neutral 对谁都不敌对），所以这里
/// 把散修二分到 Attack / Defend，保证后续阶段（P1 配对 / P2 战死）一定能在同 zone
/// 内凑出敌对对，否则 `faction: None` 让所有阶段空转。具名多宗留 P5 的关系矩阵。
///
/// 用与 RNG 同源的 `deterministic_hash`（salt=0），保证同 char_id 跨重启稳定分派。
fn seed_rogue_faction(char_id: &str) -> FactionMembership {
    let faction_id = if deterministic_hash(char_id, 0) % 2 == 0 {
        FactionId::Attack
    } else {
        FactionId::Defend
    };
    FactionMembership {
        faction_id,
        rank: FactionRank::Disciple,
        reputation: Reputation::default(),
        lineage: None,
        mission_queue: MissionQueue::default(),
    }
}

fn dormant_rogue_seed_snapshot(
    zone: &crate::world::zone::Zone,
    index: u32,
    zone_local_index: u32,
    tick: u64,
    max_initial_age_ratio: f64,
) -> NpcDormantSnapshot {
    let archetype = NpcArchetype::Rogue;
    let position = dormant_seed_scatter_position(zone, zone_local_index);
    let patrol_target = zone.center();
    let char_id = format!("dormant:rogue:{index}");
    let cultivation = Cultivation::default();
    let mut meridian_system = MeridianSystem::default();
    meridian_system.get_mut(MeridianId::Lung).opened = true;
    let lifespan = NpcLifespan::new(
        initial_age_for_index(
            index,
            archetype.default_max_age_ticks(),
            max_initial_age_ratio,
        ),
        archetype.default_max_age_ticks(),
    );
    let patrol = Some(DormantPatrolSnapshot {
        home_zone: zone.name.clone(),
        anchor_index: index as usize,
        current_target: vec3_to_array(patrol_target),
    });
    let intent = DormantBehaviorIntent::for_archetype(archetype, patrol.as_ref());

    NpcDormantSnapshot {
        char_id: char_id.clone(),
        archetype,
        dimension: zone.dimension,
        zone_name: zone.name.clone(),
        position: vec3_to_array(position),
        schedule_seed: Some(schedule_seed_from_char_id(char_id.as_str())),
        cultivation: cultivation.clone(),
        meridian_system,
        meridian_severed: MeridianSeveredPermanent::default(),
        contamination: Contamination::default(),
        lifespan,
        shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        death_registry: DeathRegistry::new(char_id.clone()),
        life_record: LifeRecord::new(char_id.clone()),
        // plan-offscreen-war-v1 P0 #1：赋派系（Attack/Defend 二分），保证 is_hostile_pair
        // 在 P1/P2 能配出敌对对。具名势力留 P5。
        faction: Some(seed_rogue_faction(char_id.as_str())),
        patrol,
        loot_table: Some(default_loot_for_archetype(archetype)),
        guardian_relic: None,
        tsy_hostile: None,
        intent,
        dormant_since_tick: tick,
        last_dormant_tick_processed: tick,
        initial_qi: cultivation.qi_current,
        qi_ledger_net: 0.0,
    }
}

pub fn advance_dormant_position(
    snapshot: &mut NpcDormantSnapshot,
    elapsed_ticks: u64,
    salt_tick: u64,
) {
    let seconds = elapsed_ticks as f64 / 20.0;
    let current = snapshot.position_vec();
    let next = match &snapshot.intent {
        DormantBehaviorIntent::Wander { drift_radius } => {
            let seed = deterministic_unit(snapshot.char_id.as_str(), salt_tick);
            let angle = seed * std::f64::consts::TAU;
            let step = seconds.clamp(0.0, 60.0);
            let drift_cap = drift_radius.max(0.0);
            DVec3::new(
                current.x + angle.cos() * step.min(drift_cap),
                current.y,
                current.z + angle.sin() * step.min(drift_cap),
            )
        }
        DormantBehaviorIntent::PatrolToward { target }
        | DormantBehaviorIntent::Retire {
            destination: target,
        } => move_toward(current, dvec3_from_array(*target), seconds.max(0.0)),
        DormantBehaviorIntent::FleeFrom { source, .. } => {
            let source = dvec3_from_array(*source);
            let away = current - source;
            let length = (away.x * away.x + away.z * away.z).sqrt();
            if length <= f64::EPSILON {
                current
            } else {
                let step = seconds.max(0.0);
                DVec3::new(
                    current.x + away.x / length * step,
                    current.y,
                    current.z + away.z / length * step,
                )
            }
        }
        DormantBehaviorIntent::Cultivate { .. } => current,
    };
    snapshot.set_position_vec(next);
}

fn move_toward(current: DVec3, target: DVec3, max_step: f64) -> DVec3 {
    let dx = target.x - current.x;
    let dz = target.z - current.z;
    let distance = (dx * dx + dz * dz).sqrt();
    if distance <= f64::EPSILON || max_step >= distance {
        return DVec3::new(target.x, current.y, target.z);
    }
    DVec3::new(
        current.x + dx / distance * max_step,
        current.y,
        current.z + dz / distance * max_step,
    )
}

fn deterministic_unit(char_id: &str, salt: u64) -> f64 {
    let hash = deterministic_hash(char_id, salt);
    (hash & 0xffff) as f64 / 65_535.0
}

fn deterministic_hash(char_id: &str, salt: u64) -> u64 {
    let mut hash = salt ^ 0x9E37_79B9_7F4A_7C15;
    for byte in char_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
    hash
}

pub fn apply_dormant_regen(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .filter(|zone| planar_distance(zone.center(), pos) <= DORMANT_ZONE_ABSORPTION_RADIUS_BLOCKS)
        .map(|zone| zone.name.clone())?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    if zone.spirit_qi <= 0.0 {
        return None;
    }

    let rate = snapshot.meridian_system.sum_rate();
    if rate <= 0.0 {
        return None;
    }
    let integrity_count = snapshot.meridian_system.iter().count() as f64;
    let avg_integrity = if integrity_count > 0.0 {
        snapshot
            .meridian_system
            .iter()
            .map(|meridian| meridian.integrity)
            .sum::<f64>()
            / integrity_count
    } else {
        1.0
    };
    let room = (snapshot.cultivation.qi_max - snapshot.cultivation.qi_current).max(0.0);
    let (gain, drain) = regen_from_zone(zone.spirit_qi, rate, avg_integrity, room);
    if gain <= 0.0 || drain <= 0.0 {
        return None;
    }

    let zone_account = QiAccountId::zone(zone.name.clone());
    let npc_account = QiAccountId::npc(snapshot.char_id.clone());
    ledger
        .set_balance(
            zone_account.clone(),
            zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY,
        )
        .ok()?;
    ledger
        .set_balance(
            npc_account.clone(),
            snapshot.cultivation.qi_current.max(0.0),
        )
        .ok()?;
    let transfer = QiTransfer::new(
        zone_account,
        npc_account.clone(),
        gain,
        QiTransferReason::CultivationRegen,
    )
    .ok()?;
    ledger.transfer(transfer.clone()).ok()?;

    snapshot.cultivation.qi_current = ledger.balance(&npc_account);
    snapshot.qi_ledger_net += gain;
    zone.spirit_qi = (zone.spirit_qi - drain).max(0.0);
    Some(transfer)
}

fn dormant_zone_qi(snapshot: &NpcDormantSnapshot, zones: &ZoneRegistry) -> Option<f64> {
    zones
        .find_zone(snapshot.dimension, snapshot.position_vec())
        .map(|zone| zone.spirit_qi)
}

fn refresh_snapshot_zone_name(snapshot: &mut NpcDormantSnapshot, zones: &ZoneRegistry) -> bool {
    let Some(zone) = zones.find_zone(snapshot.dimension, snapshot.position_vec()) else {
        return false;
    };
    if snapshot.zone_name == zone.name {
        return false;
    }
    snapshot.zone_name = zone.name.clone();
    true
}

pub fn advance_dormant_breakthrough(
    snapshot: &mut NpcDormantSnapshot,
    zone_qi: Option<f64>,
    tick: u64,
) -> Option<Result<BreakthroughSuccess, BreakthroughError>> {
    let mut roll = XorshiftRoll(deterministic_hash(&snapshot.char_id, tick));
    advance_dormant_breakthrough_with_roll(snapshot, zone_qi, tick, &mut roll)
}

fn advance_dormant_breakthrough_with_roll<R: RollSource>(
    snapshot: &mut NpcDormantSnapshot,
    zone_qi: Option<f64>,
    tick: u64,
    roll: &mut R,
) -> Option<Result<BreakthroughSuccess, BreakthroughError>> {
    let next = next_realm(snapshot.cultivation.realm)?;
    if next == Realm::Void {
        return None;
    }
    if snapshot.cultivation.qi_current < breakthrough_qi_cost(next) {
        return None;
    }
    let required_zone_qi = if next == Realm::Solidify {
        MIN_ZONE_QI_TO_GUYUAN
    } else {
        MIN_ZONE_QI_TO_BREAKTHROUGH
    };
    if zone_qi? < required_zone_qi {
        return None;
    }

    let result = try_breakthrough(
        &mut snapshot.cultivation,
        &mut snapshot.meridian_system,
        0.0,
        roll,
    );
    match result {
        Ok(success) => {
            let previous_cap = snapshot.shared_lifespan.cap_by_realm.max(1) as f64;
            let new_cap = LifespanCapTable::for_realm(success.to);
            snapshot.shared_lifespan.apply_cap(new_cap);
            snapshot.lifespan.max_age_ticks *= new_cap as f64 / previous_cap;
            snapshot
                .life_record
                .push(BiographyEntry::BreakthroughSucceeded {
                    realm: success.to,
                    tick,
                });
            Some(Ok(success))
        }
        Err(BreakthroughError::RolledFailure { severity }) => {
            snapshot
                .life_record
                .push(BiographyEntry::BreakthroughFailed {
                    realm_target: next,
                    severity,
                    tick,
                });
            Some(Err(BreakthroughError::RolledFailure { severity }))
        }
        Err(error) => Some(Err(error)),
    }
}

pub fn release_dormant_qi_to_zone(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .map(|zone| zone.name.clone())
        .or_else(|| Some(snapshot.zone_name.clone()))?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    let amount = snapshot.cultivation.qi_current.max(0.0);
    if amount <= 0.0 {
        return None;
    }

    let npc_account = QiAccountId::npc(snapshot.char_id.clone());
    let zone_account = QiAccountId::zone(zone.name.clone());
    ledger.set_balance(npc_account.clone(), amount).ok()?;
    let zone_current = zone.spirit_qi.max(0.0) * QI_ZONE_UNIT_CAPACITY;
    ledger
        .set_balance(zone_account.clone(), zone_current)
        .ok()?;
    let outcome = qi_release_to_zone(
        amount,
        npc_account,
        zone_account,
        zone_current,
        QI_ZONE_UNIT_CAPACITY,
    )
    .ok()?;
    let transfer = outcome.transfer?;
    ledger.transfer(transfer.clone()).ok()?;
    zone.spirit_qi = (outcome.zone_after / QI_ZONE_UNIT_CAPACITY).clamp(-1.0, 1.0);
    snapshot.cultivation.qi_current = outcome.overflow;
    snapshot.qi_ledger_net -= outcome.accepted;
    Some(transfer)
}

/// 构造一条 dormant 死亡通知，按**死因分支**填 `reason` / `from_dormant_combat`。
///
/// plan-offscreen-war-v1 P2：从硬编码 `NaturalAging` 改为按 `reason` 入参分支——
/// - 自然老死走 [`dormant_natural_death_notice`]（`reason=NaturalAging`，`from_dormant_combat=false`）；
/// - 离屏战死走 [`dormant_combat_death_notice`]（`reason=Combat`，`from_dormant_combat=true`），
///   让 agent / e2e 能把战死与老死区分开。
///
/// 两者都带 `pos=Some(snapshot.position)`（战场 / 陨落坐标），供派系战报定位与 e2e 断言。
fn dormant_death_notice(
    snapshot: &NpcDormantSnapshot,
    reason: NpcDeathReason,
    from_dormant_combat: bool,
) -> NpcDeathNotice {
    let life_record_snapshot = {
        let summary = snapshot.life_record.recent_summary_text(8);
        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
    };
    NpcDeathNotice {
        npc_id: snapshot.char_id.clone(),
        archetype: snapshot.archetype,
        reason,
        faction_id: snapshot
            .faction
            .as_ref()
            .map(|membership| membership.faction_id),
        life_record_snapshot,
        age_ticks: snapshot.lifespan.age_ticks,
        max_age_ticks: snapshot.lifespan.max_age_ticks,
        from_dormant_combat,
        pos: Some(snapshot.position),
    }
}

/// 自然老死通知（`reason=NaturalAging`，`from_dormant_combat=false`）。
fn dormant_natural_death_notice(snapshot: &NpcDormantSnapshot) -> NpcDeathNotice {
    dormant_death_notice(snapshot, NpcDeathReason::NaturalAging, false)
}

/// 离屏派系互殴战死通知（`reason=Combat`，`from_dormant_combat=true`）。
fn dormant_combat_death_notice(snapshot: &NpcDormantSnapshot) -> NpcDeathNotice {
    dormant_death_notice(snapshot, NpcDeathReason::Combat, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{MeridianId, Realm};
    use crate::world::dimension::DimensionKind;
    use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};
    use valence::prelude::Events;

    /// P0 bug② contract: the startup janitor purges leaked `{key}:tmp*` keys
    /// but must never delete the live persisted hash. The deletion target is
    /// decided by `tmp_keys_to_purge` over the SCAN results, so this pins that
    /// decision directly (the SCAN/DEL I/O wrapper is best-effort glue around
    /// it). Covers: empty input, mixed real-leak + legacy-nonce-leak, the
    /// off-by-one where the live key shares the prefix but is NOT a temp key,
    /// and an unrelated key.
    #[test]
    fn startup_janitor_purges_leaked_tmp_keys() {
        // Empty SCAN -> nothing to purge.
        assert!(
            tmp_keys_to_purge(&[]).is_empty(),
            "expected no purge targets from an empty SCAN because there is nothing leaked; got a non-empty list"
        );

        // The glob the janitor hands to SCAN must be anchored on the dormant key
        // and end in `:tmp*` so it catches both the deterministic temp key and
        // legacy nonce-suffixed survivors, and nothing outside the dormant key.
        assert_eq!(
            dormant_tmp_scan_pattern(),
            "bong:npc/dormant:tmp*",
            "expected the janitor SCAN glob to be `{{key}}:tmp*` so it matches every dormant temp key (deterministic + legacy nonce) and only those; got a different glob"
        );

        let scanned = vec![
            // Deterministic temp key from the current code path -> purge.
            "bong:npc/dormant:tmp".to_string(),
            // Legacy nonce-suffixed leak from the old code path -> purge.
            "bong:npc/dormant:tmp:1780000000000000000".to_string(),
            // The live persisted hash: shares the `bong:npc/dormant` prefix but
            // has NO `:tmp` segment -> must be KEPT (off-by-one boundary).
            NPC_DORMANT_REDIS_KEY.to_string(),
            // Unrelated key -> kept.
            "bong:world_state".to_string(),
        ];
        let purge = tmp_keys_to_purge(&scanned);

        assert_eq!(
            purge,
            vec![
                "bong:npc/dormant:tmp".to_string(),
                "bong:npc/dormant:tmp:1780000000000000000".to_string(),
            ],
            "expected exactly the two `{{key}}:tmp...` leaks to be purged because they are dead temp blobs, while the live `{NPC_DORMANT_REDIS_KEY}` hash and the unrelated key are preserved; got {purge:?}"
        );
        assert!(
            !purge.contains(&NPC_DORMANT_REDIS_KEY.to_string()),
            "expected the live persisted dormant hash to NEVER be a purge target (deleting it wipes all snapshots); it was selected for deletion"
        );
        assert!(
            !purge.contains(&"bong:world_state".to_string()),
            "expected unrelated keys to be left untouched by the dormant janitor; an unrelated key was selected for deletion"
        );
    }

    fn zone() -> Zone {
        Zone {
            name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(0.0, 0.0, 0.0), DVec3::new(100.0, 128.0, 100.0)),
            spirit_qi: 0.8,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(10.0, 64.0, 10.0)],
            blocked_tiles: Vec::new(),
        }
    }

    fn snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            qi_current: 0.1,
            qi_max: 1.0,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: DimensionKind::Overworld,
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            position: vec3_to_array(pos),
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: MeridianSystem::default(),
            meridian_severed: MeridianSeveredPermanent::default(),
            contamination: Contamination::default(),
            lifespan: NpcLifespan::new(0.0, 1_000.0),
            shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
            lifespan_extension_ledger: LifespanExtensionLedger::default(),
            death_registry: DeathRegistry::new(char_id),
            life_record: LifeRecord::new(char_id),
            faction: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: 0.1,
            qi_ledger_net: 0.0,
        }
    }

    struct FixedRoll(f64);

    impl RollSource for FixedRoll {
        fn roll_unit(&mut self) -> f64 {
            self.0
        }
    }

    fn open_regular_meridians(snapshot: &mut NpcDormantSnapshot, count: usize) {
        for id in MeridianId::REGULAR.into_iter().take(count) {
            let meridian = snapshot.meridian_system.get_mut(id);
            meridian.opened = true;
        }
    }

    #[test]
    fn dormant_scatter_stays_in_zone_bounds() {
        // Hydration spawns the entity at exactly this position, so an
        // out-of-bounds seed would leak NPCs outside their home zone. Every
        // R2 sample (fx,fz ∈ [0,1)) plus clamp must land inside the AABB.
        let zone = zone();
        for idx in 0..256u32 {
            let pos = dormant_seed_scatter_position(&zone, idx);
            assert!(
                zone.contains(pos),
                "zone_local_index {idx} produced out-of-bound pos {pos:?} for bounds {:?}",
                zone.bounds
            );
        }
    }

    #[test]
    fn dormant_scatter_spreads_across_zone_instead_of_clustering() {
        // Regression for the old anchor + ±2 block jitter: 64 snapshots seeded
        // into one zone must tile the whole footprint, not pile onto one anchor.
        let zone = zone();
        let (min, max) = zone.bounds;
        let width = max.x - min.x; // 100
        let depth = max.z - min.z; // 100
        let positions: Vec<DVec3> = (0..64u32)
            .map(|i| dormant_seed_scatter_position(&zone, i))
            .collect();

        // (a) Footprint span: the R2 sequence covers most of each axis.
        let span_x = positions
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max)
            - positions.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let span_z = positions
            .iter()
            .map(|p| p.z)
            .fold(f64::NEG_INFINITY, f64::max)
            - positions.iter().map(|p| p.z).fold(f64::INFINITY, f64::min);
        assert!(
            span_x > width * 0.8 && span_z > depth * 0.8,
            "64 snapshots should span >80% of the {width}x{depth} zone \
             (got span_x={span_x:.1}, span_z={span_z:.1}); the old ±2 jitter spanned <5",
        );

        // (b) No stacking: closest pair on the XZ plane stays well separated.
        let mut min_pair = f64::INFINITY;
        for (i, a) in positions.iter().enumerate() {
            for b in positions.iter().skip(i + 1) {
                let d = ((a.x - b.x).powi(2) + (a.z - b.z).powi(2)).sqrt();
                min_pair = min_pair.min(d);
            }
        }
        assert!(
            min_pair > 4.0,
            "closest pair of 64 scattered snapshots is {min_pair:.2} blocks apart; \
             expected > 4 (old jitter stacked many inside a 4-block box)",
        );
    }

    #[test]
    fn dormant_scatter_is_deterministic_and_distinct() {
        // Same index → same position (Redis restore / re-seed stays stable);
        // distinct indices → distinct positions (no silent collisions / stacking).
        let zone = zone();
        assert_eq!(
            dormant_seed_scatter_position(&zone, 7),
            dormant_seed_scatter_position(&zone, 7),
            "scatter must be a pure function of (zone, index)"
        );
        let mut seen: Vec<DVec3> = Vec::new();
        for i in 0..128u32 {
            let pos = dormant_seed_scatter_position(&zone, i);
            assert!(
                !seen
                    .iter()
                    .any(|p| (p.x - pos.x).abs() < 1e-9 && (p.z - pos.z).abs() < 1e-9),
                "index {i} collided with an earlier snapshot at {pos:?}"
            );
            seen.push(pos);
        }
    }

    #[test]
    fn store_indexes_by_archetype_and_zone() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
        store.insert(snapshot("npc_b", DVec3::new(11.0, 64.0, 10.0)));

        assert_eq!(
            store.ids_by_archetype(NpcArchetype::Rogue),
            &["npc_a".to_string(), "npc_b".to_string()]
        );
        assert_eq!(
            store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME),
            &["npc_a".to_string(), "npc_b".to_string()]
        );
    }

    /// P1 contract: a fresh store is clean, and the three real mutator
    /// categories the publish gate cares about — seed, dormant aging tick, and
    /// death/removal — each flip `is_dirty()` so the next publish cycle writes
    /// the change to Redis. A change that never raises the flag would be
    /// silently dropped by the dirty-gated publish path.
    #[test]
    fn dormant_store_dirty_set_on_seed_age_death() {
        // A default store has nothing to persist yet.
        assert!(
            !NpcDormantStore::default().is_dirty(),
            "expected a freshly constructed store to be clean because no snapshot has changed; it reported dirty"
        );

        // (1) seed: the real startup seed system populates the store and must
        // mark it dirty so the seeded population reaches Redis.
        let mut seed_app = App::new();
        seed_app.insert_resource(NpcVirtualizationConfig::default());
        seed_app.insert_resource(DormantRoguePopulationSeedConfig {
            target_count: 4,
            resource_fraction: 0.0,
            resource_spirit_qi_threshold: 0.4,
            max_initial_age_ratio: 0.0,
        });
        seed_app.insert_resource(ZoneRegistry {
            zones: vec![zone()],
        });
        seed_app.init_resource::<NpcDormantStore>();
        seed_app.add_systems(Update, seed_initial_dormant_population_on_startup);
        seed_app.update();
        let seeded = seed_app.world().resource::<NpcDormantStore>();
        assert!(
            !seeded.is_empty(),
            "seed system precondition failed: expected snapshots to be seeded before checking dirty"
        );
        assert!(
            seeded.is_dirty(),
            "expected the store to be dirty after the startup seed populated {} snapshots, so the seeded population is persisted; it stayed clean",
            seeded.len()
        );

        // (2) dormant aging tick: advancing an existing snapshot mutates its
        // age/position and must mark dirty.
        let mut age_app = App::new();
        age_app.add_event::<NpcDeathNotice>();
        age_app.add_event::<DormantCombatOutcome>();
        age_app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        age_app.insert_resource(GameTick(2400));
        age_app.insert_resource(ZoneRegistry {
            zones: vec![zone()],
        });
        age_app.insert_resource(WorldQiAccount::default());
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_age", DVec3::new(10.0, 64.0, 10.0)));
        // Clear the insert's dirty so we isolate the aging tick's effect.
        store.take_dirty();
        assert!(
            !store.is_dirty(),
            "test setup invariant: store must be clean before the aging tick so the tick is the only thing that can re-dirty it"
        );
        age_app.insert_resource(store);
        age_app.add_systems(Update, dormant_global_tick_system);
        age_app.update();
        assert!(
            age_app.world().resource::<NpcDormantStore>().is_dirty(),
            "expected the store to be dirty after a dormant aging tick advanced a snapshot (age/position changed), so the new state is persisted; it stayed clean"
        );

        // (3) death/removal: an expired snapshot whose qi is fully released is
        // removed by the tick; that removal must mark dirty.
        let mut death_app = App::new();
        death_app.add_event::<NpcDeathNotice>();
        death_app.add_event::<DormantCombatOutcome>();
        death_app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        death_app.insert_resource(GameTick(1));
        death_app.insert_resource(ZoneRegistry {
            zones: vec![zone()],
        });
        death_app.insert_resource(WorldQiAccount::default());
        let mut expired = snapshot("npc_dead", DVec3::new(10.0, 64.0, 10.0));
        expired.cultivation.qi_current = 0.0;
        expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
        let mut death_store = NpcDormantStore::default();
        death_store.insert(expired);
        death_store.take_dirty();
        death_app.insert_resource(death_store);
        death_app.add_systems(Update, dormant_global_tick_system);
        death_app.update();
        let after_death = death_app.world().resource::<NpcDormantStore>();
        assert!(
            after_death.is_empty(),
            "death tick precondition failed: expired zero-qi snapshot should have been removed before checking dirty"
        );
        assert!(
            after_death.is_dirty(),
            "expected the store to be dirty after death removed an expired snapshot, so the deletion is persisted; it stayed clean"
        );
    }

    /// P1 contract: `take_dirty` reads-and-clears in one step. After taking, an
    /// unchanged store reports clean and a second take returns false — the gate
    /// will not re-write Redis on a cycle where nothing changed.
    #[test]
    fn dormant_store_clean_after_take_dirty() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
        assert!(
            store.is_dirty(),
            "expected the store to be dirty right after an insert because the snapshot is unpersisted; it was clean"
        );

        assert!(
            store.take_dirty(),
            "expected take_dirty to return the prior dirty value (true) because an insert had occurred; it returned false"
        );
        assert!(
            !store.is_dirty(),
            "expected the store to be clean immediately after take_dirty consumed the flag; it still reported dirty"
        );
        assert!(
            !store.take_dirty(),
            "expected a second take_dirty with no intervening mutation to return false because the gate was already cleared; it returned true"
        );
    }

    /// P1 saturation: every store mutator raises dirty (insert AND remove), and
    /// the Redis restore path does NOT (loaded snapshots are already
    /// persisted). Also pins the de-dup behaviour: many mutations before one
    /// take still need only a single take to clear.
    #[test]
    fn dormant_store_dirty_per_mutator_and_clean_on_restore() {
        // insert raises dirty.
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));
        assert!(
            store.is_dirty(),
            "expected insert to mark the store dirty so the new snapshot is persisted; it did not"
        );

        // remove raises dirty (after clearing the insert's flag).
        store.take_dirty();
        let removed = store.remove("npc_a");
        assert!(
            removed.is_some(),
            "test setup invariant: the snapshot inserted above must exist so remove actually deletes it"
        );
        assert!(
            store.is_dirty(),
            "expected remove of an existing snapshot to mark the store dirty so the deletion is persisted; it did not"
        );

        // remove of a missing id must NOT raise dirty (nothing changed).
        store.take_dirty();
        let missing = store.remove("nope");
        assert!(
            missing.is_none(),
            "test setup invariant: removing an absent id must report None"
        );
        assert!(
            !store.is_dirty(),
            "expected removing an absent id to leave the store clean because nothing changed; it falsely marked dirty"
        );

        // Many mutations before a single take: one take clears them all.
        store.insert(snapshot("npc_b", DVec3::new(11.0, 64.0, 10.0)));
        store.insert(snapshot("npc_c", DVec3::new(12.0, 64.0, 10.0)));
        store.mark_dirty();
        assert!(
            store.take_dirty(),
            "expected take_dirty to return true after several mutations accumulated under one flag; it returned false"
        );
        assert!(
            !store.is_dirty(),
            "expected one take_dirty to clear the flag regardless of how many mutations preceded it; it stayed dirty"
        );

        // Redis restore path must NOT dirty the store: snapshots loaded from
        // Redis are already persisted, so writing them straight back would be a
        // wasteful no-op churn (the very thing P1 removes).
        let source = snapshot("npc_loaded", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
        let entries = HashMap::from([(source.char_id.clone(), payload)]);
        let mut restore_store = NpcDormantStore::default();
        let count =
            load_dormant_snapshots_from_hash_entries(&mut restore_store, entries).expect("load");
        assert_eq!(
            count, 1,
            "restore precondition: exactly one snapshot should have loaded"
        );
        assert!(
            restore_store.contains("npc_loaded"),
            "restore precondition: the loaded snapshot should be present"
        );
        assert!(
            !restore_store.is_dirty(),
            "expected a store restored from Redis to be CLEAN because the data is already persisted; restoring marked it dirty and would trigger a redundant write-back"
        );
    }

    #[test]
    fn dormant_regen_moves_qi_through_ledger() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger)
            .expect("dormant regen should emit a transfer");

        assert_eq!(transfer.from, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(transfer.to, QiAccountId::npc("npc_a"));
        assert!(snapshot.cultivation.qi_current > 0.1);
        assert!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi
                < 0.8
        );
        assert!(
            (snapshot.qi_ledger_net - transfer.amount).abs() < f64::EPSILON,
            "qi_ledger_net must audit the same amount as the ledger transfer"
        );
        assert!(
            (ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
                - zones
                    .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi
                    * QI_ZONE_UNIT_CAPACITY)
                .abs()
                < 1e-9,
            "ledger zone balance must use absolute qi units matching normalized zone drain"
        );
    }

    #[test]
    fn dormant_regen_requires_open_meridian_flow() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        assert!(apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none());
        assert_eq!(snapshot.cultivation.qi_current, 0.1);
        assert_eq!(
            ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            0.0
        );
    }

    #[test]
    fn dormant_realm_label_uses_shared_schema_serializer() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.realm = Realm::Condense;

        assert_eq!(snapshot.realm_label(), "Condense");
    }

    #[test]
    fn expired_dormant_npc_releases_qi_to_zone() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 0.4;
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("death release should emit a transfer");

        assert_eq!(transfer.from, QiAccountId::npc("npc_a"));
        assert_eq!(transfer.to, QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME));
        assert_eq!(snapshot.cultivation.qi_current, 0.0);
        assert!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi
                > 0.8
        );
        assert!(
            (ledger.balance(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
                - zones
                    .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi
                    * QI_ZONE_UNIT_CAPACITY)
                .abs()
                < 1e-9,
            "ledger zone balance must use the same absolute qi unit as normalized zone state"
        );
    }

    #[test]
    fn death_qi_release_preserves_zone_overflow_on_npc_account() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        let mut full_zone = zone();
        full_zone.spirit_qi = 0.99;
        let mut zones = ZoneRegistry {
            zones: vec![full_zone],
        };
        let mut ledger = WorldQiAccount::default();

        let transfer = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("near-full zone should still accept the available room");

        assert!((transfer.amount - 0.5).abs() < 1e-9);
        assert!((snapshot.cultivation.qi_current - 1.5).abs() < 1e-9);
        assert!(
            (ledger.balance(&QiAccountId::npc("npc_a")) - snapshot.cultivation.qi_current).abs()
                < 1e-9,
            "overflow must stay in the NPC ledger account instead of disappearing"
        );
    }

    #[test]
    fn dormant_global_tick_retains_expired_snapshot_until_qi_fully_released() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(GameTick(1));
        let mut full_zone = zone();
        full_zone.spirit_qi = 0.99;
        app.insert_resource(ZoneRegistry {
            zones: vec![full_zone],
        });
        app.insert_resource(WorldQiAccount::default());
        let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        expired.cultivation.qi_current = 2.0;
        expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
        let mut store = NpcDormantStore::default();
        store.insert(expired);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        let retained = store
            .snapshots
            .get("npc_a")
            .expect("overflowing qi must keep expired dormant NPC for retry");
        assert!((retained.cultivation.qi_current - 1.5).abs() < 1e-9);
        let events = app.world().resource::<Events<NpcDeathNotice>>();
        assert!(
            events.iter_current_update_events().next().is_none(),
            "death notice must wait until qi release reaches zero"
        );
    }

    #[test]
    fn dormant_wander_uses_absolute_tick_salt() {
        let mut snapshot = snapshot("npc_a", DVec3::ZERO);
        snapshot.intent = DormantBehaviorIntent::Wander {
            drift_radius: 10_000.0,
        };
        let start = snapshot.position_vec();

        advance_dormant_position(&mut snapshot, 1200, 1200);
        let first = snapshot.position_vec();
        advance_dormant_position(&mut snapshot, 1200, 2400);
        let second = snapshot.position_vec();

        let straight_line_second = DVec3::new(
            start.x + (first.x - start.x) * 2.0,
            start.y,
            start.z + (first.z - start.z) * 2.0,
        );
        assert!(
            planar_distance(second, straight_line_second) > 1e-6,
            "wander angle must vary across absolute ticks instead of repeating one straight-line heading"
        );
    }

    #[test]
    fn dormant_global_tick_clears_indexes_when_all_snapshots_expire() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(GameTick(1));
        app.insert_resource(ZoneRegistry {
            zones: vec![zone()],
        });
        app.insert_resource(WorldQiAccount::default());
        let mut expired = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
        let mut store = NpcDormantStore::default();
        store.insert(expired);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        assert!(store.is_empty());
        assert!(store.ids_by_archetype(NpcArchetype::Rogue).is_empty());
        assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
    }

    #[test]
    fn dormant_global_tick_refreshes_zone_index_after_movement() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(GameTick(2400));
        let second_zone = Zone {
            name: "east".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(120.0, 0.0, 0.0), DVec3::new(200.0, 128.0, 80.0)),
            spirit_qi: 0.5,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
        };
        app.insert_resource(ZoneRegistry {
            zones: vec![zone(), second_zone],
        });
        app.insert_resource(WorldQiAccount::default());
        let mut mover = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        mover.intent = DormantBehaviorIntent::PatrolToward {
            target: [130.0, 64.0, 10.0],
        };
        let mut store = NpcDormantStore::default();
        store.insert(mover);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        assert_eq!(store.snapshots["npc_a"].zone_name, "east");
        assert!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME).is_empty());
        assert_eq!(store.ids_by_zone("east"), &["npc_a"]);
    }

    #[test]
    fn dormant_breakthrough_uses_cultivation_rules_below_duxu() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.realm = Realm::Awaken;
        snapshot.cultivation.qi_current = 20.0;
        snapshot.cultivation.qi_max = 100.0;
        snapshot.lifespan.age_ticks = 1_100.0;
        open_regular_meridians(&mut snapshot, 3);
        let mut roll = FixedRoll(0.0);

        let result = advance_dormant_breakthrough_with_roll(
            &mut snapshot,
            Some(MIN_ZONE_QI_TO_BREAKTHROUGH),
            1200,
            &mut roll,
        )
        .expect("eligible dormant NPC should attempt breakthrough")
        .expect("fixed low roll should pass");

        assert_eq!(result.to, Realm::Induce);
        assert_eq!(snapshot.cultivation.realm, Realm::Induce);
        assert_eq!(snapshot.cultivation.qi_current, 12.0);
        assert_eq!(
            snapshot.shared_lifespan.cap_by_realm,
            LifespanCapTable::INDUCE
        );
        assert!((snapshot.lifespan.max_age_ticks - 1_666.666_666_666_666_7).abs() < 1e-9);
        assert!(!snapshot.lifespan.is_expired());
        assert!(snapshot.life_record.biography.iter().any(|entry| {
            matches!(
                entry,
                BiographyEntry::BreakthroughSucceeded {
                    realm: Realm::Induce,
                    tick: 1200
                }
            )
        }));
    }

    #[test]
    fn redis_payload_roundtrips_snapshot() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0)));

        let payloads = store.to_redis_hash_payloads().expect("serialize");
        assert_eq!(payloads[0].0, "npc_a");
        let decoded: NpcDormantSnapshot =
            serde_json::from_str(payloads[0].1.as_str()).expect("deserialize");
        assert_eq!(decoded.char_id, "npc_a");
        assert_eq!(decoded.position, [10.0, 64.0, 10.0]);
    }

    #[test]
    fn loads_dormant_snapshots_from_redis_hash_entries() {
        let source = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
        let entries = HashMap::from([(source.char_id.clone(), payload)]);
        let mut store = NpcDormantStore::default();

        let count = load_dormant_snapshots_from_hash_entries(&mut store, entries).expect("load");

        assert_eq!(count, 1);
        assert!(store.contains("npc_a"));
        assert_eq!(store.ids_by_zone(DEFAULT_SPAWN_ZONE_NAME), &["npc_a"]);
    }

    #[test]
    fn redis_hash_restore_skips_bad_entries_without_losing_good_snapshots() {
        let source = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&source).expect("serialize dormant snapshot");
        let entries = HashMap::from([
            (source.char_id.clone(), payload),
            ("npc_bad".to_string(), "{not-json".to_string()),
        ]);
        let mut store = NpcDormantStore::default();

        let count = load_dormant_snapshots_from_hash_entries(&mut store, entries).expect("load");

        assert_eq!(count, 1);
        assert!(store.contains("npc_a"));
        assert!(!store.contains("npc_bad"));
    }

    #[test]
    fn redis_hash_restore_fails_when_every_entry_is_invalid() {
        let entries = HashMap::from([("npc_bad".to_string(), "{not-json".to_string())]);
        let mut store = NpcDormantStore::default();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("all invalid entries should fail restore");

        assert!(error.contains("all 1 dormant Redis snapshot"));
        assert!(store.is_empty());
    }

    // ── plan-offscreen-war-v1 P0：确定性 env 旋钮 ─────────────────────────
    //
    // 解析逻辑用纯函数 `parse_*` 测，避免 `std::env::set_var` 在并行测试间互相污染
    // 全局进程状态（vitest/cargo test 默认多线程）。env 通路本身由真服 e2e 覆盖。

    #[test]
    fn bong_dormant_tick_interval_env_overrides_default() {
        // 合法正值覆盖默认 1200，让离屏 tick 快进到秒级（e2e 用此把 60s 压到 1 tick）。
        assert_eq!(
            parse_dormant_tick_interval(Some("5"), DORMANT_LIFECYCLE_TICK_INTERVAL),
            5,
            "正值 BONG_DORMANT_TICK_INTERVAL 必须覆盖默认 1200，否则 e2e 无法快进离屏 tick"
        );
        assert_eq!(
            parse_dormant_tick_interval(Some("  42  "), DORMANT_LIFECYCLE_TICK_INTERVAL),
            42,
            "首尾空白应被 trim 后解析，期望 42"
        );
    }

    #[test]
    fn bong_dormant_tick_interval_unset_keeps_default() {
        assert_eq!(
            parse_dormant_tick_interval(None, DORMANT_LIFECYCLE_TICK_INTERVAL),
            DORMANT_LIFECYCLE_TICK_INTERVAL,
            "env 未设时必须保持默认 1200（= 现有运行时行为）"
        );
        // from_env 默认（未注入 env 旋钮的字段）应与 default 一致。
        let cfg = NpcVirtualizationConfig::default();
        assert_eq!(
            cfg.dormant_tick_interval_ticks,
            DORMANT_LIFECYCLE_TICK_INTERVAL
        );
        assert_eq!(
            cfg.sim_seed, 0,
            "默认 sim_seed 必须为 0 = 现有 AbstractCombatSeed 行为"
        );
    }

    #[test]
    fn bong_dormant_tick_interval_zero_and_garbage_fall_back_gracefully() {
        // 0 非法（会造成 is_multiple_of(0) panic / 除零语义），必须回退默认而非采纳。
        assert_eq!(
            parse_dormant_tick_interval(Some("0"), DORMANT_LIFECYCLE_TICK_INTERVAL),
            DORMANT_LIFECYCLE_TICK_INTERVAL,
            "0 是非法 tick 间隔，期望 graceful 回退默认 1200"
        );
        // 垃圾 / 负数 / 溢出均无法 parse 为 u32，回退默认。
        for garbage in ["", "abc", "-3", "3.5", "99999999999999999999"] {
            assert_eq!(
                parse_dormant_tick_interval(Some(garbage), DORMANT_LIFECYCLE_TICK_INTERVAL),
                DORMANT_LIFECYCLE_TICK_INTERVAL,
                "非法值 {garbage:?} 期望回退默认 1200 而非 panic"
            );
        }
    }

    #[test]
    fn bong_sim_seed_makes_combat_deterministic() {
        // P0 是 plumbing 层：相同 BONG_SIM_SEED 解析出相同 u64，注入 AbstractCombatSeed
        // 后即可让 P1/P2 的 RNG 序列复现（同 seed → 同战死结果）。
        let seed_a = parse_sim_seed(Some("123456789"));
        let seed_b = parse_sim_seed(Some("123456789"));
        assert_eq!(
            seed_a, seed_b,
            "同一 BONG_SIM_SEED 必须解析出同值，否则离屏战争结果不可复现"
        );
        assert_eq!(seed_a, 123_456_789);

        // 不同 seed 必须区分（让 e2e 能跑出不同 RNG 序列）。
        assert_ne!(
            parse_sim_seed(Some("1")),
            parse_sim_seed(Some("2")),
            "不同 seed 必须解析为不同值"
        );

        // 注入路径：AbstractCombatSeed(seed) 与 NpcVirtualizationConfig.sim_seed
        // 必须携带同一个解析出的 seed，P1/P2 才能读同一个种子。
        let parsed = parse_sim_seed(Some("777"));
        let combat_seed = crate::npc::abstract_combat_system::AbstractCombatSeed(parsed);
        let cfg = NpcVirtualizationConfig {
            sim_seed: parsed,
            ..NpcVirtualizationConfig::default()
        };
        assert_eq!(
            combat_seed.0, cfg.sim_seed,
            "AbstractCombatSeed 与 config.sim_seed 必须同源，否则 abstract 与 dormant 战斗 RNG 不同步"
        );
    }

    #[test]
    fn bong_sim_seed_unset_or_garbage_defaults_to_zero() {
        assert_eq!(parse_sim_seed(None), 0, "env 未设时默认种子 0 = 现有行为");
        for garbage in ["", "abc", "-1", "1.0"] {
            assert_eq!(
                parse_sim_seed(Some(garbage)),
                0,
                "非法 seed {garbage:?} 期望回退 0 而非 panic"
            );
        }
    }

    // ── plan-offscreen-war-v1 P0 #1：派系数据化 bootstrap ─────────────────

    #[test]
    fn seed_rogue_faction_is_deterministic_per_char_id() {
        // 同 char_id 跨调用必须分到同一派系（重启后 dormant 派系稳定）。
        let a = seed_rogue_faction("dormant:rogue:42");
        let b = seed_rogue_faction("dormant:rogue:42");
        assert_eq!(
            a.faction_id, b.faction_id,
            "同 char_id 必须确定性分派，否则重启后敌对关系漂移"
        );
        assert_eq!(a.rank, FactionRank::Disciple);
    }

    #[test]
    fn seed_rogue_faction_distribution_yields_both_attack_and_defend() {
        // 哈希分布必须同时产出 Attack 与 Defend，否则 is_hostile_pair 永远配不出对。
        let mut seen_attack = false;
        let mut seen_defend = false;
        for index in 0..256u32 {
            match seed_rogue_faction(&format!("dormant:rogue:{index}")).faction_id {
                FactionId::Attack => seen_attack = true,
                FactionId::Defend => seen_defend = true,
                FactionId::Neutral => {
                    panic!("seed 派系绝不应分到 Neutral（Neutral 对谁都不敌对，会让战斗空转）")
                }
            }
            if seen_attack && seen_defend {
                break;
            }
        }
        assert!(
            seen_attack && seen_defend,
            "256 个 char_id 必须同时出现 Attack 与 Defend，保证 is_hostile_pair 有敌对对"
        );
    }

    #[test]
    fn seed_rogue_faction_pairs_are_hostile_across_factions() {
        // 端到端契约：分到不同派系的两个 rogue，FactionStore::is_hostile_pair 必须为真。
        use crate::npc::faction::FactionStore;
        let store = FactionStore::default();
        let attacker = seed_rogue_faction("seed:attack-fixture");
        let defender = (0..64u32)
            .map(|i| seed_rogue_faction(&format!("seed:defend-fixture:{i}")))
            .find(|m| m.faction_id != attacker.faction_id)
            .expect("应能找到一个异派系成员");
        assert!(
            store.is_hostile_pair(attacker.faction_id, defender.faction_id),
            "Attack↔Defend 必须敌对，否则 P1/P2 战斗无候选对"
        );
        // 同派系不敌对（确认二分不会把同派系也当敌对）。
        assert!(
            !store.is_hostile_pair(attacker.faction_id, attacker.faction_id),
            "同派系不应敌对"
        );
    }

    #[test]
    fn dormant_rogue_seed_snapshot_assigns_non_none_faction() {
        // 防回归：seed 出来的 dormant rogue 的 faction 字段必须非 None
        // （否则 e2e HGETALL 看到 faction=null，所有后续阶段空转）。
        let zone = zone();
        let snapshot = dormant_rogue_seed_snapshot(&zone, 0, 0, 0, 0.8);
        let membership = snapshot
            .faction
            .as_ref()
            .expect("seeded dormant rogue 必须带 FactionMembership，不能是 None");
        assert!(
            matches!(membership.faction_id, FactionId::Attack | FactionId::Defend),
            "seed 派系必须是 Attack 或 Defend（非 Neutral），实际 {:?}",
            membership.faction_id
        );
    }

    // ── plan-offscreen-war-v1 P2：离屏派系互殴战死闭环（饱和单测） ─────────────
    //
    // 测契约不测实现：断言 store 人口回写 / 真元守恒回灌 / death notice 死因 / telemetry
    // outcome / 防吞真元 retain，全部走 `run_dormant_combat_phase` 这个真实结算入口
    // （而非私有中间步），接入面变了也不应红。

    use crate::qi_physics::{assert_conservation, QiAccountId, WorldQiAccount};

    /// 造一个**敌对、满真元、不会自然老死**的战斗候选快照：给定 char_id / 派系 / 真元。
    /// realm 拉到 Condense 让 condition_factor 由满血+满真元主导，战力 realm-monotonic。
    fn combat_snapshot(
        char_id: &str,
        faction: FactionId,
        qi_current: f64,
        pos: DVec3,
    ) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            realm: Realm::Condense,
            qi_current,
            qi_max: 60.0,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: DimensionKind::Overworld,
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            position: vec3_to_array(pos),
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: MeridianSystem::default(),
            meridian_severed: MeridianSeveredPermanent::default(),
            contamination: Contamination::default(),
            // 长寿命 + 0 起始年龄：本轮绝不自然老死，让"死人"唯一来源是战斗。
            lifespan: NpcLifespan::new(0.0, 1_000_000.0),
            shared_lifespan: LifespanComponent::for_realm(cultivation.realm),
            lifespan_extension_ledger: LifespanExtensionLedger::default(),
            death_registry: DeathRegistry::new(char_id),
            life_record: LifeRecord::new(char_id),
            faction: Some(FactionMembership {
                faction_id: faction,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            }),
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: qi_current,
            qi_ledger_net: 0.0,
        }
    }

    /// 跑一次完整 `dormant_global_tick_system`（含 combat phase）并收回本帧 death + outcome。
    ///
    /// 用真实 `App` 驱动整条系统（最贴近运行时），把传入的 store/zones/ledger 装进资源，
    /// `update()` 一帧，读回 `Events<NpcDeathNotice>` / `Events<DormantCombatOutcome>`，
    /// 再把更新后的 store/zones/ledger 写回调用方引用。tick 通过 `GameTick` 注入；
    /// `dormant_tick_interval_ticks=1` 保证整周期、必跑。返回 (deaths, outcomes)。
    ///
    /// 关键：所有 combat snapshot 的 `last_dormant_tick_processed=0`，本帧 aging 会推进它们；
    /// 但 combat snapshot 寿命 1_000_000 ticks，aging 不会触发自然老死，故死亡唯一来源是 combat。
    fn run_combat_tick(
        store: &mut NpcDormantStore,
        zones: &mut ZoneRegistry,
        ledger: &mut WorldQiAccount,
        config: &NpcVirtualizationConfig,
        tick: u64,
    ) -> (Vec<NpcDeathNotice>, Vec<DormantCombatOutcome>) {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..config.clone()
        });
        app.insert_resource(GameTick(tick as u32));
        app.insert_resource(FactionStore::default());
        app.insert_resource(std::mem::take(store));
        app.insert_resource(ZoneRegistry {
            zones: std::mem::take(&mut zones.zones),
        });
        app.insert_resource(std::mem::take(ledger));
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let deaths = app
            .world_mut()
            .resource_mut::<Events<NpcDeathNotice>>()
            .drain()
            .collect::<Vec<_>>();
        let outcomes = app
            .world_mut()
            .resource_mut::<Events<DormantCombatOutcome>>()
            .drain()
            .collect::<Vec<_>>();

        // 写回更新后的资源到调用方引用。
        *store = app
            .world_mut()
            .remove_resource::<NpcDormantStore>()
            .unwrap();
        *zones = app.world_mut().remove_resource::<ZoneRegistry>().unwrap();
        *ledger = app.world_mut().remove_resource::<WorldQiAccount>().unwrap();
        (deaths, outcomes)
    }

    #[test]
    fn combat_death_releases_all_qi_to_zone() {
        // 一对敌对 dormant 在同 zone：战死一方的真元应**守恒回灌**给 zone（zone.spirit_qi 上升），
        // 且回灌量 == ledger transfer amount == outcome.qi_released。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        }; // spirit_qi=0.8
        let zone_before = zones.zones[0].spirit_qi;
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot(
            "atk",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot(
            "def",
            FactionId::Defend,
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        store.take_dirty();
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let (deaths, outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(
            deaths.len(),
            1,
            "一对敌对 dormant 必须恰好死一个（战斗必致死），实际死 {} 个",
            deaths.len()
        );
        assert_eq!(
            outcomes.len(),
            1,
            "恰好一条战果 outcome 对应这场战死，实际 {} 条",
            outcomes.len()
        );
        let zone_after = zones.zones[0].spirit_qi;
        assert!(
            zone_after > zone_before,
            "战死方真元必须守恒回灌 zone（spirit_qi 上升）：before={zone_before} after={zone_after}"
        );
        // 回灌量精确对账：zone 上升的归一化量 × 容量 == outcome.qi_released。
        let zone_gain_abs = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (zone_gain_abs - outcomes[0].qi_released).abs() < 1e-9,
            "zone 真元增量（{zone_gain_abs}）必须等于 outcome.qi_released（{}），否则 telemetry 与实际守恒不符",
            outcomes[0].qi_released
        );
        assert!(
            outcomes[0].qi_released > 0.0,
            "本场战死应有真元回灌（败者满真元 + zone 未满），qi_released={}",
            outcomes[0].qi_released
        );
    }

    #[test]
    fn combat_death_emits_notice_with_combat_reason_and_pos() {
        // 战死 notice 必须 reason=Combat + from_dormant_combat=true + pos=Some(战场坐标)，
        // 让 agent / e2e 能把战死与自然老死区分开。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let loser_pos = DVec3::new(11.0, 64.0, 11.0);
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot(
            "atk",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot("def", FactionId::Defend, 5.0, loser_pos));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let (deaths, outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        let notice = &deaths[0];
        assert_eq!(
            notice.reason,
            NpcDeathReason::Combat,
            "战死 notice 的 reason 必须是 Combat（非 NaturalAging），否则 agent 当成老死，实际 {:?}",
            notice.reason
        );
        assert!(
            notice.from_dormant_combat,
            "战死 notice 的 from_dormant_combat 必须为 true（区别于在场战斗 / 老死）"
        );
        assert!(
            notice.pos.is_some(),
            "战死 notice 必须带 pos（战场坐标），供 agent 派系战报定位 / e2e 断言"
        );
        // notice 的死者就是 outcome 的 loser，且 pos 来自该败者快照。
        assert_eq!(
            notice.npc_id, outcomes[0].loser,
            "death notice 的 npc_id 必须 == outcome.loser（同一场战死的两面）"
        );
    }

    #[test]
    fn combat_death_removes_loser_from_store() {
        // 战死方真元全释放后必须从 store 移除（人口回写），胜者保留。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot(
            "atk",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot(
            "def",
            FactionId::Defend,
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let (deaths, _outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(
            store.len(),
            1,
            "两个 dormant 战斗后必须剩 1 个（败者真元全释放 → 移除、胜者留），实际剩 {}",
            store.len()
        );
        let loser = &deaths[0].npc_id;
        assert!(
            !store.contains(loser),
            "战死方 `{loser}` 必须已从 store 移除"
        );
        // 剩下的就是胜者。
        let winner_id = if loser == "atk" { "def" } else { "atk" };
        assert!(
            store.contains(winner_id),
            "胜者 `{winner_id}` 必须仍在 store（未参与死亡）"
        );
    }

    #[test]
    fn winner_qi_unchanged() {
        // dormant 简化：胜者真元不变（未流动即未失衡，§10.1 #5 ③）。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot(
            "atk",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot(
            "def",
            FactionId::Defend,
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let (deaths, _outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        let loser = &deaths[0].npc_id;
        let winner_id = if loser == "atk" { "def" } else { "atk" };
        let winner = store.snapshots.get(winner_id).expect("胜者必须仍在 store");
        assert!(
            (winner.cultivation.qi_current - 5.0).abs() < 1e-9,
            "胜者真元必须保持 5.0 不变（dormant 简化不扣胜者），实际 {}",
            winner.cultivation.qi_current
        );
    }

    #[test]
    fn zone_full_retains_loser_until_released() {
        // 防吞真元：zone 已满（spirit_qi=1.0 → 容量 QI_ZONE_UNIT_CAPACITY 已满）时，败者真元
        // 无处可去 → overflow 留败者账户 → 本轮**不**移除（retain-until-released），下轮重试。
        let mut full_zone = zone();
        full_zone.spirit_qi = 1.0; // 满到 QI_ZONE_UNIT_CAPACITY，不接受任何回灌
        let mut zones = ZoneRegistry {
            zones: vec![full_zone],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot(
            "atk",
            FactionId::Attack,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot(
            "def",
            FactionId::Defend,
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let (deaths, outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(
            deaths.len(),
            1,
            "战斗仍发生、败者仍 roll 出（death notice 仍 emit），实际 {} 条",
            deaths.len()
        );
        assert!(
            (outcomes[0].qi_released - 0.0).abs() < 1e-9,
            "zone 满 → 本场回灌 0（败者真元无处可去），qi_released={}",
            outcomes[0].qi_released
        );
        assert_eq!(
            store.len(),
            2,
            "防吞真元：zone 满时败者真元未释放，本轮绝不移除（否则携带真元的快照被丢弃 = 吞真元红线），\
             store 仍 2 个，实际 {}",
            store.len()
        );
        let loser = &deaths[0].npc_id;
        let retained = store
            .snapshots
            .get(loser)
            .expect("败者真元未释放完，必须保留待下轮重试");
        assert!(
            retained.cultivation.qi_current > QI_EPSILON,
            "保留的败者必须仍携带未释放真元（{}），下轮 zone 腾出空间再释放",
            retained.cultivation.qi_current
        );
    }

    #[test]
    fn sequential_release_no_overflow() {
        // 同 zone 多败者：顺序 release（先 release 抬高 zone_qi，后者读更高基线），
        // 总回灌量受 zone 容量 clamp、不溢出。两对敌对 dormant → 两个败者同 zone 回灌。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        }; // spirit_qi=0.8
        let zone_before = zones.zones[0].spirit_qi;
        let mut store = NpcDormantStore::default();
        // 两对：a-b、c-d，全在同 zone（升序两两配对 → (a,b)+(c,d)）。
        store.insert(combat_snapshot(
            "a",
            FactionId::Attack,
            3.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot(
            "b",
            FactionId::Defend,
            3.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        store.insert(combat_snapshot(
            "c",
            FactionId::Attack,
            3.0,
            DVec3::new(12.0, 64.0, 12.0),
        ));
        store.insert(combat_snapshot(
            "d",
            FactionId::Defend,
            3.0,
            DVec3::new(13.0, 64.0, 13.0),
        ));
        let mut ledger = WorldQiAccount::default();
        // max_combats_per_zone 默认 3 ≥ 2，足够配出两对。
        let config = NpcVirtualizationConfig::default();

        let (deaths, outcomes) = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(
            deaths.len(),
            2,
            "两对敌对 dormant 必须死两个（每对死一个），实际 {}",
            deaths.len()
        );
        let zone_after = zones.zones[0].spirit_qi;
        assert!(
            zone_after <= 1.0 + 1e-9,
            "顺序回灌后 zone.spirit_qi 必须 ≤ 1.0（容量 clamp，不溢出），实际 {zone_after}"
        );
        // 两条 outcome 的 qi_released 之和 == zone 实际上升的绝对量（顺序无关、不丢不溢）。
        let total_released: f64 = outcomes.iter().map(|o| o.qi_released).sum();
        let zone_gain_abs = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
        assert!(
            (total_released - zone_gain_abs).abs() < 1e-9,
            "两个败者顺序回灌总量（{total_released}）必须精确等于 zone 上升绝对量（{zone_gain_abs}），\
             否则顺序 release 有丢失 / 重复计数",
        );
    }

    #[test]
    fn offscreen_war_conserves_total_qi() {
        // 守恒集成测试（§10.1 #5 守恒红线的诚实锁）：seed 大量敌对 dormant，跑多轮 combat tick，
        // 断言**账本（WorldQiAccount）总量严格守恒**（tolerance 0.0，本场景无 EraDecay）。
        //
        // 为什么锁账本而非 summarize_world_qi().total_observed()：经实测（见报告），
        // `total_observed = player + zone(归一化分量) + container + ledger` 把 zone 同时计入
        // ZoneRegistry 归一化分量 *和* ledger 的 zone 绝对账户两处，单位不同（归一化 vs ×50），
        // 释放时 zone.spirit_qi 上升 accepted/50 而 ledger zone 上升 accepted，故 total_observed
        // 会随每次 release 漂移 accepted/50——这是 summarize 的双计入，**不是** combat 路径造/吞
        // 真元。真正的守恒不变量是**账本双录簿总量**：败者真元经
        // `ledger.transfer(npc→zone, ReleaseToZone)` 真实搬运（零和），release 前的 set_balance
        // 把账户重新对齐到 snapshot/zone 现状（与 apply_dormant_regen 同源），全程无任何
        // `balance += X` 凭空注入、无任何携带真元的快照被直接丢弃。
        //
        // 因此守恒锁 = ① 预先把每个 dormant 的真元 + 每个 zone 的真元funded 进账本（counted
        // source）→ 拍 before；② 跑多轮 combat；③ 拍 after；④ assert_conservation(before,
        // after, 0.0) 严格相等。若 combat 路径凭空造/吞真元，账本总量必然漂移、撞红。

        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        }; // spirit_qi=0.8
        let mut store = NpcDormantStore::default();
        // ~500 dormant：250 对敌对（Attack/Defend 交替），全在同一个 zone。
        // 每个带 4.0 真元（足够多轮释放但不会一轮把 zone 灌满到完全 retain）。
        const N: u32 = 500;
        for i in 0..N {
            let faction = if i % 2 == 0 {
                FactionId::Attack
            } else {
                FactionId::Defend
            };
            // char_id 零填充让升序 == 数值升序，配对稳定。
            store.insert(combat_snapshot(
                &format!("w{i:04}"),
                faction,
                4.0,
                DVec3::new(10.0 + (i % 50) as f64, 64.0, 10.0 + (i / 50) as f64),
            ));
        }

        let mut ledger = WorldQiAccount::default();
        // ① 把每个 dormant 的真元 + zone 的真元 funded 进账本，作为 counted source。
        for snap in store.snapshots.values() {
            ledger
                .set_balance(
                    QiAccountId::npc(snap.char_id.clone()),
                    snap.cultivation.qi_current,
                )
                .expect("seed npc ledger balance");
        }
        ledger
            .set_balance(
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                zones.zones[0].spirit_qi * QI_ZONE_UNIT_CAPACITY,
            )
            .expect("seed zone ledger balance");

        let before = ledger_conservation_snapshot(&ledger);
        let initial_pop = store.len();

        // ② 多轮 combat tick（不同 tick → 不同 RNG，多对战死，含 retain-until-released 路径）。
        let config = NpcVirtualizationConfig {
            // 每轮多配几对，加速种群消耗，跑满多轮结算。
            max_combats_per_zone: 64,
            sim_seed: 20_260_531,
            ..Default::default()
        };
        let mut total_deaths = 0usize;
        for round in 0..10u64 {
            let (deaths, _outcomes) =
                run_combat_tick(&mut store, &mut zones, &mut ledger, &config, round + 1);
            total_deaths += deaths.len();
            // 每轮断言：账本总量始终严格守恒（不止 before/after，每一步都不漂）。
            let mid = ledger_conservation_snapshot(&ledger);
            assert_conservation(&before, &mid, 0.0).unwrap_or_else(|err| {
                panic!(
                    "round {round} 后账本守恒漂移（造/吞真元红线）：{err}；\
                     before_total={} mid_total={} deaths_so_far={total_deaths} pop={}",
                    before.total_observed(),
                    mid.total_observed(),
                    store.len(),
                )
            });
        }

        let after = ledger_conservation_snapshot(&ledger);
        // ③ 最终严格守恒（tolerance 0.0）。
        assert_conservation(&before, &after, 0.0).unwrap_or_else(|err| {
            panic!(
                "离屏战争多轮后账本总量未严格守恒：{err}；before={} after={} t0_pop={initial_pop} t1_pop={} deaths={total_deaths}",
                before.total_observed(),
                after.total_observed(),
                store.len(),
            )
        });
        // ④ 确实发生了离屏战死（否则守恒是空过的）。
        assert!(
            total_deaths > 0,
            "多轮 combat 必须真的死了人（否则守恒断言空过），实际死亡数 {total_deaths}"
        );
        // ⑤ 种群守恒：移除数（initial - 现存）∈ [0, total_deaths]，且不超过死亡数
        // （retain-until-released 可能让某些战死者本轮仍在 store，故移除 ≤ 死亡）。
        let removed_pop = initial_pop - store.len();
        assert!(
            removed_pop <= total_deaths,
            "移除的人口（{removed_pop}）不应超过战死数（{total_deaths}）——多出来 = 凭空消失了 NPC"
        );
        // ⑥ 防吞真元最终态：store 里残留的快照真元都已被账本如实记账（counted），
        // 没有任何携带真元的快照被直接丢弃。
        for snap in store.snapshots.values() {
            let ledger_bal = ledger.balance(&QiAccountId::npc(snap.char_id.clone()));
            assert!(
                (ledger_bal - snap.cultivation.qi_current).abs() < 1e-9,
                "存活 dormant `{}` 的账本余额（{ledger_bal}）必须等于快照真元（{}），\
                 否则账本与快照脱节（守恒不可信）",
                snap.char_id,
                snap.cultivation.qi_current,
            );
        }
    }

    /// 从账本构造一个守恒快照：把整个 `WorldQiAccount` 总量放进 `ledger_qi`，其余分量置 0。
    /// 这样 `total_observed()` == 账本双录簿总量，是 combat release 路径真正守恒的量
    /// （不掺 ZoneRegistry 归一化分量的双计入，见 `offscreen_war_conserves_total_qi` 注释）。
    fn ledger_conservation_snapshot(ledger: &WorldQiAccount) -> crate::qi_physics::WorldQiSnapshot {
        crate::qi_physics::WorldQiSnapshot {
            player_qi: 0.0,
            zone_qi: 0.0,
            container_qi: 0.0,
            ledger_qi: ledger.total(),
            era_decay_accum: 0.0,
            budget_initial_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
            budget_current_total: crate::qi_physics::constants::DEFAULT_SPIRIT_QI_TOTAL,
        }
    }
}
