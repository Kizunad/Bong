use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{
    bevy_ecs, Added, App, Bundle, Commands, Component, DVec3, Despawned, Entity, Event,
    EventReader, EventWriter, IntoSystemConfigs, IntoSystemSetConfigs, Position, Query, Res,
    ResMut, Resource, SystemSet, Update, With, Without,
};

use crate::combat::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StatusEffects, Wounds,
};
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::qi_max_for_realm;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    release_external_qi_to_zone, ActorQiIdentity, ActorQiKind, Contamination, Cultivation,
    MeridianSystem, QiColor, QiFlowError, Realm,
};
use crate::cultivation::death_hooks::{CultivationDeathTrigger, PlayerTerminated};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanEventEmitted,
    LifespanExtensionLedger, ZoneDeathKind,
};
use crate::cultivation::possession::PossessedVictim;
use crate::fauna::daozhan::DaoZhangBehaviorBlackboard;
use crate::fauna::mimic_spider::MimicSpiderBlackboard;
use crate::fauna::rat_phase::transfer_rat_drained_qi_to_zone;
#[cfg(test)]
use crate::npc::brain::canonical_npc_id;
use crate::npc::faction::{FactionId, FactionMembership};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::NpcMarker;
use crate::npc::spawn_rat::RatBlackboard;
use crate::npc::technique::NpcCooldownMap;
use crate::persistence::{
    persist_npc_terminal_qi_transaction, LifespanEventRecord, NpcTerminalLootOutboxRecord,
    NpcTerminalNarrationOutboxRecord, PersistenceSettings, TerminalPersistencePayload,
};
use crate::qi_physics::{QiTransfer, QiTransferReason, WorldQiAccount};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

type RegistryNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        Option<&'static Lifecycle>,
        Option<&'static NpcPatrol>,
    ),
    (With<NpcMarker>, Without<Despawned>),
>;

type ActiveNpcFilter = (
    With<NpcMarker>,
    Without<Despawned>,
    Without<PendingRetirement>,
);
type SharedAgingNpcQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NpcArchetype,
        &'static mut NpcLifespan,
        Option<&'static LifespanComponent>,
    ),
    ActiveNpcFilter,
>;
type TerminatedNpcReadQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static PendingRetirement>,
        Option<&'static LifespanComponent>,
        Option<&'static FactionMembership>,
        &'static LifeRecord,
        &'static Lifecycle,
        &'static DeathRegistry,
        Option<&'static Position>,
        Option<&'static CurrentDimension>,
        Option<&'static crate::skin::NpcVisualProfile>,
        &'static PendingNpcTermination,
    ),
    With<NpcMarker>,
>;

type TerminatedNpcOwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Cultivation,
        Option<&'static mut RatBlackboard>,
        Option<&'static mut DaoZhangBehaviorBlackboard>,
        Option<&'static mut MimicSpiderBlackboard>,
        Option<&'static mut crate::fauna::dying_elder::DyingElderBlackboard>,
    ),
    With<NpcMarker>,
>;

type DespawnedNpcNoticeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static LifespanComponent>,
        Option<&'static FactionMembership>,
        &'static LifeRecord,
        Option<&'static PossessedVictim>,
    ),
    (
        With<NpcMarker>,
        With<Despawned>,
        Added<Despawned>,
        Without<NpcDeathNoticeEmitted>,
    ),
>;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
#[serde(rename_all = "snake_case")]
pub enum NpcArchetype {
    #[default]
    Zombie,
    Commoner,
    Rogue,
    Beast,
    Disciple,
    GuardianRelic,
    /// plan-tsy-lifecycle-v1 §4 — 道伥（干尸激活后的诡异 NPC，伪装平凡 / 突袭玩家）。
    Daoxiang,
    /// plan-tsy-hostile-v1 §1 — 执念（精英敌对 NPC，伪装道伥后伏击）。
    Zhinian,
    /// plan-tsy-hostile-v1 §1 — 负压畸变体（环境威胁，带耗真元光环）。
    Fuya,
    /// plan-skull-fiend-v1 §P0 — 骨煞（头骨畸变体，直线冲撞猎物）。
    SkullFiend,
    /// plan-dying-elder-v1 P0 — 垂死大能（困于坍缩渊濒死化虚修士，欺骗性遭遇，可能翻脸夺舍）。
    DyingElder,
    /// plan-mundane-fauna-v1 P0 — 凡兽（无灵 MC 原版被动生物：牛/猪/羊/鸡/兔/山羊/蛙/狐/狼）。
    Mundane,
}

/// plan-npc-overhaul-v1 §P1.1 — 三桶预算系统，按 NPC 类型族群分组限额。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcBudgetBucket {
    /// 散修 + 凡人（Zombie/Commoner/Rogue/Disciple/Daoxiang/Zhinian）
    Humanoid,
    /// 野兽 / 鼠群 / 异变兽（Beast/Fuya）
    Beast,
    /// 稀有大型（GuardianRelic/SkullFiend）
    Special,
}

impl NpcBudgetBucket {
    /// 所有 bucket 变体，用于遍历。
    pub const ALL: [NpcBudgetBucket; 3] = [
        NpcBudgetBucket::Humanoid,
        NpcBudgetBucket::Beast,
        NpcBudgetBucket::Special,
    ];

    pub const fn default_cap(self) -> usize {
        match self {
            Self::Humanoid => 26,
            Self::Beast => 20,
            Self::Special => 4,
        }
    }
}

impl NpcArchetype {
    /// 所有变体，用于遍历和 exhaustiveness 测试。
    #[allow(dead_code)]
    pub const ALL: [NpcArchetype; 12] = [
        NpcArchetype::Zombie,
        NpcArchetype::Commoner,
        NpcArchetype::Rogue,
        NpcArchetype::Beast,
        NpcArchetype::Disciple,
        NpcArchetype::GuardianRelic,
        NpcArchetype::Daoxiang,
        NpcArchetype::Zhinian,
        NpcArchetype::Fuya,
        NpcArchetype::SkullFiend,
        NpcArchetype::DyingElder,
        NpcArchetype::Mundane,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zombie => "zombie",
            Self::Commoner => "commoner",
            Self::Rogue => "rogue",
            Self::Beast => "beast",
            Self::Disciple => "disciple",
            Self::GuardianRelic => "guardian_relic",
            Self::Daoxiang => "daoxiang",
            Self::Zhinian => "zhinian",
            Self::Fuya => "fuya",
            Self::SkullFiend => "skull_fiend",
            Self::DyingElder => "dying_elder",
            Self::Mundane => "mundane",
        }
    }

    /// [`as_str`](Self::as_str) 的逆——把持久化 / 序列化字符串还原成 archetype。
    ///
    /// plan-offscreen-war-v1 P3：战场遗物把 archetype 以 [`as_str`](Self::as_str) 文本存进
    /// sqlite `pending_dormant_relics.archetype`，玩家靠近 hydrate 时读回这个字符串重建
    /// archetype 喂 `default_loot_for_archetype`。与 `as_str` 严格对称（每个变体一条），
    /// 未知串返回 `None`（让调用方显式处理脏数据而非静默吞成默认 archetype）。
    /// 消费方：`npc::dormant::relic_hydrate::materialize_relic_loot`（交付物 3）。
    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "zombie" => Self::Zombie,
            "commoner" => Self::Commoner,
            "rogue" => Self::Rogue,
            "beast" => Self::Beast,
            "disciple" => Self::Disciple,
            "guardian_relic" => Self::GuardianRelic,
            "daoxiang" => Self::Daoxiang,
            "zhinian" => Self::Zhinian,
            "fuya" => Self::Fuya,
            "skull_fiend" => Self::SkullFiend,
            "dying_elder" => Self::DyingElder,
            "mundane" => Self::Mundane,
            _ => return None,
        })
    }

    /// plan-npc-overhaul-v1 §P1.1 — 映射 archetype 到预算桶。
    pub const fn budget_bucket(self) -> NpcBudgetBucket {
        match self {
            Self::Zombie
            | Self::Commoner
            | Self::Rogue
            | Self::Disciple
            | Self::Daoxiang
            | Self::Zhinian => NpcBudgetBucket::Humanoid,
            // 凡兽归 Beast 桶（"野生生物"预算池语义一致）——真正的密度控制入口是
            // plan-mundane-fauna-v1 §8.1 #4 的 `mundane_passive_budget_fn.max_alive`（独立
            // AmbientSchedulerState<MundaneFaunaMarker> passive pool），NpcBudgetBucket 只是
            // NpcRegistry 全局计数副产物，不新开第四桶（§8.1 #10 决议）。
            Self::Beast | Self::Fuya | Self::Mundane => NpcBudgetBucket::Beast,
            Self::GuardianRelic | Self::SkullFiend | Self::DyingElder => NpcBudgetBucket::Special,
        }
    }

    pub const fn default_max_age_ticks(self) -> f64 {
        match self {
            Self::Zombie => 120_000.0,
            Self::Commoner => 90_000.0,
            Self::Rogue => 110_000.0,
            Self::Beast => 80_000.0,
            Self::Disciple => 140_000.0,
            Self::GuardianRelic => 1_000_000.0,
            // 道伥承接 plan-tsy-lifecycle-v1 §4 的"不老"语义；hostile 分层不改该 balance。
            Self::Daoxiang => 1_000_000.0,
            Self::Zhinian => 180_000.0,
            Self::Fuya => 240_000.0,
            Self::SkullFiend => 260_000.0,
            // 垂死大能由 DyingElderDrainSystem 驱动自然消亡（真元耗尽），不走年龄 aging
            Self::DyingElder => 1_000_000.0,
            // 凡兽短寿（对齐 Beast 80_000 但更短——v1 种群靠 spawner 补充维持而非繁殖，
            // §8.1 #6），老死走标准 AgeingScorer→RetireAction 链，不需要专属处理。
            Self::Mundane => 60_000.0,
        }
    }

    pub const fn uses_lifespan_aging(self) -> bool {
        !matches!(
            self,
            Self::Zombie | Self::GuardianRelic | Self::Daoxiang | Self::DyingElder
        )
    }
}

#[derive(Clone, Copy, Debug, Component, Serialize, Deserialize)]
pub struct NpcLifespan {
    pub age_ticks: f64,
    pub max_age_ticks: f64,
}

