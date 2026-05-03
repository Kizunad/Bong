//! Spawn tutorial state machine (plan-spawn-tutorial-v1).
//!
//! v1 keeps the tutorial silent: no quest UI, no explicit progress packet. The
//! server only records player-driven hooks, grants the coffin spirit niche
//! stone once per player, and spawns placeholder "rat" zombies that drain qi.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, DVec3, Entity, EntityLayerId, Event,
    EventReader, IntoSystemConfigs, Position, Query, Res, ResMut, Resource, Startup, Update,
    Username, With, Without,
};

use crate::combat::CombatClock;
use crate::cultivation::breakthrough::{BreakthroughOutcome, BreakthroughSuccess};
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::npc::spawn::spawn_zombie_npc_at;
use crate::persistence::{load_player_cultivation_bundle, PersistenceSettings};
use crate::world::dimension::DimensionLayers;
use crate::world::setup_world;
use crate::world::terrain::TerrainProviders;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

pub const SPIRIT_NICHE_STONE_TEMPLATE_ID: &str = "spirit_niche_stone";
pub const TUTORIAL_LINGQUAN_REACH_RADIUS: f64 = 8.0;
pub const RAT_SWARM_SPAWN_DISTANCE: f64 = 20.0;
pub const RAT_SWARM_TRIGGER_DISTANCE: f64 = 80.0;
pub const RAT_SWARM_DRAIN_RADIUS: f64 = 4.5;
pub const RAT_SWARM_DRAIN_AMOUNT: f64 = 1.0;
pub const COMPLETION_WINDOW_TICKS: u64 = 30 * 60 * 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TutorialHook {
    SpawnEntered,
    CoffinOpened,
    Moved200Blocks,
    FirstSitMeditate,
    FirstMeridianOpened,
    RatSwarmEncounter,
    LingquanReached,
    BreakthroughWindow,
    RealmAdvancedToInduce,
}

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct TutorialState {
    pub entered_at_tick: u64,
    #[serde(default)]
    pub spawn_position: Option<[f64; 3]>,
    #[serde(default)]
    pub last_position: Option<[f64; 3]>,
    #[serde(default)]
    pub first_lingquan_pos: Option<[f64; 3]>,
    #[serde(default)]
    pub opened_coffin_pos: Option<[i32; 3]>,
    #[serde(default)]
    pub rat_swarm_spawned_at_tick: Option<u64>,
    #[serde(default)]
    pub completed_at_tick: Option<u64>,
    #[serde(default)]
    pub hooks_triggered: BTreeSet<TutorialHook>,
}

impl Default for TutorialState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TutorialState {
    pub fn new(entered_at_tick: u64) -> Self {
        let mut hooks_triggered = BTreeSet::new();
        hooks_triggered.insert(TutorialHook::SpawnEntered);
        Self {
            entered_at_tick,
            spawn_position: None,
            last_position: None,
            first_lingquan_pos: None,
            opened_coffin_pos: None,
            rat_swarm_spawned_at_tick: None,
            completed_at_tick: None,
            hooks_triggered,
        }
    }

    pub fn trigger(&mut self, hook: TutorialHook) -> bool {
        self.hooks_triggered.insert(hook)
    }

    pub fn has(&self, hook: TutorialHook) -> bool {
        self.hooks_triggered.contains(&hook)
    }
}

#[derive(Debug, Clone, Event)]
pub struct CoffinOpenRequest {
    pub player: Entity,
    pub pos: [i32; 3],
    pub tick: u64,
}

