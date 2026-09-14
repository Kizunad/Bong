//! NPC hydrate/dehydrate bridge.
//!
//! This module moves live NPCs into [`NpcDormantStore`] when they are far away
//! from all players, and spawns them back when someone comes near again.

use std::collections::{BTreeMap, HashSet};

use valence::client::ClientMarker;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, App, Commands, DVec3, Despawned, Entity, EventReader, EventWriter, IntoSystemConfigs,
    Position, Query, Res, ResMut, Update, With, Without,
};

use crate::combat::components::Lifecycle;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{DeathRegistry, LifespanComponent, LifespanExtensionLedger};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::tribulation::{
    du_xu_prereqs_met, HalfStepRechallengeTriggerEvent, InitiateXuhuaTribulation,
};
#[cfg(test)]
use crate::fauna::daozhan::FakeBehavior;
use crate::fauna::daozhan::{DaoZhangBehaviorBlackboard, DaoZhangState};
use crate::fauna::dying_elder::DyingElderBlackboard;
use crate::fauna::mimic_spider::{MimicSpiderBlackboard, SpiderDisguiseState, SpiderTrapPotential};
use crate::npc::brain::NPC_TRIBULATION_WAVES_DEFAULT;
use crate::npc::dormant::{
    durable_npc_identity_error, dvec3_from_array, planar_distance, vec3_to_array,
    DormantBehaviorIntent, DormantDaoxiangOriginSnapshot, DormantDaozhanSnapshot,
    DormantFuyaAuraSnapshot, DormantGuardianRelicSnapshot, DormantMimicSpiderSnapshot,
    DormantPatrolSnapshot, DormantTsyHostileSnapshot, DormantTsySentinelSnapshot,
    DormantZhinianPhase, NpcDormantSnapshot, NpcDormantStore, NpcVirtualizationConfig,
};
use crate::npc::faction::{FactionMembership, FactionRank};
use crate::npc::interaction_memory::NpcMemoryComponent;
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan, NpcRegistry};
use crate::npc::lod::NpcLodTier;
use crate::npc::loot::{default_loot_for_archetype, NpcLootTable};
use crate::npc::movement::GameTick;
use crate::npc::patrol::NpcPatrol;
use crate::npc::relic::{GuardianDuty, TrialEval};
use crate::npc::scenario::ScenarioNpc;
use crate::npc::schedule::{
    home_base_for_archetype, hydrate_position_for, schedule_seed_from_char_id, NpcDailySchedule,
};
use crate::npc::spawn::{
    spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at, spawn_mundane_fauna_at,
    spawn_relic_guard_npc_at, spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker,
    NpcSkinSpawnContext,
};
use crate::npc::spawn_rat::RatBlackboard;
use crate::npc::spawn_spider::spawn_ash_spider_npc_at;
use crate::npc::territory::Territory;
use crate::npc::trade::NpcPlayerReputation;
use crate::npc::tsy_hostile::{
    spawn_tsy_daoxiang_at, spawn_tsy_fuya_at, spawn_tsy_sentinel_at, spawn_tsy_skull_fiend_at,
    spawn_tsy_zhinian_at, FuyaAura, TsyHostileMarker, TsySentinelMarker, ZhinianMind, ZhinianPhase,
};
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::era::WorldEraState;
use crate::world::poi_novice::PoiNoviceRegistry;
use crate::world::tsy_container::LootContainer;
use crate::world::tsy_lifecycle::DaoxiangOrigin;
use crate::world::zone::ZoneRegistry;

const DORMANT_TRIBULATION_MIN_QI_RATIO: f64 = 0.8;
type PlayerPosition = (DimensionKind, valence::prelude::DVec3);

