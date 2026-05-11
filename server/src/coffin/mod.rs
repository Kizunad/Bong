use std::collections::HashMap;

use valence::entity::entity::Flags;
use valence::prelude::{
    bevy_ecs, Added, App, BlockPos, BlockState, ChunkLayer, Client, Commands, Component, DVec3,
    DiggingEvent, Entity, Event, EventReader, EventWriter, GameMode, IntoSystemConfigs, Position,
    Query, Res, ResMut, Resource, SneakEvent, SneakState, Update, Username, VisibleChunkLayer,
    With,
};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::craft::{
    CraftCategory, CraftRecipe, CraftRegistry, CraftRequirements, RecipeId, RegistryError,
    UnlockSource,
};
use crate::cultivation::components::Cultivation;
use crate::cultivation::lifespan::LifespanComponent;
use crate::inventory::{
    add_item_to_player_inventory, consume_item_instance_once, inventory_item_by_instance,
    InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::inventory_snapshot_emit::send_inventory_snapshot_to_client;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::player::state::PlayerState;
use crate::schema::server_data::{CoffinStateV1, ServerDataPayloadV1, ServerDataV1};
use crate::world::block_break::should_apply_default_break;

pub const MUNDANE_COFFIN_ITEM_ID: &str = "mundane_coffin";
pub const COFFIN_LIFESPAN_FACTOR: f64 = 0.9;

const COFFIN_AMBIENT_INTERVAL_TICKS: u64 = 3 * TICKS_PER_SECOND;
const COFFIN_INTERACT_MAX_DISTANCE_SQ: f64 = 36.0;

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct CoffinComponent {
    pub entered_at_tick: u64,
    pub coffin_lower: BlockPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoffinEntity {
    pub lower: BlockPos,
    pub upper: BlockPos,
    pub occupied_by: Option<Entity>,
    pub placed_at_tick: u64,
}

#[derive(Debug, Default, Resource)]
pub struct CoffinRegistry {
    pub coffins: HashMap<BlockPos, CoffinEntity>,
    pub player_in_coffin: HashMap<Entity, BlockPos>,
}

impl CoffinRegistry {
    pub fn insert(&mut self, lower: BlockPos, placed_at_tick: u64) -> bool {
        let upper = coffin_upper_half(lower);
        if self.coffins.contains_key(&lower) || self.coffins.contains_key(&upper) {
            return false;
        }

        let coffin = CoffinEntity {
            lower,
            upper,
            occupied_by: None,
            placed_at_tick,
        };
        self.write_coffin(coffin);
        true
    }

    pub fn lookup(&self, pos: BlockPos) -> Option<CoffinEntity> {
        self.coffins.get(&pos).copied()
    }

    pub fn set_occupied(&mut self, lower: BlockPos, player: Entity) -> bool {
        let Some(mut coffin) = self.lookup(lower) else {
            return false;
        };
        if coffin.occupied_by.is_some() {
            return false;
        }

        coffin.occupied_by = Some(player);
        self.write_coffin(coffin);
        self.player_in_coffin.insert(player, coffin.lower);
        true
    }

    pub fn clear_player(&mut self, player: Entity) -> Option<BlockPos> {
        let lower = self.player_in_coffin.remove(&player)?;
        if let Some(mut coffin) = self.lookup(lower) {
            if coffin.occupied_by == Some(player) {
                coffin.occupied_by = None;
                self.write_coffin(coffin);
            }
        }
        Some(lower)
    }

    pub fn remove_by_pos(&mut self, pos: BlockPos) -> Option<CoffinEntity> {
        let coffin = self.lookup(pos)?;
        self.coffins.remove(&coffin.lower);
        self.coffins.remove(&coffin.upper);
        if let Some(player) = coffin.occupied_by {
            self.player_in_coffin.remove(&player);
        }
        Some(coffin)
    }

    fn write_coffin(&mut self, coffin: CoffinEntity) {
        self.coffins.insert(coffin.lower, coffin);
        self.coffins.insert(coffin.upper, coffin);
    }
}

#[derive(Debug, Clone, Event)]
pub struct CoffinPlaceRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub item_instance_id: u64,
    pub tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinEnterRequest {
    pub player: Entity,
    pub pos: BlockPos,
    pub tick: u64,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinLeaveRequest {
    pub player: Entity,
}

#[derive(Debug, Clone, Event)]
pub struct CoffinStateChanged {
    pub player: Entity,
    pub in_coffin: bool,
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][coffin] registering mundane coffin subsystem (plan-coffin-v1)");
    app.insert_resource(CoffinRegistry::default());
    app.add_event::<CoffinPlaceRequest>();
    app.add_event::<CoffinEnterRequest>();
    app.add_event::<CoffinLeaveRequest>();
    app.add_event::<CoffinStateChanged>();
    app.add_systems(
        Update,
        (
            handle_coffin_place_requests,
            handle_coffin_enter_requests,
            handle_coffin_leave_requests,
            handle_sneak_leave_requests,
            handle_coffin_breaks,
            emit_coffin_ambient_audio,
        )
            .after(crate::network::client_request_handler::handle_client_request_payloads)
            .before(crate::network::audio_event_emit::emit_audio_play_payloads),
    );
    app.add_systems(
        Update,
        (
            pin_coffin_players,
            emit_coffin_state_payloads,
            emit_coffin_state_to_joined_clients
                .after(crate::player::attach_player_state_to_joined_clients),
        ),
    );
}

pub fn register_craft_recipes(registry: &mut CraftRegistry) -> Result<(), RegistryError> {
    registry.register(CraftRecipe {
        id: RecipeId::new("coffin.mundane_coffin"),
        category: CraftCategory::Misc,
        display_name: "凡物棺材".into(),
        materials: vec![("ling_mu_ban".into(), 6), ("ling_mu_gun".into(), 2)],
        qi_cost: 0.0,
        time_ticks: 90 * TICKS_PER_SECOND,
        output: (MUNDANE_COFFIN_ITEM_ID.into(), 1),
        requirements: CraftRequirements::default(),
        unlock_sources: vec![UnlockSource::Scroll {
            item_template: "scroll_mundane_coffin".into(),
        }],
    })
}

pub fn coffin_lifespan_multiplier(in_coffin: bool) -> f64 {
    if in_coffin {
        COFFIN_LIFESPAN_FACTOR
    } else {
        1.0
    }
}

#[allow(clippy::type_complexity)]
fn handle_coffin_place_requests(
    mut events: EventReader<CoffinPlaceRequest>,
    mut registry: ResMut<CoffinRegistry>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    mut layers: Query<&mut ChunkLayer, With<crate::world::dimension::OverworldLayer>>,
    mut players: Query<
        (
            &Username,
            &mut Client,
            &PlayerState,
            Option<&Cultivation>,
            &mut PlayerInventory,
            &Position,
        ),
        With<Client>,
    >,
) {
    for event in events.read() {
        let Ok((username, mut client, player_state, cultivation, mut inventory, position)) =
            players.get_mut(event.player)
        else {
            tracing::warn!(
                "[bong][coffin] place rejected: player {:?} has no inventory/client state",
                event.player
            );
            continue;
        };

        if !coffin_target_is_close(position, event.pos) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: target {:?} too far",
                username.0,
                event.pos
            );
            continue;
        }

        let Some(instance) = inventory_item_by_instance(&inventory, event.item_instance_id) else {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: missing item instance {}",
                username.0,
                event.item_instance_id
            );
            continue;
        };
        if instance.template_id != MUNDANE_COFFIN_ITEM_ID {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: item `{}` is not a mundane coffin",
                username.0,
                instance.template_id
            );
            continue;
        }

        let upper = coffin_upper_half(event.pos);
        if registry.lookup(event.pos).is_some() || registry.lookup(upper).is_some() {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: target {:?}/{:?} already registered",
                username.0,
                event.pos,
                upper
            );
            continue;
        }
        if let Ok(layer) = layers.get_single() {
            if !block_is_air(layer, event.pos) || !block_is_air(layer, upper) {
                tracing::warn!(
                    "[bong][coffin] place rejected for `{}`: target {:?}/{:?} not empty",
                    username.0,
                    event.pos,
                    upper
                );
                continue;
            }
        }

        if let Err(error) = consume_item_instance_once(&mut inventory, event.item_instance_id) {
            tracing::warn!(
                "[bong][coffin] place rejected for `{}`: consume failed: {error}",
                username.0
            );
            continue;
        }
        if !registry.insert(event.pos, event.tick) {
            if let Err(error) = add_item_to_player_inventory(
                &mut inventory,
                &item_registry,
                &mut allocator,
                MUNDANE_COFFIN_ITEM_ID,
                1,
            ) {
                tracing::warn!(
                    "[bong][coffin] failed to refund rejected coffin placement for `{}`: {error}",
                    username.0
                );
            }
            continue;
        }

        if let Ok(mut layer) = layers.get_single_mut() {
            layer.set_block(event.pos, BlockState::CHEST);
            layer.set_block(upper, BlockState::CHEST);
        }

        let default_cultivation = Cultivation::default();
        send_inventory_snapshot_to_client(
            event.player,
            &mut client,
            username.0.as_str(),
            &inventory,
            player_state,
            cultivation.unwrap_or(&default_cultivation),
            "coffin_place_consumed",
        );
        tracing::info!(
            "[bong][coffin] placed mundane coffin for `{}` at {:?}/{:?}",
            username.0,
            event.pos,
            upper
        );
    }
}

