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
use crate::fauna::daozhan::{DaoZhangBehaviorBlackboard, DaoZhangState};
use crate::npc::brain::NPC_TRIBULATION_WAVES_DEFAULT;
use crate::npc::dormant::{
    durable_npc_identity_error, dvec3_from_array, planar_distance, vec3_to_array,
    DormantBehaviorIntent, DormantDaoxiangOriginSnapshot, DormantDaozhanSnapshot,
    DormantFuyaAuraSnapshot, DormantGuardianRelicSnapshot, DormantPatrolSnapshot,
    DormantTsyHostileSnapshot, DormantTsySentinelSnapshot, DormantZhinianPhase, NpcDormantSnapshot,
    NpcDormantStore, NpcVirtualizationConfig,
};
use crate::npc::faction::{FactionMembership, FactionRank};
use crate::npc::interaction_memory::NpcMemoryComponent;
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan, NpcRegistry};
use crate::npc::lod::NpcLodTier;
use crate::npc::loot::{default_loot_for_archetype, NpcLootTable};
use crate::npc::movement::GameTick;
use crate::npc::patrol::NpcPatrol;
use crate::npc::relic::{GuardianDuty, TrialEval};
use crate::npc::schedule::{
    home_base_for_archetype, hydrate_position_for, schedule_seed_from_char_id, NpcDailySchedule,
};
use crate::npc::spawn::{
    spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at, spawn_mundane_fauna_at,
    spawn_relic_guard_npc_at, spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker,
    NpcSkinSpawnContext,
};
use crate::npc::spawn_rat::RatBlackboard;
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
        (With<NpcMarker>, Without<Despawned>),
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

