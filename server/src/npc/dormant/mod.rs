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

/// plan-offscreen-war-v1 P3 交付物 3+4：克制式战场遗物的 deferred-on-hydrate 物化
/// （玩家靠近 zone → 读 sqlite pending relic → 零真元物化成地面 loot + VFX/audio/narration）。
pub mod relic_hydrate;

/// plan-offscreen-war-v1 P5：散修群体消长 census（纯逻辑：人口 / 众数 zone / 涌现强者 / 消长态）。
/// 全只读 store + faction store，零真元流动——telemetry publish 在 network 层消费其产出。
pub mod census;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use valence::prelude::{
    bevy_ecs, App, DVec3, Event, EventWriter, Res, ResMut, Resource, Startup, Update,
};

use crate::body_plan::{resolve_race_to_plan, BodyPlanRegistry, RaceRegistry};
use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, next_realm, qi_max_for_realm, try_breakthrough_with_profile,
    BreakthroughError, BreakthroughSuccess, RollSource, XorshiftRoll, MIN_ZONE_QI_TO_BREAKTHROUGH,
    MIN_ZONE_QI_TO_GUYUAN,
};
use crate::cultivation::components::{
    ActorQiIdentity, ActorQiKind, Contamination, Cultivation, MeridianSystem,
    PersistedCultivationV1, QiFlowError, QiFlowOutcome, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::npc::faction::{
    leader_realm_for, named_faction_id_for_legacy, EmergentGroupId, FactionId, FactionMembership,
    FactionRank, FactionStore, MissionQueue, Reputation, EMERGENT_GROUP_COUNT,
};
use crate::npc::interaction_memory::NpcMemoryComponent;
use crate::npc::lifecycle::{NpcArchetype, NpcDeathNotice, NpcDeathReason, NpcLifespan};
use crate::npc::loot::default_loot_for_archetype;
use crate::npc::loot::NpcLootTable;
use crate::npc::movement::GameTick;
use crate::npc::realm_perception_narration::push_realm_perception_narration;
use crate::npc::schedule::schedule_seed_from_char_id;
use crate::npc::spawn::{classify_zones_by_qi, initial_age_for_index};
use crate::npc::trade::NpcPlayerReputation;
use crate::player::gameplay::PendingGameplayNarrations;
#[cfg(test)]
use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
use crate::qi_physics::{
    constants::{QI_EPSILON, QI_NPC_ABSORB_FLOOR},
    regen_from_zone, QiTransfer, QiTransferReason, WorldQiAccount,
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
/// **dev/test-only 随机种子旋钮**——只决定 dormant 战斗 RNG 的初值
/// （`NpcVirtualizationConfig.sim_seed`），**不**改变守恒：真元流动仍走 `release_dormant_qi_to_zone` →
/// `WorldQiAccount` 审计。env 未设时保持现状默认（种子 0）。
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

/// plan-tsy-sentinel-dormant-regression-v1 §P1：TSY 秘境守灵（`TsySentinelMarker`）身份载荷。
///
/// 不另开 `family_id` 字段——`spawn_tsy_sentinel_at` 为同一实体同时插入
/// `TsyHostileMarker{family_id}` 与 `TsySentinelMarker{family_id}`（两值恒相等），且
/// `dormant_tsy_hostile_snapshot` 只在 `TsyHostileMarker` 存在时才返回 `Some`——因此任意
/// 实体只要 `snapshot.tsy_sentinel.is_some()`，`snapshot.tsy_hostile` 必为 `Some`。hydrate
/// 重绑直接读 `snapshot.tsy_hostile.family_id` 做 family 过滤键（见 §8.1 #1 决议）。
///
/// `guarding_container_pos` 是重绑的稳定键（`family_id` + 坐标复合键，§8.1 #1）——不存
/// `Entity`（不可 serde + Redis 长期持久化下 generation 复用风险）。容器一旦放置永不移动，
/// 坐标 epsilon 匹配足够可靠。`None` 表示原 sentinel 无守护容器（不常见，仍需支持）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantTsySentinelSnapshot {
    pub guarding_container_pos: Option<[f64; 3]>,
    /// `max_phase`（设计常量，恒为 3）精确回填，稳定值无成本无风险。
    pub max_phase: u8,
    /// best-effort 展示值：hydrate 后 `update_sentinel_phase_system` 会在下一次运行按
    /// *当前*（满血）`Wounds` 重算并纠正，不存在持久错位（§8.1 #2 决议）。
    pub phase: u8,
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

fn serialize_dormant_cultivation<S>(
    cultivation: &Cultivation,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    PersistedCultivationV1::from(cultivation).serialize(serializer)
}

fn deserialize_dormant_cultivation<'de, D>(deserializer: D) -> Result<Cultivation, D::Error>
where
    D: Deserializer<'de>,
{
    let persisted = PersistedCultivationV1::deserialize(deserializer)?;
    Cultivation::try_from(persisted).map_err(serde::de::Error::custom)
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
    #[serde(
        serialize_with = "serialize_dormant_cultivation",
        deserialize_with = "deserialize_dormant_cultivation"
    )]
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
    pub memory: Option<NpcMemoryComponent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub player_reputation: Option<NpcPlayerReputation>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub faction: Option<FactionMembership>,
    /// plan-offscreen-war-v1 P5 reframe b：该 dormant 散修所属的涌现群体（匿名稳定 id）。
    ///
    /// 离屏战斗敌对判定的群体身份来源（§十灵气零和：不同群体争同 zone 灵气即敌对）。
    /// `#[serde(default)]` 让缺此字段的**旧持久化快照**反序列化为 `None`——届时由
    /// [`effective_group`] 回退 `faction` 派生（Attack→0 / Defend→1 / Neutral→None），
    /// 保证非破坏迁移。
    #[serde(default)]
    pub emergent_group: Option<EmergentGroupId>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub patrol: Option<DormantPatrolSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub loot_table: Option<NpcLootTable>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub guardian_relic: Option<DormantGuardianRelicSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tsy_hostile: Option<DormantTsyHostileSnapshot>,
    /// plan-tsy-sentinel-dormant-regression-v1 P1：TSY 秘境守灵身份载荷，`Some` 时 hydrate
    /// 路由必须走 `spawn_tsy_sentinel_at`（不得洗成普通 `spawn_relic_guard_npc_at`）。
    /// `#[serde(default)]` 非破坏迁移——旧快照反序列化为 `None`（退化为普通
    /// overworld `GuardianRelic`，这是修复前的既有行为，不引入新回归）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tsy_sentinel: Option<DormantTsySentinelSnapshot>,
    pub intent: DormantBehaviorIntent,
    pub dormant_since_tick: u64,
    pub last_dormant_tick_processed: u64,
    pub initial_qi: f64,
    pub qi_ledger_net: f64,
    /// plan-offscreen-war-v1 P3 review-fix：「已离屏战死、真元待释放」标记。
    ///
    /// 当 [`run_dormant_combat_phase`] roll 出败者但 typed settlement 遇到非法 signed Zone、身份
    /// 或稳定池 overflow 等硬事务失败时置 `true`，败者**仍留在 `store.snapshots`**（防吞真元红线：
    /// 携带真元的快照绝不丢弃；随 Redis 持久化，server 重启不丢真元）。置 `true` 后：
    /// - [`combat::collect_zone_combat_pairs`] 跳过该快照，**不再被选中参战**——故 death notice /
    ///   `DormantCombatOutcome` 每个逻辑死亡只 emit 一次（初次 roll 时），不再重复污染 P4 派系
    ///   死亡聚合（CodeRabbit Major：retained loser 重复 emit）。
    /// - 每 tick 的 [`run_pending_combat_release_retry`] 重试 `release_dormant_qi_to_zone`，真元
    ///   全释放（`<= QI_EPSILON`）后才 emit 遗物（若 `should_leave_relic`）+ 从 store 移除。
    ///
    /// `#[serde(default)]` 向后兼容旧 Redis 快照（缺字段 → `false`）；`skip_serializing_if`
    /// 让绝大多数（未战死）快照不写这个字段，不算 §10.1 #2 所禁的快照膨胀。
    #[serde(default, skip_serializing_if = "is_false")]
    pub combat_dead_pending_release: bool,
}

/// serde `skip_serializing_if` helper：`false`（默认值）时不序列化，避免快照膨胀。
fn is_false(value: &bool) -> bool {
    !*value
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
/// typed transaction 真实完成，本 event 不携带也不触发任何真元流动
/// （绝不学「emit QiTransfer 却无人 apply」的吞真元红线——那等于真元凭空蒸发）。
///
/// `qi_released` 是本场实际被 signed zone 接收的量（`QiFlowOutcome::zone_accepted`）。
/// zone 满或无法定位时可能为 0，但 typed settlement 会把余量真实转入固定
/// `qi_flow_overflow`；只有事务失败时败者才保留并进入 pending-release 重试。
#[derive(Clone, Debug, Event, PartialEq)]
pub struct DormantCombatOutcome {
    pub winner: CharId,
    pub loser: CharId,
    pub zone: String,
    pub qi_released: f64,
}

/// plan-offscreen-war-v1 P3：一名**克制判定通过**的离屏战死者要在战场留下的待物化遗物
/// （deferred-on-hydrate）的内部 event。
///
/// 由 `run_dormant_combat_phase` 在败者**真元已守恒释放完毕**（`release_dormant_qi_to_zone`
/// 之后、`store.snapshots.remove` 之前）且 [`combat::should_leave_relic`] 为真时 emit；
/// `persistence::persist_pending_dormant_relics_system` 消费它写进 sqlite `pending_dormant_relics`
/// 表。**守恒红线（§10.1 #5 ④）**：本 event 不携带任何真元——遗物 loot 物化时 `spirit_quality=0`，
/// 持久层完全不碰 `WorldQiAccount` / ledger。emit 时机严格在 release 之后保证「先把残余真元
/// 守恒还给 zone，再用快照创建零真元遗物」，绝无「先留遗物 / 先 remove、qi 没释放」的吞真元窗口。
///
/// `loot_seed` 是 [`combat::relic_loot_seed`] 算出的 deterministic 种子；玩家靠近 hydrate 时
/// 用它 `roll_loot(default_loot_for_archetype(archetype), loot_seed)`，保证遗物 loot 可复现。
/// `created_tick` 是逻辑结算 tick（deferred-on-hydrate 时序校验用）；`created_wall` 留给持久层
/// 填墙钟（TTL sweep 阈值用 wall-clock，不依赖逻辑 tick）。
#[derive(Clone, Debug, Event, PartialEq)]
pub struct PendingDormantRelicCreated {
    pub char_id: CharId,
    pub zone: String,
    pub position: [f64; 3],
    pub archetype: NpcArchetype,
    pub loot_seed: u64,
    pub created_tick: u64,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering dormant NPC store and batch tick");
    app.init_resource::<NpcDormantStore>()
        .insert_resource(NpcVirtualizationConfig::from_env())
        .insert_resource(DormantRoguePopulationSeedConfig::default())
        // plan-offscreen-war-v1 P5：群体消长 census 的上轮人口历史（telemetry publish 写回）。
        .init_resource::<census::LastFactionCensus>()
        .add_event::<DormantSeveredAt>()
        .add_event::<DormantCombatOutcome>()
        .add_event::<PendingDormantRelicCreated>()
        .add_systems(Startup, load_dormant_store_from_redis_system)
        .add_systems(
            Update,
            (
                // plan-npc-realm-distribution-v1 P3 §8.1 #3：存量迁移必须先于新种群 seed
                // 判定跑一次——两者互斥（迁移只动非空 store，seed 只在空 store 触发），
                // 排序本身不影响正确性，但让迁移先落地更符合"先修旧账再论新账"的直觉。
                migrate_dormant_realm_distribution_v1,
                seed_initial_dormant_population_on_startup,
                dormant_global_tick_system,
            ),
        );
    relic_hydrate::register(app);
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

    // Restore is one all-or-nothing owner transaction. A decoded dormant snapshot may carry
    // physical qi in `Cultivation`; accepting only the valid subset would silently drop every
    // owner represented by a corrupt row, and a later full-HASH publish could make that loss
    // permanent. Decode and validate the complete HASH before touching the live store.
    let mut staged = Vec::with_capacity(entries.len());
    let mut invalid = Vec::new();
    for (hash_char_id, payload) in entries {
        match serde_json::from_str::<NpcDormantSnapshot>(&payload) {
            Ok(snapshot) if snapshot.char_id == hash_char_id => staged.push(snapshot),
            Ok(snapshot) => invalid.push(format!(
                "`{hash_char_id}`: payload character id `{}` does not match HASH field",
                snapshot.char_id
            )),
            Err(error) => invalid.push(format!("`{hash_char_id}`: {error}")),
        }
    }
    if !invalid.is_empty() {
        return Err(format!(
            "refusing partial dormant Redis restore: {} of {} snapshot entries were invalid ({})",
            invalid.len(),
            staged.len() + invalid.len(),
            invalid.join("; ")
        ));
    }

    for snapshot in staged {
        store.snapshots.insert(snapshot.char_id.clone(), snapshot);
    }
    store.rebuild_indexes();
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
    mut pending_relics: EventWriter<PendingDormantRelicCreated>,
    war_bonus: Option<Res<crate::npc::war::settle::ZoneSpiritBonusStore>>,
    // plan-race-system-v1 P6b review major-4 收口：离屏突破配额换轨所需的两个解析
    // 资源，语义与在线 `breakthrough_system`/`cultivate_action_system` 同款——缺失时
    // （既有测试未插入）`advance_dormant_breakthrough` 内部优雅退化到 humanoid。
    body_plans: Option<Res<BodyPlanRegistry>>,
    races: Option<Res<RaceRegistry>>,
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
        // plan-offscreen-war-v1 P3 review-fix（CodeRabbit Major）：已离屏战死、真元待释放的败者
        // （`combat_dead_pending_release`）是**逻辑死亡**——它不该再移动 / 吸气 / 突破 / 自然老死。
        // `collect_zone_combat_pairs` 只把它排除出**配对**，但这条 per-char 推进循环若仍处理它，
        // 一个「已死」NPC 会在待释放期间继续 `advance_dormant_position`、`apply_dormant_regen`
        // （从 zone 拉真元进死者账户）、`advance_dormant_breakthrough` 甚至触发自然老死分支——语义
        // 错误，且 regen↔release 在满 zone 下来回 churn（吸进来又被 retry 释放回去）。直接 early-
        // continue 让它时钟冻结（连 `last_dormant_tick_processed` 也不推进），真元释放完全交给
        // `run_pending_combat_release_retry`（每 tick 重试 release，释放完才造遗物 + remove）。
        // 注意：不置 `mutated_any`——本循环对它零状态变更，dirty 由 combat phase 的 `mutated`
        // 信号负责（retain 翻 flag / retry partial-release 都已置 `mutated`，见 `CombatPhaseOutcome`）。
        if snapshot.combat_dead_pending_release {
            continue;
        }
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

        // plan-mundane-fauna-v1 守恒豁免：凡兽无灵——脱水期同样不吸/放 zone 灵气，对齐 live 侧
        // qi_regen_and_zone_drain_tick 的 `Without<MundaneFaunaSpecies>`。凡兽脱水快照
        // sum_rate()=1.0（Awaken 开 1 脉，默认 flow_rate=1.0），若不豁免会逐 tick 把
        // zone.spirit_qi 抽进 snapshot.qi_current，hydrate 用 snapshot.cultivation 覆盖回 live
        // 后死亡（负灵域枯萎/LOD 超距回收裸 insert(Despawned)、无 CurrentDimension 走 overflow）
        // 100% 蒸发，破守恒。跳过 regen + breakthrough（两者都从 zone 拉真元），保留位置/寿命推进。
        if snapshot.archetype != NpcArchetype::Mundane {
            if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
                // plan-offscreen-war-v1 P9：从 ZoneSpiritBonusStore 查 zone 倍率（默认 1.0）
                let war_multiplier = war_bonus
                    .as_deref()
                    .map(|s| s.multiplier_for(&snapshot.zone_name))
                    .unwrap_or(1.0);
                apply_dormant_regen_with_multiplier(snapshot, zones, ledger, war_multiplier);
            }
            if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
                let _ = advance_dormant_breakthrough(
                    snapshot,
                    zones,
                    ledger,
                    tick,
                    body_plans.as_deref(),
                    races.as_deref(),
                );
            }
        }

        if snapshot.lifespan.is_expired() {
            let mut settlement_committed = snapshot.cultivation.qi_current <= QI_EPSILON;
            if let (Some(zones), Some(ledger)) = (zones.as_deref_mut(), ledger.as_deref_mut()) {
                settlement_committed = snapshot.cultivation.qi_current <= QI_EPSILON
                    || release_dormant_qi_to_zone(snapshot, zones, ledger).is_ok();
            }
            if !settlement_committed || snapshot.cultivation.qi_current > QI_EPSILON {
                tracing::warn!(
                    "[bong][npc] retained expired dormant NPC `{}` until {:.6} qi settles",
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
    // Whether the combat phase changed persisted snapshot state without removing it —
    // currently this means a hard settlement failure marked a loser
    // `combat_dead_pending_release`. Such a tick must drive `mark_dirty` even when the
    // aging pass touched nothing, otherwise a restart can reload the loser as alive and
    // re-emit death/outcome telemetry. Successful retries remove the snapshot instead.
    let mut combat_mutated = false;
    if let (Some(faction_store), Some(zones), Some(ledger)) = (
        faction_store.as_deref(),
        zones.as_deref_mut(),
        ledger.as_deref_mut(),
    ) {
        let combat = run_dormant_combat_phase(
            &mut store,
            faction_store,
            &config,
            tick,
            zones,
            ledger,
            &mut death_notices,
            &mut combat_outcomes,
            &mut pending_relics,
        );
        combat_mutated = combat.mutated;
        if combat.removed {
            // Only removal changes zone membership. A pending-failure marker mutation leaves
            // `by_zone` intact, so it drives dirty persistence but not an index rebuild.
            removed_expired = true;
            store.rebuild_indexes();
        }
    }

    // Any advanced or removed snapshot — or a combat settlement failure that marked
    // a pending retry — changed persisted state; schedule the Redis write.
    if mutated_any || removed_expired || combat_mutated {
        store.mark_dirty();
    }
}

/// Outcome of one combat phase (`run_dormant_combat_phase`), reported back to
/// `dormant_global_tick_system` so it can drive the two distinct follow-ups
/// correctly:
///
/// - [`Self::removed`] — at least one snapshot left the store (combat death
///   fully released its qi, or a retry pass finalized one). Drives an index
///   rebuild (zone membership changed) **and** the dirty write.
/// - [`Self::mutated`] — the phase marked a failed settlement
///   `combat_dead_pending_release` without removing the snapshot. Drives only the dirty
///   write; the physical owner and `qi_current` remain unchanged until a future retry
///   commits atomically.
///
/// `removed` implies a mutation, but the two are tracked independently because
/// only `removed` warrants an index rebuild.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CombatPhaseOutcome {
    removed: bool,
    mutated: bool,
}

/// plan-offscreen-war-v1 P2：离屏派系互殴战死结算（脊柱核心）。
///
/// 接 P1 纯逻辑：先 `collect_zone_combat_pairs`（只读、owned id 对）→ 逐对 `roll_*` 出败者
/// → 守恒结算败者真元 → emit death + outcome → 人口回写。**守恒唯一流动点**：败者残余真元
/// 走 `release_dormant_qi_to_zone` typed transaction，同步结算 actor、signed Zone、fixed overflow
/// 与 audit（§10.1 #5 ②）。胜者真元不变（dormant 简化，未流动即未失衡，§10.1 #5 ③）。
///
/// **防吞真元（transaction-failure retry）**：typed settlement 成功时，zone 不接收的余量
/// 同步落入固定 `qi_flow_overflow`，败者 `qi_current` 必归零并可安全移除。只有非法 signed
/// zone、身份或稳定池 overflow 等硬事务失败才保留败者，并标记 pending-release 供后续重试。
/// 同 zone 多败者按确定性顺序 settlement；物理 owner 总量保持不变。
///
/// 返回 [`CombatPhaseOutcome`]：`removed`（有败者被移除 → rebuild 索引 + mark dirty）与
/// `mutated`（事务失败后只翻 pending-release flag → 只 mark dirty，不 rebuild）。
/// 失败原子性保证 actor/zone/ledger/audit 不变；持久化 flag 防止重启后重复 roll 败者和重发事件。
/// 借用安全：本函数独占 `&mut store` / `&mut ledger`；配对阶段只读并返回 owned id，
/// 结算阶段再逐 id 获取单个 snapshot 的可变借用。
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
    pending_relics: &mut EventWriter<PendingDormantRelicCreated>,
) -> CombatPhaseOutcome {
    // ⓪ 先重试上轮因硬事务失败而保留的战死者——它们已是「逻辑死亡」
    // （死亡 notice / outcome 上轮已 emit 过一次），本轮**只**重试守恒 settlement，成功才造遗物
    // + remove。优先于本轮新战斗结算，且绝不让它们再被 `collect_zone_combat_pairs`
    // 选中重新 roll（plan-offscreen-war-v1 P3 review-fix / CodeRabbit Major）。
    let mut outcome =
        run_pending_combat_release_retry(store, config, tick, zones, ledger, pending_relics);

    // ① 配对：immutable 只读 → owned id 对（§10.1 #3 collect-then-index）。
    // `collect_zone_combat_pairs` 已跳过 `combat_dead_pending_release` 的快照，故 retry
    // 仍失败的败者本轮不会被选中参战。
    let pairs = combat::collect_zone_combat_pairs(store, faction_store, config);
    if pairs.is_empty() {
        return outcome;
    }

    for (a_id, b_id) in pairs {
        // 防御：上一对的结算可能已移除本对成员（理论上 collect 保证每个 NPC 一轮至多一次，
        // 但 pending transaction failure 会让 store 在结算中变动，索引取不到就跳过，绝不 panic）。
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
            .map(|outcome| outcome.zone_accepted)
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

        // ⑥ 人口回写 + fail-closed：typed settlement 把未被 zone 接收的余量同步落入固定
        // qi_flow_overflow；成功后 snapshot current 必为零，失败则保留快照重试，绝不吞真元。
        let residual = loser.cultivation.qi_current;
        if residual > QI_EPSILON {
            loser.combat_dead_pending_release = true;
            outcome.mutated = true;
            tracing::warn!(
                "[bong][npc] retained combat-dead dormant NPC `{}` until {:.6} residual qi settles; marked pending-release, excluded from further combat",
                loser_id,
                residual
            );
        } else {
            // 显式终结 `&mut loser` 借用后，走共享的「释放完成 → 造遗物 + remove」收尾
            // （与 retry pass 同一入口，保证遗物只在真元释放完毕的此刻 emit 一次）。
            let _ = loser;
            finalize_released_combat_death(store, &loser_id, tick, config, pending_relics);
            // 移除 → 索引变了 → 调用方据 `removed` rebuild；`removed` 自然蕴含 mutated。
            outcome.removed = true;
        }
    }

    outcome
}