#[allow(clippy::type_complexity)]
fn handle_coffin_enter_requests(
    mut events: EventReader<CoffinEnterRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    mut players: Query<
        (
            &mut Position,
            Option<&mut Flags>,
            Option<&Username>,
            Option<&LifespanComponent>,
            Option<&CoffinComponent>,
        ),
        With<Client>,
    >,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let Some(coffin) = registry.lookup(event.pos) else {
            tracing::warn!(
                "[bong][coffin] enter rejected: no registered coffin at {:?}",
                event.pos
            );
            continue;
        };
        if coffin.occupied_by.is_some() {
            tracing::warn!(
                "[bong][coffin] enter rejected: coffin {:?} already occupied",
                coffin.lower
            );
            continue;
        }

        let Ok((mut position, flags, username, lifespan, current_coffin)) =
            players.get_mut(event.player)
        else {
            continue;
        };
        if current_coffin.is_some() || !coffin_target_is_close(&position, event.pos) {
            continue;
        }
        if !registry.set_occupied(coffin.lower, event.player) {
            continue;
        }

        if let Some(mut flags) = flags {
            flags.set_invisible(true);
        }
        position.set(coffin_player_position(coffin.lower));
        commands.entity(event.player).insert(CoffinComponent {
            entered_at_tick: event.tick,
            coffin_lower: coffin.lower,
        });
        persist_in_coffin(player_persistence.as_deref(), username, lifespan, true);
        state_events.send(CoffinStateChanged {
            player: event.player,
            in_coffin: true,
        });
        play_coffin_audio(
            &mut audio_events,
            "coffin_enter",
            event.player,
            Some(coffin.lower),
        );
    }
}

