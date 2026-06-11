//! Spawn tutorial state machine (plan-spawn-tutorial-v1).
//!
//! v1 keeps the tutorial silent: no quest UI, no explicit progress packet. The
//! server only records player-driven hooks, grants the coffin spirit niche
//! base once per player, and spawns tutorial rats that drain qi through the
//! shared RatBiteEvent path.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, Added, App, Client, Commands, Component, DVec3, Entity, EntityLayerId, Event,
    EventReader, EventWriter, IntoSystemConfigs, Local, Position, Query, Res, ResMut, Resource,
    Update, Username, With, Without,
};

use crate::alchemy::learned::LearnedRecipes;
use crate::combat::rat_bite::RatBiteEvent;
use crate::combat::CombatClock;
use crate::cultivation::breakthrough::{BreakthroughOutcome, BreakthroughSuccess};
use crate::cultivation::components::{Cultivation, MeridianSystem, Realm};
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};
use crate::forge::learned::LearnedBlueprints;
use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::npc::lifecycle::{NpcArchetype, NpcSpawnNotice, NpcSpawnSource};
use crate::npc::spawn::{
    snap_spawn_y_to_surface, spawn_notice, spawn_rogue_npc_at, NpcSkinSpawnContext,
};
use crate::npc::spawn_rat::spawn_rat_npc_at;
use crate::persistence::{load_player_cultivation_bundle, PersistenceSettings};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;
use crate::skin::{NpcSkinFallbackPolicy, SkinPool};
use crate::world::dimension::DimensionLayers;
use crate::world::terrain::TerrainProviders;
use crate::world::tsy_container::{ContainerKind, LootContainer};
use crate::world::zone::TsyDepth;
use crate::world::zone::DEFAULT_SPAWN_ZONE_NAME;

pub const SPIRIT_NICHE_BASE_TEMPLATE_ID: &str = "niche_base";
pub const TUTORIAL_KAIMAI_LOOT_POOL_ID: &str = "tutorial_kaimai_chest";
pub const COFFIN_OPEN_INTERACT_RADIUS: f64 = 6.0;
pub const TUTORIAL_LINGQUAN_REACH_RADIUS: f64 = 8.0;
pub const RAT_SWARM_SPAWN_DISTANCE: f64 = 20.0;
pub const RAT_SWARM_TRIGGER_DISTANCE: f64 = 80.0;
pub const RAT_SWARM_DRAIN_RADIUS: f64 = 4.5;
pub const RAT_SWARM_DRAIN_AMOUNT: f64 = 1.0;
pub const COMPLETION_WINDOW_TICKS: u64 = 30 * 60 * 20;

/// Base material template IDs whose presence in inventory triggers the CraftHintShown toast.
pub const BASE_MATERIAL_IDS: &[&str] = &[
    "fan_tie",
    "shou_gu",
    "zhu_pi",
    "ci_she_hao",
    "hui_yuan_zhi",
    "ling_shui",
];

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
    CraftHintShown,
    FirstAlchemyHint,
    FirstForgeHint,
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
    app.add_systems(Update, spawn_tutorial_poi_markers);
    app.add_systems(
        Update,
        (
            attach_tutorial_state_to_joined_clients,
            handle_coffin_open_requests,
            tutorial_hook_state_machine,
            dynamic_rat_swarm_spawner.after(tutorial_hook_state_machine),
            tutorial_rat_qi_drain_tick.after(dynamic_rat_swarm_spawner),
            record_tutorial_breakthrough_completion,
            check_craft_hint_on_inventory,
            check_first_alchemy_hint,
            check_first_forge_hint,
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
        let state = tutorial_state_for_join(restored, now, &mut telemetry);
        commands.entity(entity).insert(state);
    }
}

fn tutorial_state_for_join(
    restored: Option<TutorialState>,
    now: u64,
    telemetry: &mut TutorialTelemetry,
) -> TutorialState {
    if let Some(state) = restored {
        return state;
    }
    telemetry.started = telemetry.started.saturating_add(1);
    TutorialState::new(now)
}

