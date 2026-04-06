pub mod agent_bridge;
pub mod redis_bridge;

use agent_bridge::{
    build_heartbeat_payload, build_welcome_payload, AgentCommand, GameEvent, NetworkBridgeResource,
    PayloadBuildError, SERVER_DATA_CHANNEL,
};
use redis_bridge::{RedisInbound, RedisOutbound};
use valence::message::SendMessage;
use valence::prelude::{ident, Added, App, Client, Entity, Query, Res, Resource, Update};

use crate::schema::world_state::WorldStateV1;

const REDIS_URL: &str = "redis://127.0.0.1:6379";
const WORLD_STATE_PUBLISH_INTERVAL_TICKS: u64 = 200; // ~10 seconds at 20 TPS

/// Resource holding the Redis bridge channels
pub struct RedisBridgeResource {
    pub tx_outbound: crossbeam_channel::Sender<RedisOutbound>,
    pub rx_inbound: crossbeam_channel::Receiver<RedisInbound>,
}

impl Resource for RedisBridgeResource {}

/// Tick counter for world state publishing
#[derive(Default)]
pub struct WorldStateTimer {
    ticks: u64,
}

impl Resource for WorldStateTimer {}

pub fn register(app: &mut App) {
    // Legacy mock bridge systems
    app.add_systems(
        Update,
        (send_welcome_payload_on_join, process_bridge_messages),
    );

    // Redis bridge
    let (handle, tx_outbound, rx_inbound) = redis_bridge::spawn_redis_bridge(REDIS_URL);
    std::mem::drop(handle); // detach thread

    app.insert_resource(RedisBridgeResource {
        tx_outbound,
        rx_inbound,
    });
    app.insert_resource(WorldStateTimer::default());

    app.add_systems(
        Update,
        (
            publish_world_state_to_redis,
            process_redis_inbound,
        ),
    );
}

/// Periodically publish world state snapshot to Redis
fn publish_world_state_to_redis(
    redis: Res<RedisBridgeResource>,
    mut timer: valence::prelude::ResMut<WorldStateTimer>,
    clients: Query<(&valence::prelude::Position, &Client)>,
) {
    timer.ticks += 1;
    if timer.ticks % WORLD_STATE_PUBLISH_INTERVAL_TICKS != 0 {
        return;
    }

    // Build a minimal world state from current ECS
    let players: Vec<crate::schema::world_state::PlayerProfile> = clients
        .iter()
        .enumerate()
        .map(|(i, (pos, _client))| {
            let p = pos.get();
            crate::schema::world_state::PlayerProfile {
                uuid: format!("offline:player_{i}"),
                name: format!("Player{i}"),
                realm: "mortal".to_string(),
                composite_power: 0.1,
                breakdown: crate::schema::world_state::PlayerPowerBreakdown {
                    combat: 0.1,
                    wealth: 0.1,
                    social: 0.1,
                    karma: 0.0,
                    territory: 0.0,
                },
                trend: crate::schema::common::PlayerTrend::Stable,
                active_hours: 0.0,
                zone: "spawn".to_string(),
                pos: [p.x, p.y, p.z],
                recent_kills: 0,
                recent_deaths: 0,
            }
        })
        .collect();

    let state = WorldStateV1 {
        v: 1,
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        tick: timer.ticks,
        players,
        npcs: vec![],
        zones: vec![crate::schema::world_state::ZoneSnapshot {
            name: "spawn".to_string(),
            spirit_qi: 0.9,
            danger_level: 0,
            active_events: vec![],
            player_count: clients.iter().count() as u32,
        }],
        recent_events: vec![],
    };

    let _ = redis.tx_outbound.send(RedisOutbound::WorldState(state));
}

/// Process inbound messages from Redis (agent commands + narrations)
fn process_redis_inbound(
    redis: Res<RedisBridgeResource>,
    mut clients: Query<&mut Client>,
) {
    while let Ok(msg) = redis.rx_inbound.try_recv() {
        match msg {
            RedisInbound::AgentCommand(cmd) => {
                tracing::info!(
                    "[bong][network] received agent command batch {}: {} commands from {:?}",
                    cmd.id,
                    cmd.commands.len(),
                    cmd.source
                );
                // MVP: log commands, actual execution in future
                for c in &cmd.commands {
                    tracing::info!(
                        "[bong][network]   cmd: {:?} → {} params={:?}",
                        c.command_type,
                        c.target,
                        c.params
                    );
                }
            }
            RedisInbound::AgentNarration(narr) => {
                // Broadcast narrations to all connected clients as chat messages
                for n in &narr.narrations {
                    let prefix = match n.style {
                        crate::schema::common::NarrationStyle::SystemWarning => "§c[天道警示]",
                        crate::schema::common::NarrationStyle::Perception => "§7[感知]",
                        crate::schema::common::NarrationStyle::Narration => "§f[叙事]",
                        crate::schema::common::NarrationStyle::EraDecree => "§6[时代]",
                    };
                    let formatted = format!("{prefix} {}", n.text);

                    match n.scope {
                        crate::schema::common::NarrationScope::Broadcast => {
                            for mut client in &mut clients {
                                client.send_chat_message(&formatted);
                            }
                        }
                        crate::schema::common::NarrationScope::Zone
                        | crate::schema::common::NarrationScope::Player => {
                            // MVP: broadcast all for now
                            for mut client in &mut clients {
                                client.send_chat_message(&formatted);
                            }
                        }
                    }

                    tracing::info!("[bong][network] narration: {formatted}");
                }
            }
        }
    }
}

// ─── Legacy mock bridge systems (unchanged) ──────────────

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
