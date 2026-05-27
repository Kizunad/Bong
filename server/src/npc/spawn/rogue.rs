use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::prelude::{
    bevy_ecs, Bundle, Commands, DVec3, Entity, EventWriter, Res, ResMut, Resource,
};

use crate::cultivation::components::Realm;
use crate::npc::brain::{
    AgeingScorer, ChaseAction, ChaseTargetScorer, CultivateAction, CultivateState,
    CultivationDriveHistory, CultivationDriveScorer, CuriosityScorer, FleeAction, GoToPoiAction,
    MeleeAttackAction, MeleeRangeScorer, NpcDefenseAction, NpcDefenseScorer, PlayerProximityScorer,
    ReturnHomeAction, ReturnHomeScorer, SeclusionAction, SeclusionScorer, StallAction,
    StartDuXuAction, TradeStallScorer, TribulationReadyScorer, WanderScorer, WanderState,
};
use crate::npc::farming_brain::{
    HarvestAction, LingtianFarmingScorer, MigrateAction, PlantAction, ReplenishAction, TillAction,
};
use crate::npc::lifecycle::{
    npc_runtime_bundle_with_age, NpcArchetype, NpcRegistry, NpcSpawnNotice, NpcSpawnSource,
};
use crate::npc::scattered_cultivator::{FarmingTemperament, ScatteredCultivator};
use crate::npc::technique::{
    assign_npc_techniques, NpcHealAction, NpcHealScorer, NpcLastTechniqueTick, NpcTechniqueAction,
    NpcTechniqueScorer,
};
use crate::npc::trade::{assign_npc_trade_inventory, NpcPlayerReputation};
use crate::skin::{initial_age_ratio, select_npc_visual_profile, NpcSkinFallbackPolicy, SkinPool};
use crate::world::mob_spawn::{MobSpawnFilter, NaturalMobKind};
use crate::world::zone::{Zone, ZoneRegistry};

use super::common::{
    attach_player_skin, draw_npc_skin, skin_salt, spawn_notice, spawn_rogue_commoner_base,
    DeferredNpcBrain, NpcCombatLoadout, NpcSkinSpawnContext,
};
use super::PoissonSpawnSampler;

// ---------------------------------------------------------------------------
// Rogue population seed config + progress
// ---------------------------------------------------------------------------