fn spawn_tutorial_poi_markers(
    mut commands: Commands,
    mut notices: EventWriter<NpcSpawnNotice>,
    mut skin_pool: Option<ResMut<SkinPool>>,
    providers: Option<Res<TerrainProviders>>,
    layers: Option<Res<DimensionLayers>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let (Some(providers), Some(layers)) = (providers, layers) else {
        return;
    };
    if let Some(pool) = skin_pool.as_deref_mut() {
        pool.drain_ready();
        if !pool.ready_for_spawn() {
            return;
        }
    }

    let mut coffin_count = 0usize;
    let mut lingquan_count = 0usize;
    let mut chest_count = 0usize;
    let mut rogue_count = 0usize;
    let first_lingquan = providers
        .overworld
        .pois()
        .iter()
        .filter(|poi| poi.kind == "tutorial_lingquan")
        .min_by_key(|poi| parse_tag_u8(&poi.tags, "index").unwrap_or(u8::MAX))
        .map(|poi| poi_pos_dvec3(poi.pos_xyz));
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
            "tutorial_chest" => {
                commands.spawn((
                    LootContainer::new(
                        ContainerKind::StoragePouch,
                        "spawn_tutorial".to_string(),
                        TsyDepth::Shallow,
                        TUTORIAL_KAIMAI_LOOT_POOL_ID.to_string(),
                        0,
                    ),
                    Position(poi_pos_dvec3(poi.pos_xyz)),
                    EntityLayerId(layers.overworld),
                ));
                chest_count += 1;
            }
            "tutorial_rogue_anchor" => {
                let raw_pos = poi_pos_dvec3(poi.pos_xyz);
                let pos = snap_spawn_y_to_surface(raw_pos, Some(&providers.overworld));
                let patrol_target = first_lingquan.unwrap_or(pos);
                let entity = spawn_rogue_npc_at(
                    &mut commands,
                    NpcSkinSpawnContext::new(
                        skin_pool.as_deref_mut(),
                        NpcSkinFallbackPolicy::AllowFallback,
                    ),
                    layers.overworld,
                    poi.zone.as_str(),
                    pos,
                    patrol_target,
                    Realm::Awaken,
                    0.0,
                );
                notices.send(spawn_notice(
                    entity,
                    NpcArchetype::Rogue,
                    NpcSpawnSource::Startup,
                    poi.zone.as_str(),
                    pos,
                    0.0,
                ));
                rogue_count += 1;
            }
            _ => {}
        }
    }

    tracing::info!(
        "[bong][spawn-tutorial] spawned {coffin_count} coffin marker(s), {lingquan_count} lingquan marker(s), {chest_count} chest marker(s), {rogue_count} rogue(s) from POIs; client channel={SERVER_DATA_CHANNEL}"
    );
    *done = true;
}