#[derive(Debug, Clone, Event, PartialEq, Eq)]
pub struct TutorialHookEvent {
    pub player: Entity,
    pub hook: TutorialHook,
    pub tick: u64,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TutorialCoffin {
    pub pos: [i32; 3],
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TutorialLingquan {
    pub index: u8,
    pub pos: [f64; 3],
}

#[derive(Debug, Clone, Copy, Component)]
pub struct TutorialRatSwarmNpc {
    pub spawned_for: Entity,
    pub spawned_at_tick: u64,
}

#[derive(Debug, Default, Resource, Clone, PartialEq, Eq)]
pub struct TutorialTelemetry {
    pub started: u64,
    pub completed: u64,
    pub completed_within_30min: u64,
}

type JoinedTutorialClientQueryItem<'a> = (Entity, &'a Username);
type JoinedTutorialClientFilter = (Added<Client>, Without<TutorialState>);

impl TutorialTelemetry {
    pub fn completion_rate_30min(&self) -> f64 {
        if self.started == 0 {
            return 0.0;
        }
        self.completed_within_30min as f64 / self.started as f64
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(TutorialTelemetry::default());
    app.add_event::<CoffinOpenRequest>();
    app.add_event::<TutorialHookEvent>();
    app.add_systems(Startup, spawn_tutorial_poi_markers.after(setup_world));
    app.add_systems(
        Update,
        (
            attach_tutorial_state_to_joined_clients,
            handle_coffin_open_requests,
            tutorial_hook_state_machine,
            dynamic_rat_swarm_spawner.after(tutorial_hook_state_machine),
            tutorial_rat_qi_drain_tick.after(dynamic_rat_swarm_spawner),
            record_tutorial_breakthrough_completion,
        ),
    );
}

fn attach_tutorial_state_to_joined_clients(
    mut commands: Commands,
    settings: Res<PersistenceSettings>,
    clock: Option<Res<CombatClock>>,
    mut telemetry: ResMut<TutorialTelemetry>,
    joined: Query<JoinedTutorialClientQueryItem<'_>, JoinedTutorialClientFilter>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (entity, username) in &joined {
        let restored = load_player_cultivation_bundle(&settings, username.0.as_str())
            .ok()
            .flatten()
            .and_then(|bundle| bundle.get("tutorial_state").cloned())
            .and_then(|value| serde_json::from_value::<TutorialState>(value).ok());
        let state = restored.unwrap_or_else(|| TutorialState::new(now));
        telemetry.started = telemetry.started.saturating_add(1);
        commands.entity(entity).insert(state);
    }
}

fn spawn_tutorial_poi_markers(
    mut commands: Commands,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
) {
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };

    let mut coffin_count = 0usize;
    let mut lingquan_count = 0usize;
    for poi in providers.overworld.pois().iter() {
        match poi.kind.as_str() {
            "spawn_tutorial_coffin" => {
                let pos = [
                    poi.pos_xyz[0].round() as i32,
                    poi.pos_xyz[1].round() as i32,
                    poi.pos_xyz[2].round() as i32,
                ];
                commands.spawn((
                    TutorialCoffin { pos },
                    Position(DVec3::new(
                        f64::from(pos[0]),
                        f64::from(pos[1]),
                        f64::from(pos[2]),
                    )),
                    EntityLayerId(layers.overworld),
                ));
                coffin_count += 1;
            }
            "tutorial_lingquan" => {
                let index = parse_tag_u8(&poi.tags, "index").unwrap_or(0);
                let pos = [
                    f64::from(poi.pos_xyz[0]),
                    f64::from(poi.pos_xyz[1]),
                    f64::from(poi.pos_xyz[2]),
                ];
                commands.spawn((
                    TutorialLingquan { index, pos },
                    Position(DVec3::new(pos[0], pos[1], pos[2])),
                    EntityLayerId(layers.overworld),
                ));
                lingquan_count += 1;
            }
            _ => {}
        }
    }

    tracing::info!(
        "[bong][spawn-tutorial] spawned {coffin_count} coffin marker(s), {lingquan_count} lingquan marker(s) from POIs; client channel={SERVER_DATA_CHANNEL}"
    );
}

