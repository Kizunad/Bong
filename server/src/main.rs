#[allow(dead_code)]
mod alchemy;
mod botany;
mod combat;
#[allow(dead_code)]
mod cultivation;
#[allow(dead_code)]
mod forge;
mod inventory;
#[allow(dead_code)]
mod lingtian;
mod network;
mod npc;
mod player;
#[allow(dead_code)]
mod schema;
mod skill;
mod world;

use crossbeam_channel::unbounded;
use network::agent_bridge::{
    spawn_mock_bridge_daemon, AgentCommand, GameEvent, NetworkBridgeResource,
};
use valence::log::LogPlugin;
use valence::prelude::*;

fn init_tracing() {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();
}

fn main() {
    init_tracing();

    let (tx_to_game, rx_from_agent) = unbounded::<AgentCommand>();
    let (tx_to_agent, rx_from_game) = unbounded::<GameEvent>();

    std::mem::drop(spawn_mock_bridge_daemon(tx_to_game, rx_from_game));

    let mut app = App::new();
    app.insert_resource(NetworkSettings {
        connection_mode: ConnectionMode::Offline,
        ..Default::default()
    })
    .insert_resource(NetworkBridgeResource::new(tx_to_agent, rx_from_agent))
    .add_plugins(DefaultPlugins.build().disable::<LogPlugin>());

    world::register(&mut app);
    player::register(&mut app);
    inventory::register(&mut app);
    botany::register(&mut app);
    skill::register(&mut app);
    cultivation::register(&mut app);
    alchemy::register(&mut app);
    combat::register(&mut app);
    forge::register(&mut app);
    lingtian::register(&mut app);
    npc::register(&mut app);
    network::register(&mut app);

    app.run();
}