fn handle_coffin_leave_requests(
    mut events: EventReader<CoffinLeaveRequest>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    mut players: Query<
        (
            &mut Position,
            Option<&mut Flags>,
            Option<&Username>,
            Option<&LifespanComponent>,
            Option<&CoffinComponent>,
        ),
        With<Client>,
    >,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in events.read() {
        let Ok((mut position, flags, username, lifespan, current_coffin)) =
            players.get_mut(event.player)
        else {
            continue;
        };
        let Some(current_coffin) = current_coffin else {
            continue;
        };
        let lower = registry
            .clear_player(event.player)
            .unwrap_or(current_coffin.coffin_lower);
        if let Some(mut flags) = flags {
            flags.set_invisible(false);
        }
        position.set(coffin_exit_position(lower));
        commands.entity(event.player).remove::<CoffinComponent>();
        persist_in_coffin(player_persistence.as_deref(), username, lifespan, false);
        state_events.send(CoffinStateChanged {
            player: event.player,
            in_coffin: false,
        });
        play_coffin_audio(&mut audio_events, "coffin_exit", event.player, Some(lower));
    }
}

fn handle_sneak_leave_requests(
    mut sneaks: EventReader<SneakEvent>,
    players: Query<&CoffinComponent, With<Client>>,
    mut leave_tx: EventWriter<CoffinLeaveRequest>,
) {
    for event in sneaks.read() {
        if event.state != SneakState::Start || players.get(event.client).is_err() {
            continue;
        }
        leave_tx.send(CoffinLeaveRequest {
            player: event.client,
        });
    }
}

