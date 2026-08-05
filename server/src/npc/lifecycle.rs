use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use valence::prelude::{
    apply_deferred, bevy_ecs, Added, App, Bundle, Commands, Component, DVec3, Despawned, Entity,
    Event, EventReader, EventWriter, IntoSystemConfigs, IntoSystemSetConfigs, Position, Query, Res,
    ResMut, Resource, SystemSet, Update, With, Without,
};

use crate::combat::components::{
    CombatState, DerivedAttrs, Lifecycle, LifecycleState, Stamina, StatusEffects, Wounds,
};
use crate::cultivation::breakthrough::qi_max_for_realm;
use crate::cultivation::components::{
    release_external_qi_to_zone, ActorQiIdentity, ActorQiKind, Contamination, Cultivation,
    MeridianSystem, Realm,
};
use crate::cultivation::death_hooks::{
    CultivationDeathCause, CultivationDeathTrigger, PlayerTerminated,
};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::cultivation::lifespan::{
    DeathRegistry, LifespanCapTable, LifespanComponent, LifespanExtensionLedger, ZoneDeathKind,
};
use crate::cultivation::possession::PossessedVictim;
use crate::npc::brain::canonical_npc_id;
use crate::npc::faction::{FactionId, FactionMembership};
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::NpcMarker;
use crate::npc::technique::NpcCooldownMap;

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
type DespawnedNpcNoticeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static NpcArchetype,
        &'static NpcLifespan,
        Option<&'static LifespanComponent>,
        Option<&'static FactionMembership>,
        Option<&'static LifeRecord>,
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

#[derive(Clone, Debug, Component)]
pub struct PendingNpcTermination {
    pub cause: String,
    pub at_tick: u64,
    pub death_zone: ZoneDeathKind,
    pub lifespan_event: Option<crate::persistence::LifespanEventRecord>,
    pub death_insight: Option<crate::schema::death_insight::DeathInsightRequestV1>,
    pub reason: NpcDeathReason,
    pub attacker: Option<Entity>,
    pub attacker_player_id: Option<String>,
    pub authorize_loot: bool,
    pub actor_qi_identity: ActorQiIdentity,
    pub reproduction: Option<NpcReproductionRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
enum NpcTerminalBarrierSet {
    StageFlush,
    CommitFlush,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, SystemSet)]
pub enum NpcTerminalSystemSet {
    Stage,
    Commit,
    PostCommit,
}

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

#[derive(Clone, Copy, Debug, Component)]
pub struct PendingRetirement;

#[derive(Clone, Copy, Debug, Component)]
struct NpcTerminalCommitted;

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