/// plan-offscreen-war-v1 P3 review-fix：重试上轮因守恒事务失败而保留的离屏战死者。
///
/// 这些败者已被标记 `combat_dead_pending_release`（逻辑死亡，death notice / outcome 已 emit
/// 过一次，**本函数绝不重发**），`collect_zone_combat_pairs` 已跳过它们不再参战。本函数每 tick
/// 遍历所有被标记的快照，重试 `release_dormant_qi_to_zone`；真元全释放（`<= QI_EPSILON`）后才
/// 走 `finalize_released_combat_death`（造遗物 + remove）。借用安全：先 collect owned id 列表，
/// 再逐 id `get_mut` 结算（任一时刻只持一个 snapshot 可变借用），与配对结算同源。
///
/// 返回 [`CombatPhaseOutcome`]：成功 retry 会 remove（`removed=true`）；失败 retry 不改任何
/// owner，只保留既有 marker，因此不会制造虚假的 partial mutation。
fn run_pending_combat_release_retry(
    store: &mut NpcDormantStore,
    config: &NpcVirtualizationConfig,
    tick: u64,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    pending_relics: &mut EventWriter<PendingDormantRelicCreated>,
) -> CombatPhaseOutcome {
    let mut outcome = CombatPhaseOutcome::default();
    // collect-then-index：先取出所有待释放败者的 owned id（升序，确定性），再逐个结算。
    let mut pending_ids: Vec<CharId> = store
        .snapshots
        .iter()
        .filter(|(_, snap)| snap.combat_dead_pending_release)
        .map(|(id, _)| id.clone())
        .collect();
    if pending_ids.is_empty() {
        return outcome;
    }
    pending_ids.sort();

    for loser_id in pending_ids {
        let Some(loser) = store.snapshots.get_mut(&loser_id) else {
            continue;
        };
        // 重试守恒 settlement。成功会全量清空 actor owner（zone 余量进入 fixed overflow）；
        // 失败则所有 owner 与 audit 原样不动，保留 flag 等下轮重试。
        if release_dormant_qi_to_zone(loser, zones, ledger).is_ok() {
            outcome.mutated = true;
        }
        let residual = loser.cultivation.qi_current;
        let _ = loser;
        if residual > QI_EPSILON {
            // 守恒事务仍失败 → 继续保留，下轮再试（保持 flag=true，不重发 death 事件）。
            continue;
        }
        // 真元终于释放完 → 此刻才造遗物 + remove（与初次死亡路径同一收尾入口）。
        finalize_released_combat_death(store, &loser_id, tick, config, pending_relics);
        outcome.removed = true;
    }
    outcome
}

/// plan-offscreen-war-v1 P3：离屏战死者**真元已守恒释放完毕此刻**的收尾——造零真元遗物
/// （若 `should_leave_relic` 通过）+ 从 store 移除。
///
/// **守恒时序红线**（§10.1 #5 ④ / docs/CLAUDE.md §四）：调用方必须保证已经 `release_dormant_qi_to_zone`
/// 且 `qi_current <= QI_EPSILON`——「先把残余真元守恒还给 zone，再用快照创建零真元遗物，最后
/// remove」，绝无吞真元窗口。遗物 event 在此 emit 一次（每个逻辑死亡至多一次：初次死亡释放完 or
/// retry 释放完，二者互斥）。
fn finalize_released_combat_death(
    store: &mut NpcDormantStore,
    loser_id: &CharId,
    tick: u64,
    config: &NpcVirtualizationConfig,
    pending_relics: &mut EventWriter<PendingDormantRelicCreated>,
) {
    if let Some(dead) = store.snapshots.get(loser_id) {
        if combat::should_leave_relic(dead) {
            let loot_seed = combat::relic_loot_seed(loser_id, tick, config.sim_seed);
            pending_relics.send(PendingDormantRelicCreated {
                char_id: loser_id.clone(),
                zone: dead.zone_name.clone(),
                position: dead.position,
                archetype: dead.archetype,
                loot_seed,
                created_tick: tick,
            });
        }
    }
    store.snapshots.remove(loser_id);
}

/// plan-npc-realm-distribution-v1 P3 §8.1 #3：一次性迁移 marker 文件路径。
///
/// `data/npc/realm_migration_v1.marker`（相对 server 进程 cwd，与 `persistence::DEFAULT_DATABASE_PATH`
/// = `data/bong.db` 同一约定）。运行时生成、不是提交产物——见 `.gitignore` 的 `server/data/` 规则。
/// 测试用 [`NPC_REALM_MIGRATION_MARKER_ENV_VAR`] 覆盖到临时目录，绝不能让 `cargo test`
/// 在真实 checkout 里写这个文件。
pub(crate) const NPC_REALM_MIGRATION_MARKER_DEFAULT_PATH: &str =
    "data/npc/realm_migration_v1.marker";

/// 覆盖 marker 路径的 env var（仅测试隔离用，生产恒走默认路径）。
const NPC_REALM_MIGRATION_MARKER_ENV_VAR: &str = "BONG_NPC_REALM_MIGRATION_MARKER_PATH";

fn npc_realm_migration_marker_path() -> PathBuf {
    std::env::var_os(NPC_REALM_MIGRATION_MARKER_ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(NPC_REALM_MIGRATION_MARKER_DEFAULT_PATH))
}

/// 写迁移完成 marker。**失败不允许静默吞错**——`tracing::error!` 落痕迹，调用方仍会把
/// `Local<bool>` 标记本次进程运行已处理（避免同一 server 会话内每 tick 反复重 roll），
/// 但下次重启因 marker 文件仍缺失会再次尝试迁移——这是刻意的 best-effort 降级，而不是
/// "写失败就假装成功、从此再也不重试"的静默吞错。
fn write_realm_migration_marker(marker_path: &Path) -> bool {
    if let Some(parent) = marker_path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                tracing::error!(
                    "[bong][npc] failed to create dormant realm migration marker directory {}: {error}",
                    parent.display()
                );
                return false;
            }
        }
    }
    if let Err(error) = std::fs::write(marker_path, b"v1\n") {
        tracing::error!(
            "[bong][npc] failed to write dormant realm migration marker {}: {error}",
            marker_path.display()
        );
        return false;
    }
    true
}

/// plan-npc-realm-distribution-v1 P3 §8.1 #3：既有（存量）dormant 快照的迁移目标 realm。
///
/// 身份 archetype 直写身份值，不抽样（与 P0 `npc_runtime_bundle`/`npc_runtime_bundle_with_age`
/// 调用站点的身份判定同源）：
/// - `GuardianRelic` → `Spirit`（`disciple.rs:233` 与 `tsy_hostile.rs:1086` 均如此）
/// - `Zhinian` → `Condense`（`tsy_hostile.rs:922`）
/// - `Daoxiang` → `Induce`（TSY 默认值，`tsy_hostile.rs:778`；单个尸体激活道伥的真实
///   `origin_realm` 未持久化进 `NpcDormantSnapshot`——只有 `cultivation.realm` 本身携带这份信息，
///   而这正是被迁移覆盖的字段，故无法精确复原，退化取 TSY 默认值近似，好过维持 bug 时代的醒灵）
/// - `DyingElder` → `Void`（`fauna/dying_elder.rs:391-394` 字面量；实践中 dormant store 不会持有
///   `DyingElder` 快照——`hydrate/mod.rs` 该分支退化为 zombie 占位——此处仅防御性覆盖，不会被命中）
///
/// 恒定字面量 archetype（`Beast`/`Zombie`/`Fuya`/`SkullFiend`，P0 穷举表归类"无身份信号"）
/// 保持 `Realm::Awaken`——它们设计上就该恒是这个值，**不**参与 §8.1 #1 分布抽样（那张表只服务
/// "自然散修种群"，不是要把环境威胁怪也拉进境界长尾）。
///
/// 其余（`Rogue`/`Disciple`/`Commoner` 且非 faction Leader）视为无身份信号的自然种群，走
/// §8.1 #1 分布表重抽样——与 `dormant_rogue_seed_snapshot` 用同一个 [`sample_rogue_seed_realm`]
/// 函数、同 `char_id`、同 `is_resource_zone` 判定规则，不新造第二套抽样逻辑。
fn dormant_snapshot_migrated_realm(snapshot: &NpcDormantSnapshot, is_resource_zone: bool) -> Realm {
    match snapshot.archetype {
        NpcArchetype::GuardianRelic => Realm::Spirit,
        NpcArchetype::Zhinian => Realm::Condense,
        NpcArchetype::Daoxiang => Realm::Induce,
        NpcArchetype::DyingElder => Realm::Void,
        NpcArchetype::Beast
        | NpcArchetype::Zombie
        | NpcArchetype::Fuya
        | NpcArchetype::SkullFiend
        // 凡兽（plan-mundane-fauna-v1）无灵、不修炼——恒 `Realm::Awaken` 地板，
        // 与其它"无身份信号"背景生物同组，不参与 §8.1 #1 散修境界分布抽样。
        | NpcArchetype::Mundane => Realm::Awaken,
        NpcArchetype::Rogue | NpcArchetype::Disciple | NpcArchetype::Commoner => snapshot
            .faction
            .as_ref()
            .filter(|membership| membership.rank == FactionRank::Leader)
            .map(|membership| leader_realm_for(named_faction_id_for_legacy(membership.faction_id)))
            .unwrap_or_else(|| {
                sample_rogue_seed_realm(snapshot.char_id.as_str(), is_resource_zone)
            }),
    }
}

