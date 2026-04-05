pub mod agent_bridge;

use agent_bridge::{
    build_heartbeat_payload, build_welcome_payload, AgentCommand, GameEvent, NetworkBridgeResource,
    PayloadBuildError, SERVER_DATA_CHANNEL,
};
use valence::prelude::{ident, Added, App, Client, Entity, Query, Res, Update};

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (send_welcome_payload_on_join, process_bridge_messages),
    );
}

fn send_welcome_payload_on_join(mut joined_clients: Query<(Entity, &mut Client), Added<Client>>) {
    let payload = match build_welcome_payload() {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error("welcome", &error);
            return;
        }
    };

    for (entity, mut client) in &mut joined_clients {
        send_server_data_payload(&mut client, payload.as_slice());
        tracing::info!(
            "[bong][network] sent bong:server_data welcome payload to client entity {entity:?}"
        );
    }
}

fn process_bridge_messages(bridge: Res<NetworkBridgeResource>, mut clients: Query<&mut Client>) {
    let heartbeat_payload = match build_heartbeat_payload() {
        Ok(payload) => payload,
        Err(error) => {
            log_payload_build_error("heartbeat", &error);
            return;
        }
    };

    drain_bridge_commands(&bridge, || {
        for mut client in &mut clients {
            send_server_data_payload(&mut client, heartbeat_payload.as_slice());
        }
    });
}

fn send_server_data_payload(client: &mut Client, payload: &[u8]) {
    client.send_custom_payload(ident!("bong:server_data"), payload);
}

fn drain_bridge_commands(bridge: &NetworkBridgeResource, mut on_heartbeat: impl FnMut()) -> usize {
    let mut drained_messages = 0;

    while let Ok(command) = bridge.rx_from_agent.try_recv() {
        drained_messages += 1;

        match command {
            AgentCommand::Heartbeat => on_heartbeat(),
        }

        let _ = bridge.tx_to_agent.send(GameEvent::Placeholder);
    }

    drained_messages
}

fn log_payload_build_error(payload_type: &str, error: &PayloadBuildError) {
    match error {
        PayloadBuildError::Json(json_error) => tracing::error!(
            "[bong][network] failed to serialize {payload_type} payload for {}: {json_error}",
            SERVER_DATA_CHANNEL
        ),
        PayloadBuildError::Oversize { size, max } => tracing::error!(
            "[bong][network] {payload_type} payload for {} rejected as oversize: {size} > {max}",
            SERVER_DATA_CHANNEL
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{bounded, unbounded};
    use std::time::Duration;

    #[test]
    fn bridge_drain_is_non_blocking() {
        let (tx_to_agent, _rx_to_agent) = unbounded::<GameEvent>();
        let (_tx_from_agent, rx_from_agent) = unbounded::<AgentCommand>();
        let bridge = NetworkBridgeResource::new(tx_to_agent, rx_from_agent);

        let (done_tx, done_rx) = bounded::<usize>(1);

        std::thread::spawn(move || {
            let drained = drain_bridge_commands(&bridge, || {});
            let _ = done_tx.send(drained);
        });

        let drained = done_rx
            .recv_timeout(Duration::from_millis(100))
            .expect("drain should return immediately when channel is empty");

        assert_eq!(drained, 0);
    }
}