#[allow(clippy::type_complexity)]
fn handle_coffin_breaks(
    mut digs: EventReader<DiggingEvent>,
    mut commands: Commands,
    mut registry: ResMut<CoffinRegistry>,
    mut layers: Query<&mut ChunkLayer>,
    mut players: Query<
        (
            &GameMode,
            &VisibleChunkLayer,
            Option<&mut PlayerInventory>,
            Option<&Username>,
            &mut Client,
            Option<&PlayerState>,
            Option<&Cultivation>,
            Option<&mut Position>,
            Option<&mut Flags>,
            Option<&LifespanComponent>,
        ),
        With<Client>,
    >,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    player_persistence: Option<Res<crate::player::state::PlayerStatePersistence>>,
    mut state_events: EventWriter<CoffinStateChanged>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for event in digs.read() {
        let should_break = players
            .get(event.client)
            .map(|(game_mode, ..)| should_apply_default_break(event.state, *game_mode))
            .unwrap_or(false);
        if !should_break {
            continue;
        }
        let Some(coffin) = registry.remove_by_pos(event.position) else {
            continue;
        };

        if let Ok((_, visible_layer, ..)) = players.get(event.client) {
            if let Ok(mut layer) = layers.get_mut(visible_layer.0) {
                layer.set_block(coffin.lower, BlockState::AIR);
                layer.set_block(coffin.upper, BlockState::AIR);
            }
        } else if let Ok(mut layer) = layers.get_single_mut() {
            layer.set_block(coffin.lower, BlockState::AIR);
            layer.set_block(coffin.upper, BlockState::AIR);
        }

        if let Some(occupant) = coffin.occupied_by {
            commands.entity(occupant).remove::<CoffinComponent>();
            if let Ok((_, _, _, username, _, _, _, position, flags, lifespan)) =
                players.get_mut(occupant)
            {
                if let Some(mut position) = position {
                    position.set(coffin_exit_position(coffin.lower));
                }
                if let Some(mut flags) = flags {
                    flags.set_invisible(false);
                }
                persist_in_coffin(player_persistence.as_deref(), username, lifespan, false);
            }
            state_events.send(CoffinStateChanged {
                player: occupant,
                in_coffin: false,
            });
        }

        if let (Some(item_registry), Some(allocator)) =
            (item_registry.as_deref(), allocator.as_deref_mut())
        {
            if let Ok((
                _,
                _,
                Some(mut inventory),
                Some(username),
                mut client,
                player_state,
                cultivation,
                ..,
            )) = players.get_mut(event.client)
            {
                if let Err(error) = add_item_to_player_inventory(
                    &mut inventory,
                    item_registry,
                    allocator,
                    MUNDANE_COFFIN_ITEM_ID,
                    1,
                ) {
                    tracing::warn!(
                        "[bong][coffin] failed to return coffin item to `{}` after break: {error}",
                        username.0
                    );
                } else if let Some(player_state) = player_state {
                    let default_cultivation = Cultivation::default();
                    send_inventory_snapshot_to_client(
                        event.client,
                        &mut client,
                        username.0.as_str(),
                        &inventory,
                        player_state,
                        cultivation.unwrap_or(&default_cultivation),
                        "coffin_break_returned",
                    );
                }
            }
        }
        play_coffin_audio(
            &mut audio_events,
            "coffin_break",
            event.client,
            Some(coffin.lower),
        );
    }
}

fn pin_coffin_players(mut players: Query<(&CoffinComponent, &mut Position), With<Client>>) {
    for (coffin, mut position) in &mut players {
        position.set(coffin_player_position(coffin.coffin_lower));
    }
}

fn emit_coffin_ambient_audio(
    clock: Option<Res<CombatClock>>,
    players: Query<(Entity, &CoffinComponent), With<Client>>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    let Some(clock) = clock else {
        return;
    };
    if !clock.tick.is_multiple_of(COFFIN_AMBIENT_INTERVAL_TICKS) {
        return;
    }
    for (player, coffin) in &players {
        play_coffin_audio(
            &mut audio_events,
            "coffin_ambient",
            player,
            Some(coffin.coffin_lower),
        );
    }
}

fn emit_coffin_state_payloads(
    mut events: EventReader<CoffinStateChanged>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.player) else {
            continue;
        };
        send_coffin_state(&mut client, event.in_coffin);
    }
}

fn emit_coffin_state_to_joined_clients(
    mut clients: Query<(&mut Client, Option<&CoffinComponent>), Added<Client>>,
) {
    for (mut client, coffin) in &mut clients {
        send_coffin_state(&mut client, coffin.is_some());
    }
}

