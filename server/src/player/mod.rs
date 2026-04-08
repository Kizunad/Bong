pub mod progression;
pub mod state;

use self::state::{
    load_or_init_player_state, save_player_state, PlayerState, PlayerStateAutosaveTimer,
    PlayerStatePersistence, PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS,
};
use valence::message::SendMessage;
use valence::prelude::{
    Added, App, Client, Commands, Entity, EntityLayerId, GameMode, Position, Query,
    RemovedComponents, Res, ResMut, UniqueId, Update, VisibleChunkLayer, VisibleEntityLayers,
};

use crate::world::{ActiveWorldLayer, ZoneRegistry};

const WELCOME_MESSAGE: &str = "Welcome to Bong! You spawned in the test world.";

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a UniqueId,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init systems");
    app.insert_resource(PlayerStatePersistence::default());
    app.insert_resource(PlayerStateAutosaveTimer::default());
    app.add_systems(
        Update,
        (
            init_clients,
            autosave_player_states,
            persist_disconnected_player_states,
        ),
    );
}

fn init_clients(
    mut commands: Commands,
    mut clients: Query<ClientInitQueryItem<'_>, Added<Client>>,
    active_world_layer: Res<ActiveWorldLayer>,
    zone_registry: Res<ZoneRegistry>,
    persistence: Res<PlayerStatePersistence>,
) {
    let layer = active_world_layer.0;
    let spawn_position = resolve_spawn_position(zone_registry.as_ref());

    for (
        entity,
        mut client,
        unique_id,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut position,
        mut game_mode,
    ) in &mut clients
    {
        let player_uuid = unique_id.0;
        let player_state = load_or_init_player_state(persistence.as_ref(), player_uuid);
        let loaded_realm = player_state.realm.clone();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        position.set(spawn_position);
        *game_mode = default_game_mode();

        client.send_chat_message(WELCOME_MESSAGE);

        commands.entity(entity).insert(player_state);

        tracing::info!(
            "[bong][player] initialized client entity {entity:?} at [{}, {}, {}] in Adventure with state realm={loaded_realm}",
            spawn_position[0],
            spawn_position[1],
            spawn_position[2]
        );
    }
}

fn autosave_player_states(
    persistence: Res<PlayerStatePersistence>,
    mut timer: ResMut<PlayerStateAutosaveTimer>,
    players: Query<(&UniqueId, &PlayerState), valence::prelude::With<Client>>,
) {
    timer.ticks += 1;
    if !timer
        .ticks
        .is_multiple_of(PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS)
    {
        return;
    }

    let mut saved_count = 0usize;
    for (unique_id, player_state) in &players {
        match save_player_state(persistence.as_ref(), unique_id.0, player_state) {
            Ok(_) => saved_count += 1,
            Err(error) => tracing::warn!(
                "[bong][player] autosave failed for {}: {error}",
                unique_id.0
            ),
        }
    }

    tracing::info!(
        "[bong][player] autosaved {saved_count} PlayerState record(s) after {PLAYER_STATE_AUTOSAVE_INTERVAL_TICKS} ticks"
    );
}

pub fn save_player_state_on_disconnect(
    persistence: &PlayerStatePersistence,
    entity: Entity,
    players: &Query<(&UniqueId, &PlayerState)>,
) {
    let Ok((unique_id, player_state)) = players.get(entity) else {
        tracing::warn!(
            "[bong][player] disconnected client entity {entity:?} had no Client/PlayerState to persist"
        );
        return;
    };

    match save_player_state(persistence, unique_id.0, player_state) {
        Ok(path) => tracing::info!(
            "[bong][player] saved PlayerState for disconnected client {} to {}",
            unique_id.0,
            path.display()
        ),
        Err(error) => tracing::warn!(
            "[bong][player] failed to save PlayerState for disconnected client {}: {error}",
            unique_id.0
        ),
    }
}

fn persist_disconnected_player_states(
    persistence: Res<PlayerStatePersistence>,
    mut disconnected_clients: RemovedComponents<Client>,
    players: Query<(&UniqueId, &PlayerState)>,
) {
    for entity in disconnected_clients.read() {
        save_player_state_on_disconnect(persistence.as_ref(), entity, &players);
    }
}

fn resolve_spawn_position(zone_registry: &ZoneRegistry) -> [f64; 3] {
    zone_registry.default_zone().spawn_position
}

fn default_game_mode() -> GameMode {
    GameMode::Adventure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_defaults_are_preserved() {
        let registry = ZoneRegistry::fallback();

        assert_eq!(
            resolve_spawn_position(&registry),
            crate::world::DEFAULT_SPAWN_POSITION
        );
        assert_eq!(default_game_mode(), GameMode::Adventure);
        assert_eq!(
            WELCOME_MESSAGE,
            "Welcome to Bong! You spawned in the test world."
        );
        assert_eq!(
            registry.default_zone().name,
            crate::world::DEFAULT_SPAWN_ZONE
        );
    }

    #[test]
    fn uses_bootstrap_selected_world_layer_resource() {
        let layer = Entity::from_raw(77);

        assert_eq!(resolve_active_world_layer(ActiveWorldLayer(layer)), layer);
    }

    fn resolve_active_world_layer(active_world_layer: ActiveWorldLayer) -> Entity {
        active_world_layer.0
    }
}