#[derive(SystemParam)]
pub struct DormantExtraComponentQueries<'w, 's> {
    guardian_duties: Query<'w, 's, Option<&'static GuardianDuty>, With<NpcMarker>>,
    trial_evals: Query<'w, 's, Option<&'static TrialEval>, With<NpcMarker>>,
    tsy_markers: Query<'w, 's, Option<&'static TsyHostileMarker>, With<NpcMarker>>,
    zhinian_minds: Query<'w, 's, Option<&'static ZhinianMind>, With<NpcMarker>>,
    fuya_auras: Query<'w, 's, Option<&'static FuyaAura>, With<NpcMarker>>,
    daoxiang_origins: Query<'w, 's, Option<&'static DaoxiangOrigin>, With<NpcMarker>>,
    daozhan_states: Query<'w, 's, Option<&'static DaoZhangState>, With<NpcMarker>>,
    daozhan_blackboards:
        Query<'w, 's, Option<&'static DaoZhangBehaviorBlackboard>, With<NpcMarker>>,
    rat_blackboards: Query<'w, 's, Option<&'static RatBlackboard>, With<NpcMarker>>,
    spider_states: Query<'w, 's, Option<&'static SpiderDisguiseState>, With<NpcMarker>>,
    spider_blackboards: Query<'w, 's, Option<&'static MimicSpiderBlackboard>, With<NpcMarker>>,
    spider_traps: Query<'w, 's, Option<&'static SpiderTrapPotential>, With<NpcMarker>>,
    /// plan-tsy-sentinel-dormant-regression-v1 §P1：TSY 秘境守灵身份 marker（dehydrate 侧读取）。
    tsy_sentinel_markers: Query<'w, 's, Option<&'static TsySentinelMarker>, With<NpcMarker>>,
    /// dehydrate 侧 `guarding_container: Option<Entity>` 是精确已知的单个 `Entity`，
    /// `.get(entity)` 直接拿 `Position` 写快照即可——此处不存在多容器歧义，无需过滤
    /// family_id（family_id 过滤只在 P2 hydrate 反查阶段才需要，见 `resolve_sentinel_guarding_container`）。
    containers: Query<'w, 's, &'static Position, (With<LootContainer>, Without<NpcMarker>)>,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering hydrate/dehydrate bridge");
    app.add_systems(
        Update,
        (
            hydrate_dormant_near_players_system,
            hydrate_dormant_on_rechallenge_trigger,
            dehydrate_far_npcs_system,
        )
            .chain(),
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn hydrate_dormant_near_players_system(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    mut store: ResMut<NpcDormantStore>,
    mut commands: Commands,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
    dimension_layers: Option<Res<DimensionLayers>>,
    players: Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
    registry: Option<Res<NpcRegistry>>,
    pois: Option<Res<PoiNoviceRegistry>>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut tribulations: EventWriter<InitiateXuhuaTribulation>,
    zone_registry: Option<Res<ZoneRegistry>>,
    world_era: Option<Res<WorldEraState>>,
    // plan-tsy-sentinel-dormant-regression-v1 §P2：现算 relic_containers，供
    // `spawn_from_snapshot` 里 TSY 秘境守灵的两段式 family+坐标重绑使用。
    relic_containers_query: Query<(Entity, &Position, &LootContainer), With<LootContainer>>,
) {
    let tick = crate::npc::dormant::current_tick(game_tick.as_deref());
    if !crate::npc::dormant::should_run_interval(tick, config.transition_interval_ticks) {
        return;
    }

    // P1 era 注入：读取渡劫阈值系数（Resource 不存在时退回基准 1.0）。
    let tribulation_threshold_mul = world_era
        .as_deref()
        .map(|e| e.current_modifiers().tribulation_threshold_mul)
        .unwrap_or(1.0);

    let player_positions = players
        .iter()
        .map(|(pos, dimension)| (dimension_kind(dimension), pos.get()))
        .collect::<Vec<_>>();
    let Some(dimension_layers) = dimension_layers.as_deref() else {
        return;
    };

    let player_zones = player_zone_names(zone_registry.as_deref(), &player_positions);
    let relic_containers = collect_relic_containers(&relic_containers_query);

    let mut to_hydrate = BTreeMap::<String, bool>::new();
    for (char_id, snapshot) in &store.snapshots {
        // 守卫：已标记逻辑战死（combat_dead_pending_release=true）的快照不得被水化。
        // run_pending_combat_release_retry 持有这些快照的收口权——重试 release qi 到 zone、
        // 完成后由 finalize_released_combat_death 从 store 移除。若在此水化，会把已向
        // zone ledger 转账过的 qi 再度作为活 NPC 真元注入世界，造成双计（守恒红线）。
        if snapshot.combat_dead_pending_release {
            continue;
        }
        let tribulation_ready = dormant_tribulation_ready(snapshot, tribulation_threshold_mul);
        let near_player = nearest_same_dimension_player_distance(
            snapshot.position_vec(),
            snapshot.dimension,
            &player_positions,
        ) <= config.hydrate_radius_blocks;
        let in_player_zone = player_zones.contains(snapshot.zone_name.as_str());
        if tribulation_ready || near_player || in_player_zone {
            to_hydrate.insert(char_id.clone(), tribulation_ready);
        }
    }

    let live_count = registry
        .as_deref()
        .map(|registry| registry.live_npc_count)
        .unwrap_or_default();
    let mut normal_slots = config.max_hydrated_count.saturating_sub(live_count);

    for (char_id, force_tribulation) in to_hydrate {
        if !force_tribulation && normal_slots == 0 {
            continue;
        }
        if let Some(error) = store
            .snapshots
            .get(&char_id)
            .and_then(NpcDormantSnapshot::durable_identity_error)
        {
            tracing::warn!(
                character_id = %char_id,
                "[bong][npc] refusing to hydrate dormant NPC with divergent durable identity: {error}"
            );
            continue;
        }

        let Some(snapshot) = store.remove(&char_id) else {
            continue;
        };
        let entity = spawn_from_snapshot(
            &mut commands,
            &technique_registry,
            snapshot,
            dimension_layers,
            tick,
            pois.as_deref(),
            skin_pool.as_deref_mut(),
            &relic_containers,
        );
        if force_tribulation {
            tribulations.send(InitiateXuhuaTribulation {
                entity,
                waves_total: NPC_TRIBULATION_WAVES_DEFAULT,
                started_tick: tick,
            });
        } else {
            normal_slots = normal_slots.saturating_sub(1);
        }
        tracing::debug!("[bong][npc] hydrated dormant NPC into entity {entity:?}");
    }
}

/// plan-halfstep-rechallenge-integration-v1 P2：dormant HalfStep NPC 收到重渡触发时强制 hydrate。
///
/// 当 `dispatch_rechallenge_on_quota_opened_system` emit `HalfStepRechallengeTriggerEvent`
/// 且 `is_dormant==true` 时，本系统从 `NpcDormantStore` 按 `char_id` 移除快照，调用
/// `spawn_from_snapshot` 在世界中创建 NPC entity，并立即发送 `InitiateXuhuaTribulation`
/// 使其进入渡劫流程。
///
/// **设计决议**（§8 #2）：`HalfStepRechallengeTriggerEvent` 是 Bevy ECS Event，dispatch
/// system（cultivation）与本 hydrate system（npc）注册在同一 `App::new()` 实例，
/// 故直接通过 ECS event 通信，无需 Redis 回环。
///
/// **不响应 `is_dormant==false` 的事件**——那些是 hydrated 玩家/NPC，已在世界中。
///
/// **store 中无该 char_id 时安全跳过**——entry 可能已被
/// `hydrate_dormant_near_players_system` 抢先 hydrate（player 邻近触发），或被过期清理。
#[allow(clippy::too_many_arguments)]
pub fn hydrate_dormant_on_rechallenge_trigger(
    mut events: EventReader<HalfStepRechallengeTriggerEvent>,
    mut store: ResMut<NpcDormantStore>,
    mut commands: Commands,
    technique_registry: Res<crate::cultivation::known_techniques::TechniqueRegistry>,
    dimension_layers: Option<Res<DimensionLayers>>,
    game_tick: Option<Res<GameTick>>,
    pois: Option<Res<PoiNoviceRegistry>>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    mut tribulations: EventWriter<InitiateXuhuaTribulation>,
    // plan-tsy-sentinel-dormant-regression-v1 §P2：同 `hydrate_dormant_near_players_system`，
    // 现算 relic_containers 供 TSY 秘境守灵重绑使用。
    relic_containers_query: Query<(Entity, &Position, &LootContainer), With<LootContainer>>,
) {
    let tick = crate::npc::dormant::current_tick(game_tick.as_deref());
    let Some(dimension_layers) = dimension_layers.as_deref() else {
        // DimensionLayers 未初始化（常见于单元测试不注册 layer 的情况）——先收集事件避免
        // EventReader 积压，然后跳过。
        for event in events.read() {
            if event.is_dormant {
                tracing::warn!(
                    "[bong][npc] hydrate_dormant_on_rechallenge_trigger: DimensionLayers not ready, \
                     skipping dormant hydrate for char_id={}", event.char_id
                );
            }
        }
        return;
    };
    let relic_containers = collect_relic_containers(&relic_containers_query);

    for event in events.read() {
        if !event.is_dormant {
            // hydrated entity（玩家或已在世界中的 NPC），不需 hydrate，跳过
            continue;
        }

        // 守卫：combat_dead_pending_release=true 的快照不得被 rechallenge 触发水化。
        // 先 get 检查，通过后再 remove——确保快照留在 store 供 run_pending_combat_release_retry
        // 正常收口，不破坏独立 retry 系统的处理链。
        if store
            .snapshots
            .get(&event.char_id)
            .map(|s| s.combat_dead_pending_release)
            .unwrap_or(false)
        {
            tracing::debug!(
                "[bong][npc] hydrate_dormant_on_rechallenge_trigger: char_id={} is combat_dead_pending_release, \
                 skipping hydrate — retry system will handle release and cleanup",
                event.char_id
            );
            continue;
        }
        if let Some(error) = store
            .snapshots
            .get(&event.char_id)
            .and_then(NpcDormantSnapshot::durable_identity_error)
        {
            tracing::warn!(
                character_id = %event.char_id,
                "[bong][npc] refusing rechallenge hydrate with divergent durable identity: {error}"
            );
            continue;
        }

        let Some(snapshot) = store.remove(&event.char_id) else {
            // store 中无该 char_id：已被邻近 hydrate 抢先处理，或 entry 已过期——安全跳过
            tracing::debug!(
                "[bong][npc] hydrate_dormant_on_rechallenge_trigger: char_id={} not in dormant store \
                 (may have been hydrated already), skipping",
                event.char_id
            );
            continue;
        };

        let entity = spawn_from_snapshot(
            &mut commands,
            &technique_registry,
            snapshot,
            dimension_layers,
            tick,
            pois.as_deref(),
            skin_pool.as_deref_mut(),
            &relic_containers,
        );

        tribulations.send(InitiateXuhuaTribulation {
            entity,
            waves_total: NPC_TRIBULATION_WAVES_DEFAULT,
            started_tick: tick,
        });

        tracing::info!(
            "[bong][npc] hydrate_dormant_on_rechallenge_trigger: dormant NPC char_id={} hydrated \
             into entity {entity:?}, InitiateXuhuaTribulation sent",
            event.char_id
        );
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn dehydrate_far_npcs_system(
    game_tick: Option<Res<GameTick>>,
    config: Res<NpcVirtualizationConfig>,
    mut store: ResMut<NpcDormantStore>,
    mut commands: Commands,
    zone_registry: Option<Res<ZoneRegistry>>,
    players: Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
    npcs: Query<
        (
            Entity,
            &Position,
            Option<&CurrentDimension>,
            &Lifecycle,
            &NpcArchetype,
            &NpcLifespan,
            &Cultivation,
            &MeridianSystem,
            &Contamination,
            Option<&NpcDailySchedule>,
            Option<&FactionMembership>,
            Option<&NpcPatrol>,
        ),
        (
            With<NpcMarker>,
            Without<Despawned>,
            Without<ScenarioNpc>,
            Without<DyingElderBlackboard>,
        ),
    >,
    severed: Query<Option<&MeridianSeveredPermanent>, With<NpcMarker>>,
    shared_lifespan: Query<Option<&LifespanComponent>, With<NpcMarker>>,
    lifespan_extension_ledger: Query<Option<&LifespanExtensionLedger>, With<NpcMarker>>,
    death_registry: Query<Option<&DeathRegistry>, With<NpcMarker>>,
    life_record: Query<Option<&LifeRecord>, With<NpcMarker>>,
    loot_tables: Query<Option<&NpcLootTable>, With<NpcMarker>>,
    memories: Query<Option<&NpcMemoryComponent>, With<NpcMarker>>,
    player_reputations: Query<Option<&NpcPlayerReputation>, With<NpcMarker>>,
    extras: DormantExtraComponentQueries,
) {
    let tick = crate::npc::dormant::current_tick(game_tick.as_deref());
    if !crate::npc::dormant::should_run_interval(tick, config.transition_interval_ticks) {
        return;
    }

    let player_positions = players
        .iter()
        .map(|(pos, dimension)| (dimension_kind(dimension), pos.get()))
        .collect::<Vec<_>>();
    if player_positions.is_empty() && !config.dehydrate_without_players {
        return;
    }

    let zone_reg = zone_registry.as_deref();
    let player_zones = player_zone_names(zone_reg, &player_positions);
    let mut candidates = Vec::new();
    for (
        entity,
        position,
        current_dimension,
        lifecycle,
        archetype,
        lifespan,
        cultivation,
        meridian_system,
        contamination,
        schedule,
        faction,
        patrol,
    ) in npcs.iter()
    {
        let dimension = current_dimension
            .map(|dimension| dimension.0)
            .unwrap_or(DimensionKind::Overworld);
        // 脱水守护：若 NPC 满足渡劫条件（基准 1.0，不使用时代修正），则不脱水以保留渡劫机会。
        if live_tribulation_ready(cultivation, meridian_system, 1.0) {
            continue;
        }
        let nearest =
            nearest_same_dimension_player_distance(position.get(), dimension, &player_positions);

        if !player_positions.is_empty() && nearest <= config.dehydrate_radius_blocks {
            continue;
        }
        let zone_name = zone_reg
            .and_then(|zones| zones.find_zone(dimension, position.get()))
            .map(|zone| zone.name.clone())
            .or_else(|| patrol.map(|patrol| patrol.home_zone.clone()))
            .unwrap_or_else(|| "spawn".to_string());
        if player_zones.contains(zone_name.as_str()) {
            continue;
        }
        let Ok(Some(life_record)) = life_record.get(entity) else {
            tracing::warn!(
                entity = ?entity,
                character_id = %lifecycle.character_id,
                "[bong][npc] refusing to dehydrate NPC without canonical LifeRecord"
            );
            continue;
        };
        let Ok(Some(death_registry)) = death_registry.get(entity) else {
            tracing::warn!(
                entity = ?entity,
                character_id = %lifecycle.character_id,
                "[bong][npc] refusing to dehydrate NPC without canonical DeathRegistry"
            );
            continue;
        };
        if let Some(error) =
            durable_npc_identity_error(&lifecycle.character_id, life_record, death_registry)
        {
            tracing::warn!(
                entity = ?entity,
                lifecycle_id = %lifecycle.character_id,
                life_record_id = %life_record.character_id,
                death_registry_id = %death_registry.char_id,
                "[bong][npc] refusing to dehydrate NPC with invalid durable identity: {error}"
            );
            continue;
        }
        let patrol_snapshot = patrol.map(|patrol| DormantPatrolSnapshot {
            home_zone: patrol.home_zone.clone(),
            anchor_index: patrol.anchor_index,
            current_target: crate::npc::dormant::vec3_to_array(patrol.current_target),
        });
        let intent = DormantBehaviorIntent::for_archetype(*archetype, patrol_snapshot.as_ref());
        if extras.rat_blackboards.get(entity).ok().flatten().is_some() {
            tracing::warn!(
                entity = ?entity,
                character_id = %lifecycle.character_id,
                "[bong][npc] refusing to dehydrate rat without a rat-specific dormant snapshot"
            );
            continue;
        }
        let mimic_spider = match extras.spider_blackboards.get(entity).ok().flatten() {
            Some(blackboard)
                if blackboard.trapped_by.is_none()
                    && extras.spider_traps.get(entity).ok().flatten().is_none()
                    && blackboard.drained_qi.is_finite()
                    && blackboard.drained_qi >= 0.0 =>
            {
                Some(DormantMimicSpiderSnapshot {
                    state: extras
                        .spider_states
                        .get(entity)
                        .ok()
                        .flatten()
                        .copied()
                        .unwrap_or_default(),
                    home_zone: blackboard.home_zone.clone(),
                    home_pos: vec3_to_array(blackboard.home_pos),
                    drained_qi: blackboard.drained_qi,
                })
            }
            Some(_) => {
                tracing::warn!(
                    entity = ?entity,
                    character_id = %lifecycle.character_id,
                    "[bong][npc] refusing to dehydrate spider with ephemeral ownership or invalid telemetry"
                );
                continue;
            }
            None => None,
        };
        let tsy_marker = extras.tsy_markers.get(entity).ok().flatten();
        let daozhan = match extras.daozhan_blackboards.get(entity).ok().flatten() {
            Some(blackboard)
                if blackboard.daozhan_qi.is_finite() && blackboard.daozhan_qi >= 0.0 =>
            {
                Some(DormantDaozhanSnapshot {
                    state: extras
                        .daozhan_states
                        .get(entity)
                        .ok()
                        .flatten()
                        .copied()
                        .unwrap_or_default(),
                    home_zone: blackboard.home_zone.clone(),
                    home_pos: vec3_to_array(blackboard.home_pos),
                    daozhan_qi: blackboard.daozhan_qi,
                    origin_realm: blackboard.origin_realm,
                    behavior_queue: blackboard.behavior_queue.iter().copied().collect(),
                    current_behavior_ticks: blackboard.current_behavior_ticks,
                })
            }
            Some(blackboard) => {
                tracing::warn!(
                    entity = ?entity,
                    character_id = %lifecycle.character_id,
                    daozhan_qi = blackboard.daozhan_qi,
                    "[bong][npc] refusing to dehydrate invalid Daozhan external qi owner"
                );
                continue;
            }
            None => None,
        };
        if daozhan.is_some() && tsy_marker.is_none() {
            tracing::warn!(
                entity = ?entity,
                character_id = %lifecycle.character_id,
                "[bong][npc] refusing to dehydrate Daozhan without stable TSY identity"
            );
            continue;
        }

        candidates.push((
            entity,
            lifecycle.character_id.clone(),
            NpcDormantSnapshot {
                char_id: lifecycle.character_id.clone(),
                archetype: *archetype,
                dimension,
                zone_name,
                position: vec3_to_array(position.get()),
                schedule_seed: Some(schedule.map(|schedule| schedule.seed).unwrap_or_else(|| {
                    schedule_seed_from_char_id(lifecycle.character_id.as_str())
                })),
                cultivation: cultivation.clone(),
                meridian_system: meridian_system.clone(),
                meridian_severed: severed
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or_default(),
                contamination: contamination.clone(),
                lifespan: *lifespan,
                shared_lifespan: shared_lifespan
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or_else(|| LifespanComponent::for_realm(cultivation.realm)),
                lifespan_extension_ledger: lifespan_extension_ledger
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or_default(),
                death_registry: death_registry.clone(),
                life_record: life_record.clone(),
                memory: memories
                    .get(entity)
                    .ok()
                    .flatten()
                    .filter(|memory| !memory.interactions.is_empty())
                    .cloned(),
                player_reputation: player_reputations
                    .get(entity)
                    .ok()
                    .flatten()
                    .filter(|reputation| !reputation.is_empty())
                    .cloned(),
                faction: faction.cloned(),
                // plan-offscreen-war-v1 P5 reframe b：dehydration 暂不携带显式群体——离屏
                // 战斗用 effective_group 回退 faction 派生（commit 1 范围）。ECS 层的群体身份
                // 同步（live→dormant 往返）留后续；None 保非破坏。
                emergent_group: None,
                patrol: patrol_snapshot,
                loot_table: loot_tables
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .or_else(|| Some(default_loot_for_archetype(*archetype))),
                guardian_relic: dormant_guardian_relic_snapshot(
                    extras.guardian_duties.get(entity).ok().flatten(),
                    extras.trial_evals.get(entity).ok().flatten(),
                ),
                mimic_spider,
                tsy_hostile: dormant_tsy_hostile_snapshot(
                    tsy_marker,
                    extras.zhinian_minds.get(entity).ok().flatten(),
                    extras.fuya_auras.get(entity).ok().flatten(),
                    extras.daoxiang_origins.get(entity).ok().flatten(),
                    daozhan,
                ),
                tsy_sentinel: dormant_tsy_sentinel_snapshot(
                    extras.tsy_sentinel_markers.get(entity).ok().flatten(),
                    &extras.containers,
                ),
                intent,
                dormant_since_tick: tick,
                last_dormant_tick_processed: tick,
                initial_qi: cultivation.qi_current,
                qi_ledger_net: 0.0,
                combat_dead_pending_release: false,
                pending_combat_winner: None,
            },
        ));
    }

    candidates.sort_by(|left, right| left.1.cmp(&right.1));
    for (entity, char_id, mut snapshot) in candidates {
        if !can_insert_dormant_snapshot(&store, char_id.as_str(), config.max_dormant_count) {
            continue;
        }
        snapshot.patrol = snapshot.patrol.or_else(|| {
            Some(DormantPatrolSnapshot {
                home_zone: snapshot.zone_name.clone(),
                anchor_index: 0,
                current_target: snapshot.position,
            })
        });
        if store.contains(&char_id) {
            store.remove(&char_id);
        }
        store.insert(snapshot);
        commands.entity(entity).insert(Despawned);
    }
}

fn can_insert_dormant_snapshot(
    store: &NpcDormantStore,
    char_id: &str,
    max_dormant_count: usize,
) -> bool {
    store.contains(char_id) || store.len() < max_dormant_count
}

fn dormant_guardian_relic_snapshot(
    duty: Option<&GuardianDuty>,
    trial: Option<&TrialEval>,
) -> Option<DormantGuardianRelicSnapshot> {
    let duty = duty?;
    let trial = trial?;
    Some(DormantGuardianRelicSnapshot {
        relic_id: duty.relic_id.clone(),
        alarm_center: vec3_to_array(duty.alarm_center),
        alarm_radius: duty.alarm_radius,
        trial_template_id: trial.trial_template_id.clone(),
        last_offered_tick: trial.last_offered_tick,
        offer_cooldown_ticks: trial.offer_cooldown_ticks,
    })
}

fn dormant_tsy_hostile_snapshot(
    marker: Option<&TsyHostileMarker>,
    zhinian: Option<&ZhinianMind>,
    fuya: Option<&FuyaAura>,
    daoxiang_origin: Option<&DaoxiangOrigin>,
    daozhan: Option<DormantDaozhanSnapshot>,
) -> Option<DormantTsyHostileSnapshot> {
    let marker = marker?;
    Some(DormantTsyHostileSnapshot {
        family_id: marker.family_id.clone(),
        zhinian_phase: zhinian.map(|mind| dormant_zhinian_phase(mind.phase)),
        zhinian_phase_entered_at_tick: zhinian.map(|mind| mind.phase_entered_at_tick),
        fuya_aura: fuya.map(|aura| DormantFuyaAuraSnapshot {
            radius_blocks: aura.radius_blocks,
            drain_boost_multiplier: aura.drain_boost_multiplier,
        }),
        daoxiang_origin: daoxiang_origin.map(|origin| DormantDaoxiangOriginSnapshot {
            from_family: origin.from_family.clone(),
            from_corpse_death_cause: origin.from_corpse_death_cause.clone(),
            activated_at_tick: origin.activated_at_tick,
            inherited_drops: origin.inherited_drops.clone(),
        }),
        daozhan,
    })
}

/// plan-tsy-sentinel-dormant-regression-v1 §P1：dehydrate 侧 TSY 秘境守灵身份快照。
///
/// `marker` 存在才返回 `Some`——普通 overworld `GuardianRelic`（从不携带 `TsySentinelMarker`）
/// 恒返回 `None`，路由天然落到 `spawn_relic_guard_npc_at` 分支（§P2 末条决议）。
/// `guarding_container_pos` 用 `marker.guarding_container`（此刻是精确已知的单个 `Entity`，
/// 无歧义）直接 `.get()` 查 `Position` 写入；容器不存在时优雅退化为 `None`（§8.1 #2：现状
/// 验证容器从不 dehydrate/despawn，但仍写容错分支而非 `.unwrap()`）。
fn dormant_tsy_sentinel_snapshot(
    marker: Option<&TsySentinelMarker>,
    containers: &Query<&Position, (With<LootContainer>, Without<NpcMarker>)>,
) -> Option<DormantTsySentinelSnapshot> {
    let marker = marker?;
    let guarding_container_pos = marker
        .guarding_container
        .and_then(|entity| containers.get(entity).ok())
        .map(|position| vec3_to_array(position.get()));
    Some(DormantTsySentinelSnapshot {
        guarding_container_pos,
        phase: marker.phase,
        max_phase: marker.max_phase,
    })
}

fn dormant_zhinian_phase(phase: ZhinianPhase) -> DormantZhinianPhase {
    match phase {
        ZhinianPhase::Masquerade => DormantZhinianPhase::Masquerade,
        ZhinianPhase::Aggressive => DormantZhinianPhase::Aggressive,
    }
}

fn hydrate_zhinian_phase(phase: DormantZhinianPhase) -> ZhinianPhase {
    match phase {
        DormantZhinianPhase::Masquerade => ZhinianPhase::Masquerade,
        DormantZhinianPhase::Aggressive => ZhinianPhase::Aggressive,
    }
}

fn dimension_kind(dimension: Option<&CurrentDimension>) -> DimensionKind {
    dimension.map(|dimension| dimension.0).unwrap_or_default()
}

fn nearest_same_dimension_player_distance(
    position: valence::prelude::DVec3,
    dimension: DimensionKind,
    player_positions: &[PlayerPosition],
) -> f64 {
    player_positions
        .iter()
        .filter(|(player_dimension, _)| *player_dimension == dimension)
        .map(|(_, player_pos)| planar_distance(position, *player_pos))
        .fold(f64::INFINITY, f64::min)
}

fn player_zone_names(
    zone_registry: Option<&ZoneRegistry>,
    player_positions: &[PlayerPosition],
) -> HashSet<String> {
    let Some(zones) = zone_registry else {
        return HashSet::new();
    };
    player_positions
        .iter()
        .filter_map(|(dim, pos)| zones.find_zone(*dim, *pos))
        .map(|zone| zone.name.clone())
        .collect()
}

/// plan-tsy-sentinel-dormant-regression-v1 §P2：把 `LootContainer` query 现算成
/// `spawn_from_snapshot` 需要的三元组 Vec（entity / family_id / 世界坐标）。两个 hydrate
/// 调用方各自持有独立的 `Query`，故各自调用一次（非共享 system param）。
fn collect_relic_containers(
    query: &Query<(Entity, &Position, &LootContainer), With<LootContainer>>,
) -> Vec<(Entity, String, DVec3)> {
    query
        .iter()
        .map(|(entity, position, container)| (entity, container.family_id.clone(), position.get()))
        .collect()
}

/// plan-tsy-sentinel-dormant-regression-v1 §8.1 #1 补漏：容器重绑坐标 epsilon（格）。
const SENTINEL_CONTAINER_REBIND_EPSILON_BLOCKS: f64 = 0.5;

/// plan-tsy-sentinel-dormant-regression-v1 §P2：TSY 秘境守灵的 family_id 来源统一入口。
///
/// 复用 `snapshot.tsy_hostile.family_id`（§P1 已论证两者恒相等，不新增独立字段）；缺失时
/// （理论不会发生的防御分支）回退一个基于 `home_zone` 的合成 id，与既有 `guardian_relic`
/// 字段的 `unwrap_or_else(|| format!("relic:{home_zone}"))` 防御风格一致。
fn sentinel_family_id_from_snapshot(snapshot: &NpcDormantSnapshot, home_zone: &str) -> String {
    snapshot
        .tsy_hostile
        .as_ref()
        .map(|tsy| tsy.family_id.clone())
        .unwrap_or_else(|| format!("tsy_sentinel:{home_zone}"))
}

/// plan-tsy-sentinel-dormant-regression-v1 §8.1 #1 补漏（博弈 blocker）：两段式
/// family+坐标重绑，防止跨 family 同坐标容器偶合误绑。
///
/// **绝不允许跨 family 对全体 `relic_containers` 裸坐标匹配**——`spawn_tutorial.rs` 的
/// `tutorial_chest`（family_id 硬编码 `"spawn_tutorial"`）与真实 TSY 容器同为 Overworld
/// layer，坐标理论上可能碰巧落入同一 epsilon。先按 `family_id` 精确相等过滤子集，再仅在
/// 该子集内按坐标 epsilon（≤0.5 格）匹配。family 内确无匹配坐标（容器已消失，§8.1 #2
/// 决议：现状验证这不会发生，但仍需容错而非 `.unwrap()`）→ 返回 `None` 并 `tracing::warn!`
/// （sentinel 仍按守灵身份 spawn，只是不再绑定具体容器，退化为纯 aggro，不阻塞外观/HUD/掉落）。
fn resolve_sentinel_guarding_container(
    relic_containers: &[(Entity, String, DVec3)],
    family_id: &str,
    guarding_container_pos: Option<[f64; 3]>,
) -> Option<Entity> {
    let target = dvec3_from_array(guarding_container_pos?);
    let found = relic_containers
        .iter()
        .filter(|(_, candidate_family, _)| candidate_family == family_id)
        .find(|(_, _, candidate_pos)| {
            candidate_pos.distance(target) <= SENTINEL_CONTAINER_REBIND_EPSILON_BLOCKS
        })
        .map(|(entity, _, _)| *entity);
    if found.is_none() {
        tracing::warn!(
            family = %family_id,
            target_pos = ?target,
            candidate_count = relic_containers.len(),
            "[bong][npc] tsy sentinel hydrate: no container matched family+position for rebind; \
             guarding_container=None, sentinel degrades to pure aggro (no HUD/loot/phase impact)"
        );
    }
    found
}

#[allow(clippy::too_many_arguments)]
fn spawn_from_snapshot(
    commands: &mut Commands,
    technique_registry: &crate::cultivation::known_techniques::TechniqueRegistry,
    snapshot: NpcDormantSnapshot,
    dimension_layers: &DimensionLayers,
    current_tick: u64,
    pois: Option<&PoiNoviceRegistry>,
    skin_pool: Option<&mut SkinPool>,
    // plan-tsy-sentinel-dormant-regression-v1 §P2：三元组 = LootContainer entity / family_id /
    // 世界坐标，由调用方各自的 `Query<(Entity, &Position, &LootContainer), With<LootContainer>>`
    // 现算出的 Vec 传入。用于 TSY 秘境守灵 hydrate 时按 family+坐标重绑 `guarding_container`。
    relic_containers: &[(Entity, String, DVec3)],
) -> Entity {
    let layer = match snapshot.dimension {
        DimensionKind::Tsy => dimension_layers.tsy,
        _ => dimension_layers.overworld,
    };
    let schedule_seed = snapshot
        .schedule_seed
        .unwrap_or_else(|| schedule_seed_from_char_id(snapshot.char_id.as_str()));
    let schedule = NpcDailySchedule::for_archetype(snapshot.archetype, schedule_seed);
    let patrol_target = snapshot
        .patrol
        .as_ref()
        .map(|patrol| dvec3_from_array(patrol.current_target))
        .unwrap_or_else(|| snapshot.position_vec());
    let snapshot_pos = snapshot.position_vec();
    let home_base = home_base_for_archetype(snapshot.archetype, snapshot_pos);
    let pos = hydrate_position_for(
        &schedule,
        Some(home_base),
        snapshot_pos,
        current_tick,
        schedule_seed,
        pois,
    );
    let home_zone = snapshot.zone_name.as_str();
    let skin_policy = NpcSkinFallbackPolicy::AllowFallback;
    // plan-tsy-sentinel-dormant-regression-v1 §P2：GuardianRelic 分支若路由到
    // `spawn_tsy_sentinel_at`，两段式重绑解出的 `guarding_container` / `family_id` 存在这里，
    // 供下面 tail-insert 处重建 `TsySentinelMarker` 时复用——**不能**在 tail-insert 处重新
    // `&snapshot` 借用整个快照来重算（下面的 `entity_commands.insert((snapshot.cultivation, ...))`
    // 已经把 `Cultivation`（非 `Copy`）等字段从 `snapshot` 里移出，整体借用会撞
    // E0382 partial-move 借用检查；这里提前把需要的值拷进独立局部变量规避）。
    let mut resolved_sentinel_guarding_container: Option<Entity> = None;
    let mut resolved_sentinel_family_id: Option<String> = None;
    let mimic_spider = snapshot.mimic_spider.clone();
    let entity = match snapshot.archetype {
        NpcArchetype::Zombie => spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target),
        NpcArchetype::Commoner => spawn_commoner_npc_at(
            commands,
            NpcSkinSpawnContext::new(skin_pool, skin_policy),
            layer,
            home_zone,
            pos,
            patrol_target,
            snapshot.cultivation.realm,
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::Rogue => spawn_rogue_npc_at(
            commands,
            technique_registry,
            NpcSkinSpawnContext::new(skin_pool, skin_policy),
            layer,
            home_zone,
            pos,
            patrol_target,
            snapshot.cultivation.realm,
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::Beast => match mimic_spider.as_ref() {
            Some(spider) => spawn_ash_spider_npc_at(
                commands,
                layer,
                spider.home_zone.as_str(),
                pos,
                patrol_target,
            ),
            None => spawn_beast_npc_at(
                commands,
                layer,
                home_zone,
                pos,
                Territory::new(patrol_target, 40.0),
                snapshot.lifespan.age_ticks,
            ),
        },
        NpcArchetype::Disciple => spawn_disciple_npc_at(
            commands,
            technique_registry,
            NpcSkinSpawnContext::new(skin_pool, skin_policy),
            layer,
            home_zone,
            pos,
            patrol_target,
            snapshot
                .faction
                .as_ref()
                .map(|membership| membership.faction_id)
                .unwrap_or(crate::npc::faction::FactionId::Neutral),
            snapshot
                .faction
                .as_ref()
                .map(|membership| membership.rank)
                .unwrap_or(FactionRank::Disciple),
            snapshot.cultivation.realm,
            snapshot
                .faction
                .as_ref()
                .and_then(|membership| membership.lineage.as_ref())
                .and_then(|lineage| lineage.master_id.clone()),
            snapshot.lifespan.age_ticks,
        ),
        // plan-tsy-sentinel-dormant-regression-v1 §P2（核心修复）：`GuardianRelic` 双身份
        // 路由判据钉死在 snapshot 层——`snapshot.tsy_sentinel.is_some()` 是唯一判据（§8.1
        // #3 决议）。`Some` → TSY 秘境守灵，必须走 `spawn_tsy_sentinel_at` 保留
        // marker/外观/HUD/掉落身份；`None` → 纯 overworld relic guard，行为不变。
        NpcArchetype::GuardianRelic => match snapshot.tsy_sentinel.as_ref() {
            Some(sentinel_snapshot) => {
                let sentinel_family_id = sentinel_family_id_from_snapshot(&snapshot, home_zone);
                let guarding_container = resolve_sentinel_guarding_container(
                    relic_containers,
                    sentinel_family_id.as_str(),
                    sentinel_snapshot.guarding_container_pos,
                );
                resolved_sentinel_guarding_container = guarding_container;
                resolved_sentinel_family_id = Some(sentinel_family_id.clone());
                spawn_tsy_sentinel_at(
                    commands,
                    layer,
                    sentinel_family_id.as_str(),
                    home_zone,
                    pos,
                    guarding_container,
                )
            }
            None => {
                let relic = snapshot.guardian_relic.as_ref();
                spawn_relic_guard_npc_at(
                    commands,
                    technique_registry,
                    layer,
                    home_zone,
                    pos,
                    relic.map(|snapshot| snapshot.alarm_radius).unwrap_or(40.0),
                    relic
                        .map(|snapshot| snapshot.relic_id.clone())
                        .unwrap_or_else(|| format!("relic:{home_zone}")),
                    relic
                        .map(|snapshot| snapshot.trial_template_id.clone())
                        .unwrap_or_else(|| format!("trial:{home_zone}")),
                )
            }
        },
        NpcArchetype::Daoxiang => snapshot
            .tsy_hostile
            .as_ref()
            .map(|tsy| {
                spawn_tsy_daoxiang_at(
                    commands,
                    technique_registry,
                    layer,
                    tsy.family_id.as_str(),
                    home_zone,
                    pos,
                    patrol_target,
                )
            })
            .unwrap_or_else(|| spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)),
        NpcArchetype::Zhinian => snapshot
            .tsy_hostile
            .as_ref()
            .map(|tsy| {
                spawn_tsy_zhinian_at(
                    commands,
                    technique_registry,
                    layer,
                    tsy.family_id.as_str(),
                    home_zone,
                    pos,
                    patrol_target,
                )
            })
            .unwrap_or_else(|| spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)),
        NpcArchetype::Fuya => snapshot
            .tsy_hostile
            .as_ref()
            .map(|tsy| {
                spawn_tsy_fuya_at(
                    commands,
                    layer,
                    tsy.family_id.as_str(),
                    home_zone,
                    pos,
                    patrol_target,
                )
            })
            .unwrap_or_else(|| spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)),
        NpcArchetype::SkullFiend => snapshot
            .tsy_hostile
            .as_ref()
            .map(|tsy| {
                spawn_tsy_skull_fiend_at(
                    commands,
                    layer,
                    tsy.family_id.as_str(),
                    home_zone,
                    pos,
                    patrol_target,
                )
            })
            .unwrap_or_else(|| spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)),
        // plan-dying-elder-v1：垂死大能由 DyingElderSpawnSystem（P1）管理，
        // hydrate 路径退化为 zombie 占位（P1 完整实装前不会有 DyingElder snapshot）。
        NpcArchetype::DyingElder => {
            spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)
        }
        // plan-mundane-fauna-v1 P0：凡兽不持久化 `MundaneFaunaKind`——同 `spawn_beast_npc_at`
        // 用 `fauna_tag_for_beast_spawn(home_zone, seed)` 从 home_zone+位置重新派生
        // `BeastKind` 而不持久化的先例，复活时用 `mundane_species_for_position` 从
        // (home_zone, pos) 确定性重新派生同一物种（biome 池 + 位置种子，与首次 ambient
        // spawn 走同一口径）。
        NpcArchetype::Mundane => spawn_mundane_fauna_at(
            commands,
            layer,
            home_zone,
            pos,
            patrol_target,
            crate::fauna::mundane::mundane_species_for_position(home_zone, pos),
        ),
    };