fn handle_coffin_open_requests(
    mut requests: EventReader<CoffinOpenRequest>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut players: Query<(
        &mut TutorialState,
        &mut PlayerInventory,
        &Position,
        Option<&EntityLayerId>,
    )>,
    coffins: Query<(&TutorialCoffin, &Position, Option<&EntityLayerId>)>,
) {
    for request in requests.read() {
        let Some((_, coffin_position, coffin_layer)) = coffins
            .iter()
            .find(|(coffin, _, _)| coffin.pos == request.pos)
        else {
            tracing::warn!(
                "[bong][spawn-tutorial] rejected coffin_open from {:?}: no tutorial coffin at {:?}",
                request.player,
                request.pos
            );
            continue;
        };
        let Ok((mut state, mut inventory, player_position, player_layer)) =
            players.get_mut(request.player)
        else {
            continue;
        };
        if let (Some(player_layer), Some(coffin_layer)) = (player_layer, coffin_layer) {
            if player_layer.0 != coffin_layer.0 {
                tracing::warn!(
                    "[bong][spawn-tutorial] rejected coffin_open from {:?}: dimension mismatch",
                    request.player
                );
                continue;
            }
        }
        if !coffin_open_in_range(player_position.get(), coffin_position.get()) {
            tracing::warn!(
                "[bong][spawn-tutorial] rejected coffin_open from {:?}: player too far from {:?}",
                request.player,
                request.pos
            );
            continue;
        }
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
        SPIRIT_NICHE_BASE_TEMPLATE_ID,
        1,
        0,
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

fn coffin_open_in_range(player_pos: DVec3, coffin_pos: DVec3) -> bool {
    let delta = player_pos - coffin_pos;
    delta.length() <= COFFIN_OPEN_INTERACT_RADIUS
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
            let rat = spawn_rat_npc_at(
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
    rats: Query<(Entity, &Position, &TutorialRatSwarmNpc)>,
    players: Query<(Entity, &Position, &Cultivation), With<TutorialState>>,
    mut bites: EventWriter<RatBiteEvent>,
) {
    let Some(clock) = clock else {
        return;
    };
    if clock.tick % 20 != 0 {
        return;
    }

    for (player_entity, player_pos, cultivation) in &players {
        if cultivation.qi_current <= 0.0 {
            continue;
        }
        let player = player_pos.get();
        let near_rat = rats.iter().find_map(|(rat_entity, rat_pos, rat)| {
            (rat.spawned_for == player_entity
                && clock.tick.saturating_sub(rat.spawned_at_tick) <= 10 * 60 * 20
                && distance_xz(player, rat_pos.get()) <= RAT_SWARM_DRAIN_RADIUS)
                .then_some(rat_entity)
        });
        if let Some(rat_entity) = near_rat {
            bites.send(RatBiteEvent {
                rat: rat_entity,
                target: player_entity,
                qi_steal: RAT_SWARM_DRAIN_AMOUNT as u32,
            });
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

/// Returns `true` when the player's inventory contains at least one item whose
/// `template_id` is in [`BASE_MATERIAL_IDS`].
pub fn inventory_has_base_material(inventory: &PlayerInventory) -> bool {
    inventory.containers.iter().any(|c| {
        c.items
            .iter()
            .any(|item| BASE_MATERIAL_IDS.contains(&item.instance.template_id.as_str()))
    })
}

/// P2.1 -- When the player first picks up a base material, fire
/// `CraftHintShown` and push a perception toast.
fn check_craft_hint_on_inventory(
    clock: Option<Res<CombatClock>>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut players: Query<(Entity, &Username, &PlayerInventory, &mut TutorialState)>,
) {
    let now = clock.as_deref().map(|c| c.tick).unwrap_or_default();
    for (entity, username, inventory, mut state) in &mut players {
        if state.has(TutorialHook::CraftHintShown) {
            continue;
        }
        if !inventory_has_base_material(inventory) {
            continue;
        }
        if state.trigger(TutorialHook::CraftHintShown) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::CraftHintShown,
                tick: now,
            });
            if let Some(ref mut narr) = narrations {
                narr.push_player(
                    username.0.as_str(),
                    "背包中有了基础材料，可以尝试手搓合成。",
                    NarrationStyle::Perception,
                );
            }
        }
    }
}

/// P2.4 -- When the player has learned at least one recipe and is at
/// Induce realm or above, fire `FirstAlchemyHint` and push a perception toast.
fn check_first_alchemy_hint(
    clock: Option<Res<CombatClock>>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut players: Query<(
        Entity,
        &Username,
        &Cultivation,
        &LearnedRecipes,
        &mut TutorialState,
    )>,
) {
    let now = clock.as_deref().map(|c| c.tick).unwrap_or_default();
    for (entity, username, cultivation, learned, mut state) in &mut players {
        if state.has(TutorialHook::FirstAlchemyHint) {
            continue;
        }
        let at_induce_or_above = matches!(
            cultivation.realm,
            Realm::Induce | Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
        );
        if !at_induce_or_above {
            continue;
        }
        if learned.ids.is_empty() && learned.partial.is_empty() {
            continue;
        }
        if state.trigger(TutorialHook::FirstAlchemyHint) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::FirstAlchemyHint,
                tick: now,
            });
            if let Some(ref mut narr) = narrations {
                narr.push_player(
                    username.0.as_str(),
                    "已习得丹方，可以寻一座丹炉试炼了。",
                    NarrationStyle::Perception,
                );
            }
        }
    }
}

