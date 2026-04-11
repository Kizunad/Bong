pub mod gameplay;
mod progression;
pub mod state;

use self::state::{
    load_player_state, save_player_state, PlayerState, PlayerStateAutosaveTimer,
    PlayerStatePersistence, PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS,
};
use valence::message::SendMessage;
use valence::prelude::Despawned;
use valence::prelude::{
    Added, App, ChunkLayer, Client, Commands, Entity, EntityLayer, EntityLayerId, GameMode,
    IntoSystemConfigs, Position, Query, RemovedComponents, Res, ResMut, Update, Username,
    VisibleChunkLayer, VisibleEntityLayers, With, Without,
};

const SPAWN_POSITION: [f64; 3] = [8.0, 150.0, 8.0];
const WELCOME_MESSAGE: &str =
    "Welcome to Bong! Test commands: !zones, !tpzone <zone>, !top, !gm <c|a|s>, !spawn";

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

type JoinedClientsWithoutStateQueryItem<'a> = (Entity, &'a Username);
type JoinedClientsWithoutStateQueryFilter = (Added<Client>, Without<PlayerState>);

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init/cleanup systems");
    app.insert_resource(PlayerStatePersistence::default());
    app.insert_resource(PlayerStateAutosaveTimer::default());
    gameplay::register(app);
    app.add_systems(
        Update,
        (
            init_clients,
            attach_player_state_to_joined_clients.after(init_clients),
            autosave_player_states,
            despawn_disconnected_clients,
        ),
    );
}

pub fn spawn_position() -> [f64; 3] {
    SPAWN_POSITION
}

pub fn welcome_message() -> &'static str {
    WELCOME_MESSAGE
}

pub fn initial_game_mode() -> GameMode {
    GameMode::Creative
}

fn init_clients(
    mut clients: Query<ClientInitQueryItem<'_>, Added<Client>>,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    let layer = layers.single();

    for (
        entity,
        mut client,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut position,
        mut game_mode,
    ) in &mut clients
    {
        apply_spawn_defaults(
            layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );

        client.send_chat_message(welcome_message());

        let spawn_position = spawn_position();
        tracing::info!(
            "[bong][player] initialized client entity {entity:?} at [{}, {}, {}] in Adventure",
            spawn_position[0],
            spawn_position[1],
            spawn_position[2]
        );
    }
}

pub(crate) fn attach_player_state_to_joined_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    joined_clients: Query<
        JoinedClientsWithoutStateQueryItem<'_>,
        JoinedClientsWithoutStateQueryFilter,
    >,
) {
    for (entity, username) in &joined_clients {
        let player_state = load_player_state(&persistence, username.0.as_str());
        let realm = player_state.realm.clone();
        let composite_power = player_state.composite_power();

        commands.entity(entity).insert(player_state);
        tracing::info!(
            "[bong][player] attached PlayerState to client entity {entity:?} for `{}` (realm={}, composite_power={composite_power:.3})",
            username.0,
            realm,
        );
    }
}

fn apply_spawn_defaults(
    layer: Entity,
    layer_id: &mut EntityLayerId,
    visible_chunk_layer: &mut VisibleChunkLayer,
    visible_entity_layers: &mut VisibleEntityLayers,
    position: &mut Position,
    game_mode: &mut GameMode,
) {
    layer_id.0 = layer;
    visible_chunk_layer.0 = layer;
    visible_entity_layers.0.insert(layer);
    position.set(spawn_position());
    *game_mode = initial_game_mode();
}

fn despawn_disconnected_clients(
    mut commands: Commands,
    persistence: Res<PlayerStatePersistence>,
    mut disconnected_clients: RemovedComponents<Client>,
    persisted_players: Query<(&Username, &PlayerState)>,
) {
    for entity in disconnected_clients.read() {
        if let Ok((username, player_state)) = persisted_players.get(entity) {
            match save_player_state(&persistence, username.0.as_str(), player_state) {
                Ok(path) => tracing::info!(
                    "[bong][player] saved PlayerState for disconnected client `{}` to {} before cleanup",
                    username.0,
                    path.display()
                ),
                Err(error) => tracing::warn!(
                    "[bong][player] failed to save PlayerState for disconnected client `{}`: {error}",
                    username.0,
                ),
            }
        } else {
            tracing::warn!(
                "[bong][player] disconnected client entity {entity:?} had no username/PlayerState to persist before cleanup"
            );
        }

        tracing::info!("[bong][player] cleaning up disconnected client entity {entity:?}");
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(Despawned);
        }
    }
}

fn autosave_player_states(
    persistence: Res<PlayerStatePersistence>,
    mut timer: ResMut<PlayerStateAutosaveTimer>,
    players: Query<(&Username, &PlayerState), With<Client>>,
) {
    timer.ticks += 1;
    if !timer
        .ticks
        .is_multiple_of(PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;

    for (username, player_state) in &players {
        match save_player_state(&persistence, username.0.as_str(), player_state) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] autosave failed for `{}`: {error}",
                username.0,
            ),
        }
    }

    tracing::info!(
        "[bong][player] autosaved {saved_count} PlayerState record(s) after {PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS} ticks"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, DVec3};

    #[test]
    fn spawn_defaults_are_preserved() {
        let mut app = App::new();
        let initial_layer = app.world_mut().spawn_empty().id();
        let spawn_layer = app.world_mut().spawn_empty().id();
        let mut layer_id = EntityLayerId(initial_layer);
        let mut visible_chunk_layer = VisibleChunkLayer(initial_layer);
        let mut visible_entity_layers = VisibleEntityLayers::default();
        let mut position = Position::new([0.0, 0.0, 0.0]);
        let mut game_mode = GameMode::Survival;

        visible_entity_layers.0.insert(initial_layer);

        apply_spawn_defaults(
            spawn_layer,
            &mut layer_id,
            &mut visible_chunk_layer,
            &mut visible_entity_layers,
            &mut position,
            &mut game_mode,
        );

        assert_eq!(spawn_position(), [8.0, 150.0, 8.0]);
        assert_eq!(position.get(), DVec3::new(8.0, 150.0, 8.0));
        assert_eq!(initial_game_mode(), GameMode::Creative);
        assert_eq!(game_mode, GameMode::Creative);
        assert_eq!(welcome_message(), WELCOME_MESSAGE);
        assert_eq!(layer_id.0, spawn_layer);
        assert_eq!(visible_chunk_layer.0, spawn_layer);
        assert!(visible_entity_layers.0.contains(&spawn_layer));
    }
}