pub(crate) fn natural_aging_reproduction_request(
    archetype: Option<&NpcArchetype>,
    position: Option<&Position>,
    patrol: Option<&NpcPatrol>,
) -> Option<NpcReproductionRequest> {
    if archetype != Some(&NpcArchetype::Commoner) {
        return None;
    }
    position
        .zip(patrol)
        .map(|(position, patrol)| NpcReproductionRequest {
            archetype: NpcArchetype::Commoner,
            position: position.get(),
            home_zone: patrol.home_zone.clone(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        })
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
    pub npc_id: String,
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn commit_pending_npc_terminations(
    mut commands: Commands,
    pending: Query<
        (
            Entity,
            &PendingNpcTermination,
            &Lifecycle,
            &LifeRecord,
            &DeathRegistry,
            &Cultivation,
            &NpcArchetype,
            &NpcLifespan,
            Option<&FactionMembership>,
            Option<&crate::skin::NpcVisualProfile>,
            Option<&Position>,
            Option<&crate::world::dimension::CurrentDimension>,
            Option<&crate::fauna::daozhan::DaoZhangBehaviorBlackboard>,
        ),
        (
            With<NpcMarker>,
            Without<Despawned>,
            Without<NpcTerminalCommitted>,
        ),
    >,
    persistence: Option<Res<crate::persistence::PersistenceSettings>>,
    zones: Option<ResMut<crate::world::zone::ZoneRegistry>>,
    ledger: Option<ResMut<crate::qi_physics::WorldQiAccount>>,
    mut settlement_events: EventWriter<NpcTerminalSettlementSucceeded>,
    mut death_insights: EventWriter<crate::combat::events::DeathInsightRequested>,
    mut death_notices: EventWriter<NpcDeathNotice>,
    mut reproduction_requests: EventWriter<NpcReproductionRequest>,
    mut vfx_events: EventWriter<crate::network::vfx_event_emit::VfxEventRequest>,
    cooldowns: Option<ResMut<NpcCooldownMap>>,
) {
    let (Some(persistence), Some(mut zones), Some(mut ledger)) = (persistence, zones, ledger)
    else {
        return;
    };
    let mut cooldowns = cooldowns;

    for (
        entity,
        pending,
        lifecycle,
        life_record,
        death_registry,
        cultivation,
        archetype,
        lifespan,
        faction,
        npc_visual_profile,
        position,
        dimension,
        daozhan,
    ) in &pending
    {
        let Ok(actor) = ActorQiIdentity::from_life_record(life_record, ActorQiKind::Npc) else {
            continue;
        };
        if lifecycle.character_id != life_record.character_id
            || death_registry.char_id != life_record.character_id
            || pending.actor_qi_identity != actor
        {
            tracing::warn!(
                ?entity,
                "[bong][npc] retained terminal NPC after identity mismatch"
            );
            continue;
        }

        let mut staged_lifecycle = lifecycle.clone();
        let mut staged_record = life_record.clone();
        let mut staged_registry = death_registry.clone();
        let mut staged_cultivation = cultivation.clone();
        let mut staged_zones = zones.clone();
        let mut staged_ledger = ledger.clone();
        staged_lifecycle.terminate(pending.at_tick);
        staged_registry.record_death(pending.at_tick, pending.death_zone);
        staged_record.push(BiographyEntry::Terminated {
            cause: pending.cause.clone(),
            tick: pending.at_tick,
        });

        let zone_name = position.zip(dimension).and_then(|(position, dimension)| {
            staged_zones
                .find_zone(dimension.0, position.get())
                .map(|zone| zone.name.clone())
        });
        let result = staged_cultivation.release_to_zone(
            zone_name
                .as_deref()
                .and_then(|name| staged_zones.find_zone_mut(name)),
            &mut staged_ledger,
            &actor,
            staged_cultivation.qi_current,
            crate::qi_physics::QiTransferReason::ReleaseToZone,
        );
        if result.is_err() {
            continue;
        }

        let mut staged_daozhan = daozhan.cloned();
        if let Some(daozhan) = staged_daozhan.as_mut() {
            let daozhan_amount = daozhan.daozhan_qi;
            if release_external_qi_to_zone(
                &mut daozhan.daozhan_qi,
                actor.account(),
                zone_name
                    .as_deref()
                    .and_then(|name| staged_zones.find_zone_mut(name)),
                &mut staged_ledger,
                daozhan_amount,
                crate::qi_physics::QiTransferReason::ReleaseToZone,
            )
            .is_err()
            {
                continue;
            }
        }

        if crate::persistence::persist_npc_termination_with_qi_snapshot(
            &persistence,
            &staged_lifecycle,
            &staged_record,
            pending.cause.as_str(),
            pending.lifespan_event.as_ref(),
            &staged_zones,
            &staged_ledger,
        )
        .is_err()
        {
            continue;
        }

        *zones = staged_zones;
        *ledger = staged_ledger;

        crate::combat::lifecycle::emit_terminal_vfx(
            position,
            true,
            npc_visual_profile,
            &mut vfx_events,
        );
        death_notices.send(build_npc_death_notice(
            entity,
            *archetype,
            lifespan,
            faction,
            Some(&staged_record),
            pending.reason,
        ));
        if let Some(reproduction) = pending.reproduction.clone() {
            reproduction_requests.send(reproduction);
        }
        if let Some(ref mut cooldowns) = cooldowns {
            cooldowns.remove_all_for(entity);
        }

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            staged_lifecycle,
            staged_record,
            staged_registry,
            staged_cultivation,
            NpcTerminalCommitted,
            NpcDeathNoticeEmitted,
            Despawned,
        ));
        if let Some(staged_daozhan) = staged_daozhan {
            entity_commands.insert(staged_daozhan);
        }
        if let Some(payload) = pending.death_insight.clone() {
            death_insights.send(crate::combat::events::DeathInsightRequested { payload });
        }
        settlement_events.send(NpcTerminalSettlementSucceeded {
            entity,
            at_tick: pending.at_tick,
            cause: pending.cause.clone(),
            reason: pending.reason,
            attacker: pending.attacker,
            attacker_player_id: pending.attacker_player_id.clone(),
            authorize_loot: pending.authorize_loot,
            actor_qi_identity: pending.actor_qi_identity.clone(),
        });
    }
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
        .add_event::<crate::combat::events::DeathInsightRequested>()
        .add_event::<crate::network::vfx_event_emit::VfxEventRequest>()
        .configure_sets(
            Update,
            (
                NpcTerminalSystemSet::Stage,
                NpcTerminalBarrierSet::StageFlush,
                NpcTerminalSystemSet::Commit,
                NpcTerminalBarrierSet::CommitFlush,
                NpcTerminalSystemSet::PostCommit,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                update_npc_registry,
                age_npcs,
                process_npc_retire_requests,
                emit_npc_despawn_notices,
            ),
        )
        .add_systems(
            Update,
            (
                apply_deferred.in_set(NpcTerminalBarrierSet::StageFlush),
                commit_pending_npc_terminations.in_set(NpcTerminalSystemSet::Commit),
                apply_deferred.in_set(NpcTerminalBarrierSet::CommitFlush),
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
    entity: Entity,
    archetype: NpcArchetype,
    realm: Realm,
    initial_age_ticks: f64,
) -> NpcRuntimeBundle {
    let char_id = canonical_npc_id(entity);
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
    mut retire_requests: EventReader<NpcRetireRequest>,
    npcs: Query<(&NpcArchetype, &NpcLifespan), With<NpcMarker>>,
    mut cultivation_deaths: EventWriter<CultivationDeathTrigger>,
) {
    for request in retire_requests.read() {
        let Ok((archetype, lifespan)) = npcs.get(request.entity) else {
            continue;
        };

        cultivation_deaths.send(CultivationDeathTrigger {
            entity: request.entity,
            cause: CultivationDeathCause::NaturalAging,
            context: json!({
                "npc_id": canonical_npc_id(request.entity),
                "archetype": archetype.as_str(),
                "age_ticks": lifespan.age_ticks,
                "max_age_ticks": lifespan.max_age_ticks,
                "age_ratio": lifespan.age_ratio(),
                "reason": "retire_action",
            }),
        });
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
            entity,
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
    entity: Entity,
    archetype: NpcArchetype,
    lifespan: &NpcLifespan,
    faction: Option<&FactionMembership>,
    life_record: Option<&LifeRecord>,
    reason: NpcDeathReason,
) -> NpcDeathNotice {
    NpcDeathNotice {
        npc_id: life_record
            .map(|record| record.character_id.clone())
            .unwrap_or_else(|| canonical_npc_id(entity)),
        archetype,
        reason,
        faction_id: faction.map(|membership| membership.faction_id),
        life_record_snapshot: life_record
            .map(|record| record.recent_summary_text(8))
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
    fn process_retire_requests_emits_natural_aging_trigger_without_terminal_side_effects() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Zombie,
                NpcLifespan::new(99.0, 100.0),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<CultivationDeathTrigger>>();
        assert_eq!(events.len(), 1);

        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        assert_eq!(
            births.len(),
            0,
            "zombie retirement must not trigger reproduction"
        );
    }

    #[test]
    fn retire_request_defers_commoner_reproduction_until_terminal_commit() {
        let mut app = App::new();
        app.add_event::<NpcRetireRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcReproductionRequest>();
        app.add_systems(Update, process_npc_retire_requests);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcArchetype::Commoner,
                NpcLifespan::new(89_999.0, 90_000.0),
                Position::new([42.0, 66.0, 17.5]),
                crate::npc::patrol::NpcPatrol::new("forest", DVec3::new(42.0, 66.0, 17.5)),
            ))
            .id();

        app.world_mut().send_event(NpcRetireRequest { entity });
        app.update();

        let deaths = app
            .world()
            .resource::<bevy_ecs::event::Events<CultivationDeathTrigger>>();
        assert_eq!(deaths.len(), 1, "retirement must still stage natural death");
        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        assert_eq!(
            births.len(),
            0,
            "retirement must not reproduce before terminal persistence succeeds"
        );
    }

    #[test]
    fn natural_aging_reproduction_requires_complete_commoner_context() {
        let commoner = NpcArchetype::Commoner;
        let zombie = NpcArchetype::Zombie;
        let position = Position::new([42.0, 66.0, 17.5]);
        let patrol = crate::npc::patrol::NpcPatrol::new("forest", DVec3::new(42.0, 66.0, 17.5));

        assert!(
            natural_aging_reproduction_request(Some(&zombie), Some(&position), Some(&patrol))
                .is_none()
        );
        assert!(natural_aging_reproduction_request(Some(&commoner), None, Some(&patrol)).is_none());
        assert!(
            natural_aging_reproduction_request(Some(&commoner), Some(&position), None).is_none()
        );
        let request =
            natural_aging_reproduction_request(Some(&commoner), Some(&position), Some(&patrol))
                .expect("complete commoner context should authorize post-commit reproduction");
        assert_eq!(request.archetype, NpcArchetype::Commoner);
        assert_eq!(request.home_zone, "forest");
        assert_eq!(request.position, DVec3::new(42.0, 66.0, 17.5));
        assert_eq!(request.initial_age_ticks, 0.0);
        assert_eq!(request.territory_center, None);
        assert_eq!(request.territory_radius, None);
    }

    #[test]
    fn terminal_commit_defers_notices_and_reproduction_until_sqlite_succeeds() {
        use std::time::{SystemTime, UNIX_EPOCH};

        use crate::combat::events::DeathInsightRequested;
        use crate::persistence::{bootstrap_sqlite, PersistenceSettings};
        use crate::qi_physics::WorldQiAccount;
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use crate::world::zone::ZoneRegistry;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "bong-terminal-commit-side-effects-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let blocked_db_path = root.join("blocked-db-path");
        std::fs::create_dir_all(&blocked_db_path).expect("blocked db path should be a directory");
        let live_db_path = root.join("bong.db");
        let deceased_dir = root.join("deceased");

        let mut app = App::new();
        app.insert_resource(PersistenceSettings::with_paths(
            &blocked_db_path,
            &deceased_dir,
            "terminal-side-effects-fail",
        ));
        app.insert_resource(ZoneRegistry::default());
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_event::<DeathInsightRequested>();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_systems(Update, commit_pending_npc_terminations);

        let entity = app
            .world_mut()
            .spawn((NpcMarker, Position::new([0.0, 64.0, 0.0])))
            .id();
        let bundle = npc_runtime_bundle(entity, NpcArchetype::Commoner, Realm::Awaken);
        let actor_qi_identity =
            ActorQiIdentity::from_life_record(&bundle.life_record, ActorQiKind::Npc)
                .expect("fixture identity should be canonical");
        app.world_mut().entity_mut(entity).insert((
            bundle,
            CurrentDimension(DimensionKind::Overworld),
            crate::npc::patrol::NpcPatrol::new("spawn", DVec3::ZERO),
            PendingNpcTermination {
                cause: "cultivation:NaturalAging".to_string(),
                at_tick: 77,
                death_zone: ZoneDeathKind::Ordinary,
                lifespan_event: None,
                death_insight: None,
                reason: NpcDeathReason::NaturalAging,
                attacker: None,
                attacker_player_id: None,
                authorize_loot: false,
                actor_qi_identity,
                reproduction: Some(NpcReproductionRequest {
                    archetype: NpcArchetype::Commoner,
                    position: DVec3::new(0.0, 64.0, 0.0),
                    home_zone: "spawn".to_string(),
                    initial_age_ticks: 0.0,
                    territory_center: None,
                    territory_radius: None,
                }),
            },
        ));

        app.update();
        assert!(app.world().get::<Despawned>(entity).is_none());
        assert!(app.world().get::<NpcTerminalCommitted>(entity).is_none());
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcDeathNotice>>()
                .len(),
            0,
            "failed SQLite commit must not publish a death notice"
        );
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>()
                .len(),
            0,
            "failed SQLite commit must not reproduce"
        );
        assert!(app
            .world()
            .resource::<WorldQiAccount>()
            .transfers()
            .is_empty());
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
                .len(),
            0,
            "failed SQLite commit must not publish terminal VFX"
        );

        bootstrap_sqlite(&live_db_path, "terminal-side-effects-pass")
            .expect("live retry database should bootstrap");
        app.insert_resource(PersistenceSettings::with_paths(
            &live_db_path,
            &deceased_dir,
            "terminal-side-effects-pass",
        ));
        app.update();

        assert!(app.world().get::<Despawned>(entity).is_some());
        assert!(app.world().get::<NpcTerminalCommitted>(entity).is_some());
        let notices = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcDeathNotice>>();
        let notices = notices.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].reason, NpcDeathReason::NaturalAging);
        let births = app
            .world()
            .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>();
        assert_eq!(births.iter_current_update_events().count(), 1);
        assert!(
            app.world()
                .resource::<bevy_ecs::event::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
                .len()
                >= 2,
            "successful NPC terminal commit must publish soul and smoke VFX"
        );
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<NpcDeathNotice>>()
            .clear();
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<NpcReproductionRequest>>()
            .clear();
        app.world_mut()
            .resource_mut::<bevy_ecs::event::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
            .clear();

        app.update();
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcDeathNotice>>()
                .len(),
            0,
            "committed terminal carrier must not publish a duplicate notice"
        );
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<NpcReproductionRequest>>()
                .len(),
            0,
            "committed terminal carrier must not reproduce twice"
        );
        assert_eq!(
            app.world()
                .resource::<bevy_ecs::event::Events<crate::network::vfx_event_emit::VfxEventRequest>>()
                .len(),
            0,
            "committed terminal carrier must not publish terminal VFX twice"
        );
        let _ = std::fs::remove_dir_all(root);
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
    fn npc_death_notice_prefers_durable_character_id_after_hydration() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let life_record = LifeRecord::new("npc:durable:hydrated-7".to_string());
        let lifespan = NpcLifespan::new(40.0, 100.0);

        let notice = build_npc_death_notice(
            entity,
            NpcArchetype::Beast,
            &lifespan,
            None,
            Some(&life_record),
            NpcDeathReason::Combat,
        );

        assert_eq!(notice.npc_id, "npc:durable:hydrated-7");
        assert_ne!(notice.npc_id, canonical_npc_id(entity));
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
    /// → pending terminal → SQLite + qi commit → `Despawned`.
    #[test]
    fn npc_full_death_chain_from_attack_to_despawned() {
        use crate::combat::events::{
            ApplyStatusEffectIntent, AttackIntent, CombatEvent, DeathEvent, DeathInsightRequested,
            FIST_REACH,
        };
        use crate::combat::lifecycle::death_arbiter_tick;
        use crate::combat::resolve::resolve_attack_intents;
        use crate::combat::CombatClock;
        use crate::cultivation::death_hooks::CultivationDeathTrigger;
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
        app.insert_resource(PersistenceSettings::with_paths(
            &db_path,
            &deceased_dir,
            "npc-full-death-chain",
        ));
        app.insert_resource(crate::world::zone::ZoneRegistry::default());
        app.insert_resource(crate::qi_physics::WorldQiAccount::default());
        app.add_event::<AttackIntent>();
        app.add_event::<ApplyStatusEffectIntent>();
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        app.add_event::<crate::combat::weapon::WeaponBroken>();
        app.add_event::<crate::combat::weapon::ShieldBroken>();
        app.add_event::<crate::combat::weapon::ShieldBlockHit>();
        app.add_event::<crate::inventory::InventoryDurabilityChangedEvent>();
        app.add_event::<crate::network::vfx_event_emit::VfxEventRequest>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<NpcDeathNotice>();
        app.add_event::<NpcTerminalSettlementSucceeded>();
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<DeathInsightRequested>();
        app.configure_sets(
            Update,
            (
                NpcTerminalSystemSet::Stage,
                NpcTerminalBarrierSet::StageFlush,
                NpcTerminalSystemSet::Commit,
                NpcTerminalBarrierSet::CommitFlush,
                NpcTerminalSystemSet::PostCommit,
            )
                .chain(),
        );
        app.add_systems(
            Update,
            (
                resolve_attack_intents,
                death_arbiter_tick
                    .in_set(NpcTerminalSystemSet::Stage)
                    .after(resolve_attack_intents),
                apply_deferred.in_set(NpcTerminalBarrierSet::StageFlush),
                commit_pending_npc_terminations.in_set(NpcTerminalSystemSet::Commit),
                apply_deferred.in_set(NpcTerminalBarrierSet::CommitFlush),
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
        assert_eq!(victim_record.character_id, canonical_npc_id(victim));

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

        // Tick 1: resolve 写 Wounds + DeathEvent。
        // Tick 2: death_arbiter stage pending，随后同帧 SQLite + qi commit 并插入 Despawned。
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