fn spawn_from_snapshot(
    commands: &mut Commands,
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
            NpcSkinSpawnContext::new(skin_pool, skin_policy),
            layer,
            home_zone,
            pos,
            patrol_target,
            snapshot.cultivation.realm,
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::Beast => spawn_beast_npc_at(
            commands,
            layer,
            home_zone,
            pos,
            Territory::new(patrol_target, 40.0),
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::Disciple => spawn_disciple_npc_at(
            commands,
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
mod tests {
    use super::*;
    use valence::prelude::{DVec3, Events};

    use crate::cultivation::components::Realm;
    use crate::fauna::visual::FaunaVisualKind;
    use crate::npc::brain::return_home_action_system;
    use crate::npc::dormant::{DEHYDRATE_RADIUS_BLOCKS, HYDRATE_RADIUS_BLOCKS};
    use crate::npc::interaction_memory::{
        NpcInteractionOutcome, NpcInteractionType, NpcMemoryEntry,
    };
    use crate::npc::trade::RepTier;
    use crate::world::tsy_container::ContainerKind;
    use crate::world::zone::{TsyDepth, Zone, DEFAULT_SPAWN_ZONE_NAME};

    fn zone_registry() -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![Zone {
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
            }],
        }
    }

    fn snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            realm: Realm::Spirit,
            qi_current: 900.0,
            qi_max: 1000.0,
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
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
            pending_combat_winner: None,
        }
    }

    fn open_all_meridians(snapshot: &mut NpcDormantSnapshot) {
        for meridian in snapshot.meridian_system.iter_mut() {
            meridian.opened = true;
        }
    }

    fn attack_memory_for(player_uuid: &str, timestamp: u64) -> NpcMemoryComponent {
        let mut memory = NpcMemoryComponent::default();
        memory.remember(NpcMemoryEntry {
            player_uuid: player_uuid.to_string(),
            interaction_type: NpcInteractionType::Attack,
            timestamp,
            outcome: NpcInteractionOutcome::Harmed,
        });
        memory
    }

    fn high_reputation_for(player_uuid: &str) -> NpcPlayerReputation {
        let mut reputation = NpcPlayerReputation::default();
        reputation.adjust(player_uuid, 0.3);
        reputation
    }

    #[test]
    fn dehydrate_snapshot_prefers_zone_name_from_registry() {
        let registry = zone_registry();
        let zone_name = registry
            .find_zone(DimensionKind::Overworld, DVec3::new(10.0, 64.0, 10.0))
            .unwrap()
            .name
            .clone();
        assert_eq!(zone_name, DEFAULT_SPAWN_ZONE_NAME);
    }

    #[test]
    fn dormant_tribulation_ready_requires_spirit_full_meridians_and_qi() {
        let mut ready = snapshot("npc_ready", DVec3::new(10.0, 64.0, 10.0));
        open_all_meridians(&mut ready);
        assert!(dormant_tribulation_ready(&ready, 1.0));
        assert!(live_tribulation_ready(
            &ready.cultivation,
            &ready.meridian_system,
            1.0
        ));

        let mut low_qi = ready.clone();
        low_qi.cultivation.qi_current = 700.0;
        assert!(!dormant_tribulation_ready(&low_qi, 1.0));

        let mut missing_meridian = ready.clone();
        missing_meridian.meridian_system.regular[0].opened = false;
        assert!(!dormant_tribulation_ready(&missing_meridian, 1.0));
    }

    #[test]
    fn dormant_tribulation_calamity_era_raises_threshold() {
        use crate::world::era::{current_modifiers, EraType};
        let mut ready = snapshot("npc_calamity", DVec3::new(10.0, 64.0, 10.0));
        open_all_meridians(&mut ready);
        // qi = 0.85 * qi_max — 在 Unknown 时代通过，灾劫时代（×1.1 → 需 0.88）被拒
        let qi_max = ready.cultivation.qi_max;
        ready.cultivation.qi_current = qi_max * 0.85;

        let calamity_mul = current_modifiers(EraType::Calamity).tribulation_threshold_mul;
        assert!(
            !dormant_tribulation_ready(&ready, calamity_mul),
            "灾劫时代 qi=0.85*max < 0.8*1.1=0.88*max，dormant tribulation 应被拒"
        );
        assert!(
            dormant_tribulation_ready(&ready, 1.0),
            "Unknown 时代 qi=0.85*max >= 0.8*max，dormant tribulation 应通过"
        );
    }

    #[test]
    fn dormant_tribulation_deduction_era_lowers_threshold() {
        use crate::world::era::{current_modifiers, EraType};
        let mut ready = snapshot("npc_deduction", DVec3::new(10.0, 64.0, 10.0));
        open_all_meridians(&mut ready);
        // qi = 0.77 * qi_max — Unknown 时代被拒，演绎时代（×0.95 → 需 0.76）通过
        let qi_max = ready.cultivation.qi_max;
        ready.cultivation.qi_current = qi_max * 0.77;

        let deduction_mul = current_modifiers(EraType::Deduction).tribulation_threshold_mul;
        assert!(
            dormant_tribulation_ready(&ready, deduction_mul),
            "演绎时代 qi=0.77*max >= 0.8*0.95=0.76*max，dormant tribulation 应通过"
        );
        assert!(
            !dormant_tribulation_ready(&ready, 1.0),
            "Unknown 时代 qi=0.77*max < 0.8*max，dormant tribulation 应被拒"
        );
    }

    #[test]
    fn dormant_capacity_uses_store_len_not_tick_candidate_count() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_existing", DVec3::new(10.0, 64.0, 10.0)));

        assert!(!can_insert_dormant_snapshot(&store, "npc_new", 1));
        assert!(can_insert_dormant_snapshot(&store, "npc_existing", 1));
    }

    #[test]
    fn dehydrate_refuses_rat_until_rat_specific_snapshot_exists() {
        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);

        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::new(10.0, 64.0, 10.0)),
                Lifecycle {
                    character_id: "npc_rat_far".to_string(),
                    ..Default::default()
                },
                LifeRecord::new("npc_rat_far"),
                DeathRegistry::new("npc_rat_far"),
                NpcArchetype::Beast,
                NpcLifespan::new(0.0, 1_000.0),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                RatBlackboard::new("spawn", valence::prelude::ChunkPos::new(0, 0)),
            ))
            .id();

        app.update();

        assert!(
            !app.world()
                .resource::<NpcDormantStore>()
                .contains("npc_rat_far"),
            "rat without a lossless rat-specific dormant format must remain live"
        );
        assert!(
            app.world().get::<RatBlackboard>(entity).is_some(),
            "refused rat dehydration must retain the behavior and reserve mirror carrier"
        );
        assert!(
            app.world().get::<Despawned>(entity).is_none(),
            "refused rat dehydration must not mark the entity Despawned"
        );
    }

    #[test]
    fn dehydrate_requires_matching_canonical_durable_owner_components() {
        for (case, lifecycle_id, life_record_id, death_registry_id) in [
            ("missing_life_record", "npc_owner", None, Some("npc_owner")),
            (
                "missing_death_registry",
                "npc_owner",
                Some("npc_owner"),
                None,
            ),
            (
                "mismatched_life_record",
                "npc_owner",
                Some("npc_other"),
                Some("npc_owner"),
            ),
            (
                "mismatched_death_registry",
                "npc_owner",
                Some("npc_owner"),
                Some("npc_other"),
            ),
            ("blank", "", Some(""), Some("")),
            (
                "unassigned",
                "unassigned:life_record",
                Some("unassigned:life_record"),
                Some("unassigned:life_record"),
            ),
            (
                "surrounding_whitespace",
                " npc_owner ",
                Some(" npc_owner "),
                Some(" npc_owner "),
            ),
        ] {
            let mut app = App::new();
            app.insert_resource(NpcDormantStore::default());
            app.insert_resource(NpcVirtualizationConfig {
                transition_interval_ticks: 1,
                dehydrate_without_players: true,
                ..Default::default()
            });
            app.add_systems(Update, dehydrate_far_npcs_system);
            let entity = app
                .world_mut()
                .spawn((
                    NpcMarker,
                    Position(DVec3::new(10.0, 64.0, 10.0)),
                    Lifecycle {
                        character_id: lifecycle_id.to_string(),
                        ..Default::default()
                    },
                    NpcArchetype::Rogue,
                    NpcLifespan::new(0.0, 1_000.0),
                    Cultivation {
                        realm: Realm::Awaken,
                        qi_current: 10.0,
                        qi_max: 100.0,
                        ..Default::default()
                    },
                    MeridianSystem::default(),
                    Contamination::default(),
                ))
                .id();
            if let Some(character_id) = life_record_id {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(LifeRecord::new(character_id));
            }
            if let Some(character_id) = death_registry_id {
                app.world_mut()
                    .entity_mut(entity)
                    .insert(DeathRegistry::new(character_id));
            }

            app.update();

            assert!(
                app.world().resource::<NpcDormantStore>().is_empty(),
                "case={case}: invalid identity tuple must not enter durable dormant storage"
            );
            assert!(
                app.world().get::<Despawned>(entity).is_none(),
                "case={case}: failed identity preflight must leave the live NPC intact"
            );
        }
    }

    #[test]
    fn dehydrate_marks_live_npc_despawned_for_valence_layer_cleanup() {
        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);

        let lifecycle = Lifecycle {
            character_id: "npc_far".to_string(),
            ..Default::default()
        };
        let entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::new(10.0, 64.0, 10.0)),
                lifecycle,
                LifeRecord::new("npc_far"),
                DeathRegistry::new("npc_far"),
                NpcArchetype::Rogue,
                NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 42),
                NpcLifespan::new(0.0, 1_000.0),
                Cultivation {
                    realm: Realm::Awaken,
                    qi_current: 10.0,
                    qi_max: 100.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
            ))
            .id();

        app.update();

        assert!(app
            .world()
            .resource::<NpcDormantStore>()
            .contains("npc_far"));
        assert_eq!(
            app.world()
                .resource::<NpcDormantStore>()
                .snapshots
                .get("npc_far")
                .and_then(|snapshot| snapshot.schedule_seed),
            Some(42)
        );
        assert!(
            app.world().get::<Despawned>(entity).is_some(),
            "dehydrated NPC must be marked Despawned instead of raw despawned"
        );
    }

    #[test]
    fn dehydrate_snapshot_carries_npc_memory_and_player_reputation() {
        const PLAYER_ID: &str = "player-memory-1";

        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);

        let memory = attack_memory_for(PLAYER_ID, 77);
        let reputation = high_reputation_for(PLAYER_ID);
        app.world_mut().spawn((
            NpcMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
            Lifecycle {
                character_id: "npc_memory_rep".to_string(),
                ..Default::default()
            },
            LifeRecord::new("npc_memory_rep"),
            DeathRegistry::new("npc_memory_rep"),
            NpcArchetype::Rogue,
            NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 42),
            NpcLifespan::new(0.0, 1_000.0),
            Cultivation {
                realm: Realm::Awaken,
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
            MeridianSystem::default(),
            Contamination::default(),
            memory.clone(),
            reputation,
        ));

        app.update();

        let snapshot = app
            .world()
            .resource::<NpcDormantStore>()
            .snapshots
            .get("npc_memory_rep")
            .expect("dehydrated NPC should have a dormant snapshot");
        assert_eq!(
            snapshot
                .memory
                .as_ref()
                .expect("non-empty memory should be carried")
                .interactions,
            memory.interactions
        );
        assert_eq!(
            snapshot
                .player_reputation
                .as_ref()
                .expect("non-empty reputation should be carried")
                .tier(PLAYER_ID),
            RepTier::High
        );
    }

    #[test]
    fn hydrate_roundtrip_preserves_attack_memory_for_same_char_id() {
        const PLAYER_ID: &str = "player-attacker-1";

        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let mut snap = snapshot("npc_remembers_attack", DVec3::new(10.0, 64.0, 10.0));
        snap.memory = Some(attack_memory_for(PLAYER_ID, 99));
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);
        app.world_mut()
            .spawn((ClientMarker, Position(DVec3::new(10.0, 64.0, 10.0))));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        let (char_id, life_record_id, death_registry_id, remembers_attack) = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<
                (&Lifecycle, &LifeRecord, &DeathRegistry, &NpcMemoryComponent),
                With<NpcMarker>,
            >();
            query
                .iter(world)
                .map(|(lifecycle, life_record, death_registry, memory)| {
                    (
                        lifecycle.character_id.clone(),
                        life_record.character_id.clone(),
                        death_registry.char_id.clone(),
                        memory.has_been_attacked_by(PLAYER_ID),
                    )
                })
                .next()
                .expect("hydrated NPC should carry identity and memory components")
        };
        assert_eq!(char_id, "npc_remembers_attack");
        assert_eq!(life_record_id, char_id);
        assert_eq!(death_registry_id, char_id);
        assert!(
            remembers_attack,
            "hydrated NPC with the same char_id must still remember the attacker"
        );
    }

    #[test]
    fn hydrate_roundtrip_preserves_trade_reputation_tier() {
        const PLAYER_ID: &str = "player-trader-1";

        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let mut snap = snapshot("npc_remembers_trade_rep", DVec3::new(10.0, 64.0, 10.0));
        snap.player_reputation = Some(high_reputation_for(PLAYER_ID));
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);
        app.world_mut()
            .spawn((ClientMarker, Position(DVec3::new(10.0, 64.0, 10.0))));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        let tier = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&NpcPlayerReputation, With<NpcMarker>>();
            query
                .iter(world)
                .map(|reputation| reputation.tier(PLAYER_ID))
                .next()
                .expect("hydrated NPC should carry reputation component")
        };
        assert_eq!(
            tier,
            RepTier::High,
            "hydrated NPC must preserve per-player trade reputation tier"
        );
    }

    #[test]
    fn player_proximity_ignores_other_dimensions() {
        let players = vec![(DimensionKind::Tsy, DVec3::new(10.0, 64.0, 10.0))];

        assert_eq!(
            nearest_same_dimension_player_distance(
                DVec3::new(10.0, 64.0, 10.0),
                DimensionKind::Overworld,
                &players,
            ),
            f64::INFINITY
        );
        assert_eq!(
            nearest_same_dimension_player_distance(
                DVec3::new(10.0, 64.0, 10.0),
                DimensionKind::Tsy,
                &players,
            ),
            0.0
        );
    }

    #[test]
    fn dormant_guardian_relic_snapshot_carries_guard_and_trial_metadata() {
        let duty = GuardianDuty::new("relic:old", DVec3::new(2.0, 64.0, 3.0)).with_radius(32.0);
        let mut trial = TrialEval::new("trial:old");
        trial.last_offered_tick = Some(42);
        trial.offer_cooldown_ticks = 900;

        let snapshot = dormant_guardian_relic_snapshot(Some(&duty), Some(&trial))
            .expect("guardian relic metadata should snapshot");

        assert_eq!(snapshot.relic_id, "relic:old");
        assert_eq!(snapshot.alarm_center, [2.0, 64.0, 3.0]);
        assert_eq!(snapshot.alarm_radius, 32.0);
        assert_eq!(snapshot.trial_template_id, "trial:old");
        assert_eq!(snapshot.last_offered_tick, Some(42));
        assert_eq!(snapshot.offer_cooldown_ticks, 900);
    }

    #[test]
    fn dormant_tsy_hostile_snapshot_carries_family_phase_and_aura() {
        let marker = TsyHostileMarker {
            family_id: "family-a".to_string(),
        };
        let mind = ZhinianMind {
            phase: ZhinianPhase::Aggressive,
            phase_entered_at_tick: 77,
            combat_memory: Default::default(),
        };
        let aura = FuyaAura {
            radius_blocks: 12.0,
            drain_boost_multiplier: 2.0,
        };

        let snapshot =
            dormant_tsy_hostile_snapshot(Some(&marker), Some(&mind), Some(&aura), None, None)
                .expect("TSY hostile metadata should snapshot");

        assert_eq!(snapshot.family_id, "family-a");
        assert_eq!(
            snapshot.zhinian_phase,
            Some(DormantZhinianPhase::Aggressive)
        );
        assert_eq!(snapshot.zhinian_phase_entered_at_tick, Some(77));
        assert_eq!(
            snapshot.fuya_aura,
            Some(DormantFuyaAuraSnapshot {
                radius_blocks: 12.0,
                drain_boost_multiplier: 2.0,
            })
        );
    }

    fn disciple_snapshot(char_id: &str, pos: DVec3) -> NpcDormantSnapshot {
        let cultivation = Cultivation {
            realm: Realm::Awaken,
            qi_current: 10.0,
            qi_max: 100.0,
            ..Default::default()
        };
        NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Disciple,
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
            faction: Some(crate::npc::faction::FactionMembership {
                faction_id: crate::npc::faction::FactionId::Attack,
                rank: FactionRank::Disciple,
                reputation: crate::npc::faction::Reputation::default(),
                lineage: None,
                mission_queue: crate::npc::faction::MissionQueue::default(),
            }),
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
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
            pending_combat_winner: None,
        }
    }

    /// Helper: build a real (non-fallback) SignedSkin for testing.
    fn real_signed_skin(label: &str) -> crate::skin::SignedSkin {
        crate::skin::SignedSkin {
            value: format!("value-{label}"),
            signature: format!("sig-{label}"),
            source: crate::skin::SkinSource::MineSkinRandom {
                hash: label.to_string(),
            },
        }
    }

    /// Helper: create a SkinPool pre-loaded with real skins for every
    /// PREFETCH_KEYS bucket so that `next_for_profile` returns a non-fallback
    /// skin regardless of the resolved pool key.
    fn pool_with_real_skins() -> crate::skin::SkinPool {
        let mut pool = crate::skin::SkinPool::default();
        for key in crate::skin::npc_skin_selector::NpcSkinPoolKey::PREFETCH_KEYS {
            for i in 0..3 {
                pool.insert_for_key(key, real_signed_skin(&format!("{}-{i}", key.as_str())));
            }
        }
        pool
    }

    #[test]
    fn hydrate_disciple_with_skin_pool_attaches_player_skin_and_player_kind() {
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let snap = disciple_snapshot("disciple_with_skin", DVec3::new(10.0, 64.0, 10.0));
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);

        let pool = pool_with_real_skins();
        app.insert_resource(pool);

        // Place a player nearby so the hydrate system picks up the NPC.
        app.world_mut().spawn((
            valence::client::ClientMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
        ));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        // The dormant store should be drained.
        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "disciple snapshot should have been consumed from dormant store"
        );

        // Find the hydrated entity.
        let (entity_kind, has_npc_skin) = {
            let world = app.world_mut();
            let mut query = world.query::<(
                &valence::prelude::EntityKind,
                Option<&crate::skin::NpcPlayerSkin>,
            )>();
            let results: Vec<_> = query
                .iter(world)
                .filter(|(kind, _)| **kind == valence::prelude::EntityKind::PLAYER)
                .collect();
            // There should be at least one PLAYER entity (the hydrated NPC);
            // the spawned ClientMarker test entity does not get EntityKind.
            assert!(
                !results.is_empty(),
                "expected at least one PLAYER entity after hydrating disciple with real skin pool"
            );
            let (kind, skin) = results[0];
            (*kind, skin.is_some())
        };
        assert_eq!(
            entity_kind,
            valence::prelude::EntityKind::PLAYER,
            "hydrated disciple with real skins should be PLAYER, got {:?}",
            entity_kind
        );
        assert!(
            has_npc_skin,
            "hydrated disciple with real skins must have NpcPlayerSkin component"
        );
    }

    #[test]
    fn hydrate_disciple_without_skin_pool_falls_back_no_npc_player_skin() {
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let snap = disciple_snapshot("disciple_no_skin", DVec3::new(10.0, 64.0, 10.0));
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);
        // No SkinPool resource inserted — skin_pool is None.

        app.world_mut().spawn((
            valence::client::ClientMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
        ));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "disciple snapshot should have been consumed even without skin pool"
        );

        // All hydrated NPCs should fall back — no NpcPlayerSkin, VILLAGER kind.
        let results = {
            let world = app.world_mut();
            let mut query = world.query::<(
                &valence::prelude::EntityKind,
                Option<&crate::skin::NpcPlayerSkin>,
                &NpcArchetype,
            )>();
            query
                .iter(world)
                .filter(|(_, _, arch)| **arch == NpcArchetype::Disciple)
                .map(|(kind, skin, _)| (*kind, skin.is_some()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            results.len(),
            1,
            "expected exactly one hydrated disciple entity"
        );
        let (kind, has_skin) = results[0];
        assert_eq!(
            kind,
            valence::prelude::EntityKind::VILLAGER,
            "hydrated disciple without skin pool should fall back to VILLAGER, got {:?}",
            kind
        );
        assert!(
            !has_skin,
            "hydrated disciple without skin pool must NOT have NpcPlayerSkin component"
        );
    }

    #[test]
    fn hydrate_preserves_non_default_snapshot_realm_regardless_of_bundle_default() {
        // plan-npc-realm-distribution-v1 P0 R2 骨架（不是 P1 分布测试）：
        // spawn_from_snapshot（hydrate/mod.rs:720-724）用 `snapshot.cultivation`
        // 整体覆盖 spawn_disciple_npc_at 内部 bundle 算出的 Cultivation ——
        // P0 choke-point 修复对 hydrate 路径零效果，这是既有行为不是本 plan 引入。
        // 本测试手工构造非默认 realm 快照（不依赖尚未落地的 P1 seeder产出的分布），
        // 锁定"snapshot 内容保真"这条真实回归线，不是恒真的"hydrate 不丢 realm"。
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let mut snap = disciple_snapshot("disciple_solidify", DVec3::new(20.0, 64.0, 20.0));
        snap.cultivation.realm = Realm::Solidify;
        snap.shared_lifespan = LifespanComponent::for_realm(Realm::Solidify);
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);

        app.world_mut()
            .spawn((ClientMarker, Position(DVec3::new(20.0, 64.0, 20.0))));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        let hit = {
            let world = app.world_mut();
            let mut query = world.query::<(&NpcArchetype, &Cultivation)>();
            query
                .iter(world)
                .find(|(archetype, _)| **archetype == NpcArchetype::Disciple)
                .map(|(_, cultivation)| cultivation.realm)
        };
        assert_eq!(
            hit,
            Some(Realm::Solidify),
            "hydrate 必须原样保留快照 realm(Solidify)，不受 spawn_disciple_npc_at \
             内部 bundle 默认值影响"
        );
    }

    #[test]
    fn hydrate_mundane_snapshot_produces_entity_with_species_marker() {
        // plan-mundane-fauna-v1：dormant→hydrate round-trip 走 spawn_from_snapshot 的
        // NpcArchetype::Mundane 分支（hydrate/mod.rs:724 → spawn_mundane_fauna_at）——复活的
        // 凡兽必须重新挂上 `MundaneFaunaSpecies`，否则 qi_regen 的 `Without<MundaneFaunaSpecies>`
        // 豁免在复活后失效，凡兽会重新开始抽 zone 灵气破守恒。此测试锁死新 enum 变体状态转换。
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let mut snap = snapshot("mundane_rabbit_dormant", DVec3::new(20.0, 64.0, 20.0));
        snap.archetype = NpcArchetype::Mundane;
        snap.cultivation.realm = Realm::Awaken;
        snap.shared_lifespan = LifespanComponent::for_realm(Realm::Awaken);
        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);

        app.world_mut()
            .spawn((ClientMarker, Position(DVec3::new(20.0, 64.0, 20.0))));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(
            &NpcArchetype,
            Option<&crate::fauna::mundane::MundaneFaunaSpecies>,
        )>();
        let mundane = query
            .iter(world)
            .find(|(archetype, _)| **archetype == NpcArchetype::Mundane);
        assert!(
            mundane.is_some(),
            "hydrate 必须产出一个 NpcArchetype::Mundane 实体"
        );
        assert!(
            mundane.unwrap().1.is_some(),
            "复活的凡兽必须重新携带 MundaneFaunaSpecies——否则 qi_regen 豁免在 hydrate 后失效"
        );
    }

    #[test]
    fn home_base_uses_scatter_position_not_zone_center() {
        const ARRIVAL_DISTANCE: f64 = 1.8;

        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let scatter_pos = DVec3::new(-1600.0, 101.0, 3980.0);
        let zone_center = DVec3::new(-2500.0, 128.0, 2500.0);
        let mut snap = snapshot("edge_rogue", scatter_pos);
        snap.patrol = Some(DormantPatrolSnapshot {
            home_zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            anchor_index: 0,
            current_target: vec3_to_array(zone_center),
        });

        let mut store = NpcDormantStore::default();
        store.insert(snap);
        app.insert_resource(store);
        app.world_mut().spawn((
            ClientMarker,
            Position(scatter_pos + DVec3::new(1.0, 0.0, 1.0)),
        ));

        app.add_systems(Update, hydrate_dormant_near_players_system);
        app.update();

        let (position, home, patrol) = {
            let world = app.world_mut();
            let mut query =
                world.query::<(&Position, &crate::npc::schedule::NpcHomeBase, &NpcPatrol)>();
            query
                .iter(world)
                .next()
                .map(|(pos, home, patrol)| (pos.get(), *home, patrol.current_target))
                .expect("edge dormant rogue should hydrate")
        };

        assert!(
            home.center().distance(scatter_pos) <= ARRIVAL_DISTANCE,
            "home_base must be anchored at the NPC scatter position; home={:?}, scatter={:?}",
            home.center(),
            scatter_pos,
        );
        assert!(
            position.distance(scatter_pos) <= ARRIVAL_DISTANCE,
            "non-rest hydrate position should remain at scatter position; pos={position:?}, scatter={scatter_pos:?}"
        );
        assert!(
            home.center().distance(zone_center) > 800.0,
            "home_base must not keep using the far zone center; home={:?}, center={zone_center:?}",
            home.center(),
        );
        assert_eq!(
            patrol, zone_center,
            "patrol target should remain the original zone center; this plan only decouples home"
        );
    }

    #[test]
    fn dormant_edge_seed_returns_home_without_astar_flood() {
        // Regression: after the npc-return-home-freeze fix, home_base is set to
        // the scatter position (not the far zone center). This test verifies that
        // the return-home action actually exercises navigator_tick_system (real
        // pathfinding runs) and that the NPC navigates toward its local scatter
        // home WITHOUT triggering repeated A* failure backoff (the "站桩 / flood"
        // bug).
        //
        // The "pre-fix" behaviour had home_base == zone_center, which is hundreds
        // of blocks away and beyond MAX_PATH_ITERS → every A* attempt fails →
        // consecutive_path_failures grows rapidly. This test catches that by
        // asserting failures == 0 on flat terrain.
        //
        // Setup: flat stone ground at y=66 in chunk [0,0]. NPC home anchored at
        // (8,67,8); NPC displaced to (0.5,67,0.5) — 11.3 blocks away — simulating
        // "wandered away after combat". navigator_tick_system is registered so real
        // pathfinding runs every tick.
        //
        // NOTE ON ARRIVAL: the navigator's GOAL_REACH_XZ=2 "fuzzy-arrival" zone
        // causes A* to terminate at the cheapest block within 2 of the target
        // (not at target_block itself), leaving the NPC ~2.8 blocks short of
        // ARRIVAL_DISTANCE=1.8. Full arrival therefore cannot be asserted here in
        // a unit harness. The test instead verifies meaningful progress and zero
        // A* failures — sufficient to lock the regression.
        use bevy_transform::components::Transform;
        use valence::entity::{HeadYaw, Look};
        use valence::prelude::{BlockState, Chunk, ChunkLayer, UnloadedChunk};
        use valence::testing::ScenarioSingleClient;

        const GROUND_Y: i32 = 66;
        const WALK_Y: f64 = 67.0;
        // 120 ticks = enough for the NPC to travel from 11.3 → ~3 blocks from
        // home on flat terrain (bucket/repath overhead included). Well under
        // RETURN_HOME_MAX_TICKS=300 so no timeout.
        const NAVIGATE_TICKS: u32 = 120;
        // The NPC must close more than half the initial gap to prove real navigation
        // (not just a 1-block shuffle). With GOAL_REACH_XZ=2 gap, the navigator
        // will stop ~2–3 blocks from home center; threshold set conservatively.
        const PROGRESS_FRACTION: f64 = 0.5;
        // arrival threshold (from brain::RETURN_HOME_ARRIVAL_DISTANCE)
        const ARRIVAL_DISTANCE: f64 = 1.8;

        let home_world_pos = DVec3::new(8.0, WALK_Y, 8.0);
        let displaced_pos = DVec3::new(0.5, WALK_Y, 0.5);
        let home_base = crate::npc::schedule::NpcHomeBase::from_world_pos(home_world_pos, 0.6);
        let home_center = home_base.center();
        let initial_distance = displaced_pos.distance(home_center);

        // Sanity: the displaced position must be well beyond the arrival threshold
        // so the first Executing tick does NOT shortcircuit into the "arrived" branch.
        assert!(
            initial_distance > ARRIVAL_DISTANCE + 5.0,
            "test setup broken: displaced pos must be far enough from home to force navigation; \
             initial_distance={initial_distance:.2}, arrival_threshold={ARRIVAL_DISTANCE}",
        );

        // Build an app with a real ChunkLayer (needed by navigator_tick_system for
        // collision and ground-snap). ScenarioSingleClient is the same harness used
        // by navigator::tests::make_navigator_app_with_ground.
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);

        // Fill chunk [0,0] with flat stone at GROUND_Y so A* finds a clean path
        // and consecutive_path_failures stays 0.
        {
            let layer_entity = {
                let world = app.world_mut();
                let mut q = world.query_filtered::<Entity, With<ChunkLayer>>();
                q.iter(world).next().unwrap()
            };
            let mut layer = app.world_mut().get_mut::<ChunkLayer>(layer_entity).unwrap();
            let mut chunk = UnloadedChunk::with_height(384);
            let min_y = layer.min_y();
            let local_y = (GROUND_Y - min_y) as u32;
            for lx in 0..16u32 {
                for lz in 0..16u32 {
                    chunk.set_block_state(lx, local_y, lz, BlockState::STONE);
                }
            }
            layer.insert_chunk([0, 0], chunk);
        }

        // Spawn the NPC at the displaced position. home_base mirrors what
        // hydrate_dormant_near_players_system sets for an edge-zone 散修:
        // scatter position as home, NOT the far zone center.
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(displaced_pos),
                Transform::default(),
                Look::default(),
                HeadYaw::default(),
                crate::npc::navigator::Navigator::new(),
                home_base,
                crate::npc::brain::RestState::default(),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                crate::npc::brain::ReturnHomeAction,
                big_brain::prelude::Actor(npc),
                big_brain::prelude::ActionState::Requested,
            ))
            .id();

        // Both systems MUST be registered:
        //   - navigator_tick_system: runs A* and advances Position each tick
        //   - return_home_action_system: drives the navigator goal toward home
        // Without navigator_tick_system the NPC never moves (original fake-green bug).
        //
        // GameTick must advance so should_repath_in_bucket cycles through all
        // 10 buckets — the NPC entity's bucket fires within the first 10 ticks.
        app.insert_resource(GameTick(0));
        app.add_systems(
            Update,
            (
                crate::npc::navigator::navigator_tick_system,
                return_home_action_system,
            ),
        );

        for i in 0..NAVIGATE_TICKS {
            app.world_mut().resource_mut::<GameTick>().0 = i;
            app.update();
        }

        let final_pos = app.world().get::<Position>(npc).unwrap().get();
        let final_distance = final_pos.distance(home_center);
        let failures = app
            .world()
            .get::<crate::npc::navigator::Navigator>(npc)
            .unwrap()
            .consecutive_path_failures_for_test();

        // Primary regression guard: A* must NOT flood on flat terrain. The old
        // bug caused a zone-center goal ~500+ blocks away → every A* attempt
        // exceeded MAX_PATH_ITERS → failures grew unboundedly.
        assert_eq!(
            failures, 0,
            "edge return-home must not enter A* failure backoff on flat terrain; \
             {failures} failure(s) recorded — indicates repeated A* flood / 站桩 regression \
             (did home_base revert to using the far zone center instead of scatter pos?)",
        );

        // Secondary guard: the NPC must have actually navigated toward home,
        // not stayed put. Proves navigator_tick_system was exercised.
        // NOTE: due to navigator GOAL_REACH_XZ=2 fuzzy-arrival, the NPC will stop
        // ~2-3 blocks short of home center (not within ARRIVAL_DISTANCE=1.8).
        // We assert >50% gap closure as a meaningful lower bound.
        let min_expected_progress = initial_distance * PROGRESS_FRACTION;
        assert!(
            final_distance < initial_distance - min_expected_progress,
            "edge rogue must navigate at least {min_expected_progress:.1} blocks toward home \
             within {NAVIGATE_TICKS} ticks; started {initial_distance:.1} blocks away, \
             ended {final_distance:.1} blocks away at {final_pos:?} \
             (home={home_center:?}) — was navigator_tick_system actually running?",
        );

        // Confirm the action did NOT time out or fail: it should still be Executing
        // (the NPC is navigating toward home, just not arrived yet in this window).
        let action_state = app
            .world()
            .get::<big_brain::prelude::ActionState>(action)
            .unwrap();
        assert!(
            matches!(
                action_state,
                big_brain::prelude::ActionState::Executing
                    | big_brain::prelude::ActionState::Success
            ),
            "ReturnHomeAction must be Executing or Success after {NAVIGATE_TICKS} ticks, \
             got {action_state:?} — Failure here means the navigator could not navigate \
             toward home (check home_base is scatter-anchored, not zone-center)",
        );
    }

    #[test]
    fn home_base_tracks_extreme_zone_positions_and_center_degenerate_case() {
        let center = DVec3::new(-2_500.0, 128.0, -2_500.0);
        let positions = [
            DVec3::new(-3_000.0, 64.0, -3_000.0),
            DVec3::new(-2_000.0, 64.0, -3_000.0),
            DVec3::new(-3_000.0, 64.0, -2_000.0),
            DVec3::new(-2_000.0, 64.0, -2_000.0),
            center,
        ];

        for (index, pos) in positions.into_iter().enumerate() {
            let mut app = App::new();
            app.add_event::<InitiateXuhuaTribulation>();

            let overworld = app.world_mut().spawn_empty().id();
            let tsy = app.world_mut().spawn_empty().id();
            app.insert_resource(DimensionLayers { overworld, tsy });
            app.insert_resource(NpcVirtualizationConfig::default());

            let mut snap = snapshot_in_zone(&format!("edge_case_{index}"), pos, "zone_b");
            snap.patrol = Some(DormantPatrolSnapshot {
                home_zone: "zone_b".to_string(),
                anchor_index: 0,
                current_target: vec3_to_array(center),
            });
            let mut store = NpcDormantStore::default();
            store.insert(snap);
            app.insert_resource(store);
            app.world_mut().spawn((ClientMarker, Position(pos)));

            app.add_systems(Update, hydrate_dormant_near_players_system);
            app.update();

            let home = {
                let world = app.world_mut();
                let mut query =
                    world.query_filtered::<&crate::npc::schedule::NpcHomeBase, With<NpcMarker>>();
                *query
                    .iter(world)
                    .next()
                    .expect("extreme zone dormant rogue should hydrate")
            };

            assert!(
                home.center().distance(pos) <= 1.8,
                "case {index}: home should track snapshot position, home={:?}, pos={pos:?}",
                home.center(),
            );
        }
    }

    #[test]
    fn tribulation_ready_dormant_hydrates_without_player_distance_gate() {
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig::default());

        let mut ready = snapshot("npc_ready", DVec3::new(10.0, 64.0, 10.0));
        open_all_meridians(&mut ready);
        let mut store = NpcDormantStore::default();
        store.insert(ready);
        app.insert_resource(store);
        app.add_systems(Update, hydrate_dormant_near_players_system);

        app.update();

        assert!(app.world().resource::<NpcDormantStore>().is_empty());
        let profiles = {
            let world = app.world_mut();
            let mut query = world.query::<&crate::skin::NpcVisualProfile>();
            query.iter(world).copied().collect::<Vec<_>>()
        };
        assert_eq!(profiles.len(), 1);
        assert_eq!(
            profiles[0].skin_tier,
            crate::skin::npc_skin_selector::NpcSkinTier::RogueHigh,
            "hydrating a Spirit rogue should keep the high-realm skin pool profile"
        );
        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all = events.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].waves_total, NPC_TRIBULATION_WAVES_DEFAULT);
    }

    fn multi_zone_registry() -> ZoneRegistry {
        ZoneRegistry {
            zones: vec![
                Zone {
                    name: "zone_a".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(4000.0, -64.0, 1000.0),
                        DVec3::new(5200.0, 320.0, 2200.0),
                    ),
                    spirit_qi: 0.5,
                    danger_level: 2,
                    active_events: Vec::new(),
                    patrol_anchors: vec![DVec3::new(4600.0, 128.0, 1600.0)],
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
                Zone {
                    name: "zone_b".to_string(),
                    dimension: DimensionKind::Overworld,
                    bounds: (
                        DVec3::new(-3000.0, -64.0, -3000.0),
                        DVec3::new(-2000.0, 320.0, -2000.0),
                    ),
                    spirit_qi: 0.6,
                    danger_level: 3,
                    active_events: Vec::new(),
                    patrol_anchors: vec![DVec3::new(-2500.0, 64.0, -2500.0)],
                    blocked_tiles: Vec::new(),
                    qi_equilibrium: 0.0,
                    qi_inflow_per_min: 0.0,
                },
            ],
        }
    }

    fn snapshot_in_zone(char_id: &str, pos: DVec3, zone: &str) -> NpcDormantSnapshot {
        let mut s = snapshot(char_id, pos);
        s.zone_name = zone.to_string();
        s
    }

    #[test]
    fn zone_based_hydration_wakes_distant_npcs_in_same_zone() {
        let player_positions: Vec<PlayerPosition> =
            vec![(DimensionKind::Overworld, DVec3::new(4600.0, 128.0, 1600.0))];
        let zones = multi_zone_registry();

        let player_zones = player_zone_names(Some(&zones), &player_positions);
        assert!(
            player_zones.contains("zone_a"),
            "player at zone_a center must resolve to zone_a"
        );

        let far_npc = snapshot_in_zone("npc_far", DVec3::new(4100.0, 64.0, 1100.0), "zone_a");
        let dist =
            crate::npc::dormant::planar_distance(far_npc.position_vec(), player_positions[0].1);
        assert!(
            dist > HYDRATE_RADIUS_BLOCKS,
            "NPC must be beyond hydrate_radius ({dist:.0} > {HYDRATE_RADIUS_BLOCKS})"
        );
        assert!(
            player_zones.contains(far_npc.zone_name.as_str()),
            "NPC in zone_a should match player's zone"
        );
    }

    #[test]
    fn zone_based_hydration_ignores_npcs_in_other_zones() {
        let player_positions: Vec<PlayerPosition> =
            vec![(DimensionKind::Overworld, DVec3::new(4600.0, 128.0, 1600.0))];
        let zones = multi_zone_registry();
        let player_zones = player_zone_names(Some(&zones), &player_positions);

        let other_zone_npc =
            snapshot_in_zone("npc_other", DVec3::new(-2500.0, 64.0, -2500.0), "zone_b");
        assert!(
            !player_zones.contains(other_zone_npc.zone_name.as_str()),
            "NPC in zone_b should NOT match player in zone_a"
        );
    }

    #[test]
    fn player_zone_names_returns_empty_without_registry() {
        let positions: Vec<PlayerPosition> =
            vec![(DimensionKind::Overworld, DVec3::new(0.0, 64.0, 0.0))];
        let result = player_zone_names(None, &positions);
        assert!(result.is_empty());
    }

    #[test]
    fn dehydrate_skips_npcs_in_player_zone() {
        let player_positions: Vec<PlayerPosition> =
            vec![(DimensionKind::Overworld, DVec3::new(4600.0, 128.0, 1600.0))];
        let zones = multi_zone_registry();
        let player_zones = player_zone_names(Some(&zones), &player_positions);

        let far_same_zone_npc =
            snapshot_in_zone("npc_far_same", DVec3::new(4100.0, 64.0, 1100.0), "zone_a");
        let dist = crate::npc::dormant::planar_distance(
            far_same_zone_npc.position_vec(),
            player_positions[0].1,
        );
        assert!(
            dist > DEHYDRATE_RADIUS_BLOCKS,
            "test NPC must exceed dehydrate_radius to verify zone exemption"
        );
        assert!(
            player_zones.contains(far_same_zone_npc.zone_name.as_str()),
            "same-zone NPC should be protected from dehydration"
        );

        let far_other_zone_npc = snapshot_in_zone(
            "npc_far_other",
            DVec3::new(-2500.0, 64.0, -2500.0),
            "zone_b",
        );
        assert!(
            !player_zones.contains(far_other_zone_npc.zone_name.as_str()),
            "different-zone NPC should be eligible for dehydration"
        );
    }

    // ─── plan-halfstep-rechallenge-integration-v1 P2 tests ───────────────────

    /// 构造最小 App：注入 DimensionLayers + NpcDormantStore + Events，添加
    /// `hydrate_dormant_on_rechallenge_trigger` 系统。
    fn rechallenge_trigger_app(store: NpcDormantStore) -> App {
        let mut app = App::new();
        app.add_event::<HalfStepRechallengeTriggerEvent>();
        app.add_event::<InitiateXuhuaTribulation>();

        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(store);
        app.add_systems(Update, hydrate_dormant_on_rechallenge_trigger);
        app
    }

    fn dormant_halfstep_snapshot(char_id: &str) -> NpcDormantSnapshot {
        let cultivation = crate::cultivation::components::Cultivation {
            realm: crate::cultivation::components::Realm::Spirit,
            qi_current: 900.0,
            qi_max: 1000.0,
            ..Default::default()
        };
        let mut s = NpcDormantSnapshot {
            char_id: char_id.to_string(),
            archetype: NpcArchetype::Rogue,
            dimension: DimensionKind::Overworld,
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            position: vec3_to_array(DVec3::new(10.0, 64.0, 10.0)),
            schedule_seed: None,
            cultivation: cultivation.clone(),
            meridian_system: crate::cultivation::components::MeridianSystem::default(),
            meridian_severed:
                crate::cultivation::meridian::severed::MeridianSeveredPermanent::default(),
            contamination: crate::cultivation::components::Contamination::default(),
            lifespan: crate::npc::lifecycle::NpcLifespan::new(0.0, 1_000.0),
            shared_lifespan: crate::cultivation::lifespan::LifespanComponent::for_realm(
                cultivation.realm,
            ),
            lifespan_extension_ledger:
                crate::cultivation::lifespan::LifespanExtensionLedger::default(),
            death_registry: crate::cultivation::lifespan::DeathRegistry::new(char_id),
            life_record: crate::cultivation::life_record::LifeRecord::new(char_id),
            memory: None,
            player_reputation: None,
            faction: None,
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            tsy_sentinel: None,
            intent: crate::npc::dormant::DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
            pending_combat_winner: None,
        };
        // Open all meridians so dormant_tribulation_ready passes
        for meridian in s.meridian_system.iter_mut() {
            meridian.opened = true;
        }
        s
    }

    fn send_rechallenge_event(app: &mut App, char_id: &str, is_dormant: bool) {
        use valence::prelude::Entity;
        app.world_mut()
            .resource_mut::<Events<HalfStepRechallengeTriggerEvent>>()
            .send(HalfStepRechallengeTriggerEvent {
                char_id: char_id.to_string(),
                entity: Entity::PLACEHOLDER,
                is_dormant,
                at_tick: 0,
            });
    }

    /// P2 happy path: dormant NPC 收到 trigger(is_dormant=true) → 从 store 移除 + entity 创建
    /// (不为 DespawnMarker/non-NpcMarker) + InitiateXuhuaTribulation 发出。
    #[test]
    fn rechallenge_trigger_dormant_hydrates_and_sends_tribulation() {
        let mut store = NpcDormantStore::default();
        store.insert(dormant_halfstep_snapshot("npc_rechallenge_dormant"));
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_rechallenge_dormant", true);
        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "dormant snapshot must be removed from NpcDormantStore after rechallenge trigger; \
             store still contains entries"
        );

        // InitiateXuhuaTribulation must have been emitted
        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            1,
            "exactly one InitiateXuhuaTribulation must be emitted after dormant rechallenge trigger; \
             got {} events",
            all.len()
        );
        assert_eq!(
            all[0].waves_total, NPC_TRIBULATION_WAVES_DEFAULT,
            "hydrated rechallenge NPC must use NPC_TRIBULATION_WAVES_DEFAULT; \
             expected {NPC_TRIBULATION_WAVES_DEFAULT}, got {}",
            all[0].waves_total
        );
    }

    #[test]
    fn rechallenge_trigger_rejects_divergent_durable_identity_without_removal() {
        let mut store = NpcDormantStore::default();
        let mut snapshot = dormant_halfstep_snapshot("npc_bad_rechallenge_identity");
        snapshot.life_record = LifeRecord::new("npc_other_owner");
        store.insert(snapshot);
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_bad_rechallenge_identity", true);
        app.update();

        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .contains("npc_bad_rechallenge_identity"),
            "identity preflight failure must retain the dormant owner for repair"
        );
        assert!(
            app.world()
                .resource::<Events<InitiateXuhuaTribulation>>()
                .iter_current_update_events()
                .next()
                .is_none(),
            "rejected identity must not emit a tribulation for a fabricated live owner"
        );
        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(
            npc_count, 0,
            "rejected rechallenge identity must not spawn a live NPC"
        );
    }

    /// P2 non-dormant event must NOT trigger hydrate: is_dormant=false 走玩家路径，
    /// store 保持不变，不发 InitiateXuhuaTribulation。
    #[test]
    fn rechallenge_trigger_not_dormant_skips_hydrate() {
        let mut store = NpcDormantStore::default();
        store.insert(dormant_halfstep_snapshot("npc_live"));
        let mut app = rechallenge_trigger_app(store);

        // Send event with is_dormant=false (live entity path, not dormant)
        send_rechallenge_event(&mut app, "npc_live", false);
        app.update();

        assert!(
            !app.world().resource::<NpcDormantStore>().is_empty(),
            "is_dormant=false event must not remove snapshot from NpcDormantStore; \
             store was incorrectly drained"
        );

        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            0,
            "is_dormant=false rechallenge trigger must NOT emit InitiateXuhuaTribulation; \
             got {} events",
            all.len()
        );
    }

    /// P2 store 中无该 char_id 时安全跳过（entry 已被邻近 hydrate 预先消费）。
    #[test]
    fn rechallenge_trigger_missing_char_id_safe_skip() {
        // Empty store — no snapshot for "npc_gone"
        let store = NpcDormantStore::default();
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_gone", true);
        // Must not panic, must not emit tribulation
        app.update();

        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            0,
            "missing char_id in dormant store must produce 0 InitiateXuhuaTribulation events; \
             got {}",
            all.len()
        );
    }

    /// P2 e2e 集成测试：dormant HalfStep NPC 入 HalfStepRechallengeQueue →
    /// AscensionQuotaOpened → dispatch_rechallenge_on_quota_opened_system emit trigger(is_dormant=true)
    /// → hydrate_dormant_on_rechallenge_trigger 消费 → NpcDormantStore 移除 + entity spawn +
    /// InitiateXuhuaTribulation 发出。单 App 一个 update 链路跑通（确认无 Redis 回环）。
    #[test]
    fn e2e_dormant_halfstep_rechallenge_full_chain_no_redis() {
        use crate::combat::CombatClock;
        use crate::cultivation::tribulation::{
            AscensionQuotaOpened, HalfStepRechallengeEntry, HalfStepRechallengeQueue,
            HalfStepRechallengeTriggerEvent, RECHALLENGE_WINDOW_TICKS,
        };

        let mut store = NpcDormantStore::default();
        let snap = dormant_halfstep_snapshot("npc_e2e_dormant");
        store.insert(snap);

        let mut app = App::new();
        // -- Events --
        app.add_event::<AscensionQuotaOpened>();
        app.add_event::<HalfStepRechallengeTriggerEvent>();
        app.add_event::<InitiateXuhuaTribulation>();
        // -- Resources --
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(store);
        app.insert_resource(CombatClock { tick: 100 });
        // Pre-populate queue with dormant NPC entry (is_dormant=true)
        let mut queue = HalfStepRechallengeQueue::default();
        queue.enqueue(HalfStepRechallengeEntry {
            char_id: "npc_e2e_dormant".to_string(),
            entity: valence::prelude::Entity::PLACEHOLDER,
            entered_at: 50,
            rechallenge_window_until: 50 + RECHALLENGE_WINDOW_TICKS,
            is_dormant: true,
            buff_applied: false,
        });
        app.insert_resource(queue);
        // -- Systems: dispatch_rechallenge (cultivation) then hydrate (npc) --
        app.add_systems(
            Update,
            (
                crate::cultivation::tribulation::dispatch_rechallenge_on_quota_opened_system,
                hydrate_dormant_on_rechallenge_trigger,
            )
                .chain(),
        );

        // Fire AscensionQuotaOpened event — kicks off the full chain
        app.world_mut()
            .resource_mut::<Events<AscensionQuotaOpened>>()
            .send(AscensionQuotaOpened { occupied_slots: 0 });

        app.update();

        // 1) NpcDormantStore must be empty — snapshot was consumed by hydrate
        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "e2e: NpcDormantStore must be empty after full rechallenge chain; \
             AscensionQuotaOpened → dispatch trigger → hydrate removed snapshot"
        );

        // 2) At least one NpcMarker entity must exist (hydrated NPC)
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert!(
            npc_count >= 1,
            "e2e: at least 1 NpcMarker entity must be spawned after rechallenge-triggered hydrate; \
             found {npc_count}"
        );

        // 3) InitiateXuhuaTribulation must have been emitted (dormant NPC enters tribulation)
        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            1,
            "e2e: exactly 1 InitiateXuhuaTribulation must be emitted via full rechallenge chain \
             (dormant→hydrate→tribulation); got {}",
            all.len()
        );
        assert_eq!(
            all[0].waves_total, NPC_TRIBULATION_WAVES_DEFAULT,
            "e2e: tribulation waves must equal NPC_TRIBULATION_WAVES_DEFAULT={}; got {}",
            NPC_TRIBULATION_WAVES_DEFAULT, all[0].waves_total
        );

        // 4) HalfStepRechallengeQueue must be empty — entry was consumed by dispatch system
        let queue = app.world().resource::<HalfStepRechallengeQueue>();
        assert!(
            queue.is_empty(),
            "e2e: HalfStepRechallengeQueue must be empty after dispatch consumed the dormant entry; \
             {} entries remain",
            queue.len()
        );
    }

    /// P2 hydrated entity 收 trigger 后 NpcMarker 被创建（实体存在于世界）。
    #[test]
    fn rechallenge_trigger_dormant_entity_is_spawned_into_world() {
        let mut store = NpcDormantStore::default();
        store.insert(dormant_halfstep_snapshot("npc_spawned"));
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_spawned", true);
        app.update();

        // At least one NpcMarker entity must exist after hydration
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert!(
            npc_count >= 1,
            "after rechallenge-triggered hydrate, at least 1 NpcMarker entity must exist; \
             found {npc_count}"
        );
    }

    /// P2 FIFO 顺序：多个 dormant NPC trigger 按事件顺序各自 hydrate，store 全空。
    #[test]
    fn rechallenge_trigger_multiple_dormant_all_hydrated_fifo() {
        let mut store = NpcDormantStore::default();
        store.insert(dormant_halfstep_snapshot("npc_fifo_a"));
        store.insert(dormant_halfstep_snapshot("npc_fifo_b"));
        let mut app = rechallenge_trigger_app(store);

        // Send both triggers in a single update
        send_rechallenge_event(&mut app, "npc_fifo_a", true);
        send_rechallenge_event(&mut app, "npc_fifo_b", true);
        app.update();

        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .is_empty(),
            "both dormant snapshots must be drained from NpcDormantStore after two rechallenge triggers; \
             store still has {} entries",
            app.world().resource::<NpcDormantStore>().len()
        );

        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            2,
            "two dormant rechallenge triggers must emit exactly 2 InitiateXuhuaTribulation events; \
             got {}",
            all.len()
        );
    }

    /// P2 hydrated NPC 保持 is_dormant=false 路径（NpcDormantStore 中存有快照但 trigger 为 live）——
    /// 确保 store 不被误清。
    #[test]
    fn rechallenge_trigger_live_entity_does_not_drain_store() {
        let mut store = NpcDormantStore::default();
        store.insert(dormant_halfstep_snapshot("npc_shared"));
        let store_len_before = store.len();
        let mut app = rechallenge_trigger_app(store);

        // Same char_id but is_dormant=false (live entity path)
        send_rechallenge_event(&mut app, "npc_shared", false);
        app.update();

        let store_len_after = app.world().resource::<NpcDormantStore>().len();
        assert_eq!(
            store_len_after, store_len_before,
            "is_dormant=false trigger for char_id with dormant snapshot must NOT remove it; \
             store had {store_len_before} before, has {store_len_after} after"
        );
    }

    // ─── combat_dead_pending_release 守卫 pin 测试 ─────────────────────────────
    //
    // 验收目标（Scope 结论）：
    //   1. combat_dead_pending_release=true 的快照不被 hydrate_dormant_near_players_system 水化
    //   2. 同样快照不被 hydrate_dormant_on_rechallenge_trigger 水化
    //   3. 正常（false）快照照常水化
    //   4. 混合批次：部分 combat_dead、部分正常——正常者水化，combat_dead 留 store
    //   5. 守恒：combat_dead 快照带 qi_current>0 时，世界中不出现该 entity（qi 不双计）

    /// 构造一个 combat_dead_pending_release=true 的快照（qi_current=300 残余未释放）。
    fn combat_dead_snapshot(char_id: &str) -> NpcDormantSnapshot {
        let mut s = snapshot(char_id, DVec3::new(10.0, 64.0, 10.0));
        // 模拟 zone 满、qi 未完全释放的典型状态（qi_current=300 > QI_EPSILON）。
        s.cultivation.qi_current = 300.0;
        s.combat_dead_pending_release = true;
        s
    }

    /// 构造一个完全释放（qi_current=0）的 combat_dead 快照（zone 已全额回灌），
    /// 但 pending_release 仍为 true（等 retry 系统做最终清理）。
    fn combat_dead_zero_qi_snapshot(char_id: &str) -> NpcDormantSnapshot {
        let mut s = snapshot(char_id, DVec3::new(10.0, 64.0, 10.0));
        s.cultivation.qi_current = 0.0;
        s.combat_dead_pending_release = true;
        s
    }

    /// 构造最小 App 供 hydrate_dormant_near_players_system 测试使用。
    /// 在玩家位置放置一个 ClientMarker entity，确保邻近触发条件满足。
    fn near_player_hydrate_app(store: NpcDormantStore) -> App {
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            ..Default::default()
        });
        app.insert_resource(store);
        // 放置玩家在 NPC 旁边（确保 near_player 条件成立）
        app.world_mut().spawn((
            valence::client::ClientMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
        ));
        app.add_systems(Update, hydrate_dormant_near_players_system);
        app
    }

    #[test]
    fn near_player_hydrate_rejects_divergent_durable_identity_without_removal() {
        let mut store = NpcDormantStore::default();
        let mut snapshot = snapshot("npc_bad_near_player_identity", DVec3::new(10.0, 64.0, 10.0));
        snapshot.death_registry = DeathRegistry::new("npc_other_owner");
        store.insert(snapshot);
        let mut app = near_player_hydrate_app(store);

        app.update();

        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .contains("npc_bad_near_player_identity"),
            "identity preflight failure must retain the dormant owner for repair"
        );
        let npc_count = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<NpcMarker>>();
            query.iter(world).count()
        };
        assert_eq!(
            npc_count, 0,
            "rejected near-player identity must not spawn a live NPC"
        );
    }

    /// 守卫 P1：combat_dead_pending_release=true 的快照在玩家靠近时不被水化。
    /// 期望：store 保留该 snapshot；世界中无新 NpcMarker entity；qi 不双计。
    #[test]
    fn combat_dead_near_player_skipped_by_hydrate_system() {
        let mut store = NpcDormantStore::default();
        store.insert(combat_dead_snapshot("npc_combat_dead"));
        let mut app = near_player_hydrate_app(store);
        app.update();

        // snapshot 必须留在 store——retry 系统负责收口
        assert!(
            !app.world().resource::<NpcDormantStore>().is_empty(),
            "期望 combat_dead_pending_release=true 快照留在 NpcDormantStore（不被水化）；\
             store 为空说明守卫失效，qi 已被双计"
        );
        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .snapshots
                .contains_key("npc_combat_dead"),
            "期望 NpcDormantStore 仍包含 npc_combat_dead；\
             该快照被水化 = qi 双计（战死时已 release 到 zone，复活又带真元进世界）"
        );

        // 世界中不应有 NpcMarker entity（没有 spawn）
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            npc_count, 0,
            "期望 world 中无 NpcMarker entity（combat_dead 快照不应被 spawn）；\
             实际 {npc_count} 个——说明守卫未阻止 spawn，qi 已双计"
        );
    }

    /// 守卫 P2：qi_current=0 的 combat_dead 快照同样不被水化（qi=0 的逻辑死亡者不该复活）。
    #[test]
    fn combat_dead_zero_qi_near_player_still_skipped() {
        let mut store = NpcDormantStore::default();
        store.insert(combat_dead_zero_qi_snapshot("npc_dead_zero"));
        let mut app = near_player_hydrate_app(store);
        app.update();

        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .snapshots
                .contains_key("npc_dead_zero"),
            "期望 qi_current=0 的 combat_dead 快照仍留 store；\
             即使 qi=0 也不应水化（逻辑上已死，等 retry finalize 清理）"
        );
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            npc_count, 0,
            "qi=0 的 combat_dead 快照不应 spawn NpcMarker；实际 {npc_count} 个"
        );
    }

    /// 守卫 P3：combat_dead_pending_release=false（正常）快照照常水化（守卫不破坏正常路径）。
    #[test]
    fn normal_snapshot_still_hydrates_when_near_player() {
        let mut store = NpcDormantStore::default();
        let normal = snapshot("npc_normal", DVec3::new(10.0, 64.0, 10.0));
        assert!(
            !normal.combat_dead_pending_release,
            "test setup: normal snapshot must have combat_dead_pending_release=false"
        );
        store.insert(normal);
        let mut app = near_player_hydrate_app(store);
        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "期望正常（combat_dead=false）快照在玩家靠近时被水化（从 store 移除）；\
             store 仍有内容说明守卫误拦截了正常路径"
        );
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert!(
            npc_count >= 1,
            "期望正常快照水化后有 NpcMarker entity；实际 {npc_count} 个"
        );
    }

    /// 守卫 P4：混合批次——combat_dead 快照留 store，正常快照被水化。
    #[test]
    fn mixed_batch_combat_dead_stays_normal_hydrates() {
        let mut store = NpcDormantStore::default();
        store.insert(combat_dead_snapshot("npc_dead_mix"));
        store.insert(snapshot("npc_alive_mix", DVec3::new(10.0, 64.0, 10.0)));
        let mut app = near_player_hydrate_app(store);
        app.update();

        let store_after = app.world().resource::<NpcDormantStore>();
        assert!(
            store_after.snapshots.contains_key("npc_dead_mix"),
            "混合批次：npc_dead_mix（combat_dead=true）必须留在 store；\
             若被水化则 qi 双计"
        );
        assert!(
            !store_after.snapshots.contains_key("npc_alive_mix"),
            "混合批次：npc_alive_mix（正常）必须已从 store 移除（已水化）；\
             若仍在 store 则守卫误拦截"
        );

        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            npc_count, 1,
            "混合批次：world 中应有且仅有 1 个 NpcMarker（来自正常快照）；\
             实际 {npc_count} 个（期望非 0 且非 2，因 combat_dead 不应 spawn）"
        );
    }

    /// 守卫 P5（rechallenge 路径）：combat_dead_pending_release=true 的快照收到
    /// rechallenge trigger(is_dormant=true) 时不被水化，store 保持不变。
    #[test]
    fn rechallenge_trigger_skips_combat_dead_snapshot() {
        let mut store = NpcDormantStore::default();
        store.insert(combat_dead_snapshot("npc_dead_rechallenge"));
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_dead_rechallenge", true);
        app.update();

        // snapshot 必须留在 store——retry 系统负责收口，rechallenge 路径不应绕过守卫
        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .snapshots
                .contains_key("npc_dead_rechallenge"),
            "期望 combat_dead_pending_release=true 快照在 rechallenge trigger 后仍留 store；\
             被移除 = retry 系统的收口被绕过，qi 可能双计"
        );

        // 不应 emit InitiateXuhuaTribulation
        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            0,
            "combat_dead 快照被 rechallenge trigger 时不应发 InitiateXuhuaTribulation；\
             逻辑死亡的 NPC 不该再度进入渡劫（got {0} events）",
            all.len()
        );

        // 世界中不应有 NpcMarker
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            npc_count, 0,
            "combat_dead rechallenge 路径不应 spawn NpcMarker；实际 {npc_count} 个"
        );
    }

    /// 守卫 P6（守恒断言）：混合 rechallenge——combat_dead 快照不被水化，正常 halfstep 快照正常水化。
    #[test]
    fn rechallenge_trigger_mixed_combat_dead_stays_normal_hydrates() {
        let mut store = NpcDormantStore::default();
        store.insert(combat_dead_snapshot("npc_dead_rch"));
        store.insert(dormant_halfstep_snapshot("npc_live_rch"));
        let mut app = rechallenge_trigger_app(store);

        send_rechallenge_event(&mut app, "npc_dead_rch", true);
        send_rechallenge_event(&mut app, "npc_live_rch", true);
        app.update();

        let store_after = app.world().resource::<NpcDormantStore>();
        assert!(
            store_after.snapshots.contains_key("npc_dead_rch"),
            "rechallenge 混合批次：npc_dead_rch（combat_dead=true）必须留 store；\
             retry 系统持有其 qi 收口权"
        );
        assert!(
            !store_after.snapshots.contains_key("npc_live_rch"),
            "rechallenge 混合批次：npc_live_rch（正常 halfstep）必须被移除（已水化）"
        );

        // 只有正常快照 spawn entity，combat_dead 不 spawn
        let npc_count = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, With<NpcMarker>>();
            q.iter(world).count()
        };
        assert_eq!(
            npc_count, 1,
            "rechallenge 混合批次：应仅 1 个 NpcMarker（来自正常快照）；\
             实际 {npc_count}（0=正常快照未水化；2=combat_dead 被错误 spawn）"
        );

        let trib_events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all: Vec<_> = trib_events.iter_current_update_events().collect();
        assert_eq!(
            all.len(),
            1,
            "rechallenge 混合批次：应仅 1 个 InitiateXuhuaTribulation（来自正常快照）；\
             实际 {0}（0=正常快照未触发；2=combat_dead 被错误触发渡劫）",
            all.len()
        );
    }

    // -----------------------------------------------------------------------
    // plan-tsy-sentinel-dormant-regression-v1：TSY 秘境守灵 dormant↔hydrate 身份
    // 回归 pin（P0/P3，§8.1 决议）。修复前：`dormant_tsy_hostile_snapshot` 完全不读
    // `TsySentinelMarker`，hydrate 时 `NpcArchetype::GuardianRelic` 无条件走
    // `spawn_relic_guard_npc_at`，秘境守灵被洗成普通 overworld relic guard（丢
    // phase/Boss HUD/专属掉落/外观）。以下 6 条测试把「目标行为」锁死。
    // -----------------------------------------------------------------------

    /// 构造一个带 `family_id`/坐标匹配的 TSY 秘境守灵 dormant snapshot（archetype
    /// 恒为 `GuardianRelic`，`tsy_hostile` 与 `tsy_sentinel` 同步携带同一 family_id，
    /// 对齐 §P1 论证的"两者恒相等"不变量）。
    fn tsy_sentinel_snapshot(
        char_id: &str,
        pos: DVec3,
        family_id: &str,
        guarding_container_pos: Option<[f64; 3]>,
        phase: u8,
        max_phase: u8,
    ) -> NpcDormantSnapshot {
        let mut snap = snapshot(char_id, pos);
        snap.archetype = NpcArchetype::GuardianRelic;
        snap.tsy_hostile = Some(DormantTsyHostileSnapshot {
            family_id: family_id.to_string(),
            zhinian_phase: None,
            zhinian_phase_entered_at_tick: None,
            fuya_aura: None,
            daoxiang_origin: None,
            daozhan: None,
        });
        snap.tsy_sentinel = Some(DormantTsySentinelSnapshot {
            guarding_container_pos,
            phase,
            max_phase,
        });
        snap
    }

    fn spawn_loot_container(app: &mut App, family_id: &str, pos: DVec3) -> Entity {
        app.world_mut()
            .spawn((
                LootContainer::new(
                    ContainerKind::RelicCore,
                    family_id.to_string(),
                    TsyDepth::Deep,
                    "relic_core_deep".to_string(),
                    0,
                ),
                Position(pos),
            ))
            .id()
    }

    /// P0/P1 pin：dehydrate 一个带 `TsySentinelMarker` 的活体秘境守灵，断言写入
    /// `NpcDormantStore` 的快照 `tsy_sentinel` 字段为 `Some`，且携带
    /// `guarding_container_pos`/`phase`/`max_phase`——修复前 `dormant_tsy_hostile_snapshot`
    /// 完全不读 `TsySentinelMarker`，这个字段恒为不存在（编译期都没有），身份从
    /// dehydrate 这一步就已经丢失。
    #[test]
    fn tsy_sentinel_dehydrates_with_sentinel_identity_payload() {
        let mut app = App::new();
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(Update, dehydrate_far_npcs_system);

        let container =
            spawn_loot_container(&mut app, "tsy_lingxu_01", DVec3::new(20.0, 64.0, 20.0));

        let lifecycle = Lifecycle {
            character_id: "npc_sentinel_far".to_string(),
            ..Default::default()
        };
        let sentinel_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::new(12.0, 64.0, 12.0)),
                lifecycle,
                LifeRecord::new("npc_sentinel_far"),
                DeathRegistry::new("npc_sentinel_far"),
                NpcArchetype::GuardianRelic,
                NpcDailySchedule::for_archetype(NpcArchetype::GuardianRelic, 7),
                NpcLifespan::new(0.0, 1_000.0),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 500.0,
                    qi_max: 1000.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                TsyHostileMarker {
                    family_id: "tsy_lingxu_01".to_string(),
                },
                TsySentinelMarker {
                    family_id: "tsy_lingxu_01".to_string(),
                    guarding_container: Some(container),
                    phase: 1,
                    max_phase: 3,
                },
            ))
            .id();

        app.update();

        let store = app.world().resource::<NpcDormantStore>();
        let snap = store
            .snapshots
            .get("npc_sentinel_far")
            .expect("sentinel snapshot must exist in NpcDormantStore after dehydrate");
        let sentinel = snap.tsy_sentinel.as_ref().expect(
            "dehydrated TSY sentinel must carry Some(tsy_sentinel) payload — regression: \
             dormant_tsy_hostile_snapshot never read TsySentinelMarker, so this field would \
             not exist pre-fix and the sentinel silently degrades to a plain GuardianRelic",
        );
        assert_eq!(
            sentinel.guarding_container_pos,
            Some([20.0, 64.0, 20.0]),
            "guarding_container_pos must capture the guarded container's live Position at dehydrate time"
        );
        assert_eq!(
            sentinel.phase, 1,
            "phase must be copied verbatim from the live TsySentinelMarker (best-effort — \
             update_sentinel_phase_system corrects it against real Wounds next tick, §8.1 #2)"
        );
        assert_eq!(
            sentinel.max_phase, 3,
            "max_phase (design constant) must be preserved exactly"
        );
        assert!(
            app.world().get::<Despawned>(sentinel_entity).is_some(),
            "dehydrated sentinel entity must be marked Despawned"
        );
    }

    /// P0/P2 pin（核心修复）：hydrate 一个带 `tsy_sentinel` 载荷的 dormant snapshot，
    /// 断言实体被 `spawn_tsy_sentinel_at` 路径重建——带 `TsySentinelMarker` +
    /// `FaunaVisualKind::TsySentinel`，且**不带** `GuardianDuty`/`TrialEval`（那是
    /// `spawn_relic_guard_npc_at` 专属组件），也不是 villager `EntityKind`。
    #[test]
    fn hydrated_tsy_sentinel_uses_spawn_tsy_sentinel_path_not_spawn_relic_guard() {
        let mut store = NpcDormantStore::default();
        store.insert(tsy_sentinel_snapshot(
            "npc_sentinel_hydrate",
            DVec3::new(10.0, 64.0, 10.0),
            "tsy_lingxu_01",
            None,
            0,
            3,
        ));
        let mut app = near_player_hydrate_app(store);
        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "sentinel snapshot should have been consumed from the dormant store"
        );

        let results = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(
                Option<&TsySentinelMarker>,
                Option<&FaunaVisualKind>,
                Option<&GuardianDuty>,
                Option<&TrialEval>,
                &valence::prelude::EntityKind,
            ), With<NpcMarker>>();
            query
                .iter(world)
                .map(|(s, v, d, t, k)| (s.cloned(), v.copied(), d.is_some(), t.is_some(), *k))
                .collect::<Vec<_>>()
        };
        assert_eq!(results.len(), 1, "expected exactly one hydrated entity");
        let (sentinel_marker, visual, has_guardian_duty, has_trial_eval, entity_kind) = &results[0];

        assert!(
            sentinel_marker.is_some(),
            "hydrated TSY sentinel snapshot must produce a TsySentinelMarker component \
             (regression this plan fixes: pre-fix hydrate silently routed to spawn_relic_guard_npc_at, \
             never inserting this marker)"
        );
        assert_eq!(
            *visual,
            Some(FaunaVisualKind::TsySentinel),
            "hydrated TSY sentinel must carry FaunaVisualKind::TsySentinel visual identity"
        );
        assert!(
            !*has_guardian_duty,
            "TSY sentinel hydrate path must NOT also carry GuardianDuty — that is exclusive \
             to spawn_relic_guard_npc_at (dual-identity drift, §8.1 #3)"
        );
        assert!(
            !*has_trial_eval,
            "TSY sentinel hydrate path must NOT also carry TrialEval (§8.1 #3)"
        );
        assert_ne!(
            *entity_kind,
            valence::prelude::EntityKind::VILLAGER,
            "TSY sentinel must not fall back to the villager-styled overworld relic guard EntityKind"
        );
    }

    /// P0/P2 pin：容器仍存在时，hydrate 必须精确回填 `max_phase`，best-effort 回填
    /// `phase`，并把 `guarding_container` 重绑到 family+坐标匹配的容器 entity。
    #[test]
    fn rehydrated_tsy_sentinel_keeps_marker_visual_and_phase_state() {
        let mut store = NpcDormantStore::default();
        store.insert(tsy_sentinel_snapshot(
            "npc_sentinel_rebind",
            DVec3::new(10.0, 64.0, 10.0),
            "tsy_lingxu_01",
            Some([20.0, 64.0, 20.0]),
            2,
            3,
        ));
        let mut app = near_player_hydrate_app(store);
        let container =
            spawn_loot_container(&mut app, "tsy_lingxu_01", DVec3::new(20.0, 64.0, 20.0));
        app.update();

        let markers = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&TsySentinelMarker, With<NpcMarker>>();
            query.iter(world).cloned().collect::<Vec<_>>()
        };
        assert_eq!(
            markers.len(),
            1,
            "expected exactly one hydrated TsySentinelMarker entity"
        );
        let marker = &markers[0];

        assert_eq!(
            marker.max_phase, 3,
            "max_phase must be precisely refilled from the snapshot"
        );
        assert_eq!(
            marker.phase, 2,
            "phase must be best-effort refilled from the snapshot (§8.1 #2: corrected next tick \
             by update_sentinel_phase_system against real, currently-full Wounds)"
        );
        assert_eq!(
            marker.guarding_container,
            Some(container),
            "guarding_container must rebind to the container matching family_id + position \
             within epsilon — the container is still present, so rebind must succeed 100%"
        );
    }

    /// P0/§8.1 #3 pin：`GuardianRelic` 的两条 hydrate 分支必须互斥——
    /// 有 `tsy_sentinel` 载荷 → 只产 `TsySentinelMarker`，不产 `GuardianDuty`/`TrialEval`；
    /// 无 → 只产 `GuardianDuty`/`TrialEval`，不产 `TsySentinelMarker`。杜绝未来"两者都长"
    /// 或"两者都没有"的漂移。
    #[test]
    fn guardian_relic_dual_identity_invariant_partitioned_by_sentinel_marker() {
        // 分支 A：tsy_sentinel = Some。
        let mut store_a = NpcDormantStore::default();
        store_a.insert(tsy_sentinel_snapshot(
            "npc_dual_sentinel",
            DVec3::new(10.0, 64.0, 10.0),
            "tsy_lingxu_01",
            None,
            0,
            3,
        ));
        let mut app_a = near_player_hydrate_app(store_a);
        app_a.update();
        {
            let world = app_a.world_mut();
            let mut query = world.query_filtered::<(
                Option<&TsySentinelMarker>,
                Option<&GuardianDuty>,
                Option<&TrialEval>,
            ), With<NpcMarker>>();
            let results = query.iter(world).collect::<Vec<_>>();
            assert_eq!(
                results.len(),
                1,
                "expected exactly one hydrated entity (branch A)"
            );
            let (sentinel, duty, trial) = results[0];
            assert!(
                sentinel.is_some(),
                "Some(tsy_sentinel) branch must produce TsySentinelMarker"
            );
            assert!(
                duty.is_none(),
                "Some(tsy_sentinel) branch must NOT also produce GuardianDuty (dual-identity drift)"
            );
            assert!(
                trial.is_none(),
                "Some(tsy_sentinel) branch must NOT also produce TrialEval (dual-identity drift)"
            );
        }

        // 分支 B：tsy_sentinel = None（纯 overworld GuardianRelic，行为不变）。
        let mut store_b = NpcDormantStore::default();
        let mut relic_snap = snapshot("npc_dual_relic", DVec3::new(10.0, 64.0, 10.0));
        relic_snap.archetype = NpcArchetype::GuardianRelic;
        relic_snap.guardian_relic = Some(DormantGuardianRelicSnapshot {
            relic_id: "relic:test".to_string(),
            alarm_center: [10.0, 64.0, 10.0],
            alarm_radius: 32.0,
            trial_template_id: "trial:test".to_string(),
            last_offered_tick: None,
            offer_cooldown_ticks: 900,
        });
        store_b.insert(relic_snap);
        let mut app_b = near_player_hydrate_app(store_b);
        app_b.update();
        {
            let world = app_b.world_mut();
            let mut query = world.query_filtered::<(
                Option<&TsySentinelMarker>,
                Option<&GuardianDuty>,
                Option<&TrialEval>,
            ), With<NpcMarker>>();
            let results = query.iter(world).collect::<Vec<_>>();
            assert_eq!(
                results.len(),
                1,
                "expected exactly one hydrated entity (branch B)"
            );
            let (sentinel, duty, trial) = results[0];
            assert!(
                sentinel.is_none(),
                "None(tsy_sentinel) branch must NOT produce TsySentinelMarker"
            );
            assert!(
                duty.is_some(),
                "None(tsy_sentinel) branch must produce GuardianDuty (plain overworld relic \
                 guard, unchanged pre-fix behavior)"
            );
            assert!(
                trial.is_some(),
                "None(tsy_sentinel) branch must produce TrialEval"
            );
        }
    }

    /// P0/§8.1 #1 补漏（博弈 blocker）pin：两段式 family+坐标重绑必须先按 `family_id`
    /// 过滤，再在子集内按坐标匹配——**绝不**允许跨 family 裸坐标匹配，即使错误 family
    /// 的候选坐标匹配得更精确、且更早出现在 `relic_containers` 切片里。
    ///
    /// 场景：两个 `LootContainer`——`tutorial_chest`（`family_id="spawn_tutorial"`，
    /// 模拟 `spawn_tutorial.rs:497` 的教程箱子，同为 Overworld，坐标与 sentinel 记录的
    /// `guarding_container_pos` **完全相同**，且先于真容器 spawn）与
    /// `tsy_lingxu_01`（sentinel 真正所属，坐标落在 epsilon 内但不完全相同，spawn 更晚）。
    /// 断言 sentinel 精确重绑到 `tsy_lingxu_01`，绝不误绑 `tutorial_chest`。
    #[test]
    fn hydrated_tsy_sentinel_container_rebind_ignores_same_position_different_family() {
        let mut store = NpcDormantStore::default();
        store.insert(tsy_sentinel_snapshot(
            "npc_sentinel_blocker",
            DVec3::new(10.0, 64.0, 10.0),
            "tsy_lingxu_01",
            Some([30.0, 64.0, 30.0]),
            0,
            3,
        ));
        let mut app = near_player_hydrate_app(store);

        // 更早 spawn、family 错误、但坐标与记录值完全一致（比真容器"匹配得更好"）。
        let tutorial_chest =
            spawn_loot_container(&mut app, "spawn_tutorial", DVec3::new(30.0, 64.0, 30.0));
        // 更晚 spawn、family 正确、坐标稍有偏移（仍在 epsilon<=0.5 内）。
        let real_container =
            spawn_loot_container(&mut app, "tsy_lingxu_01", DVec3::new(30.3, 64.0, 30.3));

        app.update();

        let markers = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<&TsySentinelMarker, With<NpcMarker>>();
            query.iter(world).cloned().collect::<Vec<_>>()
        };
        assert_eq!(
            markers.len(),
            1,
            "expected exactly one hydrated sentinel entity"
        );
        let marker = &markers[0];

        assert_ne!(
            marker.guarding_container,
            Some(tutorial_chest),
            "sentinel must NOT rebind to the earlier-spawned, exact-position-match, \
             wrong-family tutorial_chest container (§8.1 #1 blocker) — even a perfect \
             positional match must lose to a correct-family match"
        );
        assert_eq!(
            marker.guarding_container,
            Some(real_container),
            "sentinel must rebind to the family-matching tsy_lingxu_01 container despite \
             tutorial_chest appearing earlier in the query and matching position more exactly"
        );
    }

    /// P3 pin（端到端）：完整跑 dehydrate → snapshot → hydrate 一圈，容器全程未被触碰时
    /// 重绑必须 100% 成功；同时验证死亡掉落键精确走 `tsy_sentinel` 分支（不是
    /// daoxiang/zhinian/fuya/`None`）——身份没有在任何一步被洗平。
    #[test]
    fn sentinel_survives_full_dehydrate_hydrate_cycle_with_container_still_present() {
        let mut app = App::new();
        app.add_event::<InitiateXuhuaTribulation>();
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(NpcDormantStore::default());
        app.insert_resource(NpcVirtualizationConfig {
            transition_interval_ticks: 1,
            dehydrate_without_players: true,
            ..Default::default()
        });
        app.add_systems(
            Update,
            (
                hydrate_dormant_near_players_system,
                dehydrate_far_npcs_system,
            )
                .chain(),
        );

        let container =
            spawn_loot_container(&mut app, "tsy_lingxu_01", DVec3::new(30.0, 64.0, 30.0));

        let lifecycle = Lifecycle {
            character_id: "npc_sentinel_e2e".to_string(),
            ..Default::default()
        };
        let live_entity = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position(DVec3::new(10.0, 64.0, 10.0)),
                lifecycle,
                LifeRecord::new("npc_sentinel_e2e"),
                DeathRegistry::new("npc_sentinel_e2e"),
                NpcArchetype::GuardianRelic,
                NpcDailySchedule::for_archetype(NpcArchetype::GuardianRelic, 3),
                NpcLifespan::new(0.0, 1_000.0),
                Cultivation {
                    realm: Realm::Spirit,
                    qi_current: 500.0,
                    qi_max: 1000.0,
                    ..Default::default()
                },
                MeridianSystem::default(),
                Contamination::default(),
                TsyHostileMarker {
                    family_id: "tsy_lingxu_01".to_string(),
                },
                TsySentinelMarker {
                    family_id: "tsy_lingxu_01".to_string(),
                    guarding_container: Some(container),
                    phase: 0,
                    max_phase: 3,
                },
            ))
            .id();

        // Tick 0：无玩家（dehydrate_without_players=true 绕过近距守卫）——活体秘境守灵脱水进 store。
        app.update();

        assert!(
            app.world().get::<Despawned>(live_entity).is_some(),
            "sentinel entity must be marked Despawned after dehydrate"
        );
        assert!(
            app.world()
                .resource::<NpcDormantStore>()
                .snapshots
                .get("npc_sentinel_e2e")
                .and_then(|s| s.tsy_sentinel.as_ref())
                .is_some(),
            "dehydrated snapshot must carry Some(tsy_sentinel) payload before hydrate can restore it"
        );

        // 玩家靠近原位置——tick 1：hydrate 重建。
        app.world_mut().spawn((
            valence::client::ClientMarker,
            Position(DVec3::new(10.0, 64.0, 10.0)),
        ));
        app.update();

        assert!(
            app.world().resource::<NpcDormantStore>().is_empty(),
            "sentinel snapshot should have been consumed from the dormant store after hydrate"
        );

        let rehydrated = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<
                (&NpcArchetype, &TsySentinelMarker, &FaunaVisualKind),
                With<NpcMarker>,
            >();
            query
                .iter(world)
                .map(|(a, m, v)| (*a, m.clone(), *v))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rehydrated.len(),
            1,
            "expected exactly one rehydrated sentinel entity"
        );
        let (archetype, marker, visual) = &rehydrated[0];

        assert_eq!(*archetype, NpcArchetype::GuardianRelic);
        assert_eq!(*visual, FaunaVisualKind::TsySentinel);
        assert_eq!(
            marker.guarding_container,
            Some(container),
            "容器全程未被触碰（§8.1 #2：现状验证容器从不 dehydrate/despawn）时，重绑必须 \
             100% 成功指向同一容器 entity"
        );
        assert_eq!(marker.max_phase, 3);

        let drop_key = crate::npc::tsy_hostile::drop_key_for_npc(*archetype, Some(marker));
        assert_eq!(
            drop_key,
            Some("tsy_sentinel"),
            "TSY 秘境守灵死亡掉落必须精确走 tsy_sentinel 分流键——身份没有在 dehydrate/hydrate \
             任何一步被洗平成普通 GuardianRelic 或其它 archetype 的掉落表"
        );
    }
}