impl NpcLifespan {
    pub const fn new(age_ticks: f64, max_age_ticks: f64) -> Self {
        Self {
            age_ticks,
            max_age_ticks,
        }
    }

    pub fn age_ratio(&self) -> f64 {
        if self.max_age_ticks <= f64::EPSILON {
            1.0
        } else {
            (self.age_ticks / self.max_age_ticks).clamp(0.0, 16.0)
        }
    }

    pub fn is_expired(&self) -> bool {
        self.age_ticks >= self.max_age_ticks
    }
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct NpcAgingConfig {
    pub enabled: bool,
    pub rate_multiplier: f64,
}

impl Default for NpcAgingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rate_multiplier: 0.3,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct NpcRegistry {
    pub live_npc_count: usize,
    pub max_npc_count: usize,
    pub resume_npc_count: usize,
    pub spawn_paused: bool,
    pub counts_by_archetype: HashMap<NpcArchetype, usize>,
    pub per_zone_caps: HashMap<String, usize>,
    pub counts_by_zone: HashMap<String, usize>,
    /// plan-npc-overhaul-v1 §P1.1 — 当前各桶实际 NPC 数。
    pub counts_by_bucket: HashMap<NpcBudgetBucket, usize>,
    /// plan-npc-overhaul-v1 §P1.1 — 各桶上限。
    #[allow(dead_code)]
    pub bucket_caps: HashMap<NpcBudgetBucket, usize>,
}

impl Default for NpcRegistry {
    fn default() -> Self {
        let mut per_zone_caps = HashMap::new();
        per_zone_caps.insert("spawn".to_string(), 6);
        per_zone_caps.insert("qingyun_peaks".to_string(), 5);
        per_zone_caps.insert("spring_marsh".to_string(), 4);
        per_zone_caps.insert("rift_valley".to_string(), 5);
        per_zone_caps.insert("north_wastes".to_string(), 2);
        per_zone_caps.insert("lingquan_marsh".to_string(), 4);

        let mut bucket_caps = HashMap::new();
        for bucket in NpcBudgetBucket::ALL {
            bucket_caps.insert(bucket, bucket.default_cap());
        }

        Self {
            live_npc_count: 0,
            max_npc_count: 50,
            resume_npc_count: 40,
            spawn_paused: false,
            counts_by_archetype: HashMap::new(),
            per_zone_caps,
            counts_by_zone: HashMap::new(),
            counts_by_bucket: HashMap::new(),
            bucket_caps,
        }
    }
}

impl NpcRegistry {
    pub fn refresh_from_counts(
        &mut self,
        live_npc_count: usize,
        counts_by_archetype: HashMap<NpcArchetype, usize>,
        counts_by_zone: HashMap<String, usize>,
    ) {
        self.live_npc_count = live_npc_count;

        // Tally bucket counts from archetype counts.
        self.counts_by_bucket.clear();
        for (&archetype, &count) in &counts_by_archetype {
            *self
                .counts_by_bucket
                .entry(archetype.budget_bucket())
                .or_default() += count;
        }

        self.counts_by_archetype = counts_by_archetype;
        self.counts_by_zone = counts_by_zone;

        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        } else if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn reserve_spawn_batch(&mut self, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }

        if self.spawn_paused && self.live_npc_count >= self.resume_npc_count {
            return 0;
        }

        let remaining = self.max_npc_count.saturating_sub(self.live_npc_count);
        let granted = desired.min(remaining);
        self.live_npc_count = self.live_npc_count.saturating_add(granted);
        if self.live_npc_count >= self.max_npc_count {
            self.spawn_paused = true;
        }
        granted
    }

    /// plan-npc-overhaul-v1 §P1.1 — 同时检查全局上限和桶上限的预留。
    /// 返回实际批准数（受两者中较低的 remaining 限制）。
    #[allow(dead_code)]
    pub fn reserve_bucket_spawn(&mut self, bucket: NpcBudgetBucket, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }

        // Bucket remaining
        let bucket_cap = self
            .bucket_caps
            .get(&bucket)
            .copied()
            .unwrap_or(bucket.default_cap());
        let bucket_current = self.counts_by_bucket.get(&bucket).copied().unwrap_or(0);
        let bucket_remaining = bucket_cap.saturating_sub(bucket_current);
        let clamped = desired.min(bucket_remaining);
        if clamped == 0 {
            return 0;
        }

        // Global reserve
        let granted = self.reserve_spawn_batch(clamped);
        if granted > 0 {
            *self.counts_by_bucket.entry(bucket).or_default() += granted;
        }
        granted
    }

    /// 回滚已 reserve 但未实际落盘的配额。用于"先 reserve 再决定能否 spawn"
    /// 路径在早退分支未回退导致的 1-tick 暂态泄漏 —— 这一 tick 里
    /// `live_npc_count >= resume_npc_count` 会误触发 `spawn_paused=true`，
    /// 同 tick 后续 spawn 分支被误杀。
    pub fn release_spawn_batch(&mut self, count: usize) {
        if count == 0 {
            return;
        }
        self.live_npc_count = self.live_npc_count.saturating_sub(count);
        if self.live_npc_count < self.resume_npc_count {
            self.spawn_paused = false;
        }
    }

    pub fn reserve_zone_batch(&mut self, zone: &str, desired: usize) -> usize {
        if desired == 0 {
            return 0;
        }
        let zone_remaining = self
            .per_zone_caps
            .get(zone)
            .map(|cap| cap.saturating_sub(*self.counts_by_zone.get(zone).unwrap_or(&0)))
            .unwrap_or(desired);
        let desired = desired.min(zone_remaining);
        let granted = self.reserve_spawn_batch(desired);
        if granted > 0 {
            *self.counts_by_zone.entry(zone.to_string()).or_default() += granted;
        }
        granted
    }

    #[allow(dead_code)]
    pub fn release_zone_slot(&mut self, zone: &str) {
        self.release_zone_batch(zone, 1);
    }

    pub fn release_zone_batch(&mut self, zone: &str, count: usize) {
        if count == 0 {
            return;
        }
        self.release_spawn_batch(count);
        let remove = if let Some(current) = self.counts_by_zone.get_mut(zone) {
            *current = current.saturating_sub(count);
            *current == 0
        } else {
            false
        };
        if remove {
            self.counts_by_zone.remove(zone);
        }
    }

    pub fn should_reduce_population(&self) -> bool {
        self.live_npc_count >= self.max_npc_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum NpcTerminalSystemSet {
    Stage,
    Commit,
    PostCommit,
}

#[derive(Clone, Copy, Debug, Component)]
pub struct PendingRetirement;

/// NPC 的可重试终结意图。任何 lifecycle / biography / persistence / telemetry / loot /
/// reproduction / despawn 副作用都必须等同一 owner transaction 成功后才提交。
#[derive(Clone, Debug, Component)]
pub struct PendingNpcTermination {
    pub cause: String,
    pub at_tick: u64,
    pub death_zone: ZoneDeathKind,
    pub lifespan_event: Option<LifespanEventRecord>,
    pub death_insight: Option<crate::schema::death_insight::DeathInsightRequestV1>,
    pub reason: NpcDeathReason,
    pub attacker: Option<Entity>,
    pub attacker_player_id: Option<String>,
    pub authorize_loot: bool,
    pub actor_qi_identity: ActorQiIdentity,
    pub reproduction: Option<NpcReproductionRequest>,
    /// 终结前冻结的 Redis 死亡叙事 payload；与 owner transaction 同一 SQLite commit。
    pub narration_outbox: Option<NpcTerminalNarrationOutboxRecord>,
    /// 终结前冻结的 loot payload；必须和 owner transaction 同一 SQLite commit。
    pub loot_outbox: Option<NpcTerminalLootOutboxRecord>,
}

/// NPC 终结事务已成功提交；所有 loot consumer 只消费此授权事件，不能消费 raw
/// `DeathEvent` 抢跑。`at_tick` 保留原始致死 tick，保证掉落 RNG 在 retry 后仍稳定。
#[derive(Clone, Debug, Event)]
pub struct NpcTerminalSettlementSucceeded {
    pub entity: Entity,
    pub at_tick: u64,
    pub cause: String,
    pub reason: NpcDeathReason,
    pub attacker: Option<Entity>,
    pub attacker_player_id: Option<String>,
    pub authorize_loot: bool,
    pub actor_qi_identity: ActorQiIdentity,
}

#[derive(Clone, Debug, Event)]
pub struct NpcRetireRequest {
    pub entity: Entity,
}

/// 邻居生子（plan §3.3）：Commoner 老死后由 spawn 侧消费，在死者附近
/// 生一个年龄 0–5% max_age 的新生儿。受 `NpcRegistry` 预留预算约束。
///
/// Beast 领地繁衍（§8）复用同一通道：`archetype = Beast` + 必填
/// `territory_center` / `territory_radius`（新生幼崽要挂 Territory 组件，
/// spawn 侧据此重建）。避免 lifecycle.rs 反向依赖 territory.rs。
#[derive(Clone, Debug, Event)]
pub struct NpcReproductionRequest {
    pub archetype: NpcArchetype,
    pub position: DVec3,
    pub home_zone: String,
    pub initial_age_ticks: f64,
    /// Beast 必填；Commoner 忽略。
    pub territory_center: Option<DVec3>,
    /// Beast 必填；Commoner 忽略。
    pub territory_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcSpawnSource {
    Startup,
    Seed,
    Reproduction,
    AgentCommand,
}

impl NpcSpawnSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Seed => "seed",
            Self::Reproduction => "reproduction",
            Self::AgentCommand => "agent_command",
        }
    }
}

#[derive(Clone, Debug, Event)]
pub struct NpcSpawnNotice {
    pub entity: Entity,
    pub archetype: NpcArchetype,
    pub source: NpcSpawnSource,
    pub home_zone: String,
    pub position: DVec3,
    pub initial_age_ticks: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcDeathReason {
    NaturalAging,
    Combat,
    Despawned,
    DuoShe,
}

impl NpcDeathReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NaturalAging => "natural_aging",
            Self::Combat => "combat",
            Self::Despawned => "despawned",
            Self::DuoShe => "duo_she",
        }
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub struct NpcDeathNoticeEmitted;