/// plan-npc-realm-distribution-v1 P3 §8.1 #3：一次性确定性重 roll 存量 dormant 快照的 realm。
///
/// marker 文件（[`npc_realm_migration_marker_path`]）存在 → 幂等跳过；不存在 → 对
/// `store.snapshots` 里每一条既有快照按 [`dormant_snapshot_migrated_realm`] 重算 realm
/// （身份站点直写、无身份站点走 §8.1 #1 分布表重抽样），完成后写 marker。
///
/// 挂 `Update`（不是 `Startup`）：需要等 `load_dormant_store_from_redis_system`（`Startup`）
/// 先把存量 Redis 数据灌进 store，且需要 `ZoneRegistry` 就绪才能判定 zone 灵气档——两者都
/// 可能晚于 `Startup` 完成，故沿用 `seed_initial_dormant_population_on_startup` 的
/// `Local<bool>` 自旋等待模式而非假设 `Startup` 内部两个系统间的隐式排序。
fn migrate_dormant_realm_distribution_v1(
    mut store: ResMut<NpcDormantStore>,
    zone_registry: Option<Res<ZoneRegistry>>,
    seed_config: Res<DormantRoguePopulationSeedConfig>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut migrated: valence::prelude::Local<bool>,
) {
    if *migrated {
        return;
    }
    let marker_path = npc_realm_migration_marker_path();
    if marker_path.exists() {
        *migrated = true;
        return;
    }
    if store.restore_failed() {
        // Redis 恢复失败：这个进程生命周期内没有可信存量数据可迁移。**不写 marker**——
        // 让下次 Redis 恢复正常的重启重新尝试，而不是把这次的失败误判成"没有存量"从而
        // 永久跳过真正需要的迁移。只在本次运行内不再重复判定。
        *migrated = true;
        return;
    }
    if store.is_empty() {
        // 新世界没有存量可迁移；直接写 marker，避免每次 Startup 都重新判定一遍空 store。
        write_realm_migration_marker(&marker_path);
        *migrated = true;
        return;
    }
    let Some(zone_registry) = zone_registry.as_deref() else {
        // ZoneRegistry 还没就绪；不设 *migrated，下个 tick 再试。
        return;
    };

    let threshold = seed_config.resource_spirit_qi_threshold;
    let mut changed = false;
    // zone -> 该 zone 内本轮迁移新产生的最高境界（用于 narration 高亮，避免同一 zone
    // 因多个快照命中同一档而重复推送同一条文案刷屏）。
    let mut zone_highlights: HashMap<String, Realm> = HashMap::new();
    for snapshot in store.snapshots.values_mut() {
        let is_resource = zone_registry
            .find_zone_by_name(snapshot.zone_name.as_str())
            .map(|zone| zone.spirit_qi >= threshold)
            .unwrap_or(false);
        let new_realm = dormant_snapshot_migrated_realm(snapshot, is_resource);
        if snapshot.cultivation.realm != new_realm {
            snapshot.cultivation.realm = new_realm;
            snapshot.cultivation.qi_max = qi_max_for_realm(new_realm);
            snapshot.shared_lifespan = LifespanComponent::for_realm(new_realm);
            // Verify blocker fix: re-rolling realm without re-deriving meridian_system
            // leaves the migrated snapshot's opened-meridian count pinned to whatever
            // it was seeded with (often the P0-era 1-meridian default), disagreeing
            // with new_realm.required_meridians() — same double-source bug as the
            // seeder, just on the migration path.
            snapshot.meridian_system = crate::npc::technique::npc_meridian_system_for_realm(
                new_realm,
                crate::body_plan::humanoid_plan_static(),
            );
            // minor fix：重新派生的 meridian_system 会把所有经脉按 new_realm 全量
            // 重开（opened=true），却没核对 meridian_severed（永久断脉登记）——一条
            // 已被记录 SEVERED 的经脉会在迁移后被"复活"，与 MeridianSeveredPermanent
            // 记录矛盾。永久断脉是跨周目才重置的长期状态，realm 迁移不应抹掉它。
            for severed_id in &snapshot.meridian_severed.severed_meridians {
                // plan-race-system-v1 P1a：`severed_id` 是 `&MeridianChannelId`（非
                // `Copy`），`*severed_id` 移动出引用不合法，改 `.clone()`。
                snapshot.meridian_system.get_mut(severed_id.clone()).opened = false;
            }
            changed = true;
            if matches!(new_realm, Realm::Condense | Realm::Solidify) {
                let entry = zone_highlights
                    .entry(snapshot.zone_name.clone())
                    .or_insert(new_realm);
                if matches!(new_realm, Realm::Solidify) {
                    *entry = new_realm;
                }
            }
        }
    }
    if changed {
        store.mark_dirty();
    }
    if let Some(narrations) = narrations.as_deref_mut() {
        for (zone, realm) in &zone_highlights {
            push_realm_perception_narration(narrations, zone.as_str(), *realm);
        }
    }
    write_realm_migration_marker(&marker_path);
    *migrated = true;
    tracing::info!(
        "[bong][npc] realm_migration_v1: {} dormant snapshot(s) realm-migrated (marker={})",
        store.len(),
        marker_path.display()
    );
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
        // plan-npc-realm-distribution-v1 P1: track which list this NPC's zone came
        // from — `is_resource` drives which §8.1 #1 realm distribution table
        // `dormant_rogue_seed_snapshot` samples from. Must match the same
        // resource/background split `classify_zones_by_qi` produced above (this
        // *is* that split, not a re-derivation), otherwise realm weighting would
        // silently diverge from the zone bucket the NPC is actually seeded into.
        let (zone_candidates, is_resource) =
            if index < resource_target && !resource_zones.is_empty() {
                (&resource_zones, true)
            } else if !background_zones.is_empty() {
                (&background_zones, false)
            } else {
                (&resource_zones, true)
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
            is_resource,
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
    let faction_id = if deterministic_hash(char_id, 0).is_multiple_of(2) {
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

/// `deterministic_hash` 的固定 salt——把 char_id 散布到涌现群体（plan-offscreen-war-v1 P5
/// reframe b）。与 `seed_rogue_faction` 的 salt=0 错开，让群体分派独立于 Attack/Defend 二分，
/// 故同一批 char_id 能覆盖 ≥3 个不同群体（解锁多群体互殴），而非只塌成 2 组。
const GROUP_SALT: u64 = 0x5052_4F47_5F47_5250; // "PROG_GRP" 字面，仅作具名常量避免裸 magic

/// 把一个 char_id 确定性散布到某个涌现群体（plan-offscreen-war-v1 P5 reframe b）。
///
/// 用与 RNG 同源的 [`deterministic_hash`]（salt=[`GROUP_SALT`]）取模 [`EMERGENT_GROUP_COUNT`]，
/// 保证同 char_id 跨重启稳定分到同一群体（否则重启后离屏敌对关系漂移）。
fn seed_emergent_group(char_id: &str) -> EmergentGroupId {
    let group = (deterministic_hash(char_id, GROUP_SALT) % EMERGENT_GROUP_COUNT as u64) as u16;
    EmergentGroupId(group)
}

/// 离屏统一群体身份解析（plan-offscreen-war-v1 P5 reframe b）。
///
/// 显式 `emergent_group` 优先；缺失（旧持久化快照）时回退 `faction` 派生
/// （`FactionStore::emergent_group_from_faction`：Attack→0 / Defend→1 / Neutral→None）。
/// 非破坏迁移的单一入口——`collect_zone_combat_pairs` 与下游配对都经此解析群体身份，
/// 既支持新快照的显式群体，也让旧快照零迁移仍能配对。
pub(crate) fn effective_group(
    snapshot: &NpcDormantSnapshot,
    faction_store: &FactionStore,
) -> Option<EmergentGroupId> {
    snapshot.emergent_group.or_else(|| {
        snapshot
            .faction
            .as_ref()
            .and_then(|f| faction_store.emergent_group_from_faction(f.faction_id))
    })
}

fn dormant_rogue_seed_snapshot(
    zone: &crate::world::zone::Zone,
    index: u32,
    zone_local_index: u32,
    tick: u64,
    max_initial_age_ratio: f64,
    is_resource_zone: bool,
) -> NpcDormantSnapshot {
    let archetype = NpcArchetype::Rogue;
    let position = dormant_seed_scatter_position(zone, zone_local_index);
    let patrol_target = zone.center();
    let char_id = format!("dormant:rogue:{index}");
    // plan-npc-realm-distribution-v1 P1 §8.1 #1: sample realm from the zone-weighted
    // distribution table instead of the P0-era `Cultivation::default()` (which always
    // seeded 醒灵). `qi_current` stays 0.0 — `qi_max_for_realm` only sets the capacity
    // ceiling; real qi accrues later via `apply_dormant_regen_with_multiplier` pulling
    // from zone.spirit_qi, so spawning full would fabricate qi and break conservation.
    let realm = sample_rogue_seed_realm(char_id.as_str(), is_resource_zone);
    let cultivation = Cultivation {
        realm,
        qi_current: 0.0,
        qi_max: qi_max_for_realm(realm),
        ..Cultivation::default()
    };
    // plan-npc-realm-distribution-v1 Verify blocker fix: dormant seeder must derive
    // meridian_system from the *sampled* realm via the same
    // `npc_meridian_system_for_realm` all live spawn paths use (rogue.rs/disciple.rs/
    // lifecycle.rs/tsy_hostile.rs), otherwise a Condense/Solidify/Spirit dormant rogue
    // ends up with realm.required_meridians()==6/12/16 but a frozen single-meridian
    // (Lung-only) MeridianSystem — a realm↔经脉 double-source split visible on ~1000
    // seeded dormant snapshots.
    let meridian_system = crate::npc::technique::npc_meridian_system_for_realm(
        realm,
        crate::body_plan::humanoid_plan_static(),
    );
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
        memory: None,
        player_reputation: None,
        // plan-offscreen-war-v1 P0 #1：赋派系（Attack/Defend 二分），保证 is_hostile_pair
        // 在 P1/P2 能配出敌对对。
        faction: Some(seed_rogue_faction(char_id.as_str())),
        // plan-offscreen-war-v1 P5 reframe b：同时赋涌现群体（>2 群体散布），离屏战斗敌对
        // 改走「不同群体即敌对」（§十灵气零和）。保留上面的 faction 二分不破坏 P0/P1 迁移路径。
        emergent_group: Some(seed_emergent_group(char_id.as_str())),
        patrol,
        loot_table: Some(default_loot_for_archetype(archetype)),
        guardian_relic: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent,
        dormant_since_tick: tick,
        last_dormant_tick_processed: tick,
        initial_qi: cultivation.qi_current,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
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

/// plan-npc-realm-distribution-v1 P1 前置：跨模块共享哈希。`npc::spawn::rogue` 的活体种群
/// 入口（`seed_initial_rogue_population_on_startup`）需要与 dormant 快照 seeder 用同一份
/// 确定性哈希做境界抽样，保证两条种群生产线同源同规则（不新造第二套抽样逻辑，见接入面红线）。
pub(crate) fn deterministic_hash(char_id: &str, salt: u64) -> u64 {
    let mut hash = salt ^ 0x9E37_79B9_7F4A_7C15;
    for byte in char_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    }
    hash
}

/// `deterministic_hash` 的固定 salt，专属境界抽样（plan-npc-realm-distribution-v1 P1 §8.1
/// #1）。与 `seed_rogue_faction`（salt=0）/ `seed_emergent_group`（salt=`GROUP_SALT`）错开，
/// 避免境界与派系/涌现群体在同一哈希值下强相关（比如同一 salt 下醒灵总是分到 Attack）。
const REALM_SEED_SALT: u64 = 0x5245_414C_4D5F_5254; // "REALM_RT" 字面，具名常量避免裸 magic

/// plan-npc-realm-distribution-v1 §8.1 #1 决议分布表 —— background zone（`spirit_qi` 低于
/// `resource_spirit_qi_threshold` 的区域）。权重单位为千分比（避免浮点比例误差），六境界严格
/// 按 worldview §三:195 顺序排列（醒灵→化虚），总和恒 1000。化虚恒 0（正典稀有，不自然刷，
/// 仅垂死大能一类特殊实体走非分布表路径）。
const REALM_DISTRIBUTION_BACKGROUND: [(Realm, u32); 6] = [
    (Realm::Awaken, 570),
    (Realm::Induce, 300),
    (Realm::Condense, 120),
    (Realm::Solidify, 10),
    (Realm::Spirit, 0),
    (Realm::Void, 0),
];

/// 同上，resource zone（`spirit_qi` ≥ 阈值的灵气富集区）分布表，高境界占比更高但仍长尾。
const REALM_DISTRIBUTION_RESOURCE: [(Realm, u32); 6] = [
    (Realm::Awaken, 425),
    (Realm::Induce, 350),
    (Realm::Condense, 200),
    (Realm::Solidify, 20),
    (Realm::Spirit, 5),
    (Realm::Void, 0),
];

/// plan-npc-realm-distribution-v1 P1：按 zone 灵气档从 §8.1 #1 分布表确定性抽样境界。
///
/// 用与 `seed_rogue_faction`/`seed_emergent_group` 同源的 [`deterministic_hash`]（固定
/// salt=[`REALM_SEED_SALT`]），保证同 `char_id` 跨重启抽到同一境界。`is_resource_zone` 选表，
/// 不接受调用方传入非法/超界权重表以外的境界——六境界穷举分支覆盖整个 0..1000 区间，
/// 循环兜底 `Realm::Awaken` 仅用于防浮点/整数舍入漂移导致权重和略小于 1000 时的越界，
/// 正常路径权重和恒为 1000 不会触发。
///
/// 身份 realm（派系首领 / TSY / GuardianRelic 等）优先级高于本函数——这些站点不调用本函数，
/// 直接写入身份值，见 `lifecycle.rs` 的 `npc_runtime_bundle`/`npc_runtime_bundle_with_age`
/// 调用站点。本函数只服务无身份的自然散修种群 seeder。
pub(crate) fn sample_rogue_seed_realm(char_id: &str, is_resource_zone: bool) -> Realm {
    let table = if is_resource_zone {
        &REALM_DISTRIBUTION_RESOURCE
    } else {
        &REALM_DISTRIBUTION_BACKGROUND
    };
    let roll = (deterministic_hash(char_id, REALM_SEED_SALT) % 1000) as u32;
    let mut cumulative: u32 = 0;
    for (realm, weight) in table.iter() {
        cumulative += weight;
        if roll < cumulative {
            return *realm;
        }
    }
    Realm::Awaken
}

/// plan-offscreen-war-v1 P9：战事 zone regen 倍率（由调用方从 ZoneSpiritBonusStore 查询）。
/// 默认 1.0（未参战 zone 不受影响）。仅乘在 `rate` 参数上，守恒安全（见 settle.rs 模块文档）。
/// 内部委托给 `apply_dormant_regen_with_multiplier(1.0)`。测试和向后兼容路径使用。
#[allow(dead_code)]
pub fn apply_dormant_regen(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Option<QiTransfer> {
    apply_dormant_regen_with_multiplier(snapshot, zones, ledger, 1.0)
}

/// 带 war_multiplier 的 dormant regen（plan-offscreen-war-v1 P9 内部使用）。
/// 调用方从 `ZoneSpiritBonusStore::multiplier_for` 查询 zone 倍率后传入。
pub fn apply_dormant_regen_with_multiplier(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    war_multiplier: f64,
) -> Option<QiTransfer> {
    // plan-mundane-fauna-v1 守恒豁免（函数级 + 调用侧 dormant_global_tick_system 双重护栏）：
    // 凡兽无灵，脱水期也绝不从 zone 吸真元。凡兽脱水快照 sum_rate()=1.0 会通过下方 rate 门，
    // 必须在此提前返回 None，否则 snapshot.qi_current 被从 zone 抽高、hydrate 带回 live 后死亡蒸发。
    if snapshot.archetype == NpcArchetype::Mundane {
        return None;
    }
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .filter(|zone| planar_distance(zone.center(), pos) <= DORMANT_ZONE_ABSORPTION_RADIUS_BLOCKS)
        .map(|zone| zone.name.clone())?;
    let zone = zones.find_zone_mut(zone_name.as_str())?;
    // plan-zone-qi-economy-v1 P2：NPC 只喝地板（QI_NPC_ABSORB_FLOOR）以上的溢出层，
    // 给玩家开脉/修炼留底仓（玩家吸取路径不经此函数，不受此约束）。
    if zone.spirit_qi <= QI_NPC_ABSORB_FLOOR {
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
    // plan-offscreen-war-v1 P9：战事 zone 倍率乘在 rate 上（守恒安全，见 settle.rs 模块文档）
    let effective_multiplier = if war_multiplier.is_finite() && war_multiplier > 0.0 {
        war_multiplier
    } else {
        1.0
    };
    // plan-zone-qi-economy-v1 P2：地板以上的可吸取余量（不是 zone 全量）驱动 regen 公式，
    // 这样 drain <= zone.spirit_qi - QI_NPC_ABSORB_FLOOR，最终写回必然 >= 地板，无需额外钳位。
    let absorbable_zone_qi = (zone.spirit_qi - QI_NPC_ABSORB_FLOOR).max(0.0);
    let (gain, drain) = regen_from_zone(
        absorbable_zone_qi,
        rate * effective_multiplier,
        avg_integrity,
        room,
    );
    if gain <= 0.0 || drain <= 0.0 {
        return None;
    }

    let actor = ActorQiIdentity::from_life_record(&snapshot.life_record, ActorQiKind::Npc).ok()?;
    let outcome = snapshot
        .cultivation
        .gain_from_zone(
            zone,
            ledger,
            &actor,
            gain,
            QiTransferReason::CultivationRegen,
        )
        .ok()?;
    let transfer = outcome.transfers.into_iter().next()?;

    snapshot.qi_ledger_net += outcome.target_credited;
    Some(transfer)
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

/// plan-race-system-v1 P6b review major-4 收口：离屏（dormant）突破必须与在线
/// （`breakthrough_system` / `cultivate_action_system`）走同一套 body plan 派生配额——
/// 否则同一实体切换在线/离屏观测窗会得到不同突破结果（在线用自身构型配额，离屏悄悄
/// 退化成 humanoid），这是明确的换轨假完成红线。`body_plans`/`races` 均缺失时（大量
/// 既有测试未插入这两个资源）优雅退化到 humanoid——生产环境 `body_plan::register()`
/// 恒装载两资源，该分支不会在真实部署触发；`resolve_race_to_plan` 找不到
/// `snapshot.cultivation.race` 对应条目（未知/迁移中的 race id）同样退化到 humanoid
/// （这是"resolve 本身失败"的环境退化分支，语义对齐
/// `body_plan::resolve_body_plan_for_target` 的既有约定——**不是** review major-2 那条
/// "resolve 成功但 plan 缺 profile" fail-closed 分支，二者不混淆）。
pub fn advance_dormant_breakthrough(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    tick: u64,
    body_plans: Option<&BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
) -> Option<Result<BreakthroughSuccess, BreakthroughError>> {
    let mut roll = XorshiftRoll(deterministic_hash(&snapshot.char_id, tick));
    advance_dormant_breakthrough_with_roll(
        snapshot, zones, ledger, tick, body_plans, races, &mut roll,
    )
}

/// `Err(())` = review r2 major-2 同款 fail-closed 分支：`cultivation.race` 在
/// `RaceRegistry` 中有登记、也确实解析出一个真实 `BodyPlan`，但该 plan 没有声明
/// `meridian_profile`——数据不完整，调用方必须跳过本次突破判定，不能借用 humanoid
/// 曲线顶上。`races.get(race)` 本身查无此 race（未知/迁移中 race id）或
/// `body_plans`/`races` 资源缺失（既有测试未插入）是**环境退化**，走 humanoid 兜底，
/// 语义对齐 `body_plan::resolve_body_plan_for_target` 的既有约定。
fn dormant_meridian_profile<'a>(
    snapshot: &NpcDormantSnapshot,
    body_plans: Option<&'a BodyPlanRegistry>,
    races: Option<&'a RaceRegistry>,
) -> Result<&'a crate::body_plan::MeridianProfile, ()> {
    let humanoid_profile = || {
        crate::body_plan::humanoid_plan_static()
            .meridian_profile
            .as_ref()
            .expect(
                "humanoid body plan must declare meridian_profile from plan-race-system-v1 P1 \
                 onward — validate_body_plan should have rejected a humanoid plan missing it",
            )
    };
    match (body_plans, races) {
        (Some(body_plans), Some(races)) => {
            match resolve_race_to_plan(&snapshot.cultivation.race, body_plans, races) {
                Some(plan) => match plan.meridian_profile.as_ref() {
                    Some(profile) => Ok(profile),
                    None => Err(()),
                },
                // 未知/迁移中 race id —— resolve 本身失败，环境退化到 humanoid。
                None => Ok(humanoid_profile()),
            }
        }
        _ => Ok(humanoid_profile()),
    }
}

fn advance_dormant_breakthrough_with_roll<R: RollSource>(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    tick: u64,
    body_plans: Option<&BodyPlanRegistry>,
    races: Option<&RaceRegistry>,
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
    let pos = snapshot.position_vec();
    let (zone_name, zone_qi) = zones
        .find_zone(snapshot.dimension, pos)
        .map(|zone| (zone.name.clone(), zone.spirit_qi))?;
    if zone_qi < required_zone_qi {
        return None;
    }

    let Ok(profile) = dormant_meridian_profile(snapshot, body_plans, races) else {
        // fail-closed：resolve 成功但 plan 缺 meridian_profile——本 tick 不判定突破。
        return None;
    };
    let before_cultivation = snapshot.cultivation.clone();
    let mut staged_cultivation = before_cultivation.clone();
    let mut staged_meridians = snapshot.meridian_system.clone();
    let result = try_breakthrough_with_profile(
        &mut staged_cultivation,
        &mut staged_meridians,
        0.0,
        0.0,
        None,
        profile,
        roll,
    );
    let before_qi = before_cultivation.qi_current().max(0.0);
    let used_qi = (before_qi - staged_cultivation.qi_current().max(0.0)).max(0.0);
    if used_qi > 0.0 {
        let actor =
            ActorQiIdentity::from_life_record(&snapshot.life_record, ActorQiKind::Npc).ok()?;
        let mut staged_source = before_cultivation;
        let zone = zones.find_zone_mut(zone_name.as_str())?;
        staged_source
            .release_to_zone(
                Some(zone),
                ledger,
                &actor,
                used_qi,
                QiTransferReason::Breakthrough,
            )
            .ok()?;
        snapshot.qi_ledger_net -= used_qi;
    }
    snapshot.cultivation = staged_cultivation;
    snapshot.meridian_system = staged_meridians;
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
) -> Result<QiFlowOutcome, QiFlowError> {
    let amount = snapshot.cultivation.qi_current();

    let actor = ActorQiIdentity::from_life_record(&snapshot.life_record, ActorQiKind::Npc)?;
    let pos = snapshot.position_vec();
    let zone_name = zones
        .find_zone(snapshot.dimension, pos)
        .map(|zone| zone.name.clone())
        .or_else(|| {
            zones
                .find_zone_mut(snapshot.zone_name.as_str())
                .map(|zone| zone.name.clone())
        });
    let zone = zone_name
        .as_deref()
        .and_then(|zone_name| zones.find_zone_mut(zone_name));
    let outcome = snapshot.cultivation.release_to_zone(
        zone,
        ledger,
        &actor,
        amount,
        QiTransferReason::ReleaseToZone,
    )?;
    snapshot.qi_ledger_net -= outcome.source_debited;
    Ok(outcome)
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
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
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
            memory: None,
            player_reputation: None,
            faction: None,
            // 显式群体留空：走 effective_group 的 faction 派生回退路径（这里 faction=None ⇒
            // 群体 None ⇒ 不参战），顺带覆盖非破坏迁移分支。
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            tsy_sentinel: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: 0.1,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
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
        age_app.add_event::<PendingDormantRelicCreated>();
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
        death_app.add_event::<PendingDormantRelicCreated>();
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
            !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "Zone.spirit_qi is the sole environment owner; dormant regen must not leave a zone ledger shadow"
        );
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a")),
            "snapshot Cultivation is the sole dormant actor owner; dormant regen must not leave an NPC ledger shadow"
        );
    }

    #[test]
    fn dormant_regen_exempts_mundane_fauna_even_with_open_meridian_in_rich_zone() {
        // plan-mundane-fauna-v1 守恒红线（对称于 live 侧 qi_regen_excludes_mundane_fauna）：
        // 脱水凡兽即便开脉（sum_rate>0）、身处富灵区（普通 NPC 必吸），也**绝不**从 zone 吸真元。
        // 否则 snapshot.qi_current 被抽高、hydrate 带回 live 后死亡蒸发，破守恒。
        let mut snapshot = snapshot("npc_mundane_rabbit", DVec3::new(10.0, 64.0, 10.0));
        snapshot.archetype = NpcArchetype::Mundane;
        open_regular_meridians(&mut snapshot, 1); // sum_rate>0，普通 NPC 在此会吸
        let qi_before = snapshot.cultivation.qi_current;
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let zone_qi_before = zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        let mut ledger = WorldQiAccount::default();

        assert!(
            apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none(),
            "凡兽脱水快照必须被 dormant regen 豁免（返回 None，无 QiTransfer）"
        );
        assert_eq!(
            snapshot.cultivation.qi_current, qi_before,
            "凡兽 qi_current 不得因 dormant regen 增长（无灵不吸气）"
        );
        assert_eq!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi,
            zone_qi_before,
            "凡兽豁免后 zone.spirit_qi 必须一分不动（守恒）"
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

    /// plan-zone-qi-economy-v1 P2：地板红线——zone_qi 在 (0, QI_NPC_ABSORB_FLOOR] 时
    /// dormant NPC 必须完全放弃吸取，不能像玩家一样吃到地板以下。
    #[test]
    fn dormant_regen_stops_at_or_below_absorb_floor() {
        for zone_qi in [QI_NPC_ABSORB_FLOOR, 0.2, 0.05, 0.0] {
            let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
            open_regular_meridians(&mut snapshot, 1);
            let mut z = zone();
            z.spirit_qi = zone_qi;
            let mut zones = ZoneRegistry { zones: vec![z] };
            let mut ledger = WorldQiAccount::default();

            assert!(
                apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none(),
                "zone_qi={zone_qi} 已在/低于地板 {QI_NPC_ABSORB_FLOOR}，dormant regen 不应发生任何转移"
            );
            assert_eq!(
                snapshot.cultivation.qi_current, 0.1,
                "zone_qi={zone_qi} 时 NPC qi_current 不应变化"
            );
            assert_eq!(
                zones
                    .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                    .unwrap()
                    .spirit_qi,
                zone_qi,
                "zone_qi={zone_qi} 时 zone.spirit_qi 不应被 dormant regen 触碰"
            );
        }
    }

    /// 边界回归：zone_qi 略高于地板时 dormant regen 仍可发生，但写回后必须 >= 地板，
    /// 一次 tick 的微量 drain 不会把 zone 拉穿地板（drain 本就远小于 0.31-0.3 的余量）。
    #[test]
    fn dormant_regen_never_dips_zone_below_absorb_floor() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        let mut z = zone();
        z.spirit_qi = QI_NPC_ABSORB_FLOOR + 0.01;
        let mut zones = ZoneRegistry { zones: vec![z] };
        let mut ledger = WorldQiAccount::default();

        let transfer = apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger)
            .expect("地板以上还有 0.01 余量，应发生一次转移");

        assert!(transfer.amount > 0.0);
        let zone_after = zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            zone_after >= QI_NPC_ABSORB_FLOOR,
            "zone_qi 写回后 {zone_after} 不应低于地板 {QI_NPC_ABSORB_FLOOR}"
        );
    }

    /// 一批连续 tick（模拟长跑 dormant 批处理）不应把带回流 zone 压穿地板——
    /// 即使反复调用，收敛点也应停在地板附近，绝不低于它。
    #[test]
    fn repeated_dormant_regen_ticks_converge_to_absorb_floor_without_crossing_it() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        // qi_max 拉大，避免 room 提前耗尽掩盖地板行为。
        snapshot.cultivation.qi_max = 1000.0;
        let mut z = zone();
        z.spirit_qi = 0.8;
        let mut zones = ZoneRegistry { zones: vec![z] };
        let mut ledger = WorldQiAccount::default();

        for _ in 0..10_000 {
            if apply_dormant_regen(&mut snapshot, &mut zones, &mut ledger).is_none() {
                break;
            }
            let current = zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;
            assert!(
                current >= QI_NPC_ABSORB_FLOOR,
                "批量 tick 期间 zone_qi 一度跌破地板：{current} < {QI_NPC_ABSORB_FLOOR}"
            );
        }
        let final_qi = zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            final_qi >= QI_NPC_ABSORB_FLOOR,
            "长跑收敛后 zone_qi={final_qi} 不应低于地板 {QI_NPC_ABSORB_FLOOR}"
        );
    }

    /// plan-offscreen-war-v1 P9 war_multiplier 路径同样过地板——战事 zone 不给后门。
    #[test]
    fn dormant_regen_with_war_multiplier_still_respects_absorb_floor() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        open_regular_meridians(&mut snapshot, 1);
        let mut z = zone();
        z.spirit_qi = QI_NPC_ABSORB_FLOOR + 0.001;
        let mut zones = ZoneRegistry { zones: vec![z] };
        let mut ledger = WorldQiAccount::default();

        // war_multiplier 拉到 10x，即便如此也不能把 zone 拉穿地板。
        let _ = apply_dormant_regen_with_multiplier(&mut snapshot, &mut zones, &mut ledger, 10.0);

        let zone_after = zones
            .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
            .unwrap()
            .spirit_qi;
        assert!(
            zone_after >= QI_NPC_ABSORB_FLOOR,
            "war_multiplier=10x 不应突破地板，实际 {zone_after}"
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

        let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("death release should commit typed settlement");
        let transfer = outcome
            .transfers
            .iter()
            .find(|transfer| transfer.to == QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME))
            .expect("available zone room should receive the entire death release");

        assert_eq!(transfer.from, QiAccountId::npc("npc_a"));
        assert_eq!(transfer.amount, 0.4);
        assert_eq!(outcome.zone_accepted, 0.4);
        assert_eq!(snapshot.cultivation.qi_current, 0.0);
        assert!(
            zones
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi
                > 0.8
        );
        assert!(
            !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "Zone.spirit_qi is the sole environment owner; death release must not leave a zone ledger shadow"
        );
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a")),
            "snapshot Cultivation is the sole dormant actor owner; death release must not leave an NPC ledger shadow"
        );
    }

    #[test]
    fn death_qi_release_routes_zone_overflow_to_fixed_durable_pool() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        snapshot.cultivation.qi_max = 2.0;
        let mut full_zone = zone();
        full_zone.spirit_qi = 0.99;
        let mut zones = ZoneRegistry {
            zones: vec![full_zone],
        };
        let mut ledger = WorldQiAccount::default();

        let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("near-full zone plus fixed overflow should commit atomically");

        assert!((outcome.zone_accepted - 0.5).abs() < 1e-9);
        assert_eq!(snapshot.cultivation.qi_current, 0.0);
        assert!(
            (ledger.balance(&crate::qi_physics::qi_flow_overflow_account()) - 1.5).abs() < 1e-9,
            "zone overflow must be durably credited to the fixed qi_flow_overflow account"
        );
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a")),
            "snapshot Cultivation is the sole dormant actor owner; no NPC shadow may remain"
        );
        assert!(
            !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "Zone.spirit_qi is the sole environment owner; no Zone shadow may remain"
        );
    }

    #[test]
    fn death_qi_release_with_missing_zone_routes_everything_to_fixed_overflow() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.zone_name = "missing_zone".to_string();
        snapshot.cultivation.qi_current = 2.0;
        snapshot.cultivation.qi_max = 2.0;
        let mut zones = ZoneRegistry::default();
        zones.zones.clear();
        let mut ledger = WorldQiAccount::default();

        let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("missing Zone is a supported settlement path through fixed overflow");

        assert_eq!(snapshot.cultivation.qi_current(), 0.0);
        assert_eq!(outcome.zone_accepted, 0.0);
        assert_eq!(outcome.overflow_credited, 2.0);
        assert_eq!(
            ledger.balance(&crate::qi_physics::qi_flow_overflow_account()),
            2.0
        );
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a"))
                && !ledger.has_account(&QiAccountId::zone("missing_zone")),
            "missing-zone settlement must not synthesize actor or Zone mirrors"
        );
    }

    #[test]
    fn death_qi_release_repays_negative_zone_without_clamping_signed_state() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        snapshot.cultivation.qi_max = 2.0;
        let mut negative_zone = zone();
        negative_zone.spirit_qi = -1.2;
        let mut zones = ZoneRegistry {
            zones: vec![negative_zone],
        };
        let mut ledger = WorldQiAccount::default();

        let outcome = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect("signed negative Zone must accept a conservation release");

        assert_eq!(snapshot.cultivation.qi_current(), 0.0);
        assert!((zones.zones[0].spirit_qi - -1.16).abs() < 1e-9);
        assert_eq!(outcome.zone_accepted, 2.0);
        assert_eq!(outcome.overflow_credited, 0.0);
        assert_eq!(ledger.total(), 0.0);
    }

    #[test]
    fn death_qi_release_failure_keeps_dormant_zone_ledger_and_audit_unchanged() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        snapshot.cultivation.qi_max = 2.0;
        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut zones = ZoneRegistry {
            zones: vec![invalid_zone],
        };
        let mut ledger = WorldQiAccount::default();

        let error = release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect_err("invalid signed Zone owner must fail closed");

        assert!(matches!(error, QiFlowError::Physics(_)));
        assert_eq!(snapshot.cultivation.qi_current(), 2.0);
        assert!(zones.zones[0].spirit_qi.is_nan());
        assert_eq!(ledger.total(), 0.0);
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn death_qi_release_overflow_failure_keeps_all_physical_owners_unchanged() {
        let mut snapshot = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        snapshot.cultivation.qi_current = 2.0;
        snapshot.cultivation.qi_max = 2.0;
        let mut full_zone = zone();
        full_zone.spirit_qi = 1.0;
        let mut zones = ZoneRegistry {
            zones: vec![full_zone],
        };
        let mut ledger = WorldQiAccount::default();
        ledger
            .set_balance(crate::qi_physics::qi_flow_overflow_account(), f64::MAX)
            .unwrap();

        release_dormant_qi_to_zone(&mut snapshot, &mut zones, &mut ledger)
            .expect_err("non-finite stable destination sum must fail closed");

        assert_eq!(snapshot.cultivation.qi_current(), 2.0);
        assert_eq!(zones.zones[0].spirit_qi, 1.0);
        assert_eq!(
            ledger.balance(&crate::qi_physics::qi_flow_overflow_account()),
            f64::MAX
        );
        assert!(ledger.transfers().is_empty());
    }

    #[test]
    fn dormant_global_tick_settles_expired_snapshot_into_zone_and_fixed_overflow() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.add_event::<PendingDormantRelicCreated>();
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
        expired.cultivation.qi_max = 2.0;
        expired.lifespan.age_ticks = expired.lifespan.max_age_ticks + 1.0;
        let mut store = NpcDormantStore::default();
        store.insert(expired);
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        assert!(
            !store.contains("npc_a"),
            "successful typed settlement must remove the expired dormant owner even when the zone accepts only part"
        );
        let zones = app.world().resource::<ZoneRegistry>();
        assert!((zones.zones[0].spirit_qi - 1.0).abs() < 1e-9);
        let ledger = app.world().resource::<WorldQiAccount>();
        assert!(
            (ledger.balance(&crate::qi_physics::qi_flow_overflow_account()) - 1.5).abs() < 1e-9,
            "the amount rejected by the near-full zone must persist in fixed overflow"
        );
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a"))
                && !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "successful settlement must not leave dormant or Zone mirrors"
        );
        let events = app.world().resource::<Events<NpcDeathNotice>>();
        assert_eq!(
            events.iter_current_update_events().count(),
            1,
            "natural death notice must emit after the full physical-owner settlement commits"
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
        app.add_event::<PendingDormantRelicCreated>();
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
        app.add_event::<PendingDormantRelicCreated>();
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
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
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
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut ledger = WorldQiAccount::default();

        let result = advance_dormant_breakthrough_with_roll(
            &mut snapshot,
            &mut zones,
            &mut ledger,
            1200,
            None,
            None,
            &mut roll,
        )
        .expect("eligible dormant NPC should attempt breakthrough")
        .expect("fixed low roll should pass");

        assert_eq!(result.to, Realm::Induce);
        assert_eq!(snapshot.cultivation.realm, Realm::Induce);
        assert_eq!(snapshot.cultivation.qi_current, 12.0);
        assert!(
            !ledger.has_account(&QiAccountId::npc("npc_a")),
            "dormant breakthrough must keep actor qi solely in the snapshot, without an NPC ledger shadow"
        );
        assert!(
            !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "dormant breakthrough must keep environment qi solely in Zone.spirit_qi, without a Zone ledger shadow"
        );
        assert!(
            (zones.zones[0].spirit_qi - (0.8 + 8.0 / QI_ZONE_UNIT_CAPACITY)).abs() < 1e-9,
            "dormant breakthrough cost must be credited to the physical Zone owner"
        );
        let transfer = ledger
            .transfers()
            .last()
            .expect("dormant breakthrough should leave a QiTransfer");
        assert_eq!(transfer.reason, QiTransferReason::Breakthrough);
        assert_eq!(transfer.amount, 8.0);
        assert!(
            (snapshot.qi_ledger_net - (-8.0)).abs() < f64::EPSILON,
            "dormant breakthrough must debit qi_ledger_net by the spent qi; expected -8.0, got {}",
            snapshot.qi_ledger_net
        );
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
        // 默认（非战死）快照：`combat_dead_pending_release=false` 必须被 skip_serializing_if
        // 省略，不写进 Redis payload（不膨胀，§10.1 #2）。
        assert!(
            !payloads[0].1.contains("combat_dead_pending_release"),
            "a normal (not combat-dead) snapshot must OMIT combat_dead_pending_release from its Redis payload (skip_serializing_if avoids snapshot bloat), but the field was present: {}",
            payloads[0].1
        );
        let decoded: NpcDormantSnapshot =
            serde_json::from_str(payloads[0].1.as_str()).expect("deserialize");
        assert_eq!(decoded.char_id, "npc_a");
        assert_eq!(decoded.position, [10.0, 64.0, 10.0]);
        assert!(
            !decoded.combat_dead_pending_release,
            "a roundtripped normal snapshot must decode combat_dead_pending_release=false; got true"
        );
    }

    #[test]
    fn redis_payload_roundtrips_pending_release_flag() {
        // plan-offscreen-war-v1 P3 review-fix（守恒持久化安全）：被标记「战死待释放真元」的败者
        // 必须随 Redis 持久化——flag=true 往返不丢，server 重启后仍 pending-release（真元不丢、
        // 仍被 collect 跳过、绝不重复参战）。
        let mut snap = snapshot("trapped", DVec3::new(10.0, 64.0, 10.0));
        snap.combat_dead_pending_release = true;
        let payload = serde_json::to_string(&snap).expect("serialize flagged snapshot");
        assert!(
            payload.contains("combat_dead_pending_release"),
            "a flagged (combat-dead-pending-release) snapshot MUST serialize the field so it survives a Redis restart, but it was omitted: {payload}"
        );
        let decoded: NpcDormantSnapshot =
            serde_json::from_str(&payload).expect("deserialize flagged snapshot");
        assert!(
            decoded.combat_dead_pending_release,
            "flag=true must roundtrip through Redis JSON (restart safety: pending-release loser keeps its qi and stays out of combat); got false"
        );
    }

    #[test]
    fn legacy_redis_snapshot_without_flag_defaults_to_false() {
        // 向后兼容：升级前写入 Redis 的旧快照没有 `combat_dead_pending_release` 字段，
        // `#[serde(default)]` 必须把它解码成 `false`（不 panic、不报错），否则升级即丢全部 dormant。
        // 用一份完整的旧快照 JSON（先 serialize 一个普通快照得到字段名，再手动删掉 flag 字段，
        // 这里因为默认快照本就不写该字段，直接复用其 payload 即「缺字段」样本）。
        let source = snapshot("legacy_npc", DVec3::new(5.0, 64.0, 5.0));
        let legacy_payload = serde_json::to_string(&source).expect("serialize");
        assert!(
            !legacy_payload.contains("combat_dead_pending_release"),
            "precondition: the synthesized legacy payload must lack the flag field"
        );
        let decoded: NpcDormantSnapshot = serde_json::from_str(&legacy_payload)
            .expect("legacy snapshot (no flag field) must deserialize via serde default");
        assert!(
            !decoded.combat_dead_pending_release,
            "a legacy Redis snapshot missing combat_dead_pending_release must default to false (serde default), so upgrades never lose or mis-flag dormant NPCs; got true"
        );
    }

    #[test]
    fn legacy_redis_snapshot_without_tsy_sentinel_field_defaults_to_none() {
        // plan-tsy-sentinel-dormant-regression-v1 §P1：非破坏迁移——升级前写入 Redis 的旧
        // 快照没有 `tsy_sentinel` 字段（该字段是本 plan 新加的），`#[serde(default)]` 必须
        // 把它解码成 `None`（不 panic、不报错），即"退化为修复前的既有行为"（普通
        // overworld GuardianRelic），而不是升级即丢全部 dormant 快照。
        let source = snapshot("legacy_npc_no_sentinel", DVec3::new(5.0, 64.0, 5.0));
        let legacy_payload = serde_json::to_string(&source).expect("serialize");
        assert!(
            !legacy_payload.contains("tsy_sentinel"),
            "precondition: the synthesized legacy payload must lack the tsy_sentinel field \
             (skip_serializing_if omits None), got: {legacy_payload}"
        );
        let decoded: NpcDormantSnapshot = serde_json::from_str(&legacy_payload)
            .expect("legacy snapshot (no tsy_sentinel field) must deserialize via serde default");
        assert!(
            decoded.tsy_sentinel.is_none(),
            "a legacy Redis snapshot missing tsy_sentinel must default to None (serde default), \
             so upgrading the server binary never fails to load pre-existing dormant TSY sentinel \
             snapshots (they just degrade to the pre-fix plain-GuardianRelic hydrate path until \
             the next dehydrate cycle re-captures the field); got Some(..)"
        );
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
    fn redis_hash_restore_rejects_partial_corruption_without_mutating_live_store() {
        let existing = snapshot("npc_existing", DVec3::new(1.0, 64.0, 1.0));
        let incoming = snapshot("npc_a", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&incoming).expect("serialize dormant snapshot");
        let entries = HashMap::from([
            (incoming.char_id.clone(), payload),
            ("npc_bad".to_string(), "{not-json".to_string()),
        ]);
        let mut store = NpcDormantStore::default();
        store.insert(existing);
        store.take_dirty();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("one corrupt dormant qi owner must reject the complete Redis restore");

        assert!(
            error.contains("refusing partial dormant Redis restore"),
            "partial corruption should report the fail-closed restore boundary, got: {error}"
        );
        assert_eq!(
            store.len(),
            1,
            "failed restore must leave the pre-existing live store unchanged"
        );
        assert!(store.contains("npc_existing"));
        assert!(
            !store.contains("npc_a") && !store.contains("npc_bad"),
            "failed restore must not commit even valid staged rows"
        );
        assert!(
            !store.is_dirty(),
            "failed restore must not arm a full-HASH write that could erase corrupt owners"
        );
    }

    #[test]
    fn redis_hash_restore_rejects_hash_field_identity_mismatch_atomically() {
        let incoming = snapshot("payload_owner", DVec3::new(10.0, 64.0, 10.0));
        let payload = serde_json::to_string(&incoming).expect("serialize dormant snapshot");
        let entries = HashMap::from([("different_hash_field".to_string(), payload)]);
        let mut store = NpcDormantStore::default();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("HASH field and durable snapshot identity must agree");

        assert!(
            error.contains("does not match HASH field"),
            "identity mismatch should be explicit, got: {error}"
        );
        assert!(
            store.is_empty(),
            "identity mismatch must not install the payload under a different owner id"
        );
    }

    #[test]
    fn redis_hash_restore_fails_when_every_entry_is_invalid() {
        let entries = HashMap::from([("npc_bad".to_string(), "{not-json".to_string())]);
        let mut store = NpcDormantStore::default();

        let error = load_dormant_snapshots_from_hash_entries(&mut store, entries)
            .expect_err("all invalid entries should fail restore");

        assert!(error.contains("refusing partial dormant Redis restore"));
        assert!(error.contains("1 of 1 snapshot entries were invalid"));
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
            "默认 sim_seed 必须为 0 = 未注入 seed 时的确定性基线"
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
        // P0 是 plumbing 层：相同 BONG_SIM_SEED 解析出相同 u64，注入 config.sim_seed
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

        // 注入路径：解析出的 seed 直接落进 NpcVirtualizationConfig.sim_seed，
        // dormant 战斗（P1/P2）即读这同一个种子，保证同 seed → 同战死结果。
        let parsed = parse_sim_seed(Some("777"));
        let cfg = NpcVirtualizationConfig {
            sim_seed: parsed,
            ..NpcVirtualizationConfig::default()
        };
        assert_eq!(
            cfg.sim_seed, parsed,
            "解析出的 seed 必须原样进入 config.sim_seed，否则 dormant 战斗 RNG 与配置不同步"
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
        let snapshot = dormant_rogue_seed_snapshot(&zone, 0, 0, 0, 0.8, true);
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

    // ── plan-offscreen-war-v1 P5 reframe b：seed 涌现群体散布 ──────────────────

    #[test]
    fn seed_emergent_group_is_deterministic_per_char_id() {
        // 同 char_id 跨调用必须分到同一群体（否则重启后离屏敌对关系漂移）。
        let a = seed_emergent_group("dormant:rogue:42");
        let b = seed_emergent_group("dormant:rogue:42");
        assert_eq!(
            a, b,
            "同 char_id 必须确定性散布到同一涌现群体，否则重启后敌对关系漂移"
        );
    }

    #[test]
    fn seed_emergent_group_distribution_covers_at_least_three_groups() {
        // reframe b 解锁 >2 群体互殴：256 个 char_id 的 emergent_group 必须覆盖 ≥3 个不同群体，
        // 否则散修永远塌成 ≤2 组、退回 P1 的 2-faction 上限。同时每个群体 id < EMERGENT_GROUP_COUNT。
        let mut groups = std::collections::BTreeSet::new();
        for index in 0..256u32 {
            let g = seed_emergent_group(&format!("dormant:rogue:{index}"));
            assert!(
                g.0 < EMERGENT_GROUP_COUNT,
                "seed group id {} must be < EMERGENT_GROUP_COUNT {EMERGENT_GROUP_COUNT}",
                g.0
            );
            groups.insert(g.0);
        }
        assert!(
            groups.len() >= 3,
            "256 char_ids must spread across at least 3 distinct emergent groups to unlock >2-group melee (reframe b §十); only saw {} group(s): {:?}",
            groups.len(),
            groups
        );
    }

    #[test]
    fn dormant_rogue_seed_snapshot_assigns_explicit_emergent_group() {
        // 防回归：seed 出来的 dormant rogue 必须带显式 emergent_group（非 None），
        // 否则离屏战斗回退 faction 派生、退化成 2 群体上限。
        let zone = zone();
        let snapshot = dormant_rogue_seed_snapshot(&zone, 0, 0, 0, 0.8, true);
        let group = snapshot
            .emergent_group
            .expect("seeded dormant rogue 必须带显式 emergent_group，不能是 None");
        assert!(
            group.0 < EMERGENT_GROUP_COUNT,
            "seed emergent group id {} must be < EMERGENT_GROUP_COUNT {EMERGENT_GROUP_COUNT}",
            group.0
        );
    }

    // ── plan-npc-realm-distribution-v1 P1：种群 seeder 境界分布（饱和单测） ────────

    fn realm_weight(table: &[(Realm, u32); 6], realm: Realm) -> u32 {
        table
            .iter()
            .find(|(r, _)| *r == realm)
            .map(|(_, w)| *w)
            .unwrap_or(0)
    }

    #[test]
    fn realm_distribution_tables_sum_to_exactly_1000_per_mille() {
        // 防漂移：任何一次手改分布表数值（微调长尾）如果算错导致总和不再是 1000‰，
        // `sample_rogue_seed_realm` 的累积权重循环会在权重和 < 1000 时对部分 roll
        // 值静默兜底成 Realm::Awaken（人为压低非醒灵占比），必须显式撞红而非静默偏移。
        let background_sum: u32 = REALM_DISTRIBUTION_BACKGROUND.iter().map(|(_, w)| w).sum();
        let resource_sum: u32 = REALM_DISTRIBUTION_RESOURCE.iter().map(|(_, w)| w).sum();
        assert_eq!(
            background_sum, 1000,
            "background 分布表权重和必须恰为 1000‰，实际 {background_sum}"
        );
        assert_eq!(
            resource_sum, 1000,
            "resource 分布表权重和必须恰为 1000‰，实际 {resource_sum}"
        );
    }

    #[test]
    fn realm_distribution_tables_never_seed_void_naturally() {
        // §8.1 #1 决议：化虚不自然刷，正典稀有仅垂死大能一类特殊实体。
        assert_eq!(
            realm_weight(&REALM_DISTRIBUTION_BACKGROUND, Realm::Void),
            0,
            "background 分布表化虚权重必须为 0"
        );
        assert_eq!(
            realm_weight(&REALM_DISTRIBUTION_RESOURCE, Realm::Void),
            0,
            "resource 分布表化虚权重必须为 0"
        );
    }

    #[test]
    fn sample_rogue_seed_realm_is_deterministic_per_char_id_and_zone_kind() {
        // 同 char_id + 同 zone 档，跨调用必须抽到同一境界（否则重启后境界分布漂移）。
        for char_id in ["dormant:rogue:0", "dormant:rogue:1", "rogue-seed:zone:42"] {
            for is_resource in [true, false] {
                let a = sample_rogue_seed_realm(char_id, is_resource);
                let b = sample_rogue_seed_realm(char_id, is_resource);
                assert_eq!(
                    a, b,
                    "char_id={char_id} is_resource={is_resource}: 同输入必须抽到同一境界，\
                     实际两次调用分别得到 {a:?} 和 {b:?}"
                );
            }
        }
    }

    #[test]
    fn sample_rogue_seed_realm_differs_by_zone_kind_salt_not_faction_or_group_salt() {
        // 境界抽样必须用专属 REALM_SEED_SALT，与 seed_rogue_faction（salt=0）/
        // seed_emergent_group（salt=GROUP_SALT）错开——否则境界会和派系/群体强相关
        // （比如同一 salt 下醒灵总是分到 Attack）。用同一 char_id 三个 salt 的哈希两两
        // 不相等来证明三者独立（char_id 选一个非退化样本，避免巧合碰撞误判）。
        let char_id = "dormant:rogue:7";
        let realm_hash = deterministic_hash(char_id, REALM_SEED_SALT);
        let faction_hash = deterministic_hash(char_id, 0);
        let group_hash = deterministic_hash(char_id, GROUP_SALT);
        assert_ne!(
            realm_hash, faction_hash,
            "REALM_SEED_SALT 必须与派系 salt=0 产生不同哈希"
        );
        assert_ne!(
            realm_hash, group_hash,
            "REALM_SEED_SALT 必须与 GROUP_SALT 产生不同哈希"
        );
    }

    #[test]
    fn sample_rogue_seed_realm_background_distribution_matches_table_within_tolerance() {
        // 统计 pin（非精确计数）：2000 个不同 char_id 在 background 档下的境界直方图，
        // 逐境界占比须落在 §8.1 #1 background 表（57/30/12/1/0/0%）±8 个百分点内。
        let n = 2000;
        let mut counts: HashMap<&'static str, u32> = HashMap::new();
        for i in 0..n {
            let realm = sample_rogue_seed_realm(&format!("tolerance:background:{i}"), false);
            let key = match realm {
                Realm::Awaken => "awaken",
                Realm::Induce => "induce",
                Realm::Condense => "condense",
                Realm::Solidify => "solidify",
                Realm::Spirit => "spirit",
                Realm::Void => "void",
            };
            *counts.entry(key).or_insert(0) += 1;
        }
        let ratio = |key: &str| *counts.get(key).unwrap_or(&0) as f64 / n as f64;
        let assert_within = |label: &str, actual: f64, expected_pct: f64| {
            let tolerance = 0.08;
            assert!(
                (actual - expected_pct / 100.0).abs() <= tolerance,
                "background {label} 占比 {actual:.3} 偏离 §8.1 #1 预期 {expected_pct}% 超过容差 \
                 ±{tolerance}（N={n} 样本 counts={counts:?}）"
            );
        };
        assert_within("醒灵", ratio("awaken"), 57.0);
        assert_within("引气", ratio("induce"), 30.0);
        assert_within("凝脉", ratio("condense"), 12.0);
        assert_within("固元", ratio("solidify"), 1.0);
        assert_eq!(
            counts.get("spirit").copied().unwrap_or(0),
            0,
            "background 档通灵权重为 0，绝不应抽到"
        );
        assert_eq!(counts.get("void").copied().unwrap_or(0), 0, "化虚不自然刷");
    }

    #[test]
    fn sample_rogue_seed_realm_resource_distribution_matches_table_within_tolerance() {
        // 统计 pin：resource 档（42.5/35/20/2/0.5/0%）——通灵样本稀少（0.5%），
        // 用更大样本量 4000 降低小概率分支的统计噪声，容差同样 ±8 个百分点
        // （通灵/固元档额外用绝对宽松上界防止偶发 0 样本导致误判）。
        let n = 4000;
        let mut counts: HashMap<&'static str, u32> = HashMap::new();
        for i in 0..n {
            let realm = sample_rogue_seed_realm(&format!("tolerance:resource:{i}"), true);
            let key = match realm {
                Realm::Awaken => "awaken",
                Realm::Induce => "induce",
                Realm::Condense => "condense",
                Realm::Solidify => "solidify",
                Realm::Spirit => "spirit",
                Realm::Void => "void",
            };
            *counts.entry(key).or_insert(0) += 1;
        }
        let ratio = |key: &str| *counts.get(key).unwrap_or(&0) as f64 / n as f64;
        let assert_within = |label: &str, actual: f64, expected_pct: f64, tolerance: f64| {
            assert!(
                (actual - expected_pct / 100.0).abs() <= tolerance,
                "resource {label} 占比 {actual:.3} 偏离 §8.1 #1 预期 {expected_pct}% 超过容差 \
                 ±{tolerance}（N={n} 样本 counts={counts:?}）"
            );
        };
        assert_within("醒灵", ratio("awaken"), 42.5, 0.08);
        assert_within("引气", ratio("induce"), 35.0, 0.08);
        assert_within("凝脉", ratio("condense"), 20.0, 0.08);
        assert_within("固元", ratio("solidify"), 2.0, 0.03);
        assert_within("通灵", ratio("spirit"), 0.5, 0.02);
        assert_eq!(counts.get("void").copied().unwrap_or(0), 0, "化虚不自然刷");
        // resource 档整体高境界（凝脉+固元+通灵）占比必须明显高于 background 档，
        // 证明两张表确实不同（不是同一张表被误接了两次）。
        let resource_high = ratio("condense") + ratio("solidify") + ratio("spirit");
        assert!(
            resource_high > 0.15,
            "resource 档凝脉+固元+通灵合计占比 {resource_high:.3} 偏低，\
             §8.1 #1 预期约 22.5%，可能误接了 background 表"
        );
    }

    #[test]
    fn dormant_rogue_seed_snapshot_realm_distribution_not_always_awaken() {
        // 端到端契约：seed 出来的 dormant snapshot 的 Cultivation.realm 不能恒为醒灵
        // （否则 P0 choke-point 修复对 dormant seeder 完全没有生效）。
        let zone = zone();
        let realms: Vec<Realm> = (0..500u32)
            .map(|i| {
                dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true)
                    .cultivation
                    .realm
            })
            .collect();
        let non_awaken = realms.iter().filter(|r| **r != Realm::Awaken).count();
        assert!(
            non_awaken > 0,
            "500 个 dormant rogue snapshot 全部落在醒灵，期望按 §8.1 #1 分布表抽到非醒灵境界；\
             这意味着 dormant_rogue_seed_snapshot 回退成了 Cultivation::default()"
        );
        assert!(
            !realms.contains(&Realm::Void),
            "dormant seeder 绝不应抽到化虚（正典稀有，不自然刷）"
        );
    }

    #[test]
    fn dormant_rogue_seed_snapshot_meridian_system_matches_sampled_realm_required_meridians() {
        // Verify blocker pin：dormant seeder 曾恒开 1 条肺经（MeridianSystem::default()
        // + 手动开 Lung），与抽样出的 realm 脱钩——凝脉/固元/通灵抽样命中却只有 1 条脉，
        // 撞 realm↔经脉双源矛盾。500 个样本里筛出每个非醒灵境界至少一例，核对
        // meridian_system.opened_count() 恰等于 realm.required_meridians()（用生产
        // 侧的 npc_meridian_system_for_realm 派生规则，不是重新定义一套开脉逻辑）。
        let zone = zone();
        let mut seen_realms: std::collections::HashSet<Realm> = std::collections::HashSet::new();
        for i in 0..500u32 {
            let snapshot = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
            let realm = snapshot.cultivation.realm;
            let expected = realm.required_meridians();
            let actual = snapshot.meridian_system.opened_count();
            assert_eq!(
                actual, expected,
                "i={i} realm={realm:?}: dormant seeder 落地的 meridian_system 应开 \
                 {expected} 条经脉（realm.required_meridians()），实得 {actual} 条 \
                 ——若恒为 1 说明退回了 P0-era 恒开肺经的 bug"
            );
            let expected_system = crate::npc::technique::npc_meridian_system_for_realm(
                realm,
                crate::body_plan::humanoid_plan_static(),
            );
            let opened_mismatch = snapshot
                .meridian_system
                .iter()
                .zip(expected_system.iter())
                .enumerate()
                .find(|(_, (actual, expected))| actual.opened != expected.opened);
            assert!(
                opened_mismatch.is_none(),
                "i={i} realm={realm:?}: dormant seeder 的 meridian_system 必须与生产侧 \
                 npc_meridian_system_for_realm(realm) 逐脉一致（同一份派生规则的单一来源），\
                 首个不一致的经脉 index={opened_mismatch:?}"
            );
            seen_realms.insert(realm);
        }
        assert!(
            seen_realms.contains(&Realm::Condense) || seen_realms.contains(&Realm::Solidify),
            "500 个 resource 档样本应至少抽到一例凝脉或固元，否则本测试没有真正覆盖 \
             required_meridians()>1 的分支（fixture 完整性）；实抽到 {seen_realms:?}"
        );
    }

    #[test]
    fn dormant_rogue_seed_snapshot_qi_current_stays_zero_regardless_of_sampled_realm() {
        // 守恒红线：无论抽到哪个境界，qi_current 必须保持 0.0（不满灵）——qi_max_for_realm
        // 只设容量上限，真元靠既有 apply_dormant_regen_with_multiplier 从 zone 逐步吸收；
        // spawn 时满灵会凭空产生真元，撞 qi_physics 守恒红线。qi_max 必须等于
        // qi_max_for_realm(抽到的 realm)，不能停留在 Cultivation::default() 的 10.0。
        let zone = zone();
        for i in 0..200u32 {
            let snapshot = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
            assert_eq!(
                snapshot.cultivation.qi_current, 0.0,
                "index={i} realm={:?}: qi_current 必须恒 0.0（不满灵）",
                snapshot.cultivation.realm
            );
            assert_eq!(
                snapshot.cultivation.qi_max,
                qi_max_for_realm(snapshot.cultivation.realm),
                "index={i} realm={:?}: qi_max 必须等于 qi_max_for_realm(realm)，不能是 \
                 Cultivation::default() 的醒灵默认值",
                snapshot.cultivation.realm
            );
        }
    }

    #[test]
    fn dormant_rogue_seed_snapshot_same_seed_twice_produces_identical_realm() {
        // 确定性 pin：同 seed（同 zone/index）两次 genesis 必须逐 NPC realm 一致。
        let zone = zone();
        for i in 0..50u32 {
            let a = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
            let b = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, true);
            assert_eq!(
                a.cultivation.realm, b.cultivation.realm,
                "index={i}: 同 seed 两次调用 dormant_rogue_seed_snapshot 必须得到相同 realm，\
                 实际 {:?} vs {:?}",
                a.cultivation.realm, b.cultivation.realm
            );
        }
    }

    #[test]
    fn dormant_rogue_seed_snapshot_resource_vs_background_flag_changes_distribution() {
        // is_resource_zone 标志必须真正切换分布表：同一批 index 在 resource=true 下
        // 高境界（凝脉起）占比必须明显高于 resource=false（否则该参数被忽略/接反）。
        let zone = zone();
        let n = 1000u32;
        let count_high = |is_resource: bool| {
            (0..n)
                .filter(|&i| {
                    let realm = dormant_rogue_seed_snapshot(&zone, i, i, 0, 0.8, is_resource)
                        .cultivation
                        .realm;
                    matches!(realm, Realm::Condense | Realm::Solidify | Realm::Spirit)
                })
                .count()
        };
        let resource_high = count_high(true);
        let background_high = count_high(false);
        assert!(
            resource_high > background_high,
            "resource 档凝脉+固元+通灵计数 {resource_high} 必须明显高于 background 档 \
             {background_high}（N={n}），否则 is_resource_zone 参数未真正接线"
        );
    }

    // ── plan-offscreen-war-v1 P2：离屏派系互殴战死闭环（饱和单测） ─────────────
    //
    // 测契约不测实现：断言 store 人口回写 / 真元守恒回灌 / death notice 死因 / telemetry
    // outcome / 防吞真元 retain，全部走 `run_dormant_combat_phase` 这个真实结算入口
    // （而非私有中间步），接入面变了也不应红。

    use crate::qi_physics::{QiAccountId, WorldQiAccount};

    /// 一帧 `dormant_global_tick_system` 后收回的全部相关 event（P2 死亡/战果 + P3 待物化遗物）。
    /// 用具名 struct 而非裸三元组，让断言读起来是 `events.relics` 而非 `.2`。
    struct CombatTickEvents {
        deaths: Vec<NpcDeathNotice>,
        outcomes: Vec<DormantCombatOutcome>,
        relics: Vec<PendingDormantRelicCreated>,
    }

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
            memory: None,
            player_reputation: None,
            faction: Some(FactionMembership {
                faction_id: faction,
                rank: FactionRank::Disciple,
                reputation: Reputation::default(),
                lineage: None,
                mission_queue: MissionQueue::default(),
            }),
            // 显式群体留空：combat 候选默认走 faction 派生（Attack→0 / Defend→1），覆盖
            // 非破坏迁移路径；需要显式群体的测试单独 set 该字段。
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            tsy_sentinel: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
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
    ) -> CombatTickEvents {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.add_event::<PendingDormantRelicCreated>();
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
        let relics = app
            .world_mut()
            .resource_mut::<Events<PendingDormantRelicCreated>>()
            .drain()
            .collect::<Vec<_>>();

        // 写回更新后的资源到调用方引用。
        *store = app
            .world_mut()
            .remove_resource::<NpcDormantStore>()
            .unwrap();
        *zones = app.world_mut().remove_resource::<ZoneRegistry>().unwrap();
        *ledger = app.world_mut().remove_resource::<WorldQiAccount>().unwrap();
        CombatTickEvents {
            deaths,
            outcomes,
            relics,
        }
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

        let CombatTickEvents {
            deaths, outcomes, ..
        } = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

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

        let CombatTickEvents {
            deaths, outcomes, ..
        } = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

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

        let CombatTickEvents { deaths, .. } =
            run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

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

        let CombatTickEvents { deaths, .. } =
            run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

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
    fn zone_full_settles_loser_into_fixed_overflow_without_shadow() {
        let mut full_zone = zone();
        full_zone.spirit_qi = 1.0;
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

        let CombatTickEvents {
            deaths, outcomes, ..
        } = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(deaths.len(), 1);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].qi_released, 0.0);
        assert_eq!(store.len(), 1, "a durably settled loser must be removed");
        assert!(
            (ledger.balance(&crate::qi_physics::qi_flow_overflow_account()) - 5.0).abs() < 1e-9,
            "full-zone death must move every rejected qi unit into fixed durable overflow"
        );
        assert!(
            !ledger.iter_balances().any(|(account, _)| matches!(
                account.kind,
                crate::qi_physics::QiAccountKind::Npc | crate::qi_physics::QiAccountKind::Zone
            )),
            "full-zone settlement must not leave NPC or Zone ledger shadows"
        );
    }

    #[test]
    fn settlement_failure_retains_loser_and_marks_store_dirty() {
        const RUN_TICK: u64 = 9;

        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut atk = combat_snapshot("atk", FactionId::Attack, 5.0, DVec3::new(10.0, 64.0, 10.0));
        let mut def = combat_snapshot("def", FactionId::Defend, 5.0, DVec3::new(11.0, 64.0, 11.0));
        atk.last_dormant_tick_processed = RUN_TICK;
        def.last_dormant_tick_processed = RUN_TICK;

        let mut store = NpcDormantStore::default();
        store.insert(atk);
        store.insert(def);

        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<DormantCombatOutcome>();
        app.add_event::<PendingDormantRelicCreated>();
        app.insert_resource(NpcVirtualizationConfig {
            dormant_tick_interval_ticks: 1,
            ..NpcVirtualizationConfig::default()
        });
        app.insert_resource(GameTick(RUN_TICK as u32));
        app.insert_resource(FactionStore::default());
        app.insert_resource(ZoneRegistry {
            zones: vec![invalid_zone],
        });
        app.insert_resource(WorldQiAccount::default());
        store.take_dirty();
        app.insert_resource(store);
        app.add_systems(Update, dormant_global_tick_system);

        app.update();

        let mut store = app
            .world_mut()
            .remove_resource::<NpcDormantStore>()
            .expect("store resource must survive the tick");
        assert_eq!(
            store.len(),
            2,
            "an invalid signed-zone owner must fail closed and retain the dead snapshot"
        );
        let retained_loser = store
            .snapshots
            .values()
            .find(|snapshot| snapshot.combat_dead_pending_release)
            .expect("the logically dead loser must be excluded from future combat while settlement retries");
        assert!(
            retained_loser.cultivation.qi_current() > QI_EPSILON,
            "failed settlement must leave the dormant physical owner untouched"
        );
        assert!(
            store.take_dirty(),
            "retaining a logically dead snapshot must persist the retry marker"
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

        let CombatTickEvents {
            deaths, outcomes, ..
        } = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

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
    fn offscreen_war_conserves_physical_owner_total_without_actor_or_zone_shadows() {
        // R5 owner model: dormant Cultivation + signed Zone.spirit_qi + durable stable pools
        // are the physical owners. Player/NPC/Zone ledger mirrors must not be seeded or left behind.
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut store = NpcDormantStore::default();
        const N: u32 = 500;
        for i in 0..N {
            let faction = if i % 2 == 0 {
                FactionId::Attack
            } else {
                FactionId::Defend
            };
            store.insert(combat_snapshot(
                &format!("w{i:04}"),
                faction,
                4.0,
                DVec3::new(10.0 + (i % 50) as f64, 64.0, 10.0 + (i / 50) as f64),
            ));
        }

        let mut ledger = WorldQiAccount::default();
        let before = physical_owner_total(&store, &zones, &ledger);
        let initial_pop = store.len();
        let config = NpcVirtualizationConfig {
            max_combats_per_zone: 64,
            sim_seed: 20_260_531,
            ..Default::default()
        };
        let mut total_deaths = 0usize;
        for round in 0..10u64 {
            let CombatTickEvents { deaths, .. } =
                run_combat_tick(&mut store, &mut zones, &mut ledger, &config, round + 1);
            total_deaths += deaths.len();
            let mid = physical_owner_total(&store, &zones, &ledger);
            assert!(
                (before - mid).abs() < 1e-9,
                "round {round} changed dormant+zone+stable-owner qi: before={before}, after={mid}, deaths={total_deaths}"
            );
            assert_no_dormant_or_zone_shadows(&store, &zones, &ledger);
        }

        assert!(
            total_deaths > 0,
            "multi-round dormant combat must settle real deaths; got none"
        );
        assert!(
            initial_pop - store.len() <= total_deaths,
            "removed dormant population cannot exceed declared combat deaths"
        );
        assert!(
            (before - physical_owner_total(&store, &zones, &ledger)).abs() < 1e-9,
            "final dormant+zone+stable-owner qi total drifted"
        );
    }

    // ── plan-offscreen-war-v1 P3：克制式战场遗物结算（守恒 + 克制 + 时序窗口） ────

    /// 造一个**指定 archetype** 的战斗候选快照（其余同 `combat_snapshot`）。
    /// 用于 P3 区分"知名战死者（Disciple/GuardianRelic/有派系）留遗物 vs 普通 rogue 不留"。
    fn combat_snapshot_named(
        char_id: &str,
        archetype: NpcArchetype,
        faction: Option<FactionId>,
        qi_current: f64,
        pos: DVec3,
    ) -> NpcDormantSnapshot {
        // faction=None 时仍需 is_hostile 才能配对 → 用一个有 faction 的对手开打；这里允许
        // None（测试里始终配一个有派系的对手保证开战）。
        let mut snap = combat_snapshot(
            char_id,
            faction.unwrap_or(FactionId::Attack),
            qi_current,
            pos,
        );
        snap.archetype = archetype;
        snap.faction = faction.map(|faction_id| FactionMembership {
            faction_id,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        });
        snap
    }

    #[test]
    fn combat_death_emits_pending_relic_for_named_disciple() {
        // 一对敌对 dormant，败者是 Disciple（必留遗物）。真元充足 zone（spirit_qi=0.8）→ 全额
        // 释放 → 败者本轮移除 → 遗物 event 必 emit。遗物字段（zone/pos/archetype/seed）正确。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot_named(
            "fallen_disciple",
            NpcArchetype::Disciple,
            Some(FactionId::Attack),
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot_named(
            "rival",
            NpcArchetype::Disciple,
            Some(FactionId::Defend),
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        store.take_dirty();
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let events = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert_eq!(
            events.deaths.len(),
            1,
            "exactly one of the two hostile disciples must die per round, got {}",
            events.deaths.len()
        );
        // 两名都是 Disciple ⇒ 无论谁死都该留遗物。
        assert_eq!(
            events.relics.len(),
            1,
            "a fallen Disciple (named, factioned) must emit exactly one PendingDormantRelicCreated; got {} relic events",
            events.relics.len()
        );
        let relic = &events.relics[0];
        let loser_id = &events.deaths[0].npc_id;
        assert_eq!(
            &relic.char_id, loser_id,
            "the relic must belong to the NPC that actually died ({loser_id}), got {}",
            relic.char_id
        );
        assert_eq!(
            relic.archetype,
            NpcArchetype::Disciple,
            "relic archetype must match the fallen NPC's archetype for deterministic loot rolling, got {:?}",
            relic.archetype
        );
        assert_eq!(
            relic.created_tick, 7,
            "relic created_tick must be the settlement tick (7) for deferred-on-hydrate ordering, got {}",
            relic.created_tick
        );
        // loot_seed 必须 == relic_loot_seed(loser, tick, sim_seed)（确定性可复现）。
        let expected_seed = combat::relic_loot_seed(loser_id, 7, config.sim_seed);
        assert_eq!(
            relic.loot_seed, expected_seed,
            "relic loot_seed must equal relic_loot_seed(loser, tick, sim_seed) so hydrate re-rolls identical loot; got {} expected {}",
            relic.loot_seed, expected_seed
        );
    }

    #[test]
    fn combat_death_emits_no_relic_for_plain_factionless_rogue_pair() {
        // 一对**无派系、低境（Awaken）** Rogue。它们无 faction 无法用 faction 配对——为强制
        // 开打，给二者一个共同敌对关系：通过 FactionStore 让 Attack↔Defend 敌对，但 archetype
        // 仍是 Rogue。这里走"有 faction 才配对"的现实约束：给二者 Attack/Defend faction 会
        // 触发 should_leave_relic 的 faction 支。故本测试改测"realm 太低 + 仍有 faction"不成立，
        // 转而锁住真正的"普通无名散修"：Awaken Rogue **无 faction**，用直接调结算函数验证
        // should_leave_relic=false（配对需 faction 是 collect 层的事，与遗物判定解耦）。
        let plain = combat_snapshot_named(
            "nameless_rogue",
            NpcArchetype::Rogue,
            None,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        );
        // 降到最低境，确保 realm 支也不成立。
        let mut plain = plain;
        plain.cultivation.realm = Realm::Awaken;
        assert!(
            !combat::should_leave_relic(&plain),
            "a nameless factionless Awaken-realm Rogue must NOT leave a relic; the combat settlement must skip relic emission for it"
        );
    }

    #[test]
    fn no_relic_emitted_for_factionless_rogue_through_full_combat_tick() {
        // 端到端：两名 **Awaken 无派系** Rogue 无法用 faction 配对开打。为让它们真的开战且
        // 验证"打死了但不留遗物"，借 collect 的现实：配对需 faction。故构造"一名有派系的
        // 高手 vs 一名普通无名 rogue"会让无名方有概率活/死，难确定性断言无名方一定死。
        // 改为更干净的契约锁：两名 **有派系但凝脉（ordinal 2 < 固元）** 的 Rogue 互殴——
        // 它们因 faction 而配对 + 因 faction 而**留**遗物（faction 支）。这验证 faction 支生效。
        // 真正"普通 rogue 不留"由上一条 should_leave_relic 单测 + combat.rs 饱和单测锁死。
        //
        // 这里专门验证一个**反向**端到端事实：把一对 Rogue 的 faction 都设成 None 后，即便
        // 直接喂进 store，combat phase 因无法配对而**根本不开战** ⇒ 0 死亡 0 遗物（绝不
        // 凭空给无派系者造遗物）。
        let mut zones = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut store = NpcDormantStore::default();
        let mut a = combat_snapshot_named(
            "rogue_a",
            NpcArchetype::Rogue,
            None,
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        );
        a.cultivation.realm = Realm::Awaken;
        let mut b = combat_snapshot_named(
            "rogue_b",
            NpcArchetype::Rogue,
            None,
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        );
        b.cultivation.realm = Realm::Awaken;
        store.insert(a);
        store.insert(b);
        store.take_dirty();
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let events = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);

        assert!(
            events.deaths.is_empty(),
            "two factionless Rogues cannot be paired (no hostile faction) ⇒ no combat ⇒ no deaths, got {}",
            events.deaths.len()
        );
        assert!(
            events.relics.is_empty(),
            "no combat ⇒ no relics; factionless Rogues must never produce a battlefield relic, got {}",
            events.relics.len()
        );
    }

    #[test]
    fn settlement_failure_emits_no_relic_and_pending_loser_stays_frozen() {
        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut zones = ZoneRegistry {
            zones: vec![invalid_zone],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot_named(
            "fallen_disciple",
            NpcArchetype::Disciple,
            Some(FactionId::Attack),
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot_named(
            "rival",
            NpcArchetype::Disciple,
            Some(FactionId::Defend),
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let tick_one = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
        assert_eq!(tick_one.deaths.len(), 1);
        assert!(tick_one.relics.is_empty());
        let loser_id = tick_one.deaths[0].npc_id.clone();
        let retained = store
            .snapshots
            .get(&loser_id)
            .expect("failed settlement must retain the dormant physical owner");
        assert!(retained.combat_dead_pending_release);
        let position = retained.position;
        let qi = retained.cultivation.qi_current();
        let realm = retained.cultivation.realm;

        let tick_two = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);
        assert!(tick_two.deaths.is_empty());
        assert!(tick_two.outcomes.is_empty());
        assert!(tick_two.relics.is_empty());
        let still = store
            .snapshots
            .get(&loser_id)
            .expect("repeated failed settlement must keep the same dormant owner");
        assert!(still.combat_dead_pending_release);
        assert_eq!(still.position, position);
        assert_eq!(still.cultivation.qi_current(), qi);
        assert_eq!(still.cultivation.realm, realm);
    }

    #[test]
    fn pending_release_retries_then_emits_one_relic_after_zone_recovers() {
        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut zones = ZoneRegistry {
            zones: vec![invalid_zone],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot_named(
            "fallen_disciple",
            NpcArchetype::Disciple,
            Some(FactionId::Attack),
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot_named(
            "rival",
            NpcArchetype::Disciple,
            Some(FactionId::Defend),
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let tick_one = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
        let loser_id = tick_one.deaths[0].npc_id.clone();
        assert!(store.snapshots[&loser_id].combat_dead_pending_release);
        zones.zones[0].spirit_qi = 0.8;

        let tick_two = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 8);
        assert!(tick_two.deaths.is_empty());
        assert!(tick_two.outcomes.is_empty());
        assert_eq!(tick_two.relics.len(), 1);
        assert_eq!(tick_two.relics[0].char_id, loser_id);
        assert!(!store.contains(&loser_id));
        assert!(
            !ledger.has_account(&QiAccountId::npc(loser_id.clone()))
                && !ledger.has_account(&QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME)),
            "retry settlement must still avoid dormant and Zone ledger shadows"
        );
    }
    #[test]
    fn pending_release_retry_freezes_per_char_state_and_preserves_owners_and_audit_each_tick() {
        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut zones = ZoneRegistry {
            zones: vec![invalid_zone],
        };
        let mut store = NpcDormantStore::default();
        let mut pending = combat_snapshot_named(
            "fallen_disciple",
            NpcArchetype::Disciple,
            Some(FactionId::Attack),
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        );
        pending.combat_dead_pending_release = true;
        pending.intent = DormantBehaviorIntent::PatrolToward {
            target: vec3_to_array(DVec3::new(90.0, 64.0, 90.0)),
        };
        pending.last_dormant_tick_processed = 0;
        pending.lifespan.age_ticks = 10.0;
        let pending_id = pending.char_id.clone();
        store.insert(pending);
        store.take_dirty();
        let baseline = store.snapshots[&pending_id].clone();
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        for tick in [1200, 2400, 3600, 4800] {
            let zone_before = zones.zones[0].spirit_qi;
            let ledger_before = ledger.clone();
            let events = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, tick);

            assert!(events.deaths.is_empty(), "tick={tick}");
            assert!(events.outcomes.is_empty(), "tick={tick}");
            assert!(events.relics.is_empty(), "tick={tick}");
            let retained = store
                .snapshots
                .get(&pending_id)
                .expect("failed retry must retain the same physical owner");
            assert!(retained.combat_dead_pending_release, "tick={tick}");
            assert_eq!(retained.position, baseline.position, "tick={tick}");
            assert_eq!(retained.intent, baseline.intent, "tick={tick}");
            assert_eq!(
                retained.last_dormant_tick_processed, baseline.last_dormant_tick_processed,
                "logical death must freeze the dormant clock at tick={tick}"
            );
            assert_eq!(
                retained.lifespan.age_ticks, baseline.lifespan.age_ticks,
                "tick={tick}"
            );
            assert_eq!(
                retained.lifespan.max_age_ticks, baseline.lifespan.max_age_ticks,
                "tick={tick}"
            );
            assert_eq!(retained.cultivation, baseline.cultivation, "tick={tick}");
            assert_eq!(
                retained.life_record.character_id, baseline.life_record.character_id,
                "tick={tick}"
            );
            assert_eq!(
                retained.life_record.created_at, baseline.life_record.created_at,
                "tick={tick}"
            );
            assert_eq!(
                retained.life_record.biography.len(),
                baseline.life_record.biography.len(),
                "tick={tick}"
            );
            assert_eq!(
                retained.life_record.death_insights, baseline.life_record.death_insights,
                "tick={tick}"
            );
            assert_eq!(
                retained.life_record.skill_milestones, baseline.life_record.skill_milestones,
                "tick={tick}"
            );
            assert!(zones.zones[0].spirit_qi.is_nan() && zone_before.is_nan());
            assert_eq!(
                ledger.balance(&crate::qi_physics::qi_flow_overflow_account()),
                0.0,
                "tick={tick}"
            );
            assert_eq!(ledger.transfers(), ledger_before.transfers(), "tick={tick}");
            assert_eq!(ledger.total(), ledger_before.total(), "tick={tick}");
            assert_no_dormant_or_zone_shadows(&store, &zones, &ledger);
        }
    }

    #[test]
    fn failed_pending_release_roundtrips_without_duplicate_events_then_finalizes_once() {
        let mut invalid_zone = zone();
        invalid_zone.spirit_qi = f64::NAN;
        let mut zones = ZoneRegistry {
            zones: vec![invalid_zone],
        };
        let mut store = NpcDormantStore::default();
        store.insert(combat_snapshot_named(
            "fallen_disciple",
            NpcArchetype::Disciple,
            Some(FactionId::Attack),
            5.0,
            DVec3::new(10.0, 64.0, 10.0),
        ));
        store.insert(combat_snapshot_named(
            "rival",
            NpcArchetype::Disciple,
            Some(FactionId::Defend),
            5.0,
            DVec3::new(11.0, 64.0, 11.0),
        ));
        store.take_dirty();
        let mut ledger = WorldQiAccount::default();
        let config = NpcVirtualizationConfig::default();

        let initial = run_combat_tick(&mut store, &mut zones, &mut ledger, &config, 7);
        assert_eq!(initial.deaths.len(), 1);
        assert_eq!(initial.outcomes.len(), 1);
        assert!(initial.relics.is_empty());
        let loser_id = initial.deaths[0].npc_id.clone();
        let retained_qi = store.snapshots[&loser_id].cultivation.qi_current();
        let payload = store
            .to_redis_hash_payloads()
            .expect("pending loser must serialize")
            .into_iter()
            .find(|(id, _)| id == &loser_id)
            .expect("pending loser must be present in the Redis hash payload")
            .1;
        let restored_loser: NpcDormantSnapshot = serde_json::from_str(&payload)
            .expect("pending loser must survive Redis JSON roundtrip");
        assert!(restored_loser.combat_dead_pending_release);
        assert_eq!(restored_loser.cultivation.qi_current(), retained_qi);

        let mut restored_store = NpcDormantStore::default();
        restored_store.insert(restored_loser);
        restored_store.take_dirty();
        for tick in [8, 9, 10] {
            let events =
                run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, tick);
            assert!(events.deaths.is_empty(), "tick={tick}");
            assert!(events.outcomes.is_empty(), "tick={tick}");
            assert!(events.relics.is_empty(), "tick={tick}");
            assert_eq!(
                restored_store.snapshots[&loser_id].cultivation.qi_current(),
                retained_qi,
                "failed retry must preserve the actor owner at tick={tick}"
            );
            assert!(ledger.transfers().is_empty(), "tick={tick}");
        }

        zones.zones[0].spirit_qi = 0.8;
        let recovered = run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, 11);
        assert!(recovered.deaths.is_empty());
        assert!(recovered.outcomes.is_empty());
        assert_eq!(recovered.relics.len(), 1);
        assert_eq!(recovered.relics[0].char_id, loser_id);
        assert!(!restored_store.contains(&loser_id));
        assert_eq!(ledger.transfers().len(), 1);
        assert_eq!(
            ledger.transfers()[0].reason,
            QiTransferReason::ReleaseToZone
        );

        let after_finalize =
            run_combat_tick(&mut restored_store, &mut zones, &mut ledger, &config, 12);
        assert!(after_finalize.deaths.is_empty());
        assert!(after_finalize.outcomes.is_empty());
        assert!(after_finalize.relics.is_empty());
        assert_eq!(
            ledger.transfers().len(),
            1,
            "a finalized pending loser must not settle or emit again"
        );
    }

    fn physical_owner_total(
        store: &NpcDormantStore,
        zones: &ZoneRegistry,
        ledger: &WorldQiAccount,
    ) -> f64 {
        let dormant_qi: f64 = store
            .snapshots
            .values()
            .map(|snapshot| snapshot.cultivation.qi_current())
            .sum();
        let zone_qi: f64 = zones
            .zones
            .iter()
            .map(|zone| zone.spirit_qi * QI_ZONE_UNIT_CAPACITY)
            .sum();
        dormant_qi + zone_qi + ledger.total()
    }

    fn assert_no_dormant_or_zone_shadows(
        store: &NpcDormantStore,
        zones: &ZoneRegistry,
        ledger: &WorldQiAccount,
    ) {
        for snapshot in store.snapshots.values() {
            assert!(
                !ledger.has_account(&QiAccountId::npc(snapshot.char_id.clone())),
                "dormant actor `{}` must remain owned by its snapshot, not a ledger shadow",
                snapshot.char_id
            );
        }
        for zone in &zones.zones {
            assert!(
                !ledger.has_account(&QiAccountId::zone(zone.name.clone())),
                "zone `{}` must remain owned by Zone.spirit_qi, not a ledger shadow",
                zone.name
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // plan-npc-realm-distribution-v1 P3 §8.1 #3 — 存量 dormant 快照迁移 + marker 幂等
    // ─────────────────────────────────────────────────────────────────────────
    //
    // 每个测试都用 `BONG_NPC_REALM_MIGRATION_MARKER_PATH` 把 marker 路径钉到临时目录，
    // 绝不能碰真实 checkout 里的 `server/data/npc/realm_migration_v1.marker`
    // （那是运行时生成的非提交产物，见 .gitignore）。`ENV_LOCK` 序列化对该 env var 的
    // 读写，防止并行跑的测试互相踩脚（`cargo test` 默认多线程跑同进程内的测试）。

    // `cargo test` 默认多线程并发跑同进程内的测试，而 `std::env::set_var` 是进程级全局
    // 状态——若锁只在 `set`/`drop` 内瞬时持有，两个测试仍可能交错（A 设置 env var 后、
    // 在 A 的 `app.update()` 读取它之前，B 的 `set()` 把它改成另一个路径），A 就会读到
    // 错误的 marker 路径而误判"marker 不存在"。必须让 guard 存活到整个测试结束（挂在
    // `ScopedMarkerEnvVar` 实例上，随其 Drop 才释放），而不是只在设置那一刻短暂加锁。
    struct ScopedMarkerEnvVar {
        _guard: std::sync::MutexGuard<'static, ()>,
        previous: Option<std::ffi::OsString>,
    }

    static MARKER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    impl ScopedMarkerEnvVar {
        fn set(path: &std::path::Path) -> Self {
            let guard = MARKER_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            let previous = std::env::var_os(NPC_REALM_MIGRATION_MARKER_ENV_VAR);
            std::env::set_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR, path);
            Self {
                _guard: guard,
                previous,
            }
        }
    }

    impl Drop for ScopedMarkerEnvVar {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR, previous);
            } else {
                std::env::remove_var(NPC_REALM_MIGRATION_MARKER_ENV_VAR);
            }
        }
    }

    fn unique_marker_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("current time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("bong-realm-migration-{label}-{nanos}.marker"))
    }

    fn legacy_rogue_snapshot(char_id: &str) -> NpcDormantSnapshot {
        let mut snap = snapshot(char_id, DVec3::new(5.0, 64.0, 5.0));
        // P0-era bug state: realm 恒醒灵，qi_max 恒 Cultivation::default() 的 10.0。
        snap.cultivation.realm = Realm::Awaken;
        snap.cultivation.qi_max = 10.0;
        snap.shared_lifespan = LifespanComponent::for_realm(Realm::Awaken);
        snap
    }

    fn migration_test_app(zone_registry: ZoneRegistry, store: NpcDormantStore) -> App {
        let mut app = App::new();
        app.insert_resource(zone_registry);
        app.insert_resource(store);
        app.insert_resource(DormantRoguePopulationSeedConfig::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.add_systems(Update, migrate_dormant_realm_distribution_v1);
        app
    }

    #[test]
    fn migration_marker_path_pinned_to_spec() {
        // §8.1 #3 落点原文：`data/npc/realm_migration_v1.marker`。防止路径漂移。
        assert_eq!(
            NPC_REALM_MIGRATION_MARKER_DEFAULT_PATH, "data/npc/realm_migration_v1.marker",
            "marker 默认路径必须逐字对拍 plan §8.1 #3 落点原文，不允许漂移"
        );
    }

    #[test]
    fn no_marker_triggers_reroll_and_writes_marker_file() {
        let marker_path = unique_marker_path("reroll");
        let _env = ScopedMarkerEnvVar::set(&marker_path);
        assert!(
            !marker_path.exists(),
            "precondition: 临时 marker 路径不应预先存在"
        );

        let mut store = NpcDormantStore::default();
        for i in 0..40u32 {
            let snap = legacy_rogue_snapshot(&format!("legacy:rogue:{i}"));
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();

        let mut z = zone();
        z.spirit_qi = 0.1; // 低于 default threshold 0.4 -> background zone
        let registry = ZoneRegistry { zones: vec![z] };

        let mut app = migration_test_app(registry, store);
        app.update();

        assert!(
            marker_path.exists(),
            "迁移完成后应写出 marker 文件到 {}",
            marker_path.display()
        );

        let migrated_store = app.world().resource::<NpcDormantStore>();
        assert!(
            migrated_store.is_dirty(),
            "至少一条快照 realm 变化应把 store 标脏，否则重 roll 结果不会持久化"
        );

        // 逐条对拍：迁移结果必须与「直接调用同一份 §8.1 #1 抽样函数」完全一致——
        // 测契约（"迁移是否正确委托给规范抽样函数"），不是重新验证分布算法本身
        // （分布算法已由 sample_rogue_seed_realm 的专属测试饱和覆盖）。
        let mut saw_non_awaken = false;
        for i in 0..40u32 {
            let char_id = format!("legacy:rogue:{i}");
            let expected = sample_rogue_seed_realm(char_id.as_str(), false);
            let actual = migrated_store
                .snapshots
                .get(&char_id)
                .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"))
                .cultivation
                .realm;
            assert_eq!(
                actual, expected,
                "char_id={char_id}: 迁移后的 realm 应等于 sample_rogue_seed_realm 对同一 \
                 char_id/is_resource_zone 的确定性抽样结果，实得 {actual:?} 期望 {expected:?}"
            );
            if actual != Realm::Awaken {
                saw_non_awaken = true;
            }
            // qi_max 必须随新 realm 同步重算，不能停在 bug 时代的 10.0（除非新 realm 恰好
            // 仍是 Awaken，此时 10.0 本就是对的）。
            let expected_qi_max = qi_max_for_realm(expected);
            let actual_qi_max = migrated_store
                .snapshots
                .get(&char_id)
                .unwrap()
                .cultivation
                .qi_max;
            assert!(
                (actual_qi_max - expected_qi_max).abs() < 1e-9,
                "char_id={char_id}: qi_max 应随迁移后的 realm={expected:?} 重算为 \
                 {expected_qi_max}，实得 {actual_qi_max}"
            );
        }
        assert!(
            saw_non_awaken,
            "40 条 legacy 快照跑一遍 §8.1 #1 分布表重抽样，至少应有一条不再是醒灵 \
             （分布表醒灵权重远小于 100%），否则说明重 roll 根本没生效"
        );
    }

    #[test]
    fn migration_reroll_resyncs_meridian_system_to_new_realm_required_meridians() {
        // Verify blocker pin：迁移器此前只重算 realm/qi_max/shared_lifespan，从不重派
        // meridian_system——重 roll 到凝脉/固元/通灵后仍停在迁移前的开脉数，与新 realm
        // 脱钩。legacy_rogue_snapshot 起点固定 Realm::Awaken + snapshot() 默认全闭经脉
        // （P0-era 状态），迁移后必须让 meridian_system.opened_count() 追上新 realm。
        let marker_path = unique_marker_path("meridian-resync");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        let mut store = NpcDormantStore::default();
        for i in 0..40u32 {
            let mut snap = legacy_rogue_snapshot(&format!("legacy:meridian:{i}"));
            // 复刻真实 P0-era 生产快照的形状：修 dormant_rogue_seed_snapshot 之前恒开
            // 1 条肺经（而非 legacy_rogue_snapshot 继承的通用 test helper 全闭默认值）
            // ——否则本测试会在「重抽样恰好落回 Awaken（未改变）」的分支上，把「legacy
            // fixture 本身形状失真」误判成「迁移器没有同步重派」的假阳性。
            snap.meridian_system = MeridianSystem::default();
            snap.meridian_system
                .get_mut(crate::cultivation::components::MeridianId::Lung)
                .opened = true;
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();

        let mut z = zone();
        z.spirit_qi = 0.9; // 高于阈值 -> resource zone，拉高凝脉/固元/通灵命中率
        let registry = ZoneRegistry { zones: vec![z] };

        let mut app = migration_test_app(registry, store);
        app.update();

        let migrated_store = app.world().resource::<NpcDormantStore>();
        let mut saw_multi_meridian_realm = false;
        for i in 0..40u32 {
            let char_id = format!("legacy:meridian:{i}");
            let snap = migrated_store
                .snapshots
                .get(&char_id)
                .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"));
            let expected_count = snap.cultivation.realm.required_meridians();
            let actual_count = snap.meridian_system.opened_count();
            assert_eq!(
                actual_count, expected_count,
                "char_id={char_id}: 迁移重 roll 后 realm={:?} 要求开 {expected_count} 条经脉，\
                 但 meridian_system 实开 {actual_count} 条——meridian_system 没有随 realm 重 roll \
                 同步重派（迁移器只改了 realm/qi_max/shared_lifespan）",
                snap.cultivation.realm
            );
            if expected_count > 1 {
                saw_multi_meridian_realm = true;
            }
        }
        assert!(
            saw_multi_meridian_realm,
            "40 条 legacy 快照在 resource zone 下重抽样，至少应有一条落在 required_meridians()>1 \
             的境界（凝脉=6/固元=12/通灵=16），否则本测试没有真正覆盖迁移器重派 \
             meridian_system 的分支（fixture 完整性）"
        );
    }

    #[test]
    fn migration_reroll_respects_permanently_severed_meridians() {
        // minor fix pin：迁移器重派 meridian_system 时用 npc_meridian_system_for_realm
        // 整段覆盖，会把「已被 MeridianSeveredPermanent 永久记录断绝」的经脉也一并
        // 按新 realm 重新打开——这与"永久断脉"语义矛盾（断脉只应在跨周目重置，
        // realm 迁移这种同一角色的境界重 roll 不该复活它）。用 Lung（MeridianId::ALL[0]，
        // 任何 realm 的 required_meridians() >= 1 都会覆盖到它）作为永久断脉标的，
        // 断言迁移后依旧 opened=false。
        let marker_path = unique_marker_path("meridian-severed-respect");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        let mut store = NpcDormantStore::default();
        for i in 0..40u32 {
            let mut snap = legacy_rogue_snapshot(&format!("legacy:severed:{i}"));
            snap.meridian_system = MeridianSystem::default();
            snap.meridian_severed
                .severed_meridians
                .insert(crate::cultivation::components::MeridianId::Lung.channel_id());
            snap.meridian_severed.severed_at.insert(
                crate::cultivation::components::MeridianId::Lung.channel_id(),
                crate::cultivation::meridian::severed::SeveredRecord {
                    at_tick: 0,
                    source: crate::cultivation::meridian::severed::SeveredSource::CombatWound,
                },
            );
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();

        let mut z = zone();
        z.spirit_qi = 0.9; // 高于阈值 -> resource zone，拉高高境界命中率，确保有 realm 变化分支被覆盖
        let registry = ZoneRegistry { zones: vec![z] };

        let mut app = migration_test_app(registry, store);
        app.update();

        let migrated_store = app.world().resource::<NpcDormantStore>();
        let mut saw_changed_realm = false;
        for i in 0..40u32 {
            let char_id = format!("legacy:severed:{i}");
            let snap = migrated_store
                .snapshots
                .get(&char_id)
                .unwrap_or_else(|| panic!("snapshot {char_id} should still exist after migration"));
            if snap.cultivation.realm != Realm::Awaken {
                saw_changed_realm = true;
            }
            let lung = snap
                .meridian_system
                .get(crate::cultivation::components::MeridianId::Lung);
            assert!(
                !lung.opened,
                "char_id={char_id}: Lung 经脉在 meridian_severed 中被永久记录断绝，\
                 迁移重派 meridian_system 后仍必须保持 opened=false（实际 opened=true），\
                 否则永久断脉被 realm 迁移悄悄复活，与 MeridianSeveredPermanent 记录矛盾"
            );
            assert!(
                snap.meridian_severed
                    .severed_meridians
                    .contains(&crate::cultivation::components::MeridianId::Lung.channel_id()),
                "char_id={char_id}: 迁移不应改动 meridian_severed 记录本身"
            );
        }
        assert!(
            saw_changed_realm,
            "40 条 legacy 快照在 resource zone 下重抽样，至少应有一条 realm 发生变化，\
             否则本测试没有真正覆盖迁移器重派 meridian_system 的分支（fixture 完整性）"
        );
    }

    #[test]
    fn marker_already_exists_skips_reroll_idempotently() {
        let marker_path = unique_marker_path("skip");
        let _env = ScopedMarkerEnvVar::set(&marker_path);
        // 预先写好 marker——模拟"上次已经迁移过"。
        if let Some(parent) = marker_path.parent() {
            std::fs::create_dir_all(parent).expect("temp dir must be creatable");
        }
        std::fs::write(&marker_path, b"v1\n").expect("precondition marker write must succeed");

        let mut store = NpcDormantStore::default();
        for i in 0..10u32 {
            let snap = legacy_rogue_snapshot(&format!("legacy:skip:{i}"));
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();
        // 显式确认起点是 clean（构造过程没有调用任何 mark_dirty 路径）。
        assert!(!store.is_dirty());

        let registry = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut app = migration_test_app(registry, store);
        app.update();

        let after = app.world().resource::<NpcDormantStore>();
        assert!(
            !after.is_dirty(),
            "marker 已存在时不应做任何重 roll，store 不该被标脏"
        );
        for i in 0..10u32 {
            let char_id = format!("legacy:skip:{i}");
            let realm = after.snapshots.get(&char_id).unwrap().cultivation.realm;
            assert_eq!(
                realm,
                Realm::Awaken,
                "char_id={char_id}: marker 已存在应跳过重 roll，realm 应保持迁移前的 \
                 Realm::Awaken（bug 时代遗留值），实得 {realm:?}"
            );
        }

        // 内容也保持不变（同一份写入的 marker 内容原样保留，未被二次改写）。
        let content = std::fs::read_to_string(&marker_path).expect("marker should still exist");
        assert_eq!(content, "v1\n");
    }

    #[test]
    fn identity_archetypes_write_identity_realm_not_sampled() {
        let marker_path = unique_marker_path("identity");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        let mut store = NpcDormantStore::default();

        let mut guardian = legacy_rogue_snapshot("legacy:guardian");
        guardian.archetype = NpcArchetype::GuardianRelic;
        store.snapshots.insert(guardian.char_id.clone(), guardian);

        let mut zhinian = legacy_rogue_snapshot("legacy:zhinian");
        zhinian.archetype = NpcArchetype::Zhinian;
        store.snapshots.insert(zhinian.char_id.clone(), zhinian);

        let mut daoxiang = legacy_rogue_snapshot("legacy:daoxiang");
        daoxiang.archetype = NpcArchetype::Daoxiang;
        store.snapshots.insert(daoxiang.char_id.clone(), daoxiang);

        let mut beast = legacy_rogue_snapshot("legacy:beast");
        beast.archetype = NpcArchetype::Beast;
        store.snapshots.insert(beast.char_id.clone(), beast);

        let mut leader = legacy_rogue_snapshot("legacy:leader");
        leader.archetype = NpcArchetype::Disciple;
        leader.faction = Some(FactionMembership {
            faction_id: FactionId::Defend, // CangyuanMerchants -> Spirit
            rank: FactionRank::Leader,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        });
        store
            .snapshots
            .insert(leader.char_id.clone(), leader.clone());

        store.rebuild_indexes();

        let registry = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut app = migration_test_app(registry, store);
        app.update();

        let after = app.world().resource::<NpcDormantStore>();
        let realm_of = |id: &str| after.snapshots.get(id).unwrap().cultivation.realm;

        assert_eq!(
            realm_of("legacy:guardian"),
            Realm::Spirit,
            "GuardianRelic 身份 realm 应直写 Spirit，不参与分布抽样"
        );
        assert_eq!(
            realm_of("legacy:zhinian"),
            Realm::Condense,
            "Zhinian 身份 realm 应直写 Condense"
        );
        assert_eq!(
            realm_of("legacy:daoxiang"),
            Realm::Induce,
            "Daoxiang 身份 realm 应直写 TSY 默认值 Induce"
        );
        assert_eq!(
            realm_of("legacy:beast"),
            Realm::Awaken,
            "Beast 恒字面量 Awaken，迁移不应把它拉进分布抽样（保持设计上的恒低威胁）"
        );
        assert_eq!(
            realm_of("legacy:leader"),
            Realm::Spirit,
            "faction Leader（Defend/CangyuanMerchants）应直写 leader_realm_for 对应的 Spirit，\
             不受分布表影响"
        );
    }

    #[test]
    fn marker_write_failure_does_not_silently_swallow_error() {
        // 制造一个必然写失败的路径：父目录的父目录其实是个*文件*，create_dir_all 会报错。
        let blocked_parent = unique_marker_path("blocked-parent-file");
        std::fs::write(&blocked_parent, b"i am a file, not a directory")
            .expect("setup: create the blocking regular file");
        let marker_path = blocked_parent.join("sub").join("realm_migration_v1.marker");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        let mut store = NpcDormantStore::default();
        let snap = legacy_rogue_snapshot("legacy:writefail");
        store.snapshots.insert(snap.char_id.clone(), snap);
        store.rebuild_indexes();

        // 显式钉 zone 灵气档为 background（低于 default threshold 0.4），让下面的
        // `sample_rogue_seed_realm(..., false)` 探测与迁移器内部真实算出的 is_resource
        // 保持一致，不依赖 `zone()` helper 默认值今后是否变动。
        let mut z = zone();
        z.spirit_qi = 0.1;
        let registry = ZoneRegistry { zones: vec![z] };
        let mut app = migration_test_app(registry, store);

        // 直接调用底层写函数验证返回值语义（true=成功/false=失败），不靠系统副作用间接推断。
        assert!(
            !write_realm_migration_marker(&marker_path),
            "父目录路径被文件挡住时，写 marker 必须返回失败而不是假装成功"
        );
        assert!(
            !marker_path.exists(),
            "写失败后 marker 文件不应该神奇地出现"
        );

        // 即使 marker 落盘失败，system 跑一遍仍必须完成本次的 realm 重 roll（降级只影响
        // "下次重启是否会重复重 roll"这个幂等信号，不能连本次的迁移本体都吞掉）。
        app.update();
        let after = app.world().resource::<NpcDormantStore>();
        let realm = after
            .snapshots
            .get("legacy:writefail")
            .unwrap()
            .cultivation
            .realm;
        let expected = sample_rogue_seed_realm("legacy:writefail", false);
        assert_eq!(
            realm, expected,
            "marker 写失败不该连带吞掉本次 reroll 本身——快照 realm 仍应等于确定性抽样结果"
        );

        let _ = std::fs::remove_file(&blocked_parent);
    }

    #[test]
    fn empty_store_writes_marker_without_touching_anything() {
        let marker_path = unique_marker_path("empty-store");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        let store = NpcDormantStore::default();
        assert!(store.is_empty());
        let registry = ZoneRegistry {
            zones: vec![zone()],
        };
        let mut app = migration_test_app(registry, store);
        app.update();

        assert!(
            marker_path.exists(),
            "空 store（新世界，无存量）也应该写 marker，避免每次 Startup 重复判定空 store"
        );
        assert!(
            !app.world().resource::<NpcDormantStore>().is_dirty(),
            "空 store 没有任何快照可改，不应被标脏"
        );
    }

    #[test]
    fn migration_pushes_zone_perception_narration_for_upgraded_realms() {
        let marker_path = unique_marker_path("narration");
        let _env = ScopedMarkerEnvVar::set(&marker_path);

        // 显式钉 zone 灵气档为 background（低于 default threshold 0.4），让下面的
        // `sample_rogue_seed_realm(..., false)` 探测与迁移器内部真实算出的 is_resource
        // 保持一致，不依赖 `zone()` helper 默认值今后是否变动。
        //
        // 固定挑一个 char_id，其分布抽样结果已知会命中 Condense 或更高（用同一份抽样函数
        // 先探测，避免测试跟迁移函数各自实现一套判定逻辑）。
        let mut store = NpcDormantStore::default();
        let mut hit_condense_or_above = None;
        for i in 0..200u32 {
            let char_id = format!("legacy:narration:{i}");
            let realm = sample_rogue_seed_realm(char_id.as_str(), false);
            if matches!(realm, Realm::Condense | Realm::Solidify) {
                hit_condense_or_above = Some((char_id.clone(), realm));
            }
            let snap = legacy_rogue_snapshot(&char_id);
            store.snapshots.insert(snap.char_id.clone(), snap);
        }
        store.rebuild_indexes();
        let (hit_char_id, hit_realm) = hit_condense_or_above.expect(
            "200 条随机 char_id 里按分布表理应至少命中一条 Condense/Solidify，\
             否则下面的 narration 断言无法验证任何真实行为",
        );

        let mut z = zone();
        z.spirit_qi = 0.1;
        let registry = ZoneRegistry { zones: vec![z] };
        let mut app = migration_test_app(registry, store);
        app.update();

        let mut narrations = app.world_mut().resource_mut::<PendingGameplayNarrations>();
        let drained = narrations.drain();
        assert!(
            !drained.is_empty(),
            "命中 char_id={hit_char_id} 应重 roll 出 {hit_realm:?}，理应推送至少一条 \
             zone-scope 境界识破 narration，实得 0 条"
        );
        assert!(
            drained
                .iter()
                .all(|n| n.scope == crate::schema::common::NarrationScope::Zone
                    && n.style == crate::schema::common::NarrationStyle::Perception),
            "迁移触发的境界识破 narration 必须是 Zone scope + Perception style，实得 {drained:?}"
        );
    }
}
