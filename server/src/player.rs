use valence::message::SendMessage;
use valence::prelude::{
    Added, App, ChunkLayer, Client, Commands, Entity, EntityLayer, EntityLayerId, GameMode,
    Position, Query, RemovedComponents, Update, VisibleChunkLayer, VisibleEntityLayers, With,
};

const SPAWN_POSITION: [f64; 3] = [8.0, 66.0, 8.0];

type ClientInitQueryItem<'a> = (
    Entity,
    &'a mut Client,
    &'a mut EntityLayerId,
    &'a mut VisibleChunkLayer,
    &'a mut VisibleEntityLayers,
    &'a mut Position,
    &'a mut GameMode,
);

pub fn register(app: &mut App) {
    tracing::info!("[bong][player] registering player init/cleanup systems");
    app.add_systems(Update, (init_clients, despawn_disconnected_clients));
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
        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        position.set(SPAWN_POSITION);
        *game_mode = GameMode::Adventure;

        client.send_chat_message("Welcome to Bong! You spawned in the test world.");

        tracing::info!(
            "[bong][player] initialized client entity {entity:?} at [{}, {}, {}] in Adventure",
            SPAWN_POSITION[0],
            SPAWN_POSITION[1],
            SPAWN_POSITION[2]
        );
    }
}

fn despawn_disconnected_clients(
    mut commands: Commands,
    mut disconnected_clients: RemovedComponents<Client>,
) {
    for entity in disconnected_clients.read() {
        tracing::info!("[bong][player] cleaning up disconnected client entity {entity:?}");
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn();
        }
    }
}