#[derive(Clone, Debug, Event)]
#[allow(dead_code)]
pub struct NpcDeathNotice {
    pub npc_id: String,
    pub archetype: NpcArchetype,
    pub reason: NpcDeathReason,
    pub faction_id: Option<FactionId>,
    pub life_record_snapshot: Option<String>,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
    /// plan-offscreen-war-v1 P0：是否为离屏 dormant 派系互殴所致（区别于自然老死 / 在场战斗）。
    /// P0 全部构造点回填 `false`；P2 dormant 战死闭环落地后才会出现 `true`。
    pub from_dormant_combat: bool,
    /// plan-offscreen-war-v1 P0：死亡坐标（有则带，无则 None）。dormant 死亡用 snapshot.position，
    /// 实体死亡当前无坐标上下文回填 None（后续阶段可补）。
    pub pos: Option<[f64; 3]>,
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcAgingConfig::default())
        .insert_resource(NpcRegistry::default())
        .add_event::<CultivationDeathTrigger>()
        .add_event::<PlayerTerminated>()
        .add_event::<NpcRetireRequest>()
        .add_event::<NpcReproductionRequest>()
        .add_event::<NpcSpawnNotice>()
        .add_event::<NpcDeathNotice>()
        .add_event::<NpcTerminalSettlementSucceeded>()
        .configure_sets(
            Update,
            (
                NpcTerminalSystemSet::Stage,
                NpcTerminalSystemSet::Commit,
                NpcTerminalSystemSet::PostCommit,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                update_npc_registry,
                age_npcs,
                process_npc_retire_requests.in_set(NpcTerminalSystemSet::Stage),
                settle_pending_npc_termination.in_set(NpcTerminalSystemSet::Commit),
                emit_npc_despawn_notices.in_set(NpcTerminalSystemSet::PostCommit),
            ),
        );
}

#[derive(Bundle)]
pub struct NpcRuntimeBundle {
    pub archetype: NpcArchetype,
    pub lifespan: NpcLifespan,
    pub shared_lifespan: LifespanComponent,
    pub death_registry: DeathRegistry,
    pub life_record: LifeRecord,
    pub lifespan_extension_ledger: LifespanExtensionLedger,
    pub cultivation: Cultivation,
    pub meridian_system: MeridianSystem,
    pub contamination: Contamination,
    pub wounds: Wounds,
    pub stamina: Stamina,
    pub combat_state: CombatState,
    pub status_effects: StatusEffects,
    pub derived_attrs: DerivedAttrs,
    pub lifecycle: Lifecycle,
}

pub fn npc_runtime_bundle(
    entity: Entity,
    archetype: NpcArchetype,
    realm: Realm,
) -> NpcRuntimeBundle {
    npc_runtime_bundle_with_age(entity, archetype, realm, 0.0)
}

pub fn npc_runtime_bundle_with_age(
    _entity: Entity,
    archetype: NpcArchetype,
    realm: Realm,
    initial_age_ticks: f64,
) -> NpcRuntimeBundle {
    let char_id = format!("npc:{}", Uuid::now_v7());
    let cultivation = Cultivation {
        realm,
        qi_current: 0.0,
        qi_max: qi_max_for_realm(realm),
        ..Cultivation::default()
    };
    let mut bundle = NpcRuntimeBundle {
        archetype,
        lifespan: NpcLifespan::new(
            initial_age_ticks.max(0.0),
            archetype.default_max_age_ticks(),
        ),
        shared_lifespan: LifespanComponent::for_realm(realm),
        death_registry: DeathRegistry::new(char_id.clone()),
        life_record: LifeRecord::new(char_id.clone()),
        lifespan_extension_ledger: LifespanExtensionLedger::default(),
        cultivation,
        // plan-npc-realm-distribution-v1 P2：经脉必须和 Cultivation.realm 同源，
        // 否则 spawn 侧用 npc_meridian_system_for_realm(realm) 算出「已开 N 脉」去
        // 筛技能，entity 上真正落地的 MeridianSystem 却仍是全闭的 default()——
        // 一个 realm 派生出两份互相矛盾的经脉状态。
        meridian_system: crate::npc::technique::npc_meridian_system_for_realm(
            realm,
            crate::body_plan::humanoid_plan_static(),
        ),
        contamination: Contamination::default(),
        wounds: Wounds::default(),
        stamina: Stamina::default(),
        combat_state: CombatState::default(),
        status_effects: StatusEffects::default(),
        derived_attrs: DerivedAttrs::default(),
        lifecycle: Lifecycle {
            character_id: char_id,
            fortune_remaining: 0,
            ..Default::default()
        },
    };
    if archetype == NpcArchetype::Commoner {
        bundle.shared_lifespan.apply_cap(LifespanCapTable::MORTAL);
    }
    sync_shared_lifespan_from_npc_age(&mut bundle.shared_lifespan, &bundle.lifespan);
    bundle
}

fn sync_shared_lifespan_from_npc_age(
    shared_lifespan: &mut LifespanComponent,
    npc_lifespan: &NpcLifespan,
) {
    let ratio = npc_lifespan.age_ratio().clamp(0.0, 1.0);
    shared_lifespan.years_lived = shared_lifespan.cap_by_realm as f64 * ratio;
}

fn update_npc_registry(mut registry: ResMut<NpcRegistry>, npcs: RegistryNpcQuery<'_, '_>) {
    let mut counts_by_archetype = HashMap::new();
    let mut counts_by_zone = HashMap::new();
    let mut live_npc_count = 0usize;

    for (archetype, lifecycle, patrol) in &npcs {
        if lifecycle.is_some_and(|lifecycle| lifecycle.state == LifecycleState::Terminated) {
            continue;
        }

        live_npc_count += 1;
        *counts_by_archetype.entry(*archetype).or_default() += 1;
        if let Some(patrol) = patrol {
            *counts_by_zone.entry(patrol.home_zone.clone()).or_default() += 1;
        }
    }

    registry.refresh_from_counts(live_npc_count, counts_by_archetype, counts_by_zone);
}

