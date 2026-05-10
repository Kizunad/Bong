use bevy_transform::components::{GlobalTransform, Transform};
use big_brain::prelude::{BigBrainSet, FirstToScore, Thinker, ThinkerBuilder};
use valence::entity::marker::MarkerEntityBundle;
use valence::entity::player::PlayerEntityBundle;
use valence::entity::villager::VillagerEntityBundle;
use valence::entity::witch::WitchEntityBundle;
use valence::entity::zombie::ZombieEntityBundle;
use valence::prelude::{
    bevy_ecs, App, Bundle, Commands, Component, DVec3, Entity, EntityKind, EntityLayerId,
    EventReader, EventWriter, IntoSystemConfigs, Position, PostStartup, PreUpdate, Query, Res,
    ResMut, Resource, UniqueId, Update, With,
};

use crate::combat::components::WoundKind;
use crate::combat::events::{AttackReach, FIST_REACH, SPEAR_REACH, SWORD_REACH};
use crate::fauna::components::{fauna_spawn_seed, fauna_tag_for_beast_spawn};
use crate::fauna::visual::{entity_kind_for_beast, visual_kind_for_beast};
use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, CultivateAction, CultivateState,
    CultivationDriveHistory, CultivationDriveScorer, CuriosityScorer, DashAction, DashScorer,
    FarmAction, FearCultivatorScorer, FleeAction, FleeCultivatorAction, HungerScorer,
    MeleeAttackAction, MeleeRangeScorer, PlayerProximityScorer, RetireAction, SeclusionAction,
    SeclusionScorer, StartDuXuAction, TribulationReadyScorer, WanderAction, WanderScorer,
    WanderState,
};
use crate::npc::faction::{
    FactionId, FactionMembership, FactionRank, Lineage, MissionExecuteAction, MissionExecuteState,
    MissionQueue, MissionQueueScorer, Reputation,
};
use crate::npc::farming_brain::{
    HarvestAction, LingtianFarmingScorer, MigrateAction, PlantAction, ReplenishAction, TillAction,
};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{
    npc_runtime_bundle, npc_runtime_bundle_with_age, NpcArchetype, NpcRegistry,
    NpcReproductionRequest, NpcSpawnNotice, NpcSpawnSource,
};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::relic::{
    GuardAction, GuardState, GuardianDuty, GuardianDutyScorer, GuardianRelicTag, TrialAction,
    TrialEval, TrialEvalScorer, TrialState,
};
use crate::npc::scattered_cultivator::{FarmingTemperament, ScatteredCultivator};
use crate::npc::social::{FactionDuelScorer, SocializeAction, SocializeScorer, SocializeState};
use crate::npc::territory::{
    HuntAction, HuntState, ProtectYoungAction, ProtectYoungScorer, ProtectYoungState, Territory,
    TerritoryIntruderScorer, TerritoryPatrolAction, TerritoryPatrolState,
};
use crate::skin::{npc_uuid, NpcPlayerSkin, NpcSkinFallbackPolicy, SignedSkin, SkinPool};
use crate::world::mob_spawn::{MobSpawnFilter, NaturalMobKind};
use crate::world::zone::{Zone, ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const NPC_SPAWN_POSITION: [f64; 3] = [14.0, 66.0, 14.0];
const ROGUE_SEED_BATCH_SIZE: u32 = 10;

pub fn snap_spawn_y_to_surface(
    pos: DVec3,
    terrain: Option<&impl crate::world::terrain::SurfaceProvider>,
) -> DVec3 {
    if let Some(terrain) = terrain {
        let info = terrain.query_surface(pos.x.floor() as i32, pos.z.floor() as i32);
        if info.passable {
            return DVec3::new(pos.x, f64::from(info.y + 1), pos.z);
        }
    }
    pos
}

#[derive(Clone, Copy, Debug, Default, Component)]
pub struct NpcMarker;

pub struct NpcSkinSpawnContext<'a> {
    pub pool: Option<&'a mut SkinPool>,
    pub policy: NpcSkinFallbackPolicy,
}

impl NpcSkinSpawnContext<'_> {
    pub const fn new(
        pool: Option<&mut SkinPool>,
        policy: NpcSkinFallbackPolicy,
    ) -> NpcSkinSpawnContext<'_> {
        NpcSkinSpawnContext { pool, policy }
    }
}

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq)]
enum DeferredNpcBrain {
    ScatteredCultivator,
}

impl DeferredNpcBrain {
    fn build(self) -> ThinkerBuilder {
        match self {
            Self::ScatteredCultivator => scattered_cultivator_thinker(),
        }
    }
}

#[derive(Debug, Clone)]
struct RogueSeedJob {
    zone: Zone,
    count: u32,
}

#[derive(Debug, Default)]
struct RogueSeedProgress {
    initialized: bool,
    done: bool,
    jobs: Vec<RogueSeedJob>,
    job_index: usize,
    spawned_in_job: u32,
    spawned_total: u32,
    resource_zone_count: usize,
    resource_reserved: u32,
    other_zone_count: usize,
    other_reserved: u32,
}

#[derive(Clone, Copy, Debug, Component)]
#[allow(dead_code, unfulfilled_lint_expectations)]
pub struct NpcBlackboard {
    pub nearest_player: Option<Entity>,
    pub player_distance: f32,
    /// Cached world position of the current target (player or duel opponent).
    pub target_position: Option<DVec3>,
    /// GameTick of the last melee attack (for cooldown tracking).
    pub last_melee_tick: u32,
}

