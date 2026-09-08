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

use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use valence::prelude::{
    bevy_ecs, App, DVec3, Event, EventWriter, IntoSystemConfigs, Res, ResMut, Resource, Startup,
    Update,
};

use crate::body_plan::{resolve_race_to_plan, BodyPlanRegistry, RaceRegistry};
use crate::cultivation::breakthrough::{
    breakthrough_qi_cost, next_realm, qi_max_for_realm, try_breakthrough_with_profile,
    BreakthroughError, BreakthroughSuccess, RollSource, XorshiftRoll, MIN_ZONE_QI_TO_BREAKTHROUGH,
    MIN_ZONE_QI_TO_GUYUAN,
};
use crate::cultivation::components::{
    release_external_qi_to_zone, ActorQiIdentity, ActorQiKind, Contamination, Cultivation,
    MeridianSystem, PersistedCultivationV1, QiFlowError, QiFlowOutcome, Realm,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::fauna::daozhan::{DaoZhangState, FakeBehavior};
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
use crate::qi_physics::{
    constants::QI_NPC_ABSORB_FLOOR, regen_from_zone, QiTransfer, QiTransferReason, WorldQiAccount,
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
pub struct DormantMimicSpiderSnapshot {
    pub state: crate::fauna::mimic_spider::SpiderDisguiseState,
    pub home_zone: String,
    pub home_pos: [f64; 3],
    pub drained_qi: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantDaozhanSnapshot {
    pub state: DaoZhangState,
    pub home_zone: String,
    pub home_pos: [f64; 3],
    pub daozhan_qi: f64,
    pub origin_realm: Option<Realm>,
    pub behavior_queue: Vec<FakeBehavior>,
    pub current_behavior_ticks: u32,
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
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub daozhan: Option<DormantDaozhanSnapshot>,
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
    pub mimic_spider: Option<DormantMimicSpiderSnapshot>,
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
    /// 所有 [`run_dormant_combat_phase`] roll 出的败者都会先置 `true` 并持久化胜者上下文，
    /// 在 Redis HASH 成功确认前**仍留在 `store.snapshots` 且不碰真元**。这关闭了终局事件先于
    /// 逻辑死亡落盘的重启窗口；随后的 typed settlement 若遇到非法 signed Zone、身份或稳定池
    /// overflow 等硬事务失败，败者继续保留（防吞真元红线：携带真元的快照绝不丢弃）。置 `true` 后：
    /// - [`combat::collect_zone_combat_pairs`] 跳过该快照，**不再被选中参战**。
    /// - `pending_combat_winner` 持久化延迟发布所需的胜者身份；失败 tick 不发布 death/outcome。
    /// - 每 tick 的 [`run_pending_combat_release_retry`] 重试 `release_dormant_qi_to_zone`，严格
    ///   成功且 source 为零后才发布唯一 death/outcome，emit 遗物并从 store 移除。
    ///
    /// `#[serde(default)]` 向后兼容旧 Redis 快照（缺字段 → `false`）；`skip_serializing_if`
    /// 让绝大多数（未战死）快照不写这个字段，不算 §10.1 #2 所禁的快照膨胀。
    #[serde(default, skip_serializing_if = "is_false")]
    pub combat_dead_pending_release: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pending_combat_winner: Option<CharId>,
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

    pub fn durable_identity_error(&self) -> Option<String> {
        durable_npc_identity_error(&self.char_id, &self.life_record, &self.death_registry)
    }

    fn durable_qi_owner_error(&self) -> Option<String> {
        self.tsy_hostile
            .as_ref()
            .and_then(|hostile| hostile.daozhan.as_ref())
            .filter(|daozhan| !daozhan.daozhan_qi.is_finite() || daozhan.daozhan_qi < 0.0)
            .map(|daozhan| {
                format!(
                    "invalid dormant Daozhan qi owner `{}` for `{}`",
                    daozhan.daozhan_qi, self.char_id
                )
            })
    }
}

pub fn durable_npc_identity_error(
    canonical_char_id: &str,
    life_record: &LifeRecord,
    death_registry: &DeathRegistry,
) -> Option<String> {
    let valid_canonical = !canonical_char_id.is_empty()
        && canonical_char_id.trim() == canonical_char_id
        && canonical_char_id != "unassigned:life_record";
    if valid_canonical
        && life_record.character_id == canonical_char_id
        && death_registry.char_id == canonical_char_id
    {
        return None;
    }
    Some(format!(
        "durable identity tuple mismatch or invalid canonical id (canonical=`{canonical_char_id}`, life_record=`{}`, death_registry=`{}`)",
        life_record.character_id, death_registry.char_id
    ))
}

#[derive(Clone, Debug)]
struct DormantPersistenceRuntime {
    mutation_revision: u64,
    persisted_revision: u64,
    in_flight_revision: Option<u64>,
    receipt_tx: Sender<crate::network::redis_bridge::RedisDeliveryReceipt>,
    receipt_rx: Receiver<crate::network::redis_bridge::RedisDeliveryReceipt>,
    tombstones: HashMap<CharId, crate::persistence::DormantTerminalCommitRecord>,
}

impl Default for DormantPersistenceRuntime {
    fn default() -> Self {
        let (receipt_tx, receipt_rx) = crossbeam_channel::unbounded();
        Self {
            mutation_revision: 0,
            persisted_revision: 0,
            in_flight_revision: None,
            receipt_tx,
            receipt_rx,
            tombstones: HashMap::new(),
        }
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
    #[serde(skip, default)]
    persistence: DormantPersistenceRuntime,
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
        if !self.dirty {
            self.persistence.mutation_revision =
                self.persistence.mutation_revision.saturating_add(1);
        }
        self.dirty = true;
    }

    pub fn persistence_receipt_sender(
        &self,
    ) -> Sender<crate::network::redis_bridge::RedisDeliveryReceipt> {
        self.persistence.receipt_tx.clone()
    }

    pub fn begin_persistence(&mut self) -> Option<u64> {
        if self.restore_failed || !self.dirty || self.persistence.in_flight_revision.is_some() {
            return None;
        }
        self.dirty = false;
        let revision = self.persistence.mutation_revision;
        self.persistence.in_flight_revision = Some(revision);
        Some(revision)
    }

    pub fn requeue_persistence(&mut self, revision: u64) {
        if self.persistence.in_flight_revision == Some(revision) {
            self.persistence.in_flight_revision = None;
        }
        self.dirty = true;
    }

    pub fn install_terminal_tombstones(
        &mut self,
        records: Vec<crate::persistence::DormantTerminalCommitRecord>,
    ) {
        self.persistence.tombstones = records
            .into_iter()
            .map(|record| (record.char_id.clone(), record))
            .collect();
        if !self.persistence.tombstones.is_empty() {
            self.mark_dirty();
        }
    }

    fn has_terminal_tombstone(&self, char_id: &str) -> bool {
        self.persistence.tombstones.contains_key(char_id)
    }

    fn track_terminal_tombstone(
        &mut self,
        record: crate::persistence::DormantTerminalCommitRecord,
    ) {
        self.persistence
            .tombstones
            .entry(record.char_id.clone())
            .or_insert(record);
    }

    pub fn bind_unbound_terminal_tombstones(
        &mut self,
        settings: &crate::persistence::PersistenceSettings,
        revision: u64,
    ) -> std::io::Result<()> {
        let char_ids: Vec<String> = self
            .persistence
            .tombstones
            .values()
            .filter(|record| record.cleanup_revision.is_none())
            .map(|record| record.char_id.clone())
            .collect();
        if char_ids.is_empty() {
            return Ok(());
        }
        crate::persistence::bind_dormant_terminal_cleanup_revision(settings, &char_ids, revision)?;
        for char_id in char_ids {
            if let Some(record) = self.persistence.tombstones.get_mut(&char_id) {
                record.cleanup_revision = Some(revision);
            }
        }
        Ok(())
    }

    pub fn apply_persistence_receipts(&mut self) {
        self.apply_persistence_receipts_inner(None);
    }

    pub fn apply_persistence_receipts_with_settings(
        &mut self,
        settings: &crate::persistence::PersistenceSettings,
    ) {
        self.apply_persistence_receipts_inner(Some(settings));
    }

    fn apply_persistence_receipts_inner(
        &mut self,
        settings: Option<&crate::persistence::PersistenceSettings>,
    ) {
        while let Ok(receipt) = self.persistence.receipt_rx.try_recv() {
            let Ok(revision) = receipt.delivery_id.parse::<u64>() else {
                tracing::warn!(
                    "[bong][npc] ignored dormant HASH receipt with invalid revision `{}`",
                    receipt.delivery_id
                );
                continue;
            };
            if self.persistence.in_flight_revision != Some(revision) {
                tracing::warn!(
                    "[bong][npc] ignored dormant HASH receipt for non-current revision {revision}"
                );
                continue;
            }
            self.persistence.in_flight_revision = None;
            match receipt.outcome {
                Ok(()) => {
                    self.persistence.persisted_revision = revision;
                    if let Some(settings) = settings {
                        match crate::persistence::clear_dormant_terminal_commits_through_revision(
                            settings, revision,
                        ) {
                            Ok(_) => self.persistence.tombstones.retain(|_, record| {
                                record.cleanup_revision.is_none_or(|bound| bound > revision)
                            }),
                            Err(error) => {
                                self.dirty = true;
                                tracing::warn!(
                                    "[bong][npc] retained terminal tombstones after HASH revision {revision}: {error}"
                                );
                            }
                        }
                    }
                }
                Err(error) => {
                    self.dirty = true;
                    tracing::warn!(
                        "[bong][npc] dormant HASH revision {revision} failed and was re-queued: {error}"
                    );
                }
            }
        }
    }

    fn pending_combat_state_is_persisted(&self) -> bool {
        self.persistence.persisted_revision >= self.persistence.mutation_revision
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
        self.mark_dirty();
        previous
    }

    pub fn remove(&mut self, char_id: &str) -> Option<NpcDormantSnapshot> {
        let removed = self.snapshots.remove(char_id);
        if removed.is_some() {
            self.rebuild_indexes();
            self.mark_dirty();
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
            .filter(|snapshot| !self.has_terminal_tombstone(&snapshot.char_id))
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
    pub winner_group: Option<EmergentGroupId>,
    pub loser_group: Option<EmergentGroupId>,
}

/// plan-offscreen-war-v1 P3：一名**克制判定通过**的离屏战死者要在战场留下的待物化遗物
/// （deferred-on-hydrate）的内部 event。
///
/// 由 `run_dormant_combat_phase` 在败者**真元已守恒释放完毕**且 SQLite terminal transaction
/// 已原子持久化 sink、tombstone 与可选遗物后 emit，作为进程内物化通知。兼容 consumer 对同一
/// deterministic relic id 的重复 upsert 是幂等的，但不再承担终局持久化授权；终局 transaction
/// 失败时不会 emit、remove 或改变任何物理 owner。遗物不携带真元，物化时 `spirit_quality=0`。
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
        .add_systems(
            Startup,
            load_dormant_store_from_redis_system.after(crate::persistence::PersistenceBootstrapSet),
        )
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

fn load_dormant_store_from_redis_system(
    mut store: ResMut<NpcDormantStore>,
    persistence: Res<crate::persistence::PersistenceSettings>,
) {
    if !store.is_empty() {
        return;
    }
    let tombstones = match crate::persistence::rearm_dormant_terminal_commits(&persistence) {
        Ok(records) => records,
        Err(error) => {
            tracing::warn!("[bong][npc] failed dormant terminal tombstone restore: {error}");
            store.mark_restore_failed();
            return;
        }
    };
    store.install_terminal_tombstones(tombstones);
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

const DORMANT_TEMP_SCAN_COUNT: usize = 512;
const DORMANT_TEMP_DELETE_BATCH: usize = 128;

fn tmp_key_delete_batches(scanned: &[String]) -> Vec<Vec<String>> {
    tmp_keys_to_purge(scanned)
        .chunks(DORMANT_TEMP_DELETE_BATCH)
        .map(<[String]>::to_vec)
        .collect()
}

/// Best-effort sweep of leaked dormant temp keys on a blocking connection.
/// Never returns an error: persistence restore must proceed even if the
/// janitor cannot run (e.g. SCAN unsupported by a proxy).
fn purge_leaked_dormant_temp_keys(connection: &mut redis::Connection) {
    let pattern = dormant_tmp_scan_pattern();
    let mut cursor = 0_u64;
    loop {
        let (next_cursor, scanned) = match redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern.as_str())
            .arg("COUNT")
            .arg(DORMANT_TEMP_SCAN_COUNT)
            .query::<(u64, Vec<String>)>(connection)
        {
            Ok(page) => page,
            Err(error) => {
                tracing::warn!(
                    "[bong][npc] dormant temp-key janitor SCAN failed (skipping remaining cleanup): {error}"
                );
                return;
            }
        };

        for batch in tmp_key_delete_batches(&scanned) {
            let mut del = redis::cmd("DEL");
            for key in &batch {
                del.arg(key.as_str());
            }
            match del.query::<i64>(connection) {
                Ok(deleted) => tracing::info!(
                    "[bong][npc] dormant temp-key janitor purged {deleted} leaked `{NPC_DORMANT_REDIS_KEY}:tmp*` key(s)"
                ),
                Err(error) => tracing::warn!(
                    "[bong][npc] dormant temp-key janitor DEL failed (left {} key(s)): {error}",
                    batch.len()
                ),
            }
        }

        if next_cursor == 0 {
            break;
        }
        cursor = next_cursor;
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
        if store.has_terminal_tombstone(&hash_char_id) {
            tracing::warn!(
                "[bong][npc] suppressed stale dormant Redis source `{hash_char_id}` after terminal commit"
            );
            continue;
        }
        match serde_json::from_str::<NpcDormantSnapshot>(&payload) {
            Ok(snapshot) => {
                let validation_error = if snapshot.char_id != hash_char_id {
                    Some(format!(
                        "durable identity `{}` does not match HASH field `{hash_char_id}`",
                        snapshot.char_id
                    ))
                } else {
                    snapshot
                        .durable_identity_error()
                        .or_else(|| snapshot.durable_qi_owner_error())
                };
                if let Some(error) = validation_error {
                    invalid.push(format!("`{hash_char_id}`: {error}"));
                } else {
                    staged.push(snapshot);
                }
            }
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
    persistence: Option<Res<crate::persistence::PersistenceSettings>>,
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
    if let Some(persistence) = persistence.as_deref() {
        store.apply_persistence_receipts_with_settings(persistence);
    } else {
        store.apply_persistence_receipts();
    }
    if !should_run_interval(tick, config.dormant_tick_interval_ticks) {
        return;
    }
    let mut ids = store.snapshots.keys().cloned().collect::<Vec<_>>();
    ids.sort();

    let mut expired = Vec::new();
    let mut committed_tombstones = Vec::new();
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
            let mut staged_snapshot = snapshot.clone();
            let Some(zones) = zones.as_deref_mut() else {
                continue;
            };
            let Some(ledger) = ledger.as_deref_mut() else {
                continue;
            };
            let mut staged_zones = zones.clone();
            let mut staged_ledger = ledger.clone();
            let settlement = if dormant_terminal_qi_is_settled(&staged_snapshot) {
                QiFlowOutcome {
                    requested: 0.0,
                    source_debited: 0.0,
                    target_credited: 0.0,
                    zone_accepted: 0.0,
                    overflow_credited: 0.0,
                    untransferred: 0.0,
                    transfers: Vec::new(),
                }
            } else {
                let Ok(settlement) = release_dormant_qi_to_zone(
                    &mut staged_snapshot,
                    &mut staged_zones,
                    &mut staged_ledger,
                ) else {
                    tracing::warn!(
                        "[bong][npc] retained expired dormant NPC `{}` until all qi owners settle",
                        snapshot.char_id
                    );
                    continue;
                };
                settlement
            };
            if !dormant_terminal_qi_is_settled(&staged_snapshot) {
                continue;
            }

            let tombstone = crate::persistence::DormantTerminalCommitRecord {
                char_id: snapshot.char_id.clone(),
                cause: "natural_aging".to_string(),
                at_tick: tick,
                zone: snapshot.zone_name.clone(),
                winner: None,
                winner_group: None,
                loser_group: faction_store
                    .as_deref()
                    .and_then(|store| effective_group(snapshot, store))
                    .map(|group| u64::from(group.0)),
                zone_accepted: settlement.zone_accepted,
                cleanup_revision: None,
            };
            let first_commit = if let Some(persistence) = persistence.as_deref() {
                match crate::persistence::persist_dormant_terminal_commit(
                    persistence,
                    &tombstone,
                    &staged_zones,
                    &staged_ledger,
                    None,
                ) {
                    Ok(crate::persistence::PersistDormantTerminalOutcome::Committed) => true,
                    Ok(crate::persistence::PersistDormantTerminalOutcome::AlreadyCommitted) => {
                        false
                    }
                    Err(error) => {
                        tracing::warn!(
                            "[bong][npc] retained expired dormant NPC `{}` after terminal persistence failure: {error}",
                            snapshot.char_id
                        );
                        continue;
                    }
                }
            } else {
                true
            };
            if first_commit {
                *snapshot = staged_snapshot;
                *zones = staged_zones;
                *ledger = staged_ledger;
                death_notices.send(dormant_natural_death_notice(snapshot));
            }
            expired.push(char_id);
            committed_tombstones.push(tombstone);
        }
    }

    for tombstone in committed_tombstones {
        store.track_terminal_tombstone(tombstone);
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
    // every newly rolled combat death first records a durable pending marker and
    // winner context. This must drive `mark_dirty` even when the aging pass touched
    // nothing, otherwise a restart can reload the loser as alive. Confirmed pending
    // rows settle and emit terminal events on a later phase.
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
            persistence.as_deref(),
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

    // Any advanced, removed, or newly pending snapshot changed persisted state;
    // schedule the next revision-aware Redis write.
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
/// - [`Self::mutated`] — the phase recorded a logical combat death as
///   `combat_dead_pending_release` without removing the snapshot. Drives only the dirty
///   write; the physical owner and `qi_current` remain unchanged until Redis confirms the
///   marker and a future retry commits settlement.
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
/// **持久化先行**：本轮 roll 出败者后只记录 pending marker + winner context，不碰 qi owner，
/// 也不发布终局事件。Redis HASH 成功回执确认该 revision 后，下一轮 retry 才执行 typed
/// settlement；成功时 zone 不接收的余量同步落入固定 `qi_flow_overflow`，败者
/// `qi_current` 必归零才可发 death/outcome 并移除。硬事务失败则保留 owner 继续重试。
/// 同 zone 多败者按确定性顺序 settlement；物理 owner 总量保持不变。
///
/// 返回 [`CombatPhaseOutcome`]：`removed`（有败者被移除 → rebuild 索引 + mark dirty）与
/// `mutated`（新逻辑死亡写入 pending marker → 只 mark dirty，不 rebuild）。
/// 失败原子性保证 actor/zone/ledger/audit 不变；持久化 flag 防止重启后重复 roll 败者。
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
    persistence: Option<&crate::persistence::PersistenceSettings>,
    death_notices: &mut EventWriter<NpcDeathNotice>,
    combat_outcomes: &mut EventWriter<DormantCombatOutcome>,
    pending_relics: &mut EventWriter<PendingDormantRelicCreated>,
) -> CombatPhaseOutcome {
    // ⓪ 先处理已持久化的逻辑战死者。首次终局 death/outcome 只会在 pending HASH
    // 获得成功回执后发布；typed settlement 失败则保留 owner 和 marker 继续等待后续 retry。
    // pending 快照始终被 `collect_zone_combat_pairs` 排除，不会重新 roll。
    let mut outcome = run_pending_combat_release_retry(
        store,
        faction_store,
        config,
        tick,
        zones,
        ledger,
        persistence,
        death_notices,
        combat_outcomes,
        pending_relics,
    );

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

        // ③ 先持久化逻辑死亡与胜者上下文。本轮不碰任何 qi owner，也不发布终局事件；
        // 只有 Redis HASH 成功确认这个 revision 后，retry 才执行 typed settlement、事件和移除。
        let Some(loser) = store.snapshots.get_mut(&loser_id) else {
            continue;
        };
        loser.combat_dead_pending_release = true;
        loser.pending_combat_winner = Some(winner_id);
        outcome.mutated = true;
    }

    outcome
}

/// plan-offscreen-war-v1 P3 review-fix：重试上轮因守恒事务失败而保留的离屏战死者。
///
/// pending snapshot 已持久化 `pending_combat_winner`，因此本函数可以在 typed settlement
/// 成功且 source 严格为零后发布唯一 death/outcome，再造遗物并移除 owner。缺 winner 的历史
/// 行仍结算并终局，只跳过依赖 winner 的 combat outcome；不猜测缺失的事件上下文。
///
/// 返回 [`CombatPhaseOutcome`]：成功 retry 会 remove（`removed=true`）；失败 retry 不改任何
/// owner，只保留既有 marker，因此不会制造虚假的 partial mutation。
#[allow(clippy::too_many_arguments)]
fn run_pending_combat_release_retry(
    store: &mut NpcDormantStore,
    faction_store: &FactionStore,
    config: &NpcVirtualizationConfig,
    tick: u64,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
    persistence: Option<&crate::persistence::PersistenceSettings>,
    death_notices: &mut EventWriter<NpcDeathNotice>,
    combat_outcomes: &mut EventWriter<DormantCombatOutcome>,
    pending_relics: &mut EventWriter<PendingDormantRelicCreated>,
) -> CombatPhaseOutcome {
    let mut outcome = CombatPhaseOutcome::default();
    if !store.pending_combat_state_is_persisted() {
        return outcome;
    }
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
        let (winner, winner_group, loser_group) = {
            let Some(loser) = store.snapshots.get(&loser_id) else {
                continue;
            };
            let winner = loser.pending_combat_winner.clone();
            let winner_group = winner.as_ref().and_then(|winner_id| {
                store
                    .snapshots
                    .get(winner_id)
                    .and_then(|winner| effective_group(winner, faction_store))
            });
            let loser_group = effective_group(loser, faction_store);
            (winner, winner_group, loser_group)
        };
        let Some(original) = store.snapshots.get(&loser_id).cloned() else {
            continue;
        };
        let mut staged_loser = original.clone();
        let mut staged_zones = zones.clone();
        let mut staged_ledger = ledger.clone();
        let Ok(settlement) =
            release_dormant_qi_to_zone(&mut staged_loser, &mut staged_zones, &mut staged_ledger)
        else {
            continue;
        };
        if !dormant_terminal_qi_is_settled(&staged_loser) {
            continue;
        }
        let relic = if combat::should_leave_relic(&staged_loser) {
            Some(PendingDormantRelicCreated {
                char_id: loser_id.clone(),
                zone: staged_loser.zone_name.clone(),
                position: staged_loser.position,
                archetype: staged_loser.archetype,
                loot_seed: combat::relic_loot_seed(&loser_id, tick, config.sim_seed),
                created_tick: tick,
            })
        } else {
            None
        };
        let tombstone = crate::persistence::DormantTerminalCommitRecord {
            char_id: loser_id.clone(),
            cause: "combat".to_string(),
            at_tick: tick,
            zone: staged_loser.zone_name.clone(),
            winner: winner.clone(),
            winner_group: winner_group.map(|group| u64::from(group.0)),
            loser_group: loser_group.map(|group| u64::from(group.0)),
            zone_accepted: settlement.zone_accepted,
            cleanup_revision: None,
        };
        let first_commit = if let Some(persistence) = persistence {
            match crate::persistence::persist_dormant_terminal_commit(
                persistence,
                &tombstone,
                &staged_zones,
                &staged_ledger,
                relic.as_ref(),
            ) {
                Ok(crate::persistence::PersistDormantTerminalOutcome::Committed) => true,
                Ok(crate::persistence::PersistDormantTerminalOutcome::AlreadyCommitted) => false,
                Err(error) => {
                    tracing::warn!(
                        "[bong][npc] retained dormant combat loser `{loser_id}` after terminal persistence failure: {error}"
                    );
                    continue;
                }
            }
        } else {
            true
        };

        if first_commit {
            *zones = staged_zones;
            *ledger = staged_ledger;
            death_notices.send(dormant_combat_death_notice(&staged_loser));
            if let Some(winner) = winner {
                combat_outcomes.send(DormantCombatOutcome {
                    winner,
                    loser: loser_id.clone(),
                    zone: staged_loser.zone_name.clone(),
                    qi_released: settlement.zone_accepted,
                    winner_group,
                    loser_group,
                });
            }
            if let Some(relic) = relic {
                pending_relics.send(relic);
            }
        }
        store.snapshots.remove(&loser_id);
        store.track_terminal_tombstone(tombstone);
        outcome.mutated = true;
        outcome.removed = true;
    }
    outcome
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
        mimic_spider: None,
        tsy_hostile: None,
        tsy_sentinel: None,
        intent,
        dormant_since_tick: tick,
        last_dormant_tick_processed: tick,
        initial_qi: cultivation.qi_current,
        qi_ledger_net: 0.0,
        combat_dead_pending_release: false,
        pending_combat_winner: None,
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

fn dormant_terminal_qi_is_settled(snapshot: &NpcDormantSnapshot) -> bool {
    snapshot.cultivation.qi_current == 0.0
        && snapshot
            .tsy_hostile
            .as_ref()
            .and_then(|hostile| hostile.daozhan.as_ref())
            .is_none_or(|daozhan| daozhan.daozhan_qi == 0.0)
}

pub fn release_dormant_qi_to_zone(
    snapshot: &mut NpcDormantSnapshot,
    zones: &mut ZoneRegistry,
    ledger: &mut WorldQiAccount,
) -> Result<QiFlowOutcome, QiFlowError> {
    let actor = ActorQiIdentity::from_life_record(&snapshot.life_record, ActorQiKind::Npc)?;
    let mut staged_snapshot = snapshot.clone();
    let mut staged_zones = zones.clone();
    let mut staged_ledger = ledger.clone();
    let pos = staged_snapshot.position_vec();
    let zone_name = staged_zones
        .find_zone(staged_snapshot.dimension, pos)
        .map(|zone| zone.name.clone())
        .or_else(|| {
            staged_zones
                .find_zone_mut(staged_snapshot.zone_name.as_str())
                .map(|zone| zone.name.clone())
        });

    let cultivation_amount = staged_snapshot.cultivation.qi_current();
    let cultivation_outcome = staged_snapshot.cultivation.release_to_zone(
        zone_name
            .as_deref()
            .and_then(|name| staged_zones.find_zone_mut(name)),
        &mut staged_ledger,
        &actor,
        cultivation_amount,
        QiTransferReason::ReleaseToZone,
    )?;

    let mut outcome = cultivation_outcome;
    if let Some(daozhan) = staged_snapshot
        .tsy_hostile
        .as_mut()
        .and_then(|hostile| hostile.daozhan.as_mut())
    {
        let daozhan_amount = daozhan.daozhan_qi;
        let daozhan_outcome = release_external_qi_to_zone(
            &mut daozhan.daozhan_qi,
            actor.account(),
            zone_name
                .as_deref()
                .and_then(|name| staged_zones.find_zone_mut(name)),
            &mut staged_ledger,
            daozhan_amount,
            QiTransferReason::ReleaseToZone,
        )?;
        outcome.requested += daozhan_outcome.requested;
        outcome.source_debited += daozhan_outcome.source_debited;
        outcome.target_credited += daozhan_outcome.target_credited;
        outcome.zone_accepted += daozhan_outcome.zone_accepted;
        outcome.overflow_credited += daozhan_outcome.overflow_credited;
        outcome.untransferred += daozhan_outcome.untransferred;
        outcome.transfers.extend(daozhan_outcome.transfers);
    }

    staged_snapshot.qi_ledger_net -= outcome.source_debited;
    *snapshot = staged_snapshot;
    *zones = staged_zones;
    *ledger = staged_ledger;
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
#[path = "mod_tests.rs"]
mod tests;
