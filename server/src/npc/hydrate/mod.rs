//! NPC hydrate/dehydrate bridge.
//!
//! This module moves live NPCs into [`NpcDormantStore`] when they are far away
//! from all players, and spawns them back when someone comes near again.

use std::collections::BTreeMap;

use valence::client::ClientMarker;
use valence::prelude::{
    App, Commands, Despawned, Entity, EventWriter, IntoSystemConfigs, Position, Query, Res, ResMut,
    Update, With, Without,
};

use crate::combat::components::Lifecycle;
use crate::cultivation::components::{Contamination, Cultivation, MeridianSystem};
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{DeathRegistry, LifespanComponent, LifespanExtensionLedger};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::tribulation::{du_xu_prereqs_met, InitiateXuhuaTribulation};
use crate::npc::brain::NPC_TRIBULATION_WAVES_DEFAULT;
use crate::npc::dormant::{
    dvec3_from_array, planar_distance, vec3_to_array, DormantBehaviorIntent, DormantPatrolSnapshot,
    NpcDormantSnapshot, NpcDormantStore, NpcVirtualizationConfig,
};
use crate::npc::faction::{FactionMembership, FactionRank};
use crate::npc::lifecycle::{NpcArchetype, NpcLifespan, NpcRegistry};
use crate::npc::lod::NpcLodTier;
use crate::npc::loot::{default_loot_for_archetype, NpcLootTable};
use crate::npc::movement::GameTick;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{
    spawn_beast_npc_at, spawn_commoner_npc_at, spawn_disciple_npc_at, spawn_relic_guard_npc_at,
    spawn_rogue_npc_at, spawn_zombie_npc_at, NpcMarker, NpcSkinSpawnContext,
};
use crate::npc::territory::Territory;
use crate::skin::NpcSkinFallbackPolicy;
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::zone::ZoneRegistry;