impl Default for NpcBlackboard {
    fn default() -> Self {
        Self {
            nearest_player: None,
            player_distance: f32::INFINITY,
            target_position: None,
            last_melee_tick: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Component)]
pub enum NpcMeleeArchetype {
    #[default]
    Brawler,
    Sword,
    Spear,
}

impl NpcMeleeArchetype {
    pub const fn profile(self) -> NpcMeleeProfile {
        match self {
            Self::Brawler => NpcMeleeProfile::fist(),
            Self::Sword => NpcMeleeProfile::sword(),
            Self::Spear => NpcMeleeProfile::spear(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Component)]
pub struct NpcMeleeProfile {
    pub reach: AttackReach,
    pub wound_kind: WoundKind,
    pub preferred_distance: f32,
    pub disengage_distance: f32,
}

impl NpcMeleeProfile {
    pub const fn from_reach(reach: AttackReach, wound_kind: WoundKind) -> Self {
        Self {
            reach,
            wound_kind,
            preferred_distance: reach.base,
            disengage_distance: reach.max * 1.5,
        }
    }

    pub const fn fist() -> Self {
        Self::from_reach(FIST_REACH, WoundKind::Blunt)
    }

    pub const fn sword() -> Self {
        Self::from_reach(SWORD_REACH, WoundKind::Cut)
    }

    pub const fn spear() -> Self {
        Self::from_reach(SPEAR_REACH, WoundKind::Pierce)
    }
}

impl Default for NpcMeleeProfile {
    fn default() -> Self {
        NpcMeleeArchetype::default().profile()
    }
}

#[derive(Clone, Debug, Component)]
pub struct NpcCombatLoadout {
    pub melee_archetype: NpcMeleeArchetype,
    pub movement_capabilities: MovementCapabilities,
}

impl NpcCombatLoadout {
    pub const fn new(
        melee_archetype: NpcMeleeArchetype,
        movement_capabilities: MovementCapabilities,
    ) -> Self {
        Self {
            melee_archetype,
            movement_capabilities,
        }
    }

    pub const fn civilian() -> Self {
        Self::new(
            NpcMeleeArchetype::Brawler,
            MovementCapabilities {
                can_sprint: true,
                can_dash: false,
            },
        )
    }

    pub const fn fighter(melee_archetype: NpcMeleeArchetype) -> Self {
        Self::new(
            melee_archetype,
            MovementCapabilities {
                can_sprint: true,
                can_dash: true,
            },
        )
    }

    pub const fn melee_profile(&self) -> NpcMeleeProfile {
        self.melee_archetype.profile()
    }
}

impl Default for NpcCombatLoadout {
    fn default() -> Self {
        Self::civilian()
    }
}

/// Override target for NPC-vs-NPC scenarios (e.g. duel).
/// When present, the NPC targets this entity instead of the nearest player.
#[derive(Clone, Copy, Debug, Component)]
pub struct DuelTarget(pub Entity);

/// 启动时预生成散修种群（plan §7 Phase 7 等 agent 实装前的硬编码替身）。
/// `resource_fraction` 比例进入 `spirit_qi >= resource_spirit_qi_threshold` 的区域，
/// 其余随机铺到其它 zone；`initial_age_ticks` 按索引分 10 档离散分布，
/// 避免全员同时达到风烛年龄导致批量 retire。
#[derive(Debug, Clone, Resource)]
pub struct RoguePopulationSeedConfig {
    pub target_count: u32,
    pub resource_fraction: f32,
    pub resource_spirit_qi_threshold: f64,
    pub max_initial_age_ratio: f64,
}

impl Default for RoguePopulationSeedConfig {
    fn default() -> Self {
        // 允许通过 `BONG_ROGUE_SEED_COUNT` 环境变量覆盖 target_count。
        // 用途：默认恢复 100 rogue seed；低负载本地调试或隔离 IPC 闭环时仍可
        // 显式设置为 0/10。
        let target_count = std::env::var("BONG_ROGUE_SEED_COUNT")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(100);
        Self {
            target_count,
            resource_fraction: 0.8,
            resource_spirit_qi_threshold: 0.4,
            max_initial_age_ratio: 0.8,
        }
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering startup spawn systems");
    app.insert_resource(RoguePopulationSeedConfig::default())
        .add_systems(
            PostStartup,
            (
                spawn_single_zombie_npc_on_startup,
                log_npc_marker_count.after(spawn_single_zombie_npc_on_startup),
            ),
        )
        .add_systems(
            Update,
            (
                process_npc_reproduction_requests,
                // 种群播种只跑一次（`Local<bool>` 守护），PostStartup 时机在
                // valence ScenarioSingleClient 下 layer 未必就绪，改到 Update 更稳。
                seed_initial_rogue_population_on_startup,
            ),
        )
        .add_systems(
            PreUpdate,
            attach_deferred_npc_brain_system.before(BigBrainSet::Scorers),
        );
}

fn attach_deferred_npc_brain_system(
    mut commands: Commands,
    npcs: Query<(Entity, &DeferredNpcBrain, Option<&NpcLodTier>), With<NpcMarker>>,
) {
    for (entity, deferred, tier) in &npcs {
        if matches!(tier, Some(NpcLodTier::Dormant) | None) {
            continue;
        }
        commands
            .entity(entity)
            .remove::<DeferredNpcBrain>()
            .insert(deferred.build());
    }
}

pub(crate) fn classify_zones_by_qi(zones: &[Zone], threshold: f64) -> (Vec<&Zone>, Vec<&Zone>) {
    zones
        .iter()
        .filter(|z| MobSpawnFilter::default_candidates_for_zone(z).contains(&NaturalMobKind::Rogue))
        .partition(|z| z.spirit_qi >= threshold)
}

pub(crate) fn distribute_counts_evenly(total: u32, buckets: usize) -> Vec<u32> {
    if buckets == 0 || total == 0 {
        return vec![0; buckets];
    }
    let base = total / buckets as u32;
    let remainder = total % buckets as u32;
    (0..buckets)
        .map(|i| {
            if (i as u32) < remainder {
                base + 1
            } else {
                base
            }
        })
        .collect()
}

pub(crate) fn seed_position_for_zone(zone: &Zone, index: u32) -> (DVec3, DVec3) {
    let anchor = if zone.patrol_anchors.is_empty() {
        zone.center()
    } else {
        zone.patrol_anchors[(index as usize) % zone.patrol_anchors.len()]
    };
    // 确定性伪随机 jitter（entity-independent，仅靠 index）。
    let seed = (index as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0xbf58_476d_1ce4_e5b9);
    let jx = (((seed & 0xFFF) as f64) / 4096.0 - 0.5) * 4.0;
    let jz = ((((seed >> 16) & 0xFFF) as f64) / 4096.0 - 0.5) * 4.0;
    let raw = DVec3::new(anchor.x + jx, anchor.y, anchor.z + jz);
    (zone.clamp_position(raw), zone.center())
}

pub(crate) fn initial_age_for_index(index: u32, max_age_ticks: f64, max_ratio: f64) -> f64 {
    let bucket = ((index % 10) as f64) / 10.0;
    (bucket * max_ratio).clamp(0.0, 1.0) * max_age_ticks
}

#[allow(clippy::too_many_arguments)]
fn seed_initial_rogue_population_on_startup(
    mut commands: Commands,
    mut notices: EventWriter<NpcSpawnNotice>,
    config: Option<Res<RoguePopulationSeedConfig>>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut registry: Option<ResMut<NpcRegistry>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
    mut progress: valence::prelude::Local<RogueSeedProgress>,
) {
    if progress.done {
        return;
    }
    let Some(cfg) = config.as_deref() else {
        return;
    };
    if cfg.target_count == 0 {
        progress.done = true;
        return;
    }
    let Some(layer) = layers.iter().next() else {
        // Layer 未 ready（常见于第一 tick），保留 `already_seeded=false` 等下一 tick。
        return;
    };

    if !progress.initialized {
        let Some(zones) = zone_registry.as_deref() else {
            tracing::warn!("[bong][npc] rogue seed skipped — ZoneRegistry missing");
            return;
        };

        // P2-5: 先 classify，确认 at least one zone 可 spawn 再 reserve —— 否则
        // 空 ZoneRegistry 会让 reserve 留下 1-tick 暂态泄漏，误触发 spawn_paused。
        let (resource_zones, other_zones) =
            classify_zones_by_qi(&zones.zones, cfg.resource_spirit_qi_threshold);
        if resource_zones.is_empty() && other_zones.is_empty() {
            tracing::warn!("[bong][npc] rogue seed skipped — no spawnable zones");
            progress.done = true;
            return;
        }

        let (desired_resource_count, desired_other_count) =
            match (resource_zones.is_empty(), other_zones.is_empty()) {
                (true, true) => {
                    return;
                }
                (true, false) => (0u32, cfg.target_count),
                (false, true) => (cfg.target_count, 0u32),
                (false, false) => {
                    let r = ((cfg.target_count as f32) * cfg.resource_fraction).round() as u32;
                    (
                        r.min(cfg.target_count),
                        cfg.target_count.saturating_sub(r.min(cfg.target_count)),
                    )
                }
            };

        let resource_dist = reserve_zone_distribution(
            registry.as_deref_mut(),
            &resource_zones,
            desired_resource_count,
        );
        let other_dist =
            reserve_zone_distribution(registry.as_deref_mut(), &other_zones, desired_other_count);
        let reserved = resource_dist.iter().sum::<u32>() + other_dist.iter().sum::<u32>();
        if reserved == 0 {
            tracing::warn!(
                "[bong][npc] rogue seed skipped — NpcRegistry budget exhausted (desired={})",
                cfg.target_count
            );
            return;
        }

        progress.jobs = resource_zones
            .iter()
            .zip(resource_dist.iter().copied())
            .chain(other_zones.iter().zip(other_dist.iter().copied()))
            .filter(|(_, count)| *count > 0)
            .map(|(zone, count)| RogueSeedJob {
                zone: (*zone).clone(),
                count,
            })
            .collect();
        progress.resource_zone_count = resource_zones.len();
        progress.resource_reserved = resource_dist.iter().sum::<u32>();
        progress.other_zone_count = other_zones.len();
        progress.other_reserved = other_dist.iter().sum::<u32>();
        progress.initialized = true;
    }

    let skin_policy = match skin_pool.as_deref_mut() {
        Some(pool) => {
            pool.drain_ready();
            if pool.ready_for_spawn() {
                NpcSkinFallbackPolicy::AllowFallback
            } else {
                return;
            }
        }
        None => NpcSkinFallbackPolicy::AllowFallback,
    };
    let max_age = NpcArchetype::Rogue.default_max_age_ticks();
    let mut spawned_this_tick = 0;

    while spawned_this_tick < ROGUE_SEED_BATCH_SIZE && progress.job_index < progress.jobs.len() {
        let job = &progress.jobs[progress.job_index];
        if progress.spawned_in_job >= job.count {
            progress.job_index += 1;
            progress.spawned_in_job = 0;
            continue;
        }

        let global_index = progress.spawned_total;
        let (pos, patrol_target) = seed_position_for_zone(&job.zone, global_index);
        let age = initial_age_for_index(global_index, max_age, cfg.max_initial_age_ratio);
        let entity = spawn_scattered_cultivator_at(
            &mut commands,
            NpcSkinSpawnContext::new(skin_pool.as_deref_mut(), skin_policy),
            layer,
            job.zone.name.as_str(),
            pos,
            patrol_target,
            job.zone.spirit_qi,
            age,
        );
        commands
            .entity(entity)
            .remove::<ThinkerBuilder>()
            .insert(DeferredNpcBrain::ScatteredCultivator);
        notices.send(spawn_notice(
            entity,
            NpcArchetype::Rogue,
            NpcSpawnSource::Seed,
            job.zone.name.as_str(),
            pos,
            age,
        ));

        progress.spawned_in_job += 1;
        progress.spawned_total += 1;
        spawned_this_tick += 1;
    }

    if progress.job_index >= progress.jobs.len() {
        tracing::info!(
            "[bong][npc] seeded {} rogue NPCs (resource_zones={} @ {} / other_zones={} @ {})",
            progress.spawned_total,
            progress.resource_zone_count,
            progress.resource_reserved,
            progress.other_zone_count,
            progress.other_reserved,
        );
        progress.done = true;
    }
}

fn process_npc_reproduction_requests(
    mut commands: Commands,
    mut requests: EventReader<NpcReproductionRequest>,
    mut notices: EventWriter<NpcSpawnNotice>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut registry: Option<ResMut<NpcRegistry>>,
    layers: Query<Entity, With<crate::world::dimension::OverworldLayer>>,
) {
    let Some(layer) = layers.iter().next() else {
        // If no layer yet, drain events so they don't pile up across frames.
        for _ in requests.read() {}
        return;
    };

    for request in requests.read() {
        // plan §3.3 Commoner 邻居生子 + §8 Beast 领地繁衍共享同一事件通道。
        match request.archetype {
            NpcArchetype::Commoner => {}
            NpcArchetype::Beast => {
                if request.territory_center.is_none() || request.territory_radius.is_none() {
                    tracing::warn!(
                        "[bong][npc] beast reproduction rejected — missing territory hint (zone=`{}`)",
                        request.home_zone
                    );
                    continue;
                }
            }
            other => {
                tracing::warn!(
                    "[bong][npc] reproduction archetype `{:?}` not supported yet (zone=`{}`)",
                    other,
                    request.home_zone
                );
                continue;
            }
        }

        if let Some(registry) = registry.as_deref_mut() {
            if registry.reserve_zone_batch(request.home_zone.as_str(), 1) == 0 {
                tracing::info!(
                    "[bong][npc] reproduction for `{}` rejected — registry budget exhausted",
                    request.home_zone
                );
                continue;
            }
        }

        let entity = match request.archetype {
            NpcArchetype::Commoner => spawn_commoner_npc_at(
                &mut commands,
                NpcSkinSpawnContext::new(
                    skin_pool.as_deref_mut(),
                    NpcSkinFallbackPolicy::AllowFallback,
                ),
                layer,
                request.home_zone.as_str(),
                request.position,
                request.position,
                request.initial_age_ticks.max(0.0),
            ),
            NpcArchetype::Beast => {
                let territory = Territory::new(
                    request.territory_center.expect("checked above"),
                    request.territory_radius.expect("checked above"),
                );
                spawn_beast_npc_at(
                    &mut commands,
                    layer,
                    request.home_zone.as_str(),
                    request.position,
                    territory,
                    request.initial_age_ticks.max(0.0),
                )
            }
            _ => unreachable!("archetype filter above rejects unsupported variants"),
        };
        tracing::info!(
            "[bong][npc] reproduction spawn {:?} entity={:?} zone=`{}` pos={:?}",
            request.archetype,
            entity,
            request.home_zone,
            request.position
        );
        notices.send(spawn_notice(
            entity,
            request.archetype,
            NpcSpawnSource::Reproduction,
            request.home_zone.as_str(),
            request.position,
            request.initial_age_ticks.max(0.0),
        ));
    }
}

fn reserve_zone_distribution(
    mut registry: Option<&mut NpcRegistry>,
    zones: &[&crate::world::zone::Zone],
    desired_total: u32,
) -> Vec<u32> {
    let desired = distribute_counts_evenly(desired_total, zones.len());
    zones
        .iter()
        .zip(desired)
        .map(|(zone, count)| match registry.as_deref_mut() {
            Some(registry) => {
                registry.reserve_zone_batch(zone.name.as_str(), count as usize) as u32
            }
            None => count,
        })
        .collect()
}

fn startup_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(DashScorer, DashAction)
        .when(ChaseTargetScorer, ChaseAction)
}

fn spawn_single_zombie_npc_on_startup(
    mut commands: Commands,
    dimension_layers: Option<Res<crate::world::dimension::DimensionLayers>>,
    mut notices: EventWriter<NpcSpawnNotice>,
) {
    let Some(dimension_layers) = dimension_layers else {
        return;
    };
    let layer = dimension_layers.overworld;
    let npc_entity = spawn_single_zombie_npc(&mut commands, layer);
    notices.send(spawn_notice(
        npc_entity,
        NpcArchetype::Zombie,
        NpcSpawnSource::Startup,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
        0.0,
    ));

    tracing::info!(
        "[bong][npc] spawned zombie npc entity {npc_entity:?} at [{}, {}, {}]",
        NPC_SPAWN_POSITION[0],
        NPC_SPAWN_POSITION[1],
        NPC_SPAWN_POSITION[2]
    );
}

fn commoner_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(FearCultivatorScorer, FleeCultivatorAction)
        .when(HungerScorer, FarmAction)
        .when(WanderScorer, WanderAction)
}

fn rogue_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SeclusionScorer, SeclusionAction)
        .when(TribulationReadyScorer, StartDuXuAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(CuriosityScorer, WanderAction)
        .when(WanderScorer, WanderAction)
}

fn scattered_cultivator_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SeclusionScorer, SeclusionAction)
        .when(TribulationReadyScorer, StartDuXuAction)
        .when(LingtianFarmingScorer::migrate(), MigrateAction)
        .when(LingtianFarmingScorer::harvest(), HarvestAction)
        .when(LingtianFarmingScorer::replenish(), ReplenishAction)
        .when(LingtianFarmingScorer::plant(), PlantAction)
        .when(LingtianFarmingScorer::till(), TillAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(CuriosityScorer, WanderAction)
        .when(WanderScorer, WanderAction)
}