const ROGUE_SEED_BATCH_SIZE: u32 = 5;

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
            .unwrap_or(20);
        Self {
            target_count,
            resource_fraction: 0.8,
            resource_spirit_qi_threshold: 0.4,
            max_initial_age_ratio: 0.8,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RogueSeedJob {
    pub(crate) zone: Zone,
    pub(crate) count: u32,
}

#[derive(Debug, Default)]
pub(crate) struct RogueSeedProgress {
    pub(crate) initialized: bool,
    pub(crate) done: bool,
    pub(crate) jobs: Vec<RogueSeedJob>,
    pub(crate) job_index: usize,
    pub(crate) spawned_in_job: u32,
    pub(crate) spawned_total: u32,
    pub(crate) resource_zone_count: usize,
    pub(crate) resource_reserved: u32,
    pub(crate) other_zone_count: usize,
    pub(crate) other_reserved: u32,
    /// plan-npc-overhaul-v1 §P1.3 — 各 zone 已 spawn 位置，用于 Poisson 采样。
    pub(crate) spawned_positions_by_zone:
        std::collections::HashMap<String, Vec<(DVec3, NpcArchetype)>>,
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

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

pub(crate) fn reserve_zone_distribution(
    mut registry: Option<&mut NpcRegistry>,
    zones: &[&Zone],
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

// ---------------------------------------------------------------------------
// Thinkers
// ---------------------------------------------------------------------------

pub(crate) fn rogue_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(SeclusionScorer, SeclusionAction)
        .when(TribulationReadyScorer, StartDuXuAction)
        .when(NpcHealScorer, NpcHealAction)
        .when(NpcTechniqueScorer, NpcTechniqueAction)
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(NpcDefenseScorer, NpcDefenseAction::default())
        .when(ChaseTargetScorer, ChaseAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(TradeStallScorer, StallAction)
        .when(ReturnHomeScorer, ReturnHomeAction)
        .when(CuriosityScorer, GoToPoiAction::default())
        .when(WanderScorer, GoToPoiAction::default())
}

pub(crate) fn scattered_cultivator_thinker() -> ThinkerBuilder {
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
        .when(NpcDefenseScorer, NpcDefenseAction::default())
        .when(MeleeRangeScorer, MeleeAttackAction)
        .when(ChaseTargetScorer, ChaseAction)
        .when(PlayerProximityScorer, FleeAction)
        .when(CultivationDriveScorer, CultivateAction)
        .when(TradeStallScorer, StallAction)
        .when(ReturnHomeScorer, ReturnHomeAction)
        .when(CuriosityScorer, GoToPoiAction::default())
        .when(WanderScorer, GoToPoiAction::default())
}

// ---------------------------------------------------------------------------
// ScatteredCultivatorBundle
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Spawn functions
// ---------------------------------------------------------------------------

/// Spawn a Rogue (散修) NPC. MineSkin 池可用时走假玩家 skin；否则退回 vanilla villager。
/// `initial_age_ticks` 允许 agent 投放"已修炼多年"的散修。
#[allow(clippy::too_many_arguments)]
pub fn spawn_rogue_npc_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    realm: Realm,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let profile = select_npc_visual_profile(
        NpcArchetype::Rogue,
        realm,
        None,
        None,
        initial_age_ratio(NpcArchetype::Rogue, initial_age_ticks),
    );
    let skin = draw_npc_skin(skin_context, profile, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        profile,
        loadout.clone(),
        NpcArchetype::Rogue,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin {
        attach_player_skin(commands, entity, NpcArchetype::Rogue, skin);
    }

    commands.entity(entity).insert((
        WanderState::default(),
        CultivateState::default(),
        CultivationDriveHistory::default(),
        rogue_npc_thinker(),
    ));

    // P1: NPC 功法 + 交易库存
    let meridian_sys = crate::npc::technique::npc_meridian_system_for_realm(realm);
    let empty_deps = crate::cultivation::meridian::severed::SkillMeridianDependencies::default();
    let known_techniques = assign_npc_techniques(
        NpcArchetype::Rogue,
        realm,
        &meridian_sys,
        &empty_deps,
        None,
        entity.index() as u64,
    );
    let trade_inv = assign_npc_trade_inventory(NpcArchetype::Rogue, realm, entity.index() as u64);
    commands.entity(entity).insert((
        known_techniques,
        NpcLastTechniqueTick::default(),
        trade_inv,
        NpcPlayerReputation::default(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Rogue, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
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
    realm: Realm,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let profile = select_npc_visual_profile(
        NpcArchetype::Rogue,
        realm,
        None,
        None,
        initial_age_ratio(NpcArchetype::Rogue, initial_age_ticks),
    );
    let skin = draw_npc_skin(skin_context, profile, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        profile,
        loadout.clone(),
        NpcArchetype::Rogue,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin {
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

// ---------------------------------------------------------------------------
// Seed system
// ---------------------------------------------------------------------------

use crate::npc::brain::RetireAction;

#[allow(clippy::too_many_arguments)]
pub(crate) fn seed_initial_rogue_population_on_startup(
    mut commands: Commands,
    mut notices: EventWriter<NpcSpawnNotice>,
    config: Option<Res<RoguePopulationSeedConfig>>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut registry: Option<ResMut<NpcRegistry>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    layers: valence::prelude::Query<
        Entity,
        valence::prelude::With<crate::world::dimension::OverworldLayer>,
    >,
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

        // Extract zone data before mutable borrow of progress.
        let zone_name = job.zone.name.clone();
        let zone_bounds = job.zone.bounds;
        let zone_spirit_qi = job.zone.spirit_qi;
        let global_index = progress.spawned_total;

        // plan-npc-overhaul-v1 §P1.3 — 使用 PoissonSpawnSampler 替代 anchor+jitter。
        let sampler = PoissonSpawnSampler::adaptive_for_zone(zone_bounds);
        let rng_seed = (global_index as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xbf58_476d_1ce4_e5b9);
        // Snapshot existing positions to avoid borrow conflict.
        let existing: Vec<(DVec3, NpcArchetype)> = progress
            .spawned_positions_by_zone
            .get(&zone_name)
            .cloned()
            .unwrap_or_default();
        let sampled =
            sampler.sample_position(zone_bounds, &existing, NpcArchetype::Rogue, rng_seed);
        let pos = match sampled {
            Some(p) => {
                let zone_min = zone_bounds.0;
                let zone_max = zone_bounds.1;
                DVec3::new(
                    p.x.clamp(zone_min.x, zone_max.x),
                    p.y.clamp(zone_min.y, zone_max.y),
                    p.z.clamp(zone_min.z, zone_max.z),
                )
            }
            None => {
                // Zone saturated by Poisson — skip this job instead of
                // falling back to jitter (which causes NPC clustering).
                tracing::warn!(
                    "[bong][npc] zone {} saturated by Poisson (spawned_in_job={}), skipping remaining",
                    zone_name,
                    progress.spawned_in_job,
                );
                progress.job_index += 1;
                progress.spawned_in_job = 0;
                continue;
            }
        };
        let patrol_center = DVec3::new(
            (zone_bounds.0.x + zone_bounds.1.x) * 0.5,
            (zone_bounds.0.y + zone_bounds.1.y) * 0.5,
            (zone_bounds.0.z + zone_bounds.1.z) * 0.5,
        );

        let age = initial_age_for_index(global_index, max_age, cfg.max_initial_age_ratio);
        let entity = spawn_scattered_cultivator_at(
            &mut commands,
            NpcSkinSpawnContext::new(skin_pool.as_deref_mut(), skin_policy),
            layer,
            &zone_name,
            pos,
            patrol_center,
            zone_spirit_qi,
            Realm::Awaken,
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
            &zone_name,
            pos,
            age,
        ));

        // Track for future Poisson samples.
        progress
            .spawned_positions_by_zone
            .entry(zone_name)
            .or_default()
            .push((pos, NpcArchetype::Rogue));

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