fn handle_coffin_open_requests(
    mut requests: EventReader<CoffinOpenRequest>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut players: Query<(&mut TutorialState, &mut PlayerInventory)>,
    coffins: Query<&TutorialCoffin>,
) {
    for request in requests.read() {
        if !coffins.is_empty() && !coffins.iter().any(|coffin| coffin.pos == request.pos) {
            tracing::warn!(
                "[bong][spawn-tutorial] rejected coffin_open from {:?}: no tutorial coffin at {:?}",
                request.player,
                request.pos
            );
            continue;
        }
        let Ok((mut state, mut inventory)) = players.get_mut(request.player) else {
            continue;
        };
        match grant_coffin_reward_once(
            &mut state,
            &mut inventory,
            &registry,
            &mut allocator,
            request.pos,
        ) {
            CoffinGrantOutcome::Granted { .. } => {
                hook_events.send(TutorialHookEvent {
                    player: request.player,
                    hook: TutorialHook::CoffinOpened,
                    tick: request.tick,
                });
            }
            CoffinGrantOutcome::AlreadyOpened => {}
            CoffinGrantOutcome::MissingItemTemplate { error } => {
                tracing::warn!(
                    "[bong][spawn-tutorial] failed to grant coffin reward to {:?}: {error}",
                    request.player
                );
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoffinGrantOutcome {
    Granted { instance_id: u64 },
    AlreadyOpened,
    MissingItemTemplate { error: String },
}

pub fn grant_coffin_reward_once(
    state: &mut TutorialState,
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
    coffin_pos: [i32; 3],
) -> CoffinGrantOutcome {
    if state.has(TutorialHook::CoffinOpened) {
        return CoffinGrantOutcome::AlreadyOpened;
    }

    match add_item_to_player_inventory(
        inventory,
        registry,
        allocator,
        SPIRIT_NICHE_STONE_TEMPLATE_ID,
        1,
    ) {
        Ok(receipt) => {
            state.opened_coffin_pos = Some(coffin_pos);
            state.trigger(TutorialHook::CoffinOpened);
            CoffinGrantOutcome::Granted {
                instance_id: receipt.instance_id,
            }
        }
        Err(error) => CoffinGrantOutcome::MissingItemTemplate { error },
    }
}

fn tutorial_hook_state_machine(
    clock: Option<Res<CombatClock>>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut players: Query<(
        Entity,
        &Position,
        &Cultivation,
        &MeridianSystem,
        &mut TutorialState,
    )>,
    lingquans: Query<&TutorialLingquan>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let default_lingquan = nearest_lingquan_from_query(&lingquans);

    for (entity, position, cultivation, meridians, mut state) in &mut players {
        let current = position_to_array(position);
        if state.spawn_position.is_none() {
            state.spawn_position = Some(current);
        }
        if state.first_lingquan_pos.is_none() {
            state.first_lingquan_pos = default_lingquan;
        }

        if moved_at_least_200_blocks(&state, current) && state.trigger(TutorialHook::Moved200Blocks)
        {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::Moved200Blocks,
                tick: now,
            });
        }

        if cultivation.qi_current > 0.0 && state.trigger(TutorialHook::FirstSitMeditate) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::FirstSitMeditate,
                tick: now,
            });
        }

        if meridians.opened_count() > 0 && state.trigger(TutorialHook::FirstMeridianOpened) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::FirstMeridianOpened,
                tick: now,
            });
        }

        if reached_lingquan(&state, current) && state.trigger(TutorialHook::LingquanReached) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::LingquanReached,
                tick: now,
            });
        }

        if state.has(TutorialHook::LingquanReached)
            && meridians.regular_opened_count() >= 3
            && state.trigger(TutorialHook::BreakthroughWindow)
        {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::BreakthroughWindow,
                tick: now,
            });
        }

        state.last_position = Some(current);
    }
}

fn dynamic_rat_swarm_spawner(
    mut commands: Commands,
    clock: Option<Res<CombatClock>>,
    layers: Option<Res<DimensionLayers>>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut players: Query<(Entity, &Position, &mut TutorialState)>,
) {
    let Some(layers) = layers else {
        return;
    };
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();

    for (entity, position, mut state) in &mut players {
        let current = position_to_array(position);
        if !should_spawn_rat_swarm(&state, current) {
            continue;
        }
        let Some(lingquan) = state.first_lingquan_pos else {
            continue;
        };

        let direction = normalized_xz_direction(current, lingquan).unwrap_or([1.0, 0.0]);
        let base = [
            current[0] + direction[0] * RAT_SWARM_SPAWN_DISTANCE,
            current[1],
            current[2] + direction[1] * RAT_SWARM_SPAWN_DISTANCE,
        ];
        for offset in [-2.0, 0.0, 2.0] {
            let spawn_position = DVec3::new(
                base[0] - direction[1] * offset,
                base[1],
                base[2] + direction[0] * offset,
            );
            let rat = spawn_zombie_npc_at(
                &mut commands,
                layers.overworld,
                DEFAULT_SPAWN_ZONE_NAME,
                spawn_position,
                DVec3::new(lingquan[0], lingquan[1], lingquan[2]),
            );
            commands.entity(rat).insert(TutorialRatSwarmNpc {
                spawned_for: entity,
                spawned_at_tick: now,
            });
        }
        state.rat_swarm_spawned_at_tick = Some(now);
        state.trigger(TutorialHook::RatSwarmEncounter);
        hook_events.send(TutorialHookEvent {
            player: entity,
            hook: TutorialHook::RatSwarmEncounter,
            tick: now,
        });
    }
}