fn age_npcs(config: Res<NpcAgingConfig>, mut npcs: SharedAgingNpcQuery<'_, '_>) {
    if !config.enabled {
        return;
    }

    for (archetype, mut npc_lifespan, shared_lifespan) in &mut npcs {
        if !archetype.uses_lifespan_aging() {
            continue;
        }
        if let Some(shared_lifespan) = shared_lifespan {
            let ratio = if shared_lifespan.cap_by_realm == 0 {
                1.0
            } else {
                (shared_lifespan.years_lived / shared_lifespan.cap_by_realm as f64).clamp(0.0, 1.0)
            };
            npc_lifespan.age_ticks = npc_lifespan.max_age_ticks * ratio;
        } else {
            npc_lifespan.age_ticks += config.rate_multiplier.max(0.0);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn process_npc_retire_requests(
    clock: Res<CombatClock>,
    mut commands: Commands,
    mut retire_requests: EventReader<NpcRetireRequest>,
    npcs: Query<
        (
            &NpcArchetype,
            &NpcLifespan,
            Option<&Position>,
            Option<&crate::npc::patrol::NpcPatrol>,
            Option<&LifeRecord>,
            Option<&Lifecycle>,
            Option<&DeathRegistry>,
            Option<&PendingNpcTermination>,
        ),
        With<NpcMarker>,
    >,
) {
    for request in retire_requests.read() {
        let Ok((
            archetype,
            _lifespan,
            position,
            patrol,
            life_record,
            lifecycle,
            death_registry,
            pending,
        )) = npcs.get(request.entity)
        else {
            continue;
        };
        if pending.is_some() {
            continue;
        }
        let (Some(life_record), Some(lifecycle), Some(death_registry)) =
            (life_record, lifecycle, death_registry)
        else {
            tracing::warn!(
                target = ?request.entity,
                "[bong][npc] retained retiring NPC because canonical terminal identity is missing"
            );
            continue;
        };
        let Ok(actor_qi_identity) =
            validate_npc_terminal_identity(lifecycle, life_record, death_registry)
        else {
            tracing::warn!(
                target = ?request.entity,
                "[bong][npc] retained retiring NPC because terminal identity diverged"
            );
            continue;
        };

        let reproduction = if *archetype == NpcArchetype::Commoner {
            match (position, patrol) {
                (Some(pos), Some(patrol)) => Some(NpcReproductionRequest {
                    archetype: NpcArchetype::Commoner,
                    position: pos.get(),
                    home_zone: patrol.home_zone.clone(),
                    initial_age_ticks: 0.0,
                    territory_center: None,
                    territory_radius: None,
                }),
                _ => None,
            }
        } else {
            None
        };
        commands
            .entity(request.entity)
            .insert(PendingNpcTermination {
                cause: "natural_aging".to_string(),
                at_tick: clock.tick,
                death_zone: ZoneDeathKind::Ordinary,
                lifespan_event: None,
                death_insight: None,
                reason: NpcDeathReason::NaturalAging,
                attacker: None,
                attacker_player_id: None,
                authorize_loot: false,
                actor_qi_identity,
                reproduction,
                narration_outbox: None,
                loot_outbox: None,
            });
    }
}

fn validate_npc_terminal_identity(
    lifecycle: &Lifecycle,
    life_record: &LifeRecord,
    death_registry: &DeathRegistry,
) -> Result<ActorQiIdentity, QiFlowError> {
    let identity = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)?;
    let canonical = life_record.character_id.as_str();
    if lifecycle.character_id != canonical || death_registry.char_id != canonical {
        return Err(QiFlowError::InvalidActorIdentity);
    }
    Ok(identity)
}

fn settlement_zone_mut(
    zones: &mut ZoneRegistry,
    position: Option<DVec3>,
    dimension: Option<DimensionKind>,
) -> Option<&mut crate::world::zone::Zone> {
    let (Some(position), Some(dimension)) = (position, dimension) else {
        return None;
    };
    let zone_name = zones
        .find_zone(dimension, position)
        .map(|zone| zone.name.clone());
    zone_name.and_then(|zone_name| zones.find_zone_mut(zone_name.as_str()))
}

#[allow(clippy::too_many_arguments)]
fn settle_npc_qi_owners(
    cultivation: &mut Cultivation,
    rat: Option<&mut RatBlackboard>,
    daozhan: Option<&mut DaoZhangBehaviorBlackboard>,
    spider: Option<&mut MimicSpiderBlackboard>,
    position: Option<DVec3>,
    dimension: Option<DimensionKind>,
    life_record: &LifeRecord,
    zones: Option<&mut ZoneRegistry>,
    ledger: &mut WorldQiAccount,
) -> Result<Vec<QiTransfer>, QiFlowError> {
    let identity = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)?;
    let account = identity.account();
    let mut staged_cultivation = cultivation.clone();
    let mut staged_rat = rat.as_deref().cloned();
    let mut staged_daozhan = daozhan.as_deref().cloned();
    let mut staged_spider = spider.as_deref().cloned();
    let mut staged_zones = zones.as_deref().cloned();
    let mut staged_ledger = ledger.clone();
    let audit_start = staged_ledger.transfers().len();

    let cultivation_amount = staged_cultivation.qi_current();
    staged_cultivation.release_to_zone(
        staged_zones
            .as_mut()
            .and_then(|zones| settlement_zone_mut(zones, position, dimension)),
        &mut staged_ledger,
        &identity,
        cultivation_amount,
        QiTransferReason::ReleaseToZone,
    )?;

    if staged_rat.is_some() {
        transfer_rat_drained_qi_to_zone(
            &mut staged_ledger,
            staged_zones
                .as_mut()
                .and_then(|zones| settlement_zone_mut(zones, position, dimension)),
            &identity,
        )?;
        if let Some(rat) = staged_rat.as_mut() {
            rat.drained_qi = staged_ledger.balance(&account);
        }
    }

    if let Some(daozhan) = staged_daozhan.as_mut() {
        let amount = daozhan.daozhan_qi;
        release_external_qi_to_zone(
            &mut daozhan.daozhan_qi,
            &identity,
            staged_zones
                .as_mut()
                .and_then(|zones| settlement_zone_mut(zones, position, dimension)),
            &mut staged_ledger,
            amount,
            QiTransferReason::ReleaseToZone,
        )?;
    }

    if let Some(spider) = staged_spider.as_mut() {
        spider.drained_qi = 0.0;
    }

    let transfers = staged_ledger.transfers()[audit_start..].to_vec();
    *cultivation = staged_cultivation;
    if let (Some(rat), Some(staged_rat)) = (rat, staged_rat) {
        *rat = staged_rat;
    }
    if let (Some(daozhan), Some(staged_daozhan)) = (daozhan, staged_daozhan) {
        *daozhan = staged_daozhan;
    }
    if let (Some(spider), Some(staged_spider)) = (spider, staged_spider) {
        *spider = staged_spider;
    }
    if let (Some(zones), Some(staged_zones)) = (zones, staged_zones) {
        *zones = staged_zones;
    }
    *ledger = staged_ledger;
    Ok(transfers)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_pending_npc_termination(
    mut commands: Commands,
    npcs: TerminatedNpcReadQuery<'_, '_>,
    mut owners: TerminatedNpcOwnerQuery<'_, '_>,
    persistence: Res<PersistenceSettings>,
    mut notices: EventWriter<NpcDeathNotice>,
    mut succeeded: EventWriter<NpcTerminalSettlementSucceeded>,
    mut terminated: EventWriter<PlayerTerminated>,
    mut reproduction: EventWriter<NpcReproductionRequest>,
    mut death_insights: Option<
        ResMut<bevy_ecs::event::Events<crate::combat::events::DeathInsightRequested>>,
    >,
    mut lifespan_events: Option<ResMut<bevy_ecs::event::Events<LifespanEventEmitted>>>,
    cooldowns: Option<ResMut<NpcCooldownMap>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut vfx_events: EventWriter<crate::network::vfx_event_emit::VfxEventRequest>,
) {
    let mut cooldowns = cooldowns;
    for (
        entity,
        archetype,
        lifespan,
        pending_retirement,
        shared_lifespan,
        faction,
        life_record,
        lifecycle,
        death_registry,
        position,
        dimension,
        visual_profile,
        pending,
    ) in &npcs
    {
        let Some(ledger) = ledger.as_deref_mut() else {
            tracing::warn!(
                target = ?entity,
                "[bong][npc] retained pending NPC until qi ledger is available"
            );
            continue;
        };
        let Ok(actor_qi_identity) =
            validate_npc_terminal_identity(lifecycle, life_record, death_registry)
        else {
            tracing::warn!(
                target = ?entity,
                "[bong][npc] retained pending NPC after terminal identity failed closed"
            );
            continue;
        };
        if actor_qi_identity != pending.actor_qi_identity
            || !pending
                .actor_qi_identity
                .matches_life_record(life_record, ActorQiKind::Npc)
        {
            tracing::warn!(
                target = ?entity,
                "[bong][npc] retained pending NPC after staged terminal identity changed"
            );
            continue;
        }
        if let Some(narration_outbox) = pending.narration_outbox.as_ref() {
            if narration_outbox.actor_account != pending.actor_qi_identity.account().to_string()
                || narration_outbox.created_tick != pending.at_tick
                || narration_outbox.payload.server_tick != pending.at_tick
            {
                tracing::warn!(
                    target = ?entity,
                    "[bong][npc] retained pending NPC after staged narration identity changed"
                );
                continue;
            }
        }
        if let Some(loot_outbox) = pending.loot_outbox.as_ref() {
            let actor_account = pending.actor_qi_identity.account().to_string();
            let expected_prefix = format!("dying_elder_loot:{actor_account}:");
            if loot_outbox.actor_account != actor_account
                || loot_outbox.created_tick != pending.at_tick
                || loot_outbox.outbox_id != format!("{expected_prefix}{}", pending.at_tick)
            {
                tracing::warn!(
                    target = ?entity,
                    "[bong][npc] retained pending NPC after staged loot identity changed"
                );
                continue;
            }
        }
        let Ok((mut cultivation, mut rat, mut daozhan, mut spider, mut dying_elder)) =
            owners.get_mut(entity)
        else {
            continue;
        };

        let mut staged_cultivation = cultivation.clone();
        let mut staged_rat = rat.as_deref().cloned();
        let mut staged_daozhan = daozhan.as_deref().cloned();
        let mut staged_spider = spider.as_deref().cloned();
        let mut staged_zones = zones.as_deref().cloned();
        let mut staged_ledger = ledger.clone();
        let transfers = match settle_npc_qi_owners(
            &mut staged_cultivation,
            staged_rat.as_mut(),
            staged_daozhan.as_mut(),
            staged_spider.as_mut(),
            position.map(|position| position.get()),
            dimension.map(|dimension| dimension.0),
            life_record,
            staged_zones.as_mut(),
            &mut staged_ledger,
        ) {
            Ok(transfers) => transfers,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    target = ?entity,
                    "[bong][npc] retained pending NPC after qi settlement failed closed"
                );
                continue;
            }
        };

        let mut staged_lifecycle = lifecycle.clone();
        let mut staged_life_record = life_record.clone();
        let mut staged_death_registry = death_registry.clone();
        staged_death_registry.record_death(pending.at_tick, pending.death_zone);
        if !matches!(
            staged_lifecycle.state,
            LifecycleState::NearDeath | LifecycleState::AwaitingRevival
        ) {
            staged_lifecycle.death_count = staged_lifecycle.death_count.saturating_add(1);
        }
        staged_lifecycle.terminate(pending.at_tick);
        staged_life_record.push(
            crate::cultivation::life_record::BiographyEntry::Terminated {
                cause: pending.cause.clone(),
                tick: pending.at_tick,
            },
        );
        if let Err(error) = persist_npc_terminal_qi_transaction(
            &persistence,
            &staged_lifecycle,
            &staged_life_record,
            TerminalPersistencePayload {
                death_registry_cause: Some(pending.cause.as_str()),
                lifespan_event: pending.lifespan_event.as_ref(),
                zones: staged_zones.as_ref(),
                qi_ledger: Some(&staged_ledger),
                narration_outbox: pending.narration_outbox.as_ref(),
                loot_outbox: pending.loot_outbox.as_ref(),
            },
        ) {
            tracing::warn!(
                target = ?entity,
                "[bong][persistence] retained pending NPC after terminal persistence failed: {error}"
            );
            continue;
        }

        *cultivation = staged_cultivation;
        if let Some(dying_elder) = dying_elder.as_deref_mut() {
            dying_elder.qi_current = cultivation.qi_current();
        }
        if let (Some(rat), Some(staged_rat)) = (rat.as_deref_mut(), staged_rat) {
            *rat = staged_rat;
        }
        if let (Some(daozhan), Some(staged_daozhan)) = (daozhan.as_deref_mut(), staged_daozhan) {
            *daozhan = staged_daozhan;
        }
        if let (Some(spider), Some(staged_spider)) = (spider.as_deref_mut(), staged_spider) {
            *spider = staged_spider;
        }
        if let (Some(zones), Some(staged_zones)) = (zones.as_deref_mut(), staged_zones) {
            *zones = staged_zones;
        }
        *ledger = staged_ledger;

        notices.send(build_npc_death_notice(
            *archetype,
            lifespan,
            faction,
            &staged_life_record,
            pending.reason,
        ));
        for transfer in transfers {
            qi_transfers.send(transfer);
        }
        if let Some(reproduction_request) = pending.reproduction.clone() {
            reproduction.send(reproduction_request);
        }
        if let (Some(death_insights), Some(payload)) =
            (death_insights.as_deref_mut(), pending.death_insight.clone())
        {
            death_insights.send(crate::combat::events::DeathInsightRequested { payload });
        }
        if let (Some(lifespan_events), Some(lifespan_event)) = (
            lifespan_events.as_deref_mut(),
            pending.lifespan_event.as_ref(),
        ) {
            lifespan_events.send(LifespanEventEmitted {
                payload: crate::cultivation::lifespan::lifespan_event_payload_from_record(
                    staged_life_record.character_id.clone(),
                    lifespan_event,
                ),
            });
        }
        crate::combat::lifecycle::emit_terminal_vfx(
            position,
            true,
            visual_profile,
            &mut vfx_events,
        );
        terminated.send(PlayerTerminated { entity });
        succeeded.send(NpcTerminalSettlementSucceeded {
            entity,
            at_tick: pending.at_tick,
            cause: pending.cause.clone(),
            reason: pending.reason,
            attacker: pending.attacker,
            attacker_player_id: pending.attacker_player_id.clone(),
            authorize_loot: pending.authorize_loot,
            actor_qi_identity: pending.actor_qi_identity.clone(),
        });

        if let Some(ref mut cooldowns) = cooldowns {
            cooldowns.remove_all_for(entity);
        }
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert((
                Despawned,
                NpcDeathNoticeEmitted,
                staged_lifecycle,
                staged_life_record,
                staged_death_registry,
            ));
            entity_commands.remove::<PendingRetirement>();
            entity_commands.remove::<PendingNpcTermination>();
            entity_commands.remove::<Cultivation>();
            entity_commands.remove::<MeridianSystem>();
            entity_commands.remove::<Contamination>();
            entity_commands.remove::<PracticeLog>();
            entity_commands.remove::<QiColor>();
            entity_commands
                .remove::<crate::cultivation::meridian::severed::MeridianSeveredPermanent>();
        }

        let _ = (pending_retirement, shared_lifespan);
    }
}