/// P2.4 -- When the player has learned at least one blueprint and is at
/// Induce realm or above, fire `FirstForgeHint` and push a perception toast.
fn check_first_forge_hint(
    clock: Option<Res<CombatClock>>,
    mut hook_events: ResMut<valence::prelude::Events<TutorialHookEvent>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    mut players: Query<(
        Entity,
        &Username,
        &Cultivation,
        &LearnedBlueprints,
        &mut TutorialState,
    )>,
) {
    let now = clock.as_deref().map(|c| c.tick).unwrap_or_default();
    for (entity, username, cultivation, learned, mut state) in &mut players {
        if state.has(TutorialHook::FirstForgeHint) {
            continue;
        }
        let at_induce_or_above = matches!(
            cultivation.realm,
            Realm::Induce | Realm::Condense | Realm::Solidify | Realm::Spirit | Realm::Void
        );
        if !at_induce_or_above {
            continue;
        }
        if learned.ids.is_empty() {
            continue;
        }
        if state.trigger(TutorialHook::FirstForgeHint) {
            hook_events.send(TutorialHookEvent {
                player: entity,
                hook: TutorialHook::FirstForgeHint,
                tick: now,
            });
            if let Some(ref mut narr) = narrations {
                narr.push_player(
                    username.0.as_str(),
                    "已习得图谱，可以找砧台试炼器了。",
                    NarrationStyle::Perception,
                );
            }
        }
    }
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

fn poi_pos_dvec3(pos: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::recipe_fragment::PartialRecipeKnowledge;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
        PlacedItemState, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;

    fn registry_with_spirit_niche_base() -> ItemRegistry {
        let mut templates = HashMap::new();
        templates.insert(
            SPIRIT_NICHE_BASE_TEMPLATE_ID.to_string(),
            ItemTemplate {
                id: SPIRIT_NICHE_BASE_TEMPLATE_ID.to_string(),
                display_name: "灵龛基座".to_string(),
                category: ItemCategory::Misc,
                placeable: None,
                max_stack_count: 1,
                grid_w: 2,
                grid_h: 2,
                base_weight: 6.0,
                rarity: ItemRarity::Rare,
                spirit_quality_initial: 0.5,
                description: "龛石灵铁木台组合的永久复活点基座。".to_string(),
                effect: None,
                cast_duration_ms: 1500,
                cooldown_ms: 1500,
                weapon_spec: None,
                forge_station_spec: None,
                blueprint_scroll_spec: None,
                inscription_scroll_spec: None,
                technique_scroll_spec: None,
                recipe_fragment_spec: None,
                container_spec: None,
                shelflife_profile: None,
                shield_spec: None,
                shelflife_track: None,
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
    fn coffin_open_grants_spirit_niche_base_once_per_player_state() {
        let registry = registry_with_spirit_niche_base();
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
        assert_eq!(
            inventory.containers[0].items[0].instance.template_id, SPIRIT_NICHE_BASE_TEMPLATE_ID,
            "coffin reward must be the placeable niche base, not the old material"
        );
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
    fn restored_tutorial_state_does_not_increment_started_telemetry() {
        let restored = TutorialState::new(12);
        let mut telemetry = TutorialTelemetry::default();
        let state = tutorial_state_for_join(Some(restored.clone()), 200, &mut telemetry);

        assert_eq!(state, restored);
        assert_eq!(telemetry.started, 0);

        let fresh = tutorial_state_for_join(None, 220, &mut telemetry);
        assert_eq!(fresh.entered_at_tick, 220);
        assert_eq!(telemetry.started, 1);
    }

    #[test]
    fn coffin_open_requires_player_proximity() {
        let coffin = DVec3::new(0.0, 69.0, 0.0);

        assert!(coffin_open_in_range(DVec3::new(2.0, 69.0, 2.0), coffin));
        assert!(!coffin_open_in_range(DVec3::new(12.0, 69.0, 0.0), coffin));
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

    // ── test helper ──────────────────────────────────────────────

    fn test_item(instance_id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn inventory_with_items(items: Vec<(&str, u64)>) -> PlayerInventory {
        let mut inv = empty_inventory();
        for (idx, (template_id, instance_id)) in items.iter().enumerate() {
            inv.containers[0].items.push(PlacedItemState {
                row: idx as u8,
                col: 0,
                instance: test_item(*instance_id, template_id),
            });
        }
        inv
    }

    // ── P2.1: CraftHintShown ─────────────────────────────────────

    #[test]
    fn craft_hint_shown_serde_roundtrip() {
        let hook = TutorialHook::CraftHintShown;
        let json = serde_json::to_string(&hook).expect("CraftHintShown should serialize");
        assert_eq!(
            json, "\"craft_hint_shown\"",
            "CraftHintShown serde rename_all=snake_case must produce 'craft_hint_shown', got {json}"
        );
        let back: TutorialHook =
            serde_json::from_str(&json).expect("CraftHintShown should deserialize");
        assert_eq!(back, hook);
    }

    #[test]
    fn inventory_has_base_material_detects_fan_tie() {
        let inv = inventory_with_items(vec![("fan_tie", 1)]);
        assert!(
            inventory_has_base_material(&inv),
            "inventory with fan_tie should be detected as having base material"
        );
    }

    #[test]
    fn inventory_has_base_material_ignores_non_base() {
        let inv = inventory_with_items(vec![("spirit_niche_stone", 1)]);
        assert!(
            !inventory_has_base_material(&inv),
            "spirit_niche_stone is not a base material, should not trigger"
        );
    }

    #[test]
    fn inventory_has_base_material_empty_inventory() {
        let inv = empty_inventory();
        assert!(
            !inventory_has_base_material(&inv),
            "empty inventory should not have base materials"
        );
    }

    #[test]
    fn craft_hint_fires_once_per_player() {
        let mut state = TutorialState::new(0);
        assert!(
            !state.has(TutorialHook::CraftHintShown),
            "fresh state should not have CraftHintShown"
        );
        assert!(
            state.trigger(TutorialHook::CraftHintShown),
            "first trigger should return true (newly inserted)"
        );
        assert!(
            state.has(TutorialHook::CraftHintShown),
            "state should now have CraftHintShown"
        );
        assert!(
            !state.trigger(TutorialHook::CraftHintShown),
            "second trigger should return false (already present)"
        );
    }

    #[test]
    fn all_base_material_ids_trigger_detection() {
        for &material_id in BASE_MATERIAL_IDS {
            let inv = inventory_with_items(vec![(material_id, 42)]);
            assert!(
                inventory_has_base_material(&inv),
                "BASE_MATERIAL_IDS entry '{material_id}' should be detected in inventory"
            );
        }
    }

    // ── P2.2: recipe fragment learning flow (client_request plumbing) ──

    #[test]
    fn alchemy_learn_recipe_fragment_serde_roundtrip() {
        use crate::schema::client_request::ClientRequestV1;
        let json = r#"{"type":"alchemy_learn_recipe_fragment","v":1,"item_instance_id":4242}"#;
        let req: ClientRequestV1 = serde_json::from_str(json)
            .expect("AlchemyLearnRecipeFragment should deserialize from JSON");
        match req {
            ClientRequestV1::AlchemyLearnRecipeFragment {
                v,
                item_instance_id,
            } => {
                assert_eq!(v, 1, "version should be 1");
                assert_eq!(item_instance_id, 4242, "item_instance_id should be 4242");
            }
            other => panic!("expected AlchemyLearnRecipeFragment, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_learn_recipe_fragment_rejects_extra_fields() {
        use crate::schema::client_request::ClientRequestV1;
        let json = r#"{"type":"alchemy_learn_recipe_fragment","v":1,"item_instance_id":4242,"extra":true}"#;
        assert!(
            serde_json::from_str::<ClientRequestV1>(json).is_err(),
            "extra fields should be rejected by deny_unknown_fields"
        );
    }

    // ── P2.3: blueprint & recipe asset verification ──

    #[test]
    fn recipe_hui_yuan_pill_v0_loads_and_has_stages() {
        let registry = crate::alchemy::recipe::load_recipe_registry()
            .expect("recipe registry should load from assets");
        let recipe = registry
            .get("hui_yuan_pill_v0")
            .expect("hui_yuan_pill_v0 must exist in recipe registry");
        assert!(
            !recipe.stages.is_empty(),
            "hui_yuan_pill_v0 must have at least one stage"
        );
    }

    #[test]
    fn blueprint_iron_sword_v0_loads_and_has_steps() {
        let registry = crate::forge::blueprint::BlueprintRegistry::load_dir(
            crate::forge::blueprint::DEFAULT_BLUEPRINTS_DIR,
        )
        .expect("blueprint registry should load from assets");
        let bp = registry
            .get("iron_sword_v0")
            .expect("iron_sword_v0 must exist in blueprint registry");
        assert!(
            !bp.steps.is_empty(),
            "iron_sword_v0 must have at least one step"
        );
    }

    #[test]
    fn loot_pool_surface_stash_craft_contains_blueprint_and_fragment() {
        let registry = crate::world::loot_pool::load_loot_pool_registry()
            .expect("loot_pools.json should load");
        let pool = registry
            .get("surface_stash_craft")
            .expect("surface_stash_craft pool must exist in loot_pools.json");
        let has_blueprint = pool
            .entries
            .iter()
            .any(|entry| entry.template_id.contains("blueprint_scroll"));
        assert!(
            has_blueprint,
            "surface_stash_craft loot pool should contain at least one blueprint_scroll entry"
        );
        let has_fragment = pool
            .entries
            .iter()
            .any(|entry| entry.template_id.contains("fragment_alchemy"));
        assert!(
            has_fragment,
            "surface_stash_craft loot pool should contain at least one fragment_alchemy entry"
        );
    }

    // ── P2.4: first alchemy / forge hint logic ──

    #[test]
    fn first_alchemy_hint_serde_roundtrip() {
        let hook = TutorialHook::FirstAlchemyHint;
        let json = serde_json::to_string(&hook).expect("FirstAlchemyHint should serialize");
        assert_eq!(
            json, "\"first_alchemy_hint\"",
            "FirstAlchemyHint serde rename_all=snake_case must produce 'first_alchemy_hint', got {json}"
        );
        let back: TutorialHook =
            serde_json::from_str(&json).expect("FirstAlchemyHint should deserialize");
        assert_eq!(back, hook);
    }

    #[test]
    fn first_forge_hint_serde_roundtrip() {
        let hook = TutorialHook::FirstForgeHint;
        let json = serde_json::to_string(&hook).expect("FirstForgeHint should serialize");
        assert_eq!(
            json, "\"first_forge_hint\"",
            "FirstForgeHint serde rename_all=snake_case must produce 'first_forge_hint', got {json}"
        );
        let back: TutorialHook =
            serde_json::from_str(&json).expect("FirstForgeHint should deserialize");
        assert_eq!(back, hook);
    }

    #[test]
    fn alchemy_hint_needs_at_least_one_recipe() {
        // Simulates the logic from check_first_alchemy_hint:
        // learned recipes empty → hint should NOT fire even at correct realm
        let learned = LearnedRecipes::default();
        assert!(
            learned.ids.is_empty() && learned.partial.is_empty(),
            "default LearnedRecipes should be empty"
        );

        // With a partial recipe
        let mut learned_with_partial = LearnedRecipes::default();
        learned_with_partial.partial.push(PartialRecipeKnowledge {
            recipe_id: "hui_yuan_pill_v0".into(),
            known_stages: vec![0],
            max_quality_tier: 3,
        });
        assert!(
            !learned_with_partial.ids.is_empty() || !learned_with_partial.partial.is_empty(),
            "partial recipe should satisfy the has-recipe check"
        );

        // With a full recipe
        let mut learned_with_full = LearnedRecipes::default();
        learned_with_full.ids.push("hui_yuan_pill_v0".into());
        assert!(
            !learned_with_full.ids.is_empty(),
            "full recipe should satisfy the has-recipe check"
        );
    }

    #[test]
    fn forge_hint_needs_at_least_one_blueprint() {
        // Empty
        let learned = LearnedBlueprints::default();
        assert!(
            learned.ids.is_empty(),
            "default LearnedBlueprints should be empty"
        );

        // With a blueprint
        let mut learned_with_bp = LearnedBlueprints::default();
        learned_with_bp.ids.push("iron_sword_v0".into());
        assert!(
            !learned_with_bp.ids.is_empty(),
            "learned blueprint should satisfy the has-blueprint check"
        );
    }
}