fn tutorial_rat_qi_drain_tick(
    clock: Option<Res<CombatClock>>,
    rats: Query<(&Position, &TutorialRatSwarmNpc)>,
    mut players: Query<(Entity, &Position, &mut Cultivation), With<TutorialState>>,
) {
    let Some(clock) = clock else {
        return;
    };
    if clock.tick % 20 != 0 {
        return;
    }

    for (player_entity, player_pos, mut cultivation) in &mut players {
        if cultivation.qi_current <= 0.0 {
            continue;
        }
        let player = player_pos.get();
        let near_rat = rats.iter().any(|(rat_pos, rat)| {
            rat.spawned_for == player_entity
                && clock.tick.saturating_sub(rat.spawned_at_tick) <= 10 * 60 * 20
                && distance_xz(player, rat_pos.get()) <= RAT_SWARM_DRAIN_RADIUS
        });
        if near_rat {
            cultivation.qi_current = (cultivation.qi_current - RAT_SWARM_DRAIN_AMOUNT).max(0.0);
        }
    }
}

fn record_tutorial_breakthrough_completion(
    clock: Option<Res<CombatClock>>,
    mut outcomes: EventReader<BreakthroughOutcome>,
    mut telemetry: ResMut<TutorialTelemetry>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut players: Query<(&mut TutorialState, &mut LifeRecord)>,
) {
    let now = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for outcome in outcomes.read() {
        let Ok(success) = successful_induce_breakthrough(outcome) else {
            continue;
        };
        let Ok((mut state, mut life_record)) = players.get_mut(outcome.entity) else {
            continue;
        };
        if state.completed_at_tick.is_some() {
            continue;
        }
        state.completed_at_tick = Some(now);
        state.trigger(TutorialHook::RealmAdvancedToInduce);
        let elapsed = now.saturating_sub(state.entered_at_tick);
        let minutes = (elapsed / (20 * 60)) as u32;
        life_record.push(BiographyEntry::SpawnTutorialCompleted {
            minutes_since_spawn: minutes,
            tick: now,
        });
        telemetry.completed = telemetry.completed.saturating_add(1);
        if elapsed <= COMPLETION_WINDOW_TICKS {
            telemetry.completed_within_30min = telemetry.completed_within_30min.saturating_add(1);
        }
        let completion_rate = telemetry.completion_rate_30min();
        hook_events.send(TutorialHookEvent {
            player: outcome.entity,
            hook: TutorialHook::RealmAdvancedToInduce,
            tick: now,
        });
        tracing::info!(
            "[bong][spawn-tutorial] player {:?} completed spawn tutorial to {:?} in {} minute(s)",
            outcome.entity,
            success.to,
            minutes
        );
        tracing::info!("[bong][spawn-tutorial] 30min completion rate={completion_rate:.3}");
    }
}

fn successful_induce_breakthrough(
    outcome: &BreakthroughOutcome,
) -> Result<BreakthroughSuccess, ()> {
    match outcome.result {
        Ok(success) if outcome.from == Realm::Awaken && success.to == Realm::Induce => Ok(success),
        _ => Err(()),
    }
}

pub fn moved_at_least_200_blocks(state: &TutorialState, current: [f64; 3]) -> bool {
    state
        .spawn_position
        .is_some_and(|spawn| distance_xz_arrays(spawn, current) >= 200.0)
}

pub fn reached_lingquan(state: &TutorialState, current: [f64; 3]) -> bool {
    state
        .first_lingquan_pos
        .is_some_and(|pos| distance_xz_arrays(pos, current) <= TUTORIAL_LINGQUAN_REACH_RADIUS)
}

pub fn should_spawn_rat_swarm(state: &TutorialState, current: [f64; 3]) -> bool {
    if state.rat_swarm_spawned_at_tick.is_some()
        || !state.has(TutorialHook::FirstMeridianOpened)
        || !state.has(TutorialHook::CoffinOpened)
    {
        return false;
    }
    let (Some(last), Some(lingquan)) = (state.last_position, state.first_lingquan_pos) else {
        return false;
    };
    let last_distance = distance_xz_arrays(last, lingquan);
    let current_distance = distance_xz_arrays(current, lingquan);
    current_distance <= RAT_SWARM_TRIGGER_DISTANCE && current_distance < last_distance
}