fn emit_npc_despawn_notices(
    mut commands: Commands,
    despawned: DespawnedNpcNoticeQuery<'_, '_>,
    mut notices: EventWriter<NpcDeathNotice>,
) {
    for (entity, archetype, lifespan, _shared_lifespan, faction, life_record, possessed) in
        &despawned
    {
        let reason = if possessed.is_some() {
            NpcDeathReason::DuoShe
        } else {
            NpcDeathReason::Despawned
        };
        notices.send(build_npc_death_notice(
            *archetype,
            lifespan,
            faction,
            life_record,
            reason,
        ));
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(NpcDeathNoticeEmitted);
        }
    }
}

fn build_npc_death_notice(
    archetype: NpcArchetype,
    lifespan: &NpcLifespan,
    faction: Option<&FactionMembership>,
    life_record: &LifeRecord,
    reason: NpcDeathReason,
) -> NpcDeathNotice {
    NpcDeathNotice {
        npc_id: life_record.character_id.clone(),
        archetype,
        reason,
        faction_id: faction.map(|membership| membership.faction_id),
        life_record_snapshot: Some(life_record.recent_summary_text(8))
            .filter(|summary| !summary.is_empty()),
        age_ticks: lifespan.age_ticks,
        max_age_ticks: lifespan.max_age_ticks,
        // 在场（hydrated）实体死亡不是离屏 dormant 互殴；坐标无 Position 上下文可取，留 None。
        from_dormant_combat: false,
        pos: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::fauna::mimic_spider::MimicSpiderBlackboard;
    use crate::fauna::rat_phase::{chunk_pos_from_world, RatGroupId};
    use crate::qi_physics::{qi_flow_overflow_account, QiAccountId};
    use crate::world::zone::{Zone, ZoneRegistry};

    fn terminal_test_zone(spirit_qi: f64) -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
                name: "terminal-test".to_string(),
                dimension: DimensionKind::Overworld,
                bounds: (DVec3::new(-32.0, 0.0, -32.0), DVec3::new(32.0, 128.0, 32.0)),
                spirit_qi,
                danger_level: 0,
                active_events: Vec::new(),
                patrol_anchors: Vec::new(),
                blocked_tiles: Vec::new(),
                qi_equilibrium: 0.0,
                qi_inflow_per_min: 0.0,
            }],
        }
    }

    fn terminal_test_persistence(label: &str) -> PersistenceSettings {
        let root = std::env::temp_dir().join(format!(
            "bong-r5-terminal-{label}-{}-{}",
            std::process::id(),
            Uuid::now_v7()
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("deceased");
        crate::persistence::bootstrap_sqlite(&db_path, label)
            .expect("terminal fixture sqlite bootstrap must succeed");
        PersistenceSettings::with_paths(db_path, deceased_dir, label)
    }

    fn terminal_pending_with(
        reason: NpcDeathReason,
        attacker: Option<Entity>,
        authorize_loot: bool,
        life_record: &LifeRecord,
    ) -> PendingNpcTermination {
        PendingNpcTermination {
            cause: reason.as_str().to_string(),
            at_tick: 100,
            death_zone: ZoneDeathKind::Ordinary,
            lifespan_event: None,
            death_insight: None,
            reason,
            attacker,
            attacker_player_id: None,
            authorize_loot,
            actor_qi_identity: ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc)
                .expect("terminal fixture must carry canonical NPC identity"),
            reproduction: None,
            narration_outbox: None,
            loot_outbox: None,
        }
    }

    fn terminal_pending(reason: NpcDeathReason, life_record: &LifeRecord) -> PendingNpcTermination {
        terminal_pending_with(reason, None, false, life_record)
    }

    fn terminal_app(zone_qi: f64) -> App {
        let mut app = App::new();
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(terminal_test_zone(zone_qi));
        app.insert_resource(terminal_test_persistence("terminal-app"));
        app.add_event::<PlayerTerminated>();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, settle_pending_npc_termination);
        app
    }

    #[test]
    fn registry_hysteresis_pauses_at_cap_and_resumes_below_low_watermark() {
        let mut registry = NpcRegistry::default();

        registry.refresh_from_counts(50, HashMap::new(), HashMap::new());
        assert!(
            registry.spawn_paused,
            "should pause when live_npc_count reaches max_npc_count (50)"
        );

        registry.refresh_from_counts(45, HashMap::new(), HashMap::new());
        assert!(
            registry.spawn_paused,
            "should remain paused between resume (40) and max (50)"
        );

        registry.refresh_from_counts(39, HashMap::new(), HashMap::new());
        assert!(
            !registry.spawn_paused,
            "should resume below low watermark (40)"
        );
    }

    #[test]
    fn reserve_spawn_batch_clamps_to_remaining_capacity() {
        let mut registry = NpcRegistry::default();
        registry.refresh_from_counts(48, HashMap::new(), HashMap::new());

        let granted = registry.reserve_spawn_batch(8);
        assert_eq!(
            granted, 2,
            "only 2 slots remaining (50-48), should clamp desired=8 to 2"
        );
        assert_eq!(registry.live_npc_count, 50);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn reserve_zone_batch_clamps_to_zone_cap_and_global_cap() {
        let mut registry = NpcRegistry {
            max_npc_count: 10,
            resume_npc_count: 8,
            ..Default::default()
        };
        registry.per_zone_caps.insert("forest".to_string(), 2);
        registry.counts_by_zone.insert("forest".to_string(), 1);
        registry.live_npc_count = 9;

        let granted = registry.reserve_zone_batch("forest", 5);

        assert_eq!(granted, 1);
        assert_eq!(registry.live_npc_count, 10);
        assert_eq!(registry.counts_by_zone["forest"], 2);
        assert!(registry.spawn_paused);
    }

    #[test]
    fn update_npc_registry_counts_home_zones() {
        let mut app = App::new();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, update_npc_registry);

        app.world_mut().spawn((
            NpcMarker,
            NpcArchetype::Rogue,
            crate::npc::patrol::NpcPatrol::new("forest", DVec3::ZERO),
        ));
        app.world_mut().spawn((
            NpcMarker,
            NpcArchetype::Beast,
            crate::npc::patrol::NpcPatrol::new("marsh", DVec3::ZERO),
        ));
        app.update();

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 2);
        assert_eq!(registry.counts_by_zone["forest"], 1);
        assert_eq!(registry.counts_by_zone["marsh"], 1);
    }

    #[test]
    fn process_retire_requests_stages_natural_aging_without_side_effects() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 73 });
        app.add_event::<NpcRetireRequest>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app.world_mut().spawn(NpcMarker).id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Zombie, Realm::Awaken);
        bundle.lifespan = NpcLifespan::new(99.0, 100.0);
        app.world_mut().entity_mut(entity).insert(bundle);

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let pending = app
            .world()
            .get::<PendingNpcTermination>(entity)
            .expect("retirement must stage one terminal transaction");
        assert_eq!(pending.cause, "natural_aging");
        assert_eq!(pending.at_tick, 73);
        assert_eq!(pending.reason, NpcDeathReason::NaturalAging);
        assert!(pending.reproduction.is_none());
        assert!(
            app.world().get::<Despawned>(entity).is_none(),
            "retirement intent must not despawn before terminal settlement commits"
        );
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>()
                .len(),
            0,
            "zombie retirement must not authorize reproduction"
        );
    }

    #[test]
    fn process_retire_requests_defers_commoner_reproduction_until_commit() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 91 });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(terminal_test_zone(0.5));
        app.insert_resource(terminal_test_persistence("retire-commoner"));
        app.add_event::<NpcRetireRequest>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([42.0, 66.0, 17.5]),
                CurrentDimension(DimensionKind::Overworld),
                crate::npc::patrol::NpcPatrol::new("forest", DVec3::new(42.0, 66.0, 17.5)),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Commoner, Realm::Awaken);
        bundle.lifespan = NpcLifespan::new(89_999.0, 90_000.0);
        app.world_mut().entity_mut(entity).insert(bundle);

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        assert!(app.world().get::<PendingNpcTermination>(entity).is_some());
        assert!(app.world().get::<Despawned>(entity).is_none());
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>()
                .len(),
            0,
            "commoner reproduction must remain invisible before terminal commit"
        );

        app.add_systems(Update, settle_pending_npc_termination);
        app.update();

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        let all: Vec<_> = births.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            1,
            "successful terminal commit must reproduce once"
        );
        let req = all[0];
        assert_eq!(req.archetype, NpcArchetype::Commoner);
        assert_eq!(req.home_zone, "forest");
        assert_eq!(req.position, DVec3::new(42.0, 66.0, 17.5));
        assert_eq!(req.initial_age_ticks, 0.0);
        assert!(app.world().get::<Despawned>(entity).is_some());
    }

    #[test]
    fn settle_pending_npc_termination_emits_notice_and_marks_despawned() {
        let mut app = terminal_app(0.5);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Zombie, Realm::Awaken);
        bundle.lifespan = NpcLifespan::new(120.0, 100.0);
        let pending = terminal_pending(NpcDeathReason::NaturalAging, &bundle.life_record);
        app.world_mut()
            .entity_mut(entity)
            .insert((bundle, PendingRetirement, pending));

        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcDeathNotice>>();
        assert_eq!(events.len(), 1);
        assert!(app.world().get::<Despawned>(entity).is_some());
        assert!(app.world().get::<Cultivation>(entity).is_none());
    }

    #[test]
    fn terminal_settlement_commits_all_npc_owners_before_despawn() {
        let mut app = terminal_app(0.5);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Beast, Realm::Awaken);
        bundle.cultivation.qi_current = 4.0;
        let character_id = bundle.life_record.character_id.clone();
        let account = QiAccountId::npc(character_id);
        let mut rat = RatBlackboard::new("terminal-test", chunk_pos_from_world(DVec3::ZERO));
        rat.group_id = RatGroupId(9);
        rat.drained_qi = 3.0;
        let mut daozhan =
            DaoZhangBehaviorBlackboard::new("terminal-test", DVec3::ZERO, Some(Realm::Spirit));
        daozhan.daozhan_qi = 2.0;
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(account.clone(), 3.0)
            .expect("rat reserve fixture should be valid");
        let pending = terminal_pending(NpcDeathReason::Combat, &bundle.life_record);
        app.world_mut()
            .entity_mut(entity)
            .insert((bundle, rat, daozhan, pending));

        app.update();

        assert!(app.world().get::<Despawned>(entity).is_some());
        assert_eq!(
            app.world().resource::<WorldQiAccount>().balance(&account),
            0.0,
            "canonical rat reserve must be empty before despawn"
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .zones
                .iter()
                .find(|zone| zone.name == "terminal-test")
                .expect("zone fixture should remain")
                .spirit_qi,
            0.68,
            "4 cultivation + 3 rat + 2 daozhan qi must all reach the zone"
        );
        let transfers = app.world().resource::<WorldQiAccount>().transfers();
        assert_eq!(transfers.len(), 3);
        assert!(transfers.iter().all(|transfer| transfer.from == account));
        assert!(transfers
            .iter()
            .all(|transfer| transfer.reason == QiTransferReason::ReleaseToZone));
    }

    #[test]
    fn terminal_settlement_releases_spider_owner_once_and_clears_telemetry() {
        let mut app = terminal_app(0.5);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Beast, Realm::Awaken);
        bundle.cultivation.qi_current = 4.0;
        let account = QiAccountId::npc(bundle.life_record.character_id.clone());
        let mut spider = MimicSpiderBlackboard::new("terminal-test", DVec3::ZERO);
        spider.drained_qi = 9.0;
        let pending = terminal_pending(NpcDeathReason::Combat, &bundle.life_record);
        app.world_mut()
            .entity_mut(entity)
            .insert((bundle, spider, pending));

        app.update();

        assert!(app.world().get::<Despawned>(entity).is_some());
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .zones
                .first()
                .expect("zone fixture should remain")
                .spirit_qi,
            0.54,
            "only Cultivation's physical 4 qi may reach the zone; telemetry must not double-release"
        );
        let transfers = app.world().resource::<WorldQiAccount>().transfers();
        assert_eq!(transfers.len(), 1);
        assert_eq!(transfers[0].from, account);
        assert_eq!(transfers[0].amount, 4.0);
        assert_eq!(
            app.world()
                .get::<MimicSpiderBlackboard>(entity)
                .expect("post-commit telemetry component should remain inspectable")
                .drained_qi,
            0.0,
            "successful settlement must clear the non-owner telemetry mirror"
        );
    }

    #[test]
    fn terminal_settlement_failure_is_retryable_and_atomic() {
        let mut app = terminal_app(1.0);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Beast, Realm::Awaken);
        bundle.cultivation.qi_current = 4.0;
        let character_id = bundle.life_record.character_id.clone();
        let account = QiAccountId::npc(character_id);
        let mut rat = RatBlackboard::new("terminal-test", chunk_pos_from_world(DVec3::ZERO));
        rat.drained_qi = 3.0;
        let mut daozhan =
            DaoZhangBehaviorBlackboard::new("terminal-test", DVec3::ZERO, Some(Realm::Spirit));
        daozhan.daozhan_qi = 2.0;
        {
            let mut ledger = app.world_mut().resource_mut::<WorldQiAccount>();
            ledger
                .set_balance(account.clone(), 3.0)
                .expect("rat reserve fixture should be valid");
            ledger
                .set_balance(qi_flow_overflow_account(), f64::MAX)
                .expect("saturated overflow fixture should be valid");
        }
        let pending = terminal_pending(NpcDeathReason::Combat, &bundle.life_record);
        app.world_mut()
            .entity_mut(entity)
            .insert((bundle, rat, daozhan, pending));

        app.update();

        assert!(app.world().get::<Despawned>(entity).is_none());
        assert!(app.world().get::<PendingNpcTermination>(entity).is_some());
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current(),
            4.0
        );
        assert_eq!(
            app.world().get::<RatBlackboard>(entity).unwrap().drained_qi,
            3.0
        );
        assert_eq!(
            app.world()
                .get::<DaoZhangBehaviorBlackboard>(entity)
                .unwrap()
                .daozhan_qi,
            2.0
        );
        assert_eq!(
            app.world().resource::<WorldQiAccount>().balance(&account),
            3.0
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .zones
                .first()
                .unwrap()
                .spirit_qi,
            1.0
        );
        assert!(app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty());

        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(qi_flow_overflow_account(), 0.0)
            .expect("clearing overflow should be valid");
        app.update();

        assert!(
            app.world().get::<Despawned>(entity).is_some(),
            "pending marker must retry without a second terminal intent"
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&qi_flow_overflow_account()),
            9.0
        );
    }

    #[test]
    fn terminal_settlement_publishes_attacker_and_loot_only_after_commit() {
        let mut app = terminal_app(1.0);
        let attacker = app.world_mut().spawn_empty().id();
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
            ))
            .id();
        let mut bundle = npc_runtime_bundle(entity, NpcArchetype::Beast, Realm::Awaken);
        bundle.cultivation.qi_current = 1.0;
        let pending = terminal_pending_with(
            NpcDeathReason::Combat,
            Some(attacker),
            true,
            &bundle.life_record,
        );
        app.world_mut().entity_mut(entity).insert((bundle, pending));
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(qi_flow_overflow_account(), f64::MAX)
            .expect("saturated overflow fixture should be valid");

        app.update();

        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcTerminalSettlementSucceeded>>()
                .len(),
            0,
            "failed qi settlement must not authorize loot or publish attacker attribution"
        );
        assert!(app.world().get::<Despawned>(entity).is_none());

        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(qi_flow_overflow_account(), 0.0)
            .expect("clearing overflow should be valid");
        app.update();

        let successes = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcTerminalSettlementSucceeded>>();
        let all: Vec<_> = successes.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            1,
            "retrying terminal settlement must publish once"
        );
        assert_eq!(all[0].entity, entity);
        assert_eq!(all[0].attacker, Some(attacker));
        assert!(all[0].authorize_loot);
    }

    #[test]
    fn terminal_settlement_missing_identity_retries_without_mutation() {
        let mut app = terminal_app(0.5);
        let staged_life_record = LifeRecord::new("npc:staged-terminal-owner");
        let pending = terminal_pending(NpcDeathReason::Combat, &staged_life_record);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(0.0, 100.0),
                LifeRecord::new("unassigned:life_record"),
                Lifecycle {
                    character_id: "unassigned:life_record".to_string(),
                    ..Default::default()
                },
                DeathRegistry::new("unassigned:life_record"),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 4.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                pending,
            ))
            .id();

        app.update();

        assert!(app.world().get::<Despawned>(entity).is_none());
        assert!(app.world().get::<PendingNpcTermination>(entity).is_some());
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current(),
            4.0
        );
        assert_eq!(
            app.world()
                .resource::<ZoneRegistry>()
                .zones
                .first()
                .unwrap()
                .spirit_qi,
            0.5
        );
        assert!(app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty());
    }

    #[test]
    fn terminal_settlement_rejects_collective_identity_drift_from_frozen_pending() {
        let mut app = terminal_app(0.5);
        let original_record = LifeRecord::new("npc:terminal:frozen-original");
        let pending = terminal_pending(NpcDeathReason::Combat, &original_record);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(0.0, 100.0),
                LifeRecord::new("npc:terminal:drifted-together"),
                Lifecycle {
                    character_id: "npc:terminal:drifted-together".to_string(),
                    ..Default::default()
                },
                DeathRegistry::new("npc:terminal:drifted-together"),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 4.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                Position(DVec3::ZERO),
                CurrentDimension(DimensionKind::Overworld),
                pending,
            ))
            .id();

        app.update();

        assert!(
            app.world().get::<PendingNpcTermination>(entity).is_some(),
            "three current identity mirrors drifting together must not bypass the frozen capability"
        );
        assert!(app.world().get::<Despawned>(entity).is_none());
        assert_eq!(
            app.world().get::<Cultivation>(entity).unwrap().qi_current(),
            4.0,
            "identity drift rejection must keep the physical owner unchanged"
        );
        assert_eq!(
            app.world().resource::<ZoneRegistry>().zones[0].spirit_qi,
            0.5,
            "identity drift rejection must not mutate the signed zone"
        );
        assert!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .is_empty(),
            "identity drift rejection must not publish an audit projection"
        );
    }

    #[test]
    fn npc_shared_lifespan_syncs_to_ai_age_view() {
        let mut app = App::new();
        app.insert_resource(NpcAgingConfig::default());
        app.add_systems(Update, age_npcs);

        let mut shared_lifespan = LifespanComponent::new(100);
        shared_lifespan.years_lived = 75.0;
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(0.0, 200.0),
                shared_lifespan,
            ))
            .id();

        app.update();

        let lifespan = app.world().get::<NpcLifespan>(entity).unwrap();
        assert_eq!(lifespan.age_ticks, 150.0);
    }

    #[test]
    fn npc_death_notice_fields_are_readable_for_bridge_consumers() {
        let notice = NpcDeathNotice {
            npc_id: "npc_1v1".to_string(),
            archetype: NpcArchetype::Zombie,
            reason: NpcDeathReason::NaturalAging,
            faction_id: None,
            life_record_snapshot: Some("少时醒灵".to_string()),
            age_ticks: 120.0,
            max_age_ticks: 100.0,
            from_dormant_combat: false,
            pos: None,
        };

        assert_eq!(notice.npc_id, "npc_1v1");
        assert_eq!(notice.archetype, NpcArchetype::Zombie);
        assert_eq!(notice.reason, NpcDeathReason::NaturalAging);
        assert_eq!(notice.reason.as_str(), "natural_aging");
        assert_eq!(notice.faction_id, None);
        assert_eq!(notice.life_record_snapshot.as_deref(), Some("少时醒灵"));
        assert_eq!(notice.age_ticks, 120.0);
        assert_eq!(notice.max_age_ticks, 100.0);
        assert!(
            !notice.from_dormant_combat,
            "默认构造点不是离屏 dormant 互殴"
        );
        assert_eq!(notice.pos, None);
    }

    #[test]
    fn build_death_notice_uses_life_record_durable_identity() {
        let life_record = LifeRecord::new("npc:stable:death-notice");
        let notice = build_npc_death_notice(
            NpcArchetype::Zombie,
            &NpcLifespan::new(12.0, 100.0),
            None,
            &life_record,
            NpcDeathReason::Combat,
        );

        assert_eq!(notice.npc_id, "npc:stable:death-notice");
        assert_eq!(notice.reason, NpcDeathReason::Combat);
    }

    #[test]
    fn despawn_notice_requires_canonical_life_record() {
        let mut app = App::new();
        app.add_event::<NpcDeathNotice>();
        app.add_systems(Update, emit_npc_despawn_notices);
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(12.0, 100.0),
                Despawned,
            ))
            .id();

        app.update();

        assert!(
            app.world().get::<NpcDeathNoticeEmitted>(entity).is_none(),
            "despawn telemetry must fail closed when no durable LifeRecord owner exists"
        );
        assert!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcDeathNotice>>()
                .is_empty(),
            "despawn telemetry must not invent an Entity-derived NPC identity"
        );
    }

    #[test]
    fn npc_runtime_bundle_allocates_unique_durable_identity_for_all_owner_components() {
        let mut app = App::new();
        let first_entity = app.world_mut().spawn_empty().id();
        let second_entity = app.world_mut().spawn_empty().id();
        let first = npc_runtime_bundle(first_entity, NpcArchetype::Beast, Realm::Awaken);
        let second = npc_runtime_bundle(second_entity, NpcArchetype::Beast, Realm::Awaken);

        for bundle in [&first, &second] {
            let character_id = &bundle.life_record.character_id;
            assert!(character_id.starts_with("npc:"));
            assert!(Uuid::parse_str(character_id.trim_start_matches("npc:")).is_ok());
            assert_eq!(bundle.death_registry.char_id, *character_id);
            assert_eq!(bundle.lifecycle.character_id, *character_id);
            assert!(
                ActorQiIdentity::from_life_record(&bundle.life_record, ActorQiKind::Npc).is_ok()
            );
        }
        assert_ne!(
            first.life_record.character_id, second.life_record.character_id,
            "separate initial spawns must never share a durable qi owner identity"
        );
        assert_ne!(
            first.life_record.character_id,
            canonical_npc_id(first_entity)
        );
        assert_ne!(
            second.life_record.character_id,
            canonical_npc_id(second_entity)
        );
    }

    #[test]
    fn npc_runtime_bundle_with_age_syncs_shared_lifespan_source_of_truth() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let bundle =
            npc_runtime_bundle_with_age(entity, NpcArchetype::Commoner, Realm::Awaken, 45_000.0);
        assert_eq!(bundle.lifespan.age_ticks, 45_000.0);
        assert_eq!(
            bundle.shared_lifespan.cap_by_realm,
            LifespanCapTable::MORTAL
        );
        assert!((bundle.shared_lifespan.years_lived - 40.0).abs() < 1e-9);
    }

    #[test]
    fn npc_runtime_bundle_writes_realm_and_never_satiates_qi_current() {
        // plan-npc-realm-distribution-v1 P0 choke-point 饱和覆盖：六境界全覆盖
        // ① Cultivation.realm 等于传入的 realm（不再恒 Awaken）
        // ② qi_max 等于 qi_max_for_realm(realm)（对拍 worldview 表，见 breakthrough.rs 测试）
        // ③ qi_current 显式保持 0.0（不满灵——满灵会凭空产生真元撞 qi_physics 守恒红线）
        // ④ shared_lifespan 吃的是传入 realm 而非 Cultivation::default().realm
        // ⑤（P2 收口）meridian_system 已开经脉数等于 realm.required_meridians()——
        //   此前恒为 MeridianSystem::default()（全闭），与同一 realm 派生的
        //   npc_meridian_system_for_realm(realm)（spawn 侧只用来筛技能、从不落地）
        //   互相矛盾，是「意图 realm ≠ 组件状态」双源的另一种形态。
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let mut app = App::new();
            let entity = app.world_mut().spawn_empty().id();
            let bundle = npc_runtime_bundle_with_age(entity, NpcArchetype::Beast, realm, 0.0);
            assert_eq!(
                bundle.cultivation.realm, realm,
                "realm={realm:?}: Cultivation.realm 必须等于传入的 realm，实际 {:?}",
                bundle.cultivation.realm
            );
            assert_eq!(
                bundle.cultivation.qi_current, 0.0,
                "realm={realm:?}: qi_current 必须保持 0.0（不满灵），实际 {}",
                bundle.cultivation.qi_current
            );
            assert_eq!(
                bundle.cultivation.qi_max,
                qi_max_for_realm(realm),
                "realm={realm:?}: qi_max 必须等于 qi_max_for_realm(realm)，实际 {}",
                bundle.cultivation.qi_max
            );
            assert_eq!(
                bundle.shared_lifespan.cap_by_realm,
                LifespanCapTable::for_realm(realm),
                "realm={realm:?}: shared_lifespan 必须吃传入 realm 而非默认 Awaken"
            );
            assert_eq!(
                bundle.meridian_system.opened_count(),
                realm.required_meridians(),
                "realm={realm:?}: meridian_system 已开经脉数必须等于 realm.required_meridians()（{}），实际 {}",
                realm.required_meridians(),
                bundle.meridian_system.opened_count()
            );
        }

        // 2-arg wrapper 同源透传（不允许悄悄塞 Realm::Awaken 折中默认值）。
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let bundle = npc_runtime_bundle(entity, NpcArchetype::Beast, Realm::Solidify);
        assert_eq!(bundle.cultivation.realm, Realm::Solidify);
        assert_eq!(
            bundle.meridian_system.opened_count(),
            Realm::Solidify.required_meridians(),
            "2-arg wrapper 的 meridian_system 同样必须按传入 realm 派生，不能悄悄用默认全闭"
        );
        assert_eq!(bundle.cultivation.qi_current, 0.0);
        assert_eq!(bundle.cultivation.qi_max, qi_max_for_realm(Realm::Solidify));
    }

    #[test]
    fn ageing_sync_skips_non_aging_archetypes() {
        let mut app = App::new();
        app.insert_resource(NpcAgingConfig::default());
        app.add_systems(Update, age_npcs);

        let mut shared_lifespan = LifespanComponent::new(100);
        shared_lifespan.years_lived = 90.0;
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::GuardianRelic,
                NpcLifespan::new(0.0, 200.0),
                shared_lifespan,
            ))
            .id();

        app.update();

        let lifespan = app.world().get::<NpcLifespan>(entity).unwrap();
        assert_eq!(lifespan.age_ticks, 0.0);
    }

    #[test]
    fn register_ages_lifespan_once_per_update_tick() {
        let mut app = App::new();
        register(&mut app);
        app.insert_resource(CombatClock { tick: 0 });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(terminal_test_persistence("register-aging"));
        app.add_event::<QiTransfer>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();

        let entity = app
            .world_mut()
            .spawn((NpcMarker, NpcArchetype::Rogue, NpcLifespan::new(0.0, 100.0)))
            .id();
        let rate = NpcAgingConfig::default().rate_multiplier;

        app.update();
        assert_eq!(
            app.world().get::<NpcLifespan>(entity).unwrap().age_ticks,
            rate
        );

        app.update();
        assert_eq!(
            app.world().get::<NpcLifespan>(entity).unwrap().age_ticks,
            rate * 2.0
        );
    }

    /// 端到端：致命 AttackIntent → resolve → DeathEvent → death_arbiter
    /// → NearDeath → near_death_tick 立即 Terminated（NPC 跳过等待窗口）
    /// → settle_pending_npc_termination → `Despawned`.
    #[test]
    fn npc_full_death_chain_from_attack_to_despawned() {
        use crate::combat::events::{
            ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathCinematicPublished,
            DeathEvent, FIST_REACH,
        };
        use crate::combat::lifecycle::{death_arbiter_tick, near_death_tick};
        use crate::combat::resolve::resolve_attack_intents;
        use crate::combat::CombatClock;
        use crate::cultivation::death_hooks::{
            CultivationDeathTrigger, PlayerRevived, PlayerTerminated,
        };
        use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
        use std::time::{SystemTime, UNIX_EPOCH};
        use valence::prelude::{App, IntoSystemConfigs, Position, Update};

        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-npc-full-death-chain-{}-{unique_suffix}",
            std::process::id()
        ));
        let db_path = root.join("data").join("bong.db");
        let deceased_dir = root.join("library-web").join("public").join("deceased");
        bootstrap_sqlite(&db_path, "npc-full-death-chain")
            .expect("sqlite bootstrap should succeed");

        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 100 });
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            "npc-full-death-chain",
        ));
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<PlayerTerminated>();
        app.add_event::<PlayerRevived>();
        app.add_event::<DeathCinematicPublished>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_event::<QiTransfer>();
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                death_arbiter_tick.after(resolve_attack_intents),
                near_death_tick.after(death_arbiter_tick),
                settle_pending_npc_termination.after(near_death_tick),
            ),
        );

        // 两个 NPC：attacker（满 qi）+ victim（濒死）
        let attacker = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let mut attacker_bundle = npc_runtime_bundle(attacker, NpcArchetype::Zombie, Realm::Awaken);
        attacker_bundle.cultivation.qi_current = 80.0;
        attacker_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(attacker).insert(attacker_bundle);

        let victim = app
            .world_mut()
            .spawn((NpcMarker, Position::new([1.0, 64.0, 0.0])))
            .id();
        let mut victim_bundle = npc_runtime_bundle(victim, NpcArchetype::Commoner, Realm::Awaken);
        victim_bundle.wounds.health_current = 3.0;
        victim_bundle.wounds.health_max = 100.0;
        victim_bundle.cultivation.qi_current = 80.0;
        victim_bundle.cultivation.qi_max = 100.0;
        app.world_mut().entity_mut(victim).insert(victim_bundle);

        let victim_record = app
            .world()
            .get::<crate::cultivation::life_record::LifeRecord>(victim)
            .expect("death lifecycle NPC bundle should carry LifeRecord");
        assert!(
            victim_record.character_id.starts_with("npc:"),
            "live NPC qi owner must use a durable character id rather than Bevy Entity identity"
        );
        assert!(
            Uuid::parse_str(victim_record.character_id.trim_start_matches("npc:")).is_ok(),
            "live NPC character id must carry a valid UUID"
        );
        assert_ne!(
            victim_record.character_id,
            canonical_npc_id(victim),
            "durable NPC character id must not reuse the runtime Entity key"
        );

        // 一击致命。
        app.world_mut().send_event(AttackIntent {
            attacker,
            target: Some(victim),
            issued_at_tick: 99,
            reach: FIST_REACH,
            qi_invest: 30.0,
            wound_kind: crate::combat::components::WoundKind::Blunt,
            source: crate::combat::events::AttackSource::Melee,
            debug_command: None,
        });

        // All NPCs skip the NearDeath wait window and terminate immediately.
        // Tick 1: resolve 写 Wounds + DeathEvent
        // Tick 2: death_arbiter → NearDeath → near_death_tick 立即 Terminated
        // Tick 3: settle_pending_npc_termination 插 Despawned + 发 NpcDeathNotice
        app.update();
        app.update();
        app.update();

        let victim_lifecycle = app
            .world()
            .entity(victim)
            .get::<crate::combat::components::Lifecycle>()
            .expect("victim keeps Lifecycle");
        assert_eq!(
            victim_lifecycle.state,
            crate::combat::components::LifecycleState::Terminated,
            "NPC should skip NearDeath wait and go straight to Terminated"
        );

        assert!(
            app.world()
                .get::<valence::prelude::Despawned>(victim)
                .is_some(),
            "victim should be marked Despawned after termination chain"
        );

        // attacker 存活。
        assert!(app
            .world()
            .get::<valence::prelude::Despawned>(attacker)
            .is_none());
        let attacker_life = app
            .world()
            .entity(attacker)
            .get::<crate::combat::components::Lifecycle>()
            .unwrap();
        assert_eq!(
            attacker_life.state,
            crate::combat::components::LifecycleState::Alive
        );

        let _ = std::fs::remove_dir_all(root);
    }

    // -----------------------------------------------------------------------
    // plan-npc-overhaul-v1 P1 tests
    // -----------------------------------------------------------------------

    #[test]
    fn registry_defaults_50() {
        let registry = NpcRegistry::default();
        assert_eq!(
            registry.max_npc_count, 50,
            "plan-npc-overhaul-v1 §P1.1: max_npc_count should default to 50, got {}",
            registry.max_npc_count
        );
    }

    #[test]
    fn registry_defaults_resume_40() {
        let registry = NpcRegistry::default();
        assert_eq!(
            registry.resume_npc_count, 40,
            "plan-npc-overhaul-v1 §P1.1: resume_npc_count should default to 40, got {}",
            registry.resume_npc_count
        );
    }

    #[test]
    fn bucket_independence_beast_full_does_not_block_humanoid() {
        let mut registry = NpcRegistry::default();
        // Fill beast bucket to cap.
        registry.counts_by_bucket.insert(NpcBudgetBucket::Beast, 20);
        registry.live_npc_count = 20;

        // Try to reserve humanoid — should succeed since humanoid bucket is empty.
        let granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Humanoid, 5);
        assert_eq!(
            granted, 5,
            "beast bucket full should not block humanoid reservation; got {}",
            granted
        );

        // Try to reserve beast — should fail since beast bucket is full.
        let beast_granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Beast, 1);
        assert_eq!(
            beast_granted, 0,
            "beast bucket at cap (20) should reject new beast reservation; got {}",
            beast_granted
        );
    }

    #[test]
    fn total_cap_50_three_buckets_sum() {
        let registry = NpcRegistry::default();
        let total_bucket_cap: usize = NpcBudgetBucket::ALL
            .iter()
            .map(|b| registry.bucket_caps.get(b).copied().unwrap_or(0))
            .sum();
        assert_eq!(
            total_bucket_cap, 50,
            "sum of all bucket caps (humanoid:26 + beast:20 + special:4) should equal global cap 50, got {}",
            total_bucket_cap
        );
    }

    #[test]
    fn bucket_mapping_covers_all_archetypes() {
        // Ensure every NpcArchetype variant maps to a bucket (compiler enforces
        // exhaustive match, but this test locks the mapping for regression).
        for archetype in NpcArchetype::ALL {
            let bucket = archetype.budget_bucket();
            assert!(
                NpcBudgetBucket::ALL.contains(&bucket),
                "archetype {:?} mapped to unknown bucket {:?}",
                archetype,
                bucket
            );
        }
        // Verify count matches known variant count.
        assert_eq!(
            NpcArchetype::ALL.len(),
            12,
            "NpcArchetype::ALL should contain exactly 12 variants (update if enum grows)"
        );
    }

    #[test]
    fn archetype_from_str_round_trips_every_variant() {
        // plan-offscreen-war-v1 P3：sqlite `pending_dormant_relics.archetype` 存 as_str()，
        // hydrate 时 from_str() 还原。任一变体 round-trip 失败 = 战场遗物 loot 会 roll 错表。
        for archetype in NpcArchetype::ALL {
            let restored = NpcArchetype::from_str(archetype.as_str());
            assert_eq!(
                restored,
                Some(archetype),
                "NpcArchetype::from_str(as_str({archetype:?})) must round-trip to the same variant; \
                 a missing from_str arm would silently mis-roll battlefield relic loot on hydrate"
            );
        }
    }

    #[test]
    fn archetype_from_str_rejects_unknown_string() {
        // 脏数据（旧 schema / 手写 sqlite）必须返回 None 让调用方显式跳过，绝不静默吞成
        // 默认 Zombie（那会让一个无来历的串掉出僵尸 loot）。
        assert_eq!(
            NpcArchetype::from_str("not_a_real_archetype"),
            None,
            "unknown archetype string must return None, not silently coerce to a default variant"
        );
        assert_eq!(
            NpcArchetype::from_str(""),
            None,
            "empty archetype string must return None"
        );
        // 大小写 / 旧 PascalCase 不接受（存的就是 snake_case as_str）。
        assert_eq!(
            NpcArchetype::from_str("Disciple"),
            None,
            "from_str must match the exact snake_case as_str() form, not PascalCase"
        );
    }

    #[test]
    fn reserve_bucket_respects_cap() {
        let mut registry = NpcRegistry::default();
        // Set humanoid bucket close to cap.
        registry
            .counts_by_bucket
            .insert(NpcBudgetBucket::Humanoid, 24);
        registry.live_npc_count = 24;

        let granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Humanoid, 5);
        assert_eq!(
            granted, 2,
            "humanoid bucket at 24/26 should only grant 2 more; got {}",
            granted
        );
        assert_eq!(
            registry.counts_by_bucket[&NpcBudgetBucket::Humanoid],
            26,
            "humanoid bucket count should now be at cap 26"
        );

        // Try again — should get 0.
        let granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Humanoid, 1);
        assert_eq!(
            granted, 0,
            "humanoid bucket at cap should reject; got {}",
            granted
        );
    }

    #[test]
    fn reserve_bucket_respects_global_cap() {
        let mut registry = NpcRegistry {
            live_npc_count: 49,
            ..Default::default()
        };
        registry
            .counts_by_bucket
            .insert(NpcBudgetBucket::Humanoid, 0);

        let granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Humanoid, 5);
        assert_eq!(
            granted, 1,
            "global cap at 49/50 should clamp to 1 even though bucket has room; got {}",
            granted
        );
    }

    #[test]
    fn reserve_bucket_zero_desired_returns_zero() {
        let mut registry = NpcRegistry::default();
        let granted = registry.reserve_bucket_spawn(NpcBudgetBucket::Humanoid, 0);
        assert_eq!(
            granted, 0,
            "requesting 0 should always return 0; got {}",
            granted
        );
    }

    #[test]
    fn refresh_from_counts_tallies_buckets() {
        let mut registry = NpcRegistry::default();
        let mut counts = HashMap::new();
        counts.insert(NpcArchetype::Rogue, 5);
        counts.insert(NpcArchetype::Beast, 3);
        counts.insert(NpcArchetype::GuardianRelic, 1);
        registry.refresh_from_counts(9, counts, HashMap::new());

        assert_eq!(
            registry
                .counts_by_bucket
                .get(&NpcBudgetBucket::Humanoid)
                .copied()
                .unwrap_or(0),
            5,
            "Rogue (humanoid) count should be 5"
        );
        assert_eq!(
            registry
                .counts_by_bucket
                .get(&NpcBudgetBucket::Beast)
                .copied()
                .unwrap_or(0),
            3,
            "Beast (beast) count should be 3"
        );
        assert_eq!(
            registry
                .counts_by_bucket
                .get(&NpcBudgetBucket::Special)
                .copied()
                .unwrap_or(0),
            1,
            "GuardianRelic (special) count should be 1"
        );
    }

    #[test]
    fn per_zone_caps_defaults() {
        let registry = NpcRegistry::default();
        assert_eq!(
            registry.per_zone_caps.get("spawn").copied(),
            Some(6),
            "spawn zone cap should be 6"
        );
        assert_eq!(
            registry.per_zone_caps.get("qingyun_peaks").copied(),
            Some(5),
            "qingyun_peaks zone cap should be 5"
        );
        assert_eq!(
            registry.per_zone_caps.get("spring_marsh").copied(),
            Some(4),
            "spring_marsh zone cap should be 4"
        );
        assert_eq!(
            registry.per_zone_caps.get("rift_valley").copied(),
            Some(5),
            "rift_valley zone cap should be 5"
        );
        assert_eq!(
            registry.per_zone_caps.get("north_wastes").copied(),
            Some(2),
            "north_wastes zone cap should be 2"
        );
        assert_eq!(
            registry.per_zone_caps.get("lingquan_marsh").copied(),
            Some(4),
            "lingquan_marsh zone cap should be 4"
        );
    }
}