fn beast_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(ProtectYoungScorer, ProtectYoungAction)
        .when(TerritoryIntruderScorer, HuntAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(WanderScorer, TerritoryPatrolAction)
        .when(WanderScorer, WanderAction)
}

#[allow(dead_code)]
fn disciple_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SeclusionScorer, SeclusionAction)
        .when(TribulationReadyScorer, StartDuXuAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(FactionDuelScorer, ChaseAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(MissionQueueScorer, MissionExecuteAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(SocializeScorer, SocializeAction)
        .when(CuriosityScorer, WanderAction)
        .when(WanderScorer, WanderAction)
}

#[allow(dead_code)]
fn relic_guard_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(GuardianDutyScorer, GuardAction)
        .when(TrialEvalScorer, TrialAction)
        .when(WanderScorer, WanderAction)
}

/// Spawn a Rogue (散修) NPC. MineSkin 池可用时走假玩家 skin；否则退回 vanilla villager。
/// `initial_age_ticks` 允许 agent 投放"已修炼多年"的散修。
pub fn spawn_rogue_npc_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let skin = draw_npc_skin(skin_context, NpcArchetype::Rogue, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        loadout.clone(),
        NpcArchetype::Rogue,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin.filter(|skin| !skin.is_fallback()) {
        attach_player_skin(commands, entity, NpcArchetype::Rogue, skin);
    }

    commands.entity(entity).insert((
        WanderState::default(),
        CultivateState::default(),
        CultivationDriveHistory::default(),
        rogue_npc_thinker(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Rogue, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}

#[derive(Bundle)]
pub struct ScatteredCultivatorBundle {
    pub scattered: ScatteredCultivator,
    pub wander: WanderState,
    pub cultivate: CultivateState,
    pub drive_history: CultivationDriveHistory,
    pub thinker: ThinkerBuilder,
}

impl ScatteredCultivatorBundle {
    pub fn new(temperament: FarmingTemperament) -> Self {
        Self {
            scattered: ScatteredCultivator::new(temperament),
            wander: WanderState::default(),
            cultivate: CultivateState::default(),
            drive_history: CultivationDriveHistory::default(),
            thinker: scattered_cultivator_thinker(),
        }
    }
}

/// Spawn a Rogue-based scattered cultivator that owns a farming brain.
#[allow(clippy::too_many_arguments)]
pub fn spawn_scattered_cultivator_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    qi_density: f64,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let skin = draw_npc_skin(skin_context, NpcArchetype::Rogue, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        loadout.clone(),
        NpcArchetype::Rogue,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin.filter(|skin| !skin.is_fallback()) {
        attach_player_skin(commands, entity, NpcArchetype::Rogue, skin);
    }

    let seed = skin_salt(spawn_position) ^ qi_density.to_bits();
    let temperament = FarmingTemperament::deterministic(seed);
    commands
        .entity(entity)
        .insert(ScatteredCultivatorBundle::new(temperament));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Rogue, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}

/// Spawn a Commoner NPC. MineSkin 池可用时走假玩家 skin；否则退回 vanilla villager。
/// Starting age is controlled by
/// `initial_age_ticks` — newborns pass `0.0`, agent-spawned adults can pass
/// any value below the Commoner default max age.
pub fn spawn_commoner_npc_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let skin = draw_npc_skin(skin_context, NpcArchetype::Commoner, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        loadout.clone(),
        NpcArchetype::Commoner,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin.filter(|skin| !skin.is_fallback()) {
        attach_player_skin(commands, entity, NpcArchetype::Commoner, skin);
    }

    commands.entity(entity).insert((
        Hunger::default(),
        WanderState::default(),
        commoner_npc_thinker(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Commoner, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}

fn draw_npc_skin(
    skin_context: NpcSkinSpawnContext<'_>,
    archetype: NpcArchetype,
    spawn_position: DVec3,
) -> Option<SignedSkin> {
    let pool = skin_context.pool?;
    if skin_context.policy == NpcSkinFallbackPolicy::WaitForReady && !pool.ready_for_spawn() {
        return None;
    }

    let salt = skin_salt(spawn_position);
    Some(pool.next_for(archetype, salt))
}

fn skin_salt(spawn_position: DVec3) -> u64 {
    spawn_position.x.to_bits()
        ^ spawn_position.y.to_bits().rotate_left(17)
        ^ spawn_position.z.to_bits().rotate_left(31)
}

#[allow(clippy::too_many_arguments)]
fn spawn_rogue_commoner_base(
    commands: &mut Commands,
    layer: Entity,
    spawn_position: DVec3,
    skin: &Option<SignedSkin>,
    loadout: NpcCombatLoadout,
    archetype: NpcArchetype,
    home_zone: &str,
    patrol_target: DVec3,
) -> Entity {
    let mut entity_commands = commands.spawn_empty();
    match fallback_rogue_commoner_kind(skin) {
        EntityKind::PLAYER => {
            entity_commands.insert(PlayerEntityBundle {
                kind: EntityKind::PLAYER,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
        EntityKind::WITCH => {
            entity_commands.insert(WitchEntityBundle {
                kind: EntityKind::WITCH,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
        _ => {
            entity_commands.insert(VillagerEntityBundle {
                kind: EntityKind::VILLAGER,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            });
        }
    }

    entity_commands
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            archetype,
            NpcLodTier::Dormant,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
        ))
        .id()
}

fn attach_player_skin(
    commands: &mut Commands,
    entity: Entity,
    archetype: NpcArchetype,
    skin: SignedSkin,
) {
    let uuid = npc_uuid(entity);
    commands.entity(entity).insert((
        UniqueId(uuid),
        NpcPlayerSkin {
            uuid,
            name: npc_skin_name(entity, archetype),
            skin,
        },
    ));
}

fn npc_skin_name(entity: Entity, archetype: NpcArchetype) -> String {
    let mut name = format!("bong_{}_{}", archetype.as_str(), entity.index());
    name.truncate(16);
    name
}

/// Spawn a Beast (妖兽) NPC. 视觉 shell 走 fauna custom EntityKind，由 client GeckoLib renderer 区分种类。
/// `territory` 决定领地中心 + 半径；容量由 `Territory::capacity()` 派生。
/// `initial_age_ticks` 控制年龄（繁衍出来的幼崽传 0.0）。
pub fn spawn_beast_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    territory: Territory,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Brawler);
    let fauna_seed = fauna_spawn_seed(home_zone, spawn_position.x, spawn_position.z);
    let fauna_tag = fauna_tag_for_beast_spawn(home_zone, fauna_seed);
    let visual_kind = visual_kind_for_beast(fauna_tag.beast_kind);
    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: entity_kind_for_beast(fauna_tag.beast_kind),
            layer: EntityLayerId(layer),
            position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            fauna_tag,
        ))
        .insert((
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, territory.center),
        ))
        .id();

    commands.entity(entity).insert((
        NpcLodTier::Dormant,
        Hunger::default(),
        WanderState::default(),
        territory,
        TerritoryPatrolState::default(),
        HuntState::default(),
        ProtectYoungState::default(),
        beast_npc_thinker(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Beast, initial_age_ticks);
    commands.entity(entity).insert(runtime);
    if let Some(visual_kind) = visual_kind {
        commands.entity(entity).insert(visual_kind);
    }

    entity
}

pub fn fallback_rogue_commoner_kind(skin: &Option<SignedSkin>) -> EntityKind {
    if skin.as_ref().is_some_and(|skin| !skin.is_fallback()) {
        EntityKind::PLAYER
    } else {
        EntityKind::VILLAGER
    }
}

/// Spawn a Disciple (宗门弟子) NPC. 基于 Rogue 外观 + 挂 FactionMembership。
/// `faction_id` 决定所属派系（Attack / Defend / Neutral）。`master_id` 可选
/// 挂师承；缺省表示无师父。
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn spawn_disciple_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    faction_id: FactionId,
    rank: FactionRank,
    master_id: Option<String>,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let entity = commands
        .spawn((
            VillagerEntityBundle {
                kind: EntityKind::VILLAGER,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Disciple,
            NpcLodTier::Dormant,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
        ))
        .id();

    commands.entity(entity).insert((
        WanderState::default(),
        CultivateState::default(),
        CultivationDriveHistory::default(),
        SocializeState::default(),
        MissionExecuteState::default(),
        FactionMembership {
            faction_id,
            rank,
            reputation: Reputation::default(),
            lineage: master_id.map(|id| Lineage {
                master_id: Some(id),
                disciple_ids: Vec::new(),
            }),
            mission_queue: MissionQueue::default(),
        },
        disciple_npc_thinker(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Disciple, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}

/// Spawn 仙家遗种守护者（绑定遗迹 ID + 警戒范围）。外观复用 Villager。
#[allow(dead_code)]
pub fn spawn_relic_guard_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    relic_center: DVec3,
    alarm_radius: f64,
    relic_id: impl Into<String>,
    trial_template_id: impl Into<String>,
) -> Entity {
    let loadout = NpcCombatLoadout::fighter(NpcMeleeArchetype::Sword);
    let entity = commands
        .spawn((
            VillagerEntityBundle {
                kind: EntityKind::VILLAGER,
                layer: EntityLayerId(layer),
                position: Position::new([relic_center.x, relic_center.y, relic_center.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                relic_center.x as f32,
                relic_center.y as f32,
                relic_center.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::GuardianRelic,
            NpcLodTier::Dormant,
            Navigator::new(),
            MovementController::new(),
            loadout.movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, relic_center),
        ))
        .id();

    commands.entity(entity).insert((
        WanderState::default(),
        GuardState::default(),
        TrialState::default(),
        GuardianRelicTag,
        GuardianDuty::new(relic_id, relic_center).with_radius(alarm_radius),
        TrialEval::new(trial_template_id),
        relic_guard_thinker(),
    ));

    // GuardianRelic 不老：max_age 极大 + rate_multiplier 不会撞上限
    let runtime = npc_runtime_bundle(entity, NpcArchetype::GuardianRelic);
    commands.entity(entity).insert(runtime);

    entity
}

pub fn spawn_zombie_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let entity = commands
        .spawn((
            ZombieEntityBundle {
                kind: EntityKind::ZOMBIE,
                layer: EntityLayerId(layer),
                position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
                ..Default::default()
            },
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            NpcCombatLoadout::default(),
            NpcCombatLoadout::default().melee_archetype,
            NpcCombatLoadout::default().melee_profile(),
            NpcArchetype::Zombie,
            Navigator::new(),
            MovementController::new(),
            NpcCombatLoadout::default().movement_capabilities,
            MovementCooldowns::default(),
            NpcPatrol::new(home_zone, patrol_target),
            startup_npc_thinker(),
        ))
        .id();

    commands
        .entity(entity)
        .insert(npc_runtime_bundle(entity, NpcArchetype::Zombie));

    entity
}

fn spawn_single_zombie_npc(commands: &mut Commands, layer: Entity) -> Entity {
    spawn_zombie_npc_at(
        commands,
        layer,
        DEFAULT_SPAWN_ZONE_NAME,
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
        DVec3::new(
            NPC_SPAWN_POSITION[0],
            NPC_SPAWN_POSITION[1],
            NPC_SPAWN_POSITION[2],
        ),
    )
}

pub fn spawn_notice(
    entity: Entity,
    archetype: NpcArchetype,
    source: NpcSpawnSource,
    home_zone: &str,
    position: DVec3,
    initial_age_ticks: f64,
) -> NpcSpawnNotice {
    NpcSpawnNotice {
        npc_id: crate::npc::brain::canonical_npc_id(entity),
        archetype,
        source,
        home_zone: home_zone.to_string(),
        position,
        initial_age_ticks,
    }
}

#[cfg(test)]
pub(crate) fn spawn_test_npc_runtime_shape(commands: &mut Commands, layer: Entity) -> Entity {
    spawn_single_zombie_npc(commands, layer)
}

fn log_npc_marker_count(query: Query<Entity, With<NpcMarker>>) {
    tracing::info!(
        "[bong][npc] startup marker count with NpcMarker: {}",
        query.iter().count()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::StatusEffects;
    use crate::combat::events::AttackIntent;
    use crate::npc::brain;
    use crate::npc::lifecycle::NpcLifespan;
    use crate::npc::movement::GameTick;
    use big_brain::prelude::{BigBrainPlugin, HasThinker, ThinkerBuilder};
    use std::collections::HashMap;
    use valence::client::ClientMarker;
    use valence::prelude::{
        bevy_ecs, App, Commands, DVec3, Entity, EntityKind, EntityLayerId, EventReader, Position,
        PreUpdate, Res, Resource, Update,
    };

    #[derive(Clone, Copy, Resource)]
    struct TestLayer(Entity);

    #[derive(Default)]
    struct CapturedAttackIntents(Vec<AttackIntent>);

    impl Resource for CapturedAttackIntents {}

    fn setup_test_layer(mut commands: Commands) {
        let layer = commands.spawn_empty().id();
        commands.insert_resource(TestLayer(layer));
    }

    fn spawn_test_npc(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_single_zombie_npc(&mut commands, layer.0);
    }

    fn capture_attack_intents(
        mut events: EventReader<AttackIntent>,
        mut captured: valence::prelude::ResMut<CapturedAttackIntents>,
    ) {
        captured.0.extend(events.read().cloned());
    }

    #[test]
    fn spawn_npc_creates_single_zombie_with_expected_components() {
        let mut app = App::new();
        app.add_plugins(BigBrainPlugin::new(PreUpdate));
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
        );

        app.update();
        app.update();

        let npc_entities = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };

        assert_eq!(
            npc_entities.len(),
            1,
            "expected exactly one NPC marker entity"
        );

        let npc_entity = npc_entities[0];

        let kind = app
            .world()
            .get::<EntityKind>(npc_entity)
            .expect("NPC should have EntityKind component");
        assert_eq!(*kind, EntityKind::ZOMBIE);

        let position = app
            .world()
            .get::<Position>(npc_entity)
            .expect("NPC should have Position component");
        assert_eq!(position.get(), DVec3::new(14.0, 66.0, 14.0));

        let transform = app
            .world()
            .get::<Transform>(npc_entity)
            .expect("NPC should have Transform component");
        assert_eq!(transform.translation.x, 14.0);
        assert_eq!(transform.translation.y, 66.0);
        assert_eq!(transform.translation.z, 14.0);

        let _global_transform = app
            .world()
            .get::<GlobalTransform>(npc_entity)
            .expect("NPC should have GlobalTransform component");

        let blackboard = app
            .world()
            .get::<NpcBlackboard>(npc_entity)
            .expect("NPC should have NpcBlackboard component");
        assert_eq!(blackboard.nearest_player, None);
        assert!(
            blackboard.player_distance.is_infinite(),
            "NpcBlackboard.player_distance should default to infinity"
        );

        let archetype = app
            .world()
            .get::<NpcMeleeArchetype>(npc_entity)
            .expect("NPC should have NpcMeleeArchetype component");
        let loadout = app
            .world()
            .get::<NpcCombatLoadout>(npc_entity)
            .expect("NPC should have NpcCombatLoadout component");
        let profile = app
            .world()
            .get::<NpcMeleeProfile>(npc_entity)
            .expect("NPC should have NpcMeleeProfile component");
        let capabilities = app
            .world()
            .get::<MovementCapabilities>(npc_entity)
            .expect("NPC should have MovementCapabilities component");
        let _status_effects = app
            .world()
            .get::<StatusEffects>(npc_entity)
            .expect("NPC should include StatusEffects for shared combat resolver");
        assert_eq!(
            loadout.melee_archetype,
            NpcCombatLoadout::default().melee_archetype
        );
        assert_eq!(
            loadout.movement_capabilities.can_sprint,
            NpcCombatLoadout::default().movement_capabilities.can_sprint
        );
        assert_eq!(
            loadout.movement_capabilities.can_dash,
            NpcCombatLoadout::default().movement_capabilities.can_dash
        );
        assert_eq!(*archetype, NpcMeleeArchetype::Brawler);
        assert_eq!(*profile, NpcMeleeArchetype::Brawler.profile());
        assert_eq!(profile.wound_kind, WoundKind::Blunt);
        assert_eq!(
            capabilities.can_sprint,
            NpcCombatLoadout::default().movement_capabilities.can_sprint
        );
        assert_eq!(
            capabilities.can_dash,
            NpcCombatLoadout::default().movement_capabilities.can_dash
        );

        let patrol = app
            .world()
            .get::<NpcPatrol>(npc_entity)
            .expect("NPC should have a patrol component");
        assert_eq!(patrol.home_zone, DEFAULT_SPAWN_ZONE_NAME);
        assert_eq!(patrol.current_target, DVec3::new(14.0, 66.0, 14.0));

        let layer_id = app
            .world()
            .get::<EntityLayerId>(npc_entity)
            .expect("NPC should have EntityLayerId component");
        assert_ne!(
            layer_id.0,
            Entity::PLACEHOLDER,
            "NPC should be assigned to a non-placeholder layer"
        );

        let _thinker_builder = app
            .world()
            .get::<ThinkerBuilder>(npc_entity)
            .expect("NPC should have a Thinker builder attached at spawn time");

        let npc_archetype = app
            .world()
            .get::<NpcArchetype>(npc_entity)
            .expect("NPC should include shared NpcArchetype component");
        assert_eq!(*npc_archetype, NpcArchetype::Zombie);

        let lifespan = app
            .world()
            .get::<NpcLifespan>(npc_entity)
            .expect("NPC should include shared lifespan component");
        assert_eq!(lifespan.age_ticks, 0.0);
        assert!(lifespan.max_age_ticks > 0.0);

        let has_thinker = app
            .world()
            .get::<HasThinker>(npc_entity)
            .expect("BigBrain should attach HasThinker to NPC");

        let _thinker = app
            .world()
            .get::<Thinker>(has_thinker.entity())
            .expect("BigBrain thinker entity should contain Thinker component");
    }

    #[test]
    fn startup_spawned_npc_default_thinker_emits_attack_intent_in_melee_range() {
        let mut app = App::new();
        crate::npc::lifecycle::register(&mut app);
        brain::register(&mut app);
        app.insert_resource(CapturedAttackIntents::default());
        app.insert_resource(GameTick(120));
        app.add_event::<AttackIntent>();
        app.add_systems(Update, capture_attack_intents);
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_npc.after(setup_test_layer)),
        );

        let player = app
            .world_mut()
            .spawn((ClientMarker, Position::new([14.8, 66.0, 14.0])))
            .id();

        for _ in 0..5 {
            app.update();
        }

        let captured = &app.world().resource::<CapturedAttackIntents>().0;
        assert!(
            !captured.is_empty(),
            "default startup NPC thinker should emit AttackIntent when a player enters melee range"
        );
        assert_eq!(captured[0].target, Some(player));
        assert_eq!(captured[0].reach, FIST_REACH);
        assert_eq!(captured[0].wound_kind, WoundKind::Blunt);
    }

    #[test]
    fn spawn_commoner_npc_at_attaches_commoner_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_commoner.after(setup_test_layer),
            ),
        );

        app.update();
        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        let npc = npcs[0];

        let archetype = *app.world().get::<NpcArchetype>(npc).unwrap();
        assert_eq!(archetype, NpcArchetype::Commoner);

        let kind = *app.world().get::<EntityKind>(npc).unwrap();
        assert_eq!(kind, EntityKind::VILLAGER);

        let hunger = *app
            .world()
            .get::<crate::npc::hunger::Hunger>(npc)
            .expect("commoner should have Hunger");
        assert_eq!(hunger.value, 1.0);

        let wander = *app
            .world()
            .get::<crate::npc::brain::WanderState>(npc)
            .expect("commoner should have WanderState");
        assert!(wander.destination.is_none());

        let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
        assert_eq!(lifespan.age_ticks, 2.0);
        assert!(lifespan.max_age_ticks > 0.0);
    }

    fn spawn_test_commoner(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_commoner_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(20.0, 66.0, 20.0),
            DVec3::new(20.0, 66.0, 20.0),
            2.0,
        );
    }

    #[test]
    fn spawn_rogue_npc_at_attaches_rogue_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_rogue.after(setup_test_layer)),
        );

        app.update();
        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).collect::<Vec<_>>()
        };
        assert_eq!(npcs.len(), 1);
        let npc = npcs[0];

        assert_eq!(
            *app.world().get::<NpcArchetype>(npc).unwrap(),
            NpcArchetype::Rogue
        );

        assert!(
            app.world()
                .get::<crate::npc::brain::CultivateState>(npc)
                .is_some(),
            "rogue should carry CultivateState"
        );

        let lifespan = *app.world().get::<NpcLifespan>(npc).unwrap();
        assert_eq!(
            lifespan.max_age_ticks,
            NpcArchetype::Rogue.default_max_age_ticks()
        );
    }

    #[test]
    fn deferred_seed_brain_attaches_when_lod_wakes() {
        let mut app = App::new();
        app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Far,
                DeferredNpcBrain::ScatteredCultivator,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ThinkerBuilder>(npc).is_some());
        assert!(app.world().get::<DeferredNpcBrain>(npc).is_none());
    }

    #[test]
    fn deferred_seed_brain_stays_detached_while_dormant() {
        let mut app = App::new();
        app.add_systems(PreUpdate, attach_deferred_npc_brain_system);
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcLodTier::Dormant,
                DeferredNpcBrain::ScatteredCultivator,
            ))
            .id();

        app.update();

        assert!(app.world().get::<ThinkerBuilder>(npc).is_none());
        assert!(app.world().get::<DeferredNpcBrain>(npc).is_some());
    }

    #[test]
    fn spawn_scattered_cultivator_at_attaches_farming_brain_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_scattered_cultivator.after(setup_test_layer),
            ),
        );

        app.update();
        app.update();

        let npc = only_spawned_npc(&mut app);

        assert_eq!(
            *app.world().get::<NpcArchetype>(npc).unwrap(),
            NpcArchetype::Rogue
        );
        let scattered = app
            .world()
            .get::<ScatteredCultivator>(npc)
            .expect("scattered cultivator should mark seeded Rogue NPCs");
        assert_eq!(scattered.home_plot, None);
        assert_eq!(scattered.fail_streak, 0);
        assert!(matches!(
            scattered.temperament,
            FarmingTemperament::Patient
                | FarmingTemperament::Greedy
                | FarmingTemperament::Anxious
                | FarmingTemperament::Aggressive
        ));
        assert!(
            app.world().get::<ThinkerBuilder>(npc).is_some(),
            "scattered cultivator should carry a live farming thinker"
        );
        assert!(
            app.world()
                .get::<crate::npc::brain::CultivateState>(npc)
                .is_some(),
            "scattered cultivator remains a cultivating Rogue"
        );
    }

    #[test]
    fn rogue_commoner_visual_kind_uses_player_only_for_real_skin() {
        assert_eq!(
            fallback_rogue_commoner_kind(&None),
            EntityKind::VILLAGER,
            "None skin should produce villager (neutral NPC model)",
        );
        assert_eq!(
            fallback_rogue_commoner_kind(&Some(SignedSkin::fallback())),
            EntityKind::VILLAGER,
            "MineSkin fallback skin should produce villager, not witch (散修不该是女巫模型)",
        );
        assert_eq!(
            fallback_rogue_commoner_kind(&Some(SignedSkin {
                value: "value".into(),
                signature: "sig".into(),
                source: crate::skin::SkinSource::MineSkinRandom {
                    hash: "hash".into(),
                },
            })),
            EntityKind::PLAYER,
            "real MineSkin skin should produce player entity",
        );
    }

    fn spawn_test_rogue(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_rogue_npc_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(18.0, 66.0, 18.0),
            DVec3::new(18.0, 66.0, 18.0),
            0.0,
        );
    }

    fn spawn_test_scattered_cultivator(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_scattered_cultivator_at(
            &mut commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(19.0, 66.0, 19.0),
            DVec3::new(19.0, 66.0, 19.0),
            0.9,
            0.0,
        );
    }

    #[test]
    fn spawn_beast_npc_at_attaches_live_territory_brain_components() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (setup_test_layer, spawn_test_beast.after(setup_test_layer)),
        );
        app.update();
        app.update();

        let beast = only_spawned_npc(&mut app);

        assert!(app.world().get::<TerritoryPatrolState>(beast).is_some());
        assert!(app.world().get::<HuntState>(beast).is_some());
        assert!(app.world().get::<ProtectYoungState>(beast).is_some());
        assert!(app
            .world()
            .get::<crate::fauna::components::FaunaTag>(beast)
            .is_some());
        let tag = app
            .world()
            .get::<crate::fauna::components::FaunaTag>(beast)
            .expect("beast should carry fauna tag");
        assert_eq!(
            app.world().get::<EntityKind>(beast),
            Some(&crate::fauna::visual::entity_kind_for_beast(tag.beast_kind)),
            "beast should spawn with a fauna custom visual entity kind"
        );
        assert_eq!(
            app.world()
                .get::<crate::fauna::visual::FaunaVisualKind>(beast)
                .copied(),
            crate::fauna::visual::visual_kind_for_beast(tag.beast_kind)
        );
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(beast)
            .expect("beast should carry the live territory thinker");
    }

    #[test]
    fn spawn_disciple_npc_at_attaches_mission_and_social_state() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_disciple.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let disciple = only_spawned_npc(&mut app);

        assert!(app.world().get::<MissionExecuteState>(disciple).is_some());
        assert!(app.world().get::<SocializeState>(disciple).is_some());
        assert!(app.world().get::<FactionMembership>(disciple).is_some());
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(disciple)
            .expect("disciple should carry the live faction/social thinker");
    }

    #[test]
    fn spawn_relic_guard_npc_at_attaches_guardian_trial_state() {
        let mut app = App::new();
        app.add_systems(
            valence::prelude::Startup,
            (
                setup_test_layer,
                spawn_test_relic_guard.after(setup_test_layer),
            ),
        );
        app.update();
        app.update();

        let guard = only_spawned_npc(&mut app);

        assert!(app.world().get::<GuardState>(guard).is_some());
        assert!(app.world().get::<TrialState>(guard).is_some());
        assert!(app.world().get::<GuardianDuty>(guard).is_some());
        assert!(app.world().get::<TrialEval>(guard).is_some());
        let _thinker = app
            .world()
            .get::<ThinkerBuilder>(guard)
            .expect("relic guard should carry the live guardian thinker");
    }

    fn only_spawned_npc(app: &mut App) -> Entity {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
        let npcs = query.iter(world).collect::<Vec<_>>();
        assert_eq!(npcs.len(), 1);
        npcs[0]
    }

    fn spawn_test_beast(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_beast_npc_at(
            &mut commands,
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(40.0, 66.0, 40.0),
            Territory::new(DVec3::new(40.0, 66.0, 40.0), 30.0),
            0.0,
        );
    }

    fn spawn_test_disciple(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_disciple_npc_at(
            &mut commands,
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(42.0, 66.0, 42.0),
            DVec3::new(42.0, 66.0, 42.0),
            FactionId::Attack,
            FactionRank::Disciple,
            None,
            0.0,
        );
    }

    fn spawn_test_relic_guard(mut commands: Commands, layer: Res<TestLayer>) {
        spawn_relic_guard_npc_at(
            &mut commands,
            layer.0,
            DEFAULT_SPAWN_ZONE_NAME,
            DVec3::new(44.0, 66.0, 44.0),
            24.0,
            "relic:test",
            "trial:test",
        );
    }

    // -----------------------------------------------------------------------
    // Rogue population seed — pure-function tests + full-stack spawn smoke
    // -----------------------------------------------------------------------

    fn mk_zone(name: &str, spirit_qi: f64, center: [f64; 3]) -> Zone {
        Zone {
            name: name.to_string(),
            dimension: crate::world::dimension::DimensionKind::Overworld,
            bounds: (
                DVec3::new(center[0] - 200.0, -64.0, center[2] - 200.0),
                DVec3::new(center[0] + 200.0, 320.0, center[2] + 200.0),
            ),
            spirit_qi,
            danger_level: 1,
            active_events: Vec::new(),
            patrol_anchors: vec![DVec3::new(center[0], center[1], center[2])],
            blocked_tiles: Vec::new(),
        }
    }

    #[test]
    fn classify_zones_by_qi_partitions_at_threshold() {
        let zones = vec![
            mk_zone("high", 0.7, [0.0, 66.0, 0.0]),
            mk_zone("mid", 0.4, [10.0, 66.0, 0.0]),
            mk_zone("low", 0.1, [20.0, 66.0, 0.0]),
        ];
        let (resource, other) = classify_zones_by_qi(&zones, 0.4);
        assert_eq!(resource.len(), 2, "0.7 and 0.4 should be >= 0.4");
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].name, "low");
    }

    #[test]
    fn distribute_counts_evenly_spreads_remainder_to_first_buckets() {
        assert_eq!(distribute_counts_evenly(20, 3), vec![7, 7, 6]);
        assert_eq!(distribute_counts_evenly(80, 3), vec![27, 27, 26]);
        assert_eq!(distribute_counts_evenly(10, 10), vec![1; 10]);
        assert_eq!(distribute_counts_evenly(0, 3), vec![0, 0, 0]);
        assert_eq!(distribute_counts_evenly(5, 0), Vec::<u32>::new());
    }

    #[test]
    fn seed_position_clamps_to_zone_bounds() {
        let zone = mk_zone("z", 0.5, [0.0, 66.0, 0.0]);
        for idx in 0..64u32 {
            let (pos, _) = seed_position_for_zone(&zone, idx);
            assert!(
                zone.contains(pos),
                "idx {idx} produced out-of-bound pos {pos:?}"
            );
        }
    }

    #[test]
    fn initial_age_spreads_across_10_buckets() {
        let max_age = 100_000.0;
        let ages: Vec<f64> = (0..20)
            .map(|i| initial_age_for_index(i, max_age, 0.8))
            .collect();
        // Two full cycles of 10 buckets each.
        assert_eq!(ages[0], 0.0);
        assert!(ages[9] > 0.0);
        assert_eq!(ages[0], ages[10], "bucket should repeat at index 10");
        let max_age_produced = ages.iter().cloned().fold(0.0_f64, f64::max);
        assert!(max_age_produced <= max_age * 0.8 + 1e-9);
    }

    #[test]
    fn seed_splits_100_rogues_80_20_across_zones() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);

        let mut zones = ZoneRegistry::fallback();
        // fallback gives us "spawn" @ qi=0.3; override to match prod-style mix.
        zones.zones[0].spirit_qi = 0.3;
        zones
            .zones
            .push(mk_zone("resource_a", 0.7, [1000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("resource_b", 0.5, [2000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("resource_c", 0.4, [3000.0, 70.0, 0.0]));
        zones
            .zones
            .push(mk_zone("other_a", 0.2, [0.0, 70.0, 5000.0]));
        app.insert_resource(zones);
        app.insert_resource(NpcRegistry::default());
        app.insert_resource(RoguePopulationSeedConfig::default());
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, seed_initial_rogue_population_on_startup);

        for _ in 0..(100 / ROGUE_SEED_BATCH_SIZE) {
            app.update();
        }

        let by_archetype = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query.iter(world).copied().collect::<Vec<_>>()
        };
        assert_eq!(
            by_archetype
                .iter()
                .filter(|a| **a == NpcArchetype::Rogue)
                .count(),
            100
        );

        // Sanity: 80% resource / 20% other — count by home_zone.
        let zone_counts: HashMap<String, u32> = {
            let world = app.world_mut();
            let mut counts: HashMap<String, u32> = HashMap::new();
            let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
            for patrol in query.iter(world) {
                *counts.entry(patrol.home_zone.clone()).or_insert(0) += 1;
            }
            counts
        };
        let resource_total: u32 = ["resource_a", "resource_b", "resource_c"]
            .iter()
            .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
            .sum();
        let other_total: u32 = ["spawn", "other_a"]
            .iter()
            .map(|n| zone_counts.get(*n).copied().unwrap_or(0))
            .sum();
        assert_eq!(resource_total, 80, "80% should land in resource zones");
        assert_eq!(other_total, 20, "20% should land in other zones");

        // Registry 已扣 100 配额。
        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 100);
    }

    #[test]
    fn seed_respects_disabled_config() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.insert_resource(ZoneRegistry::fallback());
        app.insert_resource(NpcRegistry::default());
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 0,
            ..RoguePopulationSeedConfig::default()
        });
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, seed_initial_rogue_population_on_startup);

        app.update();

        let count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(count, 0);
    }

    #[test]
    fn seed_falls_back_to_other_zones_when_no_resource_qualifies() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        let mut zones = ZoneRegistry::fallback();
        zones.zones[0].spirit_qi = 0.1; // 强制 < 0.4 门槛，使其归入 "other"
        app.insert_resource(zones);
        app.insert_resource(NpcRegistry::default());
        app.insert_resource(RoguePopulationSeedConfig {
            target_count: 10,
            ..RoguePopulationSeedConfig::default()
        });
        app.add_event::<NpcSpawnNotice>();
        app.add_systems(Update, seed_initial_rogue_population_on_startup);

        app.update();

        let count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(count, 10, "all 10 rogues should land in fallback zone");
        let home_zone = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcPatrol, With<NpcMarker>>();
            query.iter(world).next().unwrap().home_zone.clone()
        };
        assert_eq!(home_zone, DEFAULT_SPAWN_ZONE_NAME);
    }

    #[test]
    fn reproduction_processor_spawns_commoner_from_event_and_decrements_registry() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Commoner,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npcs = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcArchetype, With<NpcMarker>>();
            query.iter(world).copied().collect::<Vec<_>>()
        };
        assert_eq!(npcs, vec![NpcArchetype::Commoner]);

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(
            registry.live_npc_count, 1,
            "reproduction must reserve one spawn slot from NpcRegistry"
        );
    }

    #[test]
    fn reproduction_processor_dispatches_beast_request_with_territory_hint() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        let center = DVec3::new(50.0, 66.0, 50.0);
        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Beast,
            position: center,
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: Some(center),
            territory_radius: Some(30.0),
        });

        app.update();

        let (arch, has_territory) = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<(&NpcArchetype, Option<&Territory>), With<NpcMarker>>();
            let (arch, territory) = query.iter(world).next().expect("spawned beast");
            (*arch, territory.is_some())
        };
        assert_eq!(arch, NpcArchetype::Beast);
        assert!(has_territory, "beast reproduction must attach Territory");

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 1);
    }

    #[test]
    fn reproduction_processor_skips_beast_request_without_territory_hint() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Beast,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0, "beast without territory hint must not spawn");

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(
            registry.live_npc_count, 0,
            "budget must not be reserved on rejected beast request"
        );
    }

    #[test]
    fn reproduction_processor_skips_unsupported_archetype() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        app.insert_resource(NpcRegistry::default());
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Zombie,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0);

        let registry = app.world().resource::<NpcRegistry>();
        assert_eq!(registry.live_npc_count, 0);
    }

    #[test]
    fn reproduction_processor_rejects_when_registry_budget_exhausted() {
        let scenario = valence::testing::ScenarioSingleClient::new();
        let mut app = scenario.app;
        app.add_event::<NpcReproductionRequest>();
        app.add_event::<NpcSpawnNotice>();
        let mut registry = NpcRegistry::default();
        registry.live_npc_count = registry.max_npc_count;
        registry.spawn_paused = true;
        app.insert_resource(registry);
        app.add_systems(Update, process_npc_reproduction_requests);

        app.update();

        app.world_mut().send_event(NpcReproductionRequest {
            archetype: NpcArchetype::Commoner,
            position: DVec3::new(30.0, 66.0, 30.0),
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            initial_age_ticks: 0.0,
            territory_center: None,
            territory_radius: None,
        });

        app.update();

        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(npc_count, 0);
    }

    // -- Bug #2: snap_spawn_y_to_surface regression tests ------------------

    #[test]
    fn snap_spawn_y_above_ground_snaps_down() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct FlatGround;
        impl SurfaceProvider for FlatGround {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: true,
                }
            }
        }
        let terrain = FlatGround;
        let pos = DVec3::new(10.5, 200.0, 20.5);
        let snapped = snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 67.0).abs() < 0.01,
            "spawn at Y=200 should snap to surface_y+1=67, got {}",
            snapped.y,
        );
        assert!(
            (snapped.x - pos.x).abs() < 0.01 && (snapped.z - pos.z).abs() < 0.01,
            "XZ should be unchanged",
        );
    }

    #[test]
    fn snap_spawn_y_below_ground_snaps_up() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct FlatGround;
        impl SurfaceProvider for FlatGround {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: true,
                }
            }
        }
        let terrain = FlatGround;
        let pos = DVec3::new(5.0, 10.0, 5.0);
        let snapped = snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 67.0).abs() < 0.01,
            "spawn at Y=10 should snap to surface_y+1=67, got {}",
            snapped.y,
        );
    }

    #[test]
    fn snap_spawn_y_impassable_surface_keeps_original() {
        use crate::world::terrain::{SurfaceInfo, SurfaceProvider};
        struct LavaSurface;
        impl SurfaceProvider for LavaSurface {
            fn query_surface(&self, _x: i32, _z: i32) -> SurfaceInfo {
                SurfaceInfo {
                    y: 66,
                    passable: false,
                }
            }
        }
        let terrain = LavaSurface;
        let pos = DVec3::new(5.0, 80.0, 5.0);
        let snapped = snap_spawn_y_to_surface(pos, Some(&terrain));
        assert!(
            (snapped.y - 80.0).abs() < 0.01,
            "impassable surface should keep original Y=80, got {}",
            snapped.y,
        );
    }

    #[test]
    fn snap_spawn_y_no_terrain_keeps_original() {
        let pos = DVec3::new(5.0, 80.0, 5.0);
        let snapped = snap_spawn_y_to_surface(pos, None::<&crate::world::terrain::TerrainProvider>);
        assert!(
            (snapped.y - 80.0).abs() < 0.01,
            "no terrain provider should keep original Y=80, got {}",
            snapped.y,
        );
    }
}