fn send_coffin_state(client: &mut Client, in_coffin: bool) {
    let payload = ServerDataV1::new(ServerDataPayloadV1::CoffinState(CoffinStateV1 {
        in_coffin,
        lifespan_rate_multiplier: coffin_lifespan_multiplier(in_coffin),
    }));
    let payload_type = payload_type_label(payload.payload_type());
    match serialize_server_data_payload(&payload) {
        Ok(bytes) => send_server_data_payload(client, bytes.as_slice()),
        Err(error) => log_payload_build_error(payload_type, &error),
    }
}

fn play_coffin_audio(
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
    recipe_id: &str,
    player: Entity,
    pos: Option<BlockPos>,
) {
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: recipe_id.to_string(),
        instance_id: 0,
        pos: pos.map(block_pos_array),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player),
    });
}

fn persist_in_coffin(
    player_persistence: Option<&crate::player::state::PlayerStatePersistence>,
    username: Option<&Username>,
    lifespan: Option<&LifespanComponent>,
    in_coffin: bool,
) {
    let (Some(player_persistence), Some(username), Some(lifespan)) =
        (player_persistence, username, lifespan)
    else {
        return;
    };
    if let Err(error) = crate::player::state::save_player_lifespan_slice_with_coffin(
        player_persistence,
        username.0.as_str(),
        lifespan,
        in_coffin,
    ) {
        tracing::warn!(
            "[bong][coffin] failed to persist in_coffin={} for `{}`: {error}",
            in_coffin,
            username.0
        );
    }
}

fn coffin_upper_half(lower: BlockPos) -> BlockPos {
    BlockPos::new(lower.x + 1, lower.y, lower.z)
}

fn block_is_air(layer: &ChunkLayer, pos: BlockPos) -> bool {
    layer
        .block(pos)
        .map(|block| block.state == BlockState::AIR)
        .unwrap_or(true)
}

fn coffin_target_is_close(position: &Position, target: BlockPos) -> bool {
    let target_center = DVec3::new(
        f64::from(target.x) + 0.5,
        f64::from(target.y) + 0.5,
        f64::from(target.z) + 0.5,
    );
    position.get().distance_squared(target_center) <= COFFIN_INTERACT_MAX_DISTANCE_SQ
}

fn coffin_player_position(lower: BlockPos) -> [f64; 3] {
    [
        f64::from(lower.x) + 0.5,
        f64::from(lower.y) + 0.05,
        f64::from(lower.z) + 0.5,
    ]
}

fn coffin_exit_position(lower: BlockPos) -> [f64; 3] {
    [
        f64::from(lower.x) - 0.5,
        f64::from(lower.y) + 0.05,
        f64::from(lower.z) + 0.5,
    ]
}

fn block_pos_array(pos: BlockPos) -> [i32; 3] {
    [pos.x, pos.y, pos.z]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_registers_and_removes_both_halves() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(8, 64, 8);
        let upper = coffin_upper_half(lower);

        assert!(registry.insert(lower, 10));
        assert_eq!(registry.lookup(lower).unwrap().lower, lower);
        assert_eq!(registry.lookup(upper).unwrap().upper, upper);

        let removed = registry.remove_by_pos(upper).expect("coffin should remove");
        assert_eq!(removed.lower, lower);
        assert!(registry.lookup(lower).is_none());
        assert!(registry.lookup(upper).is_none());
    }

    #[test]
    fn registry_tracks_occupancy_by_player() {
        let mut registry = CoffinRegistry::default();
        let lower = BlockPos::new(8, 64, 8);
        let player = Entity::from_raw(7);

        assert!(registry.insert(lower, 10));
        assert!(registry.set_occupied(lower, player));
        assert!(!registry.set_occupied(lower, Entity::from_raw(8)));
        assert_eq!(registry.player_in_coffin.get(&player), Some(&lower));
        assert_eq!(registry.lookup(lower).unwrap().occupied_by, Some(player));

        assert_eq!(registry.clear_player(player), Some(lower));
        assert!(registry.player_in_coffin.get(&player).is_none());
        assert_eq!(registry.lookup(lower).unwrap().occupied_by, None);
    }

    #[test]
    fn coffin_lifespan_factor_is_plan_value() {
        assert_eq!(COFFIN_LIFESPAN_FACTOR, 0.9);
        assert_eq!(coffin_lifespan_multiplier(true), 0.9);
        assert_eq!(coffin_lifespan_multiplier(false), 1.0);
    }
}