fn nearest_lingquan_from_query(lingquans: &Query<&TutorialLingquan>) -> Option<[f64; 3]> {
    lingquans
        .iter()
        .min_by_key(|lingquan| lingquan.index)
        .map(|lingquan| lingquan.pos)
}

fn position_to_array(position: &Position) -> [f64; 3] {
    let pos = position.get();
    [pos.x, pos.y, pos.z]
}

fn distance_xz(a: DVec3, b: DVec3) -> f64 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn distance_xz_arrays(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    (dx * dx + dz * dz).sqrt()
}

fn normalized_xz_direction(from: [f64; 3], to: [f64; 3]) -> Option<[f64; 2]> {
    let dx = to[0] - from[0];
    let dz = to[2] - from[2];
    let len = (dx * dx + dz * dz).sqrt();
    if len <= f64::EPSILON {
        None
    } else {
        Some([dx / len, dz / len])
    }
}

fn parse_tag_u8(tags: &[String], key: &str) -> Option<u8> {
    let prefix = format!("{key}:");
    tags.iter()
        .find_map(|tag| tag.strip_prefix(prefix.as_str()))
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemRarity, ItemTemplate,
        MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn registry_with_spirit_niche_stone() -> ItemRegistry {
        let mut templates = HashMap::new();
        templates.insert(
            SPIRIT_NICHE_STONE_TEMPLATE_ID.to_string(),
            ItemTemplate {
                id: SPIRIT_NICHE_STONE_TEMPLATE_ID.to_string(),
                display_name: "龛石".to_string(),
                category: ItemCategory::Treasure,
                grid_w: 1,
                grid_h: 1,
                base_weight: 0.4,
                rarity: ItemRarity::Rare,
                spirit_quality_initial: 0.2,
                description: "test".to_string(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
            },
        );
        ItemRegistry::from_map(templates)
    }

    fn empty_inventory() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 3,
                cols: 3,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    #[test]
    fn coffin_open_grants_spirit_niche_once_per_player_state() {
        let registry = registry_with_spirit_niche_stone();
        let mut allocator = InventoryInstanceIdAllocator::new(100);
        let mut state = TutorialState::new(0);
        let mut inventory = empty_inventory();

        let first = grant_coffin_reward_once(
            &mut state,
            &mut inventory,
            &registry,
            &mut allocator,
            [0, 69, 0],
        );
        assert!(matches!(
            first,
            CoffinGrantOutcome::Granted { instance_id: 100 }
        ));
        assert_eq!(inventory.containers[0].items.len(), 1);
        assert!(state.has(TutorialHook::CoffinOpened));

        let second = grant_coffin_reward_once(
            &mut state,
            &mut inventory,
            &registry,
            &mut allocator,
            [0, 69, 0],
        );
        assert_eq!(second, CoffinGrantOutcome::AlreadyOpened);
        assert_eq!(inventory.containers[0].items.len(), 1);
    }

    #[test]
    fn moved_200_blocks_uses_spawn_anchor_not_last_position() {
        let mut state = TutorialState::new(0);
        state.spawn_position = Some([8.0, 70.0, 8.0]);
        state.last_position = Some([180.0, 70.0, 8.0]);

        assert!(!moved_at_least_200_blocks(&state, [190.0, 70.0, 8.0]));
        assert!(moved_at_least_200_blocks(&state, [210.0, 70.0, 8.0]));
    }

    #[test]
    fn rat_swarm_requires_coffin_first_meridian_and_movement_toward_lingquan() {
        let mut state = TutorialState::new(0);
        state.trigger(TutorialHook::CoffinOpened);
        state.trigger(TutorialHook::FirstMeridianOpened);
        state.last_position = Some([0.0, 70.0, 90.0]);
        state.first_lingquan_pos = Some([0.0, 70.0, 0.0]);

        assert!(should_spawn_rat_swarm(&state, [0.0, 70.0, 70.0]));
        assert!(!should_spawn_rat_swarm(&state, [0.0, 70.0, 110.0]));

        state.rat_swarm_spawned_at_tick = Some(12);
        assert!(!should_spawn_rat_swarm(&state, [0.0, 70.0, 60.0]));
    }

    #[test]
    fn telemetry_rate_handles_zero_and_completed_counts() {
        let mut telemetry = TutorialTelemetry::default();
        assert_eq!(telemetry.completion_rate_30min(), 0.0);
        telemetry.started = 4;
        telemetry.completed_within_30min = 3;
        assert_eq!(telemetry.completion_rate_30min(), 0.75);
    }
}