const DORMANT_TRIBULATION_MIN_QI_RATIO: f64 = 0.8;
type PlayerPosition = (DimensionKind, valence::prelude::DVec3);

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
    mut tribulations: EventWriter<InitiateXuhuaTribulation>,
) {
    let tick = crate::npc::dormant::current_tick(game_tick.as_deref());
    if !crate::npc::dormant::should_run_interval(tick, config.transition_interval_ticks) {
        return;
    }

    let player_positions = players
        .iter()
        .map(|(pos, dimension)| (dimension_kind(dimension), pos.get()))
        .collect::<Vec<_>>();
    let Some(dimension_layers) = dimension_layers.as_deref() else {
        return;
    };

    let mut to_hydrate = BTreeMap::<String, bool>::new();
    for (char_id, snapshot) in &store.snapshots {
        let tribulation_ready = dormant_tribulation_ready(snapshot);
        let near_player = nearest_same_dimension_player_distance(
            snapshot.position_vec(),
            snapshot.dimension,
            &player_positions,
        ) <= config.hydrate_radius_blocks;
        if tribulation_ready || near_player {
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
        let entity = spawn_from_snapshot(&mut commands, snapshot, dimension_layers);
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

    let zone_registry = zone_registry.as_deref();
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
        faction,
        patrol,
    ) in npcs.iter()
    {
        let dimension = current_dimension
            .map(|dimension| dimension.0)
            .unwrap_or(DimensionKind::Overworld);
        let nearest =
            nearest_same_dimension_player_distance(position.get(), dimension, &player_positions);

        if !player_positions.is_empty() && nearest <= config.dehydrate_radius_blocks {
            continue;
        }
        let zone_name = zone_registry
            .and_then(|zones| zones.find_zone(dimension, position.get()))
            .map(|zone| zone.name.clone())
            .or_else(|| patrol.map(|patrol| patrol.home_zone.clone()))
            .unwrap_or_else(|| "spawn".to_string());
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
                patrol: patrol_snapshot,
                loot_table: loot_tables
                    .get(entity)
                    .ok()
                    .flatten()
                    .cloned()
                    .or_else(|| Some(default_loot_for_archetype(*archetype))),
                intent,
                dormant_since_tick: tick,
                last_dormant_tick_processed: tick,
                initial_qi: cultivation.qi_current,
                qi_ledger_net: 0.0,
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
        commands.entity(entity).despawn();
    }
}

fn can_insert_dormant_snapshot(
    store: &NpcDormantStore,
    char_id: &str,
    max_dormant_count: usize,
) -> bool {
    store.contains(char_id) || store.len() < max_dormant_count
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

fn spawn_from_snapshot(
    commands: &mut Commands,
    snapshot: NpcDormantSnapshot,
    dimension_layers: &DimensionLayers,
) -> Entity {
    let layer = match snapshot.dimension {
        DimensionKind::Tsy => dimension_layers.tsy,
        _ => dimension_layers.overworld,
    };
    let pos = snapshot.position_vec();
    let patrol_target = snapshot
        .patrol
        .as_ref()
        .map(|patrol| dvec3_from_array(patrol.current_target))
        .unwrap_or(pos);
    let home_zone = snapshot.zone_name.as_str();
    let entity = match snapshot.archetype {
        NpcArchetype::Zombie => spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target),
        NpcArchetype::Commoner => spawn_commoner_npc_at(
            commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer,
            home_zone,
            pos,
            patrol_target,
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::Rogue => spawn_rogue_npc_at(
            commands,
            NpcSkinSpawnContext::new(None, NpcSkinFallbackPolicy::AllowFallback),
            layer,
            home_zone,
            pos,
            patrol_target,
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
            snapshot
                .faction
                .as_ref()
                .and_then(|membership| membership.lineage.as_ref())
                .and_then(|lineage| lineage.master_id.clone()),
            snapshot.lifespan.age_ticks,
        ),
        NpcArchetype::GuardianRelic => spawn_relic_guard_npc_at(
            commands,
            layer,
            home_zone,
            pos,
            40.0,
            format!("relic:{home_zone}"),
            format!("trial:{home_zone}"),
        ),
        NpcArchetype::Daoxiang | NpcArchetype::Zhinian | NpcArchetype::Fuya => {
            spawn_zombie_npc_at(commands, layer, home_zone, pos, patrol_target)
        }
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
    entity
}

fn dormant_tribulation_ready(snapshot: &NpcDormantSnapshot) -> bool {
    du_xu_prereqs_met(&snapshot.cultivation, &snapshot.meridian_system)
        && snapshot.cultivation.qi_current
            >= snapshot.cultivation.qi_max * DORMANT_TRIBULATION_MIN_QI_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{DVec3, Events};

    use crate::cultivation::components::Realm;
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
            intent: DormantBehaviorIntent::Cultivate {
                zone: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            },
            dormant_since_tick: 0,
            last_dormant_tick_processed: 0,
            initial_qi: cultivation.qi_current,
            qi_ledger_net: 0.0,
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
        assert!(dormant_tribulation_ready(&ready));

        let mut low_qi = ready.clone();
        low_qi.cultivation.qi_current = 700.0;
        assert!(!dormant_tribulation_ready(&low_qi));

        let mut missing_meridian = ready.clone();
        missing_meridian.meridian_system.regular[0].opened = false;
        assert!(!dormant_tribulation_ready(&missing_meridian));
    }

    #[test]
    fn dormant_capacity_uses_store_len_not_tick_candidate_count() {
        let mut store = NpcDormantStore::default();
        store.insert(snapshot("npc_existing", DVec3::new(10.0, 64.0, 10.0)));

        assert!(!can_insert_dormant_snapshot(&store, "npc_new", 1));
        assert!(can_insert_dormant_snapshot(&store, "npc_existing", 1));
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
        let events = app.world().resource::<Events<InitiateXuhuaTribulation>>();
        let all = events.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].waves_total, NPC_TRIBULATION_WAVES_DEFAULT);
    }
}
