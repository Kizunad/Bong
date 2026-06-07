//! NPC hydrate/dehydrate bridge.
//!
//! This module moves live NPCs into [`NpcDormantStore`] when they are far away
//! from all players, and spawns them back when someone comes near again.

use std::collections::{BTreeMap, HashSet};

use valence::client::ClientMarker;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, App, Commands, Despawned, Entity, EventWriter, IntoSystemConfigs, Position, Query,
    Res, ResMut, Update, With, Without,
};

use crate::combat::components::Lifecycle;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{DeathRegistry, LifespanComponent, LifespanExtensionLedger};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::tribulation::{du_xu_prereqs_met, InitiateXuhuaTribulation};
use crate::npc::brain::NPC_TRIBULATION_WAVES_DEFAULT;
use crate::npc::dormant::{
    dvec3_from_array, planar_distance, vec3_to_array, DormantBehaviorIntent,
    DormantDaoxiangOriginSnapshot, DormantFuyaAuraSnapshot, DormantGuardianRelicSnapshot,
    DormantPatrolSnapshot, DormantTsyHostileSnapshot, DormantZhinianPhase, NpcDormantSnapshot,
    NpcDormantStore, NpcVirtualizationConfig,
};
use crate::npc::faction::{FactionMembership, FactionRank};
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
    spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at, spawn_relic_guard_npc_at,
    spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker, NpcSkinSpawnContext,
};
use crate::npc::territory::Territory;
use crate::npc::tsy_hostile::{
    spawn_tsy_daoxiang_at, spawn_tsy_fuya_at, spawn_tsy_skull_fiend_at, spawn_tsy_zhinian_at,
    FuyaAura, TsyHostileMarker, ZhinianMind, ZhinianPhase,
};
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::era::WorldEraState;
use crate::world::poi_novice::PoiNoviceRegistry;
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
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering hydrate/dehydrate bridge");
    app.add_systems(
        Update,
        (
            hydrate_dormant_near_players_system,
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

    let mut to_hydrate = BTreeMap::<String, bool>::new();
    for (char_id, snapshot) in &store.snapshots {
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
        let patrol_snapshot = patrol.map(|patrol| DormantPatrolSnapshot {
            home_zone: patrol.home_zone.clone(),
            anchor_index: patrol.anchor_index,
            current_target: crate::npc::dormant::vec3_to_array(patrol.current_target),
        });
        let intent = DormantBehaviorIntent::for_archetype(*archetype, patrol_snapshot.as_ref());

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
                death_registry: death_registry
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or_else(|| DeathRegistry::new(lifecycle.character_id.clone())),
                life_record: life_record
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .unwrap_or_else(|| LifeRecord::new(lifecycle.character_id.clone())),
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
                    extras.tsy_markers.get(entity).ok().flatten(),
                    extras.zhinian_minds.get(entity).ok().flatten(),
                    extras.fuya_auras.get(entity).ok().flatten(),
                    extras.daoxiang_origins.get(entity).ok().flatten(),
                ),
                intent,
                dormant_since_tick: tick,
                last_dormant_tick_processed: tick,
                initial_qi: cultivation.qi_current,
                qi_ledger_net: 0.0,
                combat_dead_pending_release: false,
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

fn spawn_from_snapshot(
    commands: &mut Commands,
    snapshot: NpcDormantSnapshot,
    dimension_layers: &DimensionLayers,
    current_tick: u64,
    pois: Option<&PoiNoviceRegistry>,
    skin_pool: Option<&mut SkinPool>,
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
        NpcArchetype::GuardianRelic => {
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
    if let Some(tsy) = snapshot.tsy_hostile {
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
    use crate::npc::brain::return_home_action_system;
    use crate::npc::dormant::{DEHYDRATE_RADIUS_BLOCKS, HYDRATE_RADIUS_BLOCKS};
    use crate::world::zone::{Zone, DEFAULT_SPAWN_ZONE_NAME};

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
            faction: None,
            emergent_group: None,
            patrol: None,
            loot_table: None,
            guardian_relic: None,
            tsy_hostile: None,
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
        }
    }

    fn open_all_meridians(snapshot: &mut NpcDormantSnapshot) {
        for meridian in snapshot.meridian_system.iter_mut() {
            meridian.opened = true;
        }
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

        let snapshot = dormant_tsy_hostile_snapshot(Some(&marker), Some(&mind), Some(&aura), None)
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
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
            combat_dead_pending_release: false,
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
}