    let mut entity_commands = commands.entity(entity);
    entity_commands.insert((
        snapshot.archetype,
        snapshot.cultivation,
        snapshot.meridian_system,
        snapshot.meridian_severed,
        snapshot.contamination,
        snapshot.lifespan,
        snapshot.shared_lifespan,
        snapshot.lifespan_extension_ledger,
        snapshot.death_registry,
        snapshot.life_record,
        schedule,
        home_base,
        NpcLodTier::Near,
        Lifecycle {
            character_id: snapshot.char_id.clone(),
            ..Default::default()
        },
        CurrentDimension(snapshot.dimension),
    ));
    if let Some(spider) = mimic_spider {
        entity_commands.insert((
            spider.state,
            MimicSpiderBlackboard {
                home_zone: spider.home_zone,
                home_pos: dvec3_from_array(spider.home_pos),
                drained_qi: spider.drained_qi,
                trapped_by: None,
            },
        ));
    }
    if let Some(memory) = snapshot.memory {
        entity_commands.insert(memory);
    }
    if let Some(player_reputation) = snapshot.player_reputation {
        entity_commands.insert(player_reputation);
    }
    if let Some(faction) = snapshot.faction {
        entity_commands.insert(faction);
    }
    if let Some(loot_table) = snapshot.loot_table {
        entity_commands.insert(loot_table);
    }
    if let Some(patrol) = snapshot.patrol {
        let mut patrol_component =
            NpcPatrol::new(patrol.home_zone, dvec3_from_array(patrol.current_target));
        patrol_component.anchor_index = patrol.anchor_index;
        entity_commands.insert(patrol_component);
    }
    if let Some(relic) = snapshot.guardian_relic {
        entity_commands.insert((
            GuardianDuty::new(relic.relic_id, dvec3_from_array(relic.alarm_center))
                .with_radius(relic.alarm_radius),
            TrialEval {
                trial_template_id: relic.trial_template_id,
                last_offered_tick: relic.last_offered_tick,
                offer_cooldown_ticks: relic.offer_cooldown_ticks,
            },
        ));
    }
    // plan-tsy-sentinel-dormant-regression-v1 §P2：重新接好 TSY 秘境守灵专属语义——
    // `spawn_tsy_sentinel_at`（上面 match 分支）已经内部插入了默认 `phase=0/max_phase=3`
    // 的 `TsySentinelMarker`；这里用 hydrate 快照精确回填 `max_phase`（design 常量，
    // 恒定无风险）与 best-effort `phase`（下一次 `update_sentinel_phase_system` 会按
    // *当前*满血 `Wounds` 重算纠正，§8.1 #2 决议，不产生持久错位），以及两段式重绑解出
    // 的 `guarding_container`（覆盖 `spawn_tsy_sentinel_at` 内部默认的 `None`）。
    // `FaunaVisualKind::TsySentinel` / `sentinel_thinker()` 已在 `spawn_tsy_sentinel_at`
    // 内部插入，无需在此重复。`family_id` 复用上面 match 分支算好并存进
    // `resolved_sentinel_family_id` 的值（不能在此重新 `&snapshot` 整体借用重算——
    // `Cultivation` 等字段已在上面的 `entity_commands.insert((snapshot.cultivation, ...))`
    // 移出，整体借用会撞 E0382）。
    if let Some(sentinel) = snapshot.tsy_sentinel.as_ref() {
        entity_commands.insert(TsySentinelMarker {
            family_id: resolved_sentinel_family_id
                .clone()
                .unwrap_or_else(|| format!("tsy_sentinel:{home_zone}")),
            guarding_container: resolved_sentinel_guarding_container,
            phase: sentinel.phase,
            max_phase: sentinel.max_phase,
        });
    }
    if let Some(tsy) = snapshot.tsy_hostile {
        let daozhan = tsy.daozhan;
        entity_commands.insert(TsyHostileMarker {
            family_id: tsy.family_id,
        });
        if let (Some(phase), Some(phase_entered_at_tick)) =
            (tsy.zhinian_phase, tsy.zhinian_phase_entered_at_tick)
        {
            entity_commands.insert(ZhinianMind {
                phase: hydrate_zhinian_phase(phase),
                phase_entered_at_tick,
                combat_memory: Default::default(),
            });
        }
        if let Some(aura) = tsy.fuya_aura {
            entity_commands.insert(FuyaAura {
                radius_blocks: aura.radius_blocks,
                drain_boost_multiplier: aura.drain_boost_multiplier,
            });
        }
        if let Some(origin) = tsy.daoxiang_origin {
            entity_commands.insert(DaoxiangOrigin {
                from_family: origin.from_family,
                from_corpse_death_cause: origin.from_corpse_death_cause,
                activated_at_tick: origin.activated_at_tick,
                inherited_drops: origin.inherited_drops,
            });
        }
        if let Some(daozhan) = daozhan {
            let mut blackboard = DaoZhangBehaviorBlackboard::new(
                daozhan.home_zone.as_str(),
                dvec3_from_array(daozhan.home_pos),
                daozhan.origin_realm,
            );
            blackboard.daozhan_qi = daozhan.daozhan_qi;
            blackboard.behavior_queue = daozhan.behavior_queue.into_iter().collect();
            blackboard.current_behavior_ticks = daozhan.current_behavior_ticks;
            entity_commands.insert((daozhan.state, blackboard));
        }
    }
    entity
}

fn dormant_tribulation_ready(
    snapshot: &NpcDormantSnapshot,
    tribulation_threshold_mul: f64,
) -> bool {
    live_tribulation_ready(
        &snapshot.cultivation,
        &snapshot.meridian_system,
        tribulation_threshold_mul,
    )
}

/// P1 era 注入：渡劫阈值乘以 `tribulation_threshold_mul`（来自 WorldEraState::current_modifiers）。
/// 灾劫时代 mul > 1.0 → 需要更高 qi 才能触发渡劫；演绎时代 mul < 1.0 → 阈值微降。
fn live_tribulation_ready(
    cultivation: &Cultivation,
    meridian_system: &MeridianSystem,
    tribulation_threshold_mul: f64,
) -> bool {
    let effective_ratio = DORMANT_TRIBULATION_MIN_QI_RATIO * tribulation_threshold_mul;
    du_xu_prereqs_met(cultivation, meridian_system)
        && cultivation.qi_current >= cultivation.qi_max * effective_ratio
}

#[cfg(test)]
#[path = "../hydrate_tests.rs"]
mod tests;
