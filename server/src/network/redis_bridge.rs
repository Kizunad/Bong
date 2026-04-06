//! Redis IPC bridge — connects Valence server to the 天道 Agent via Redis pub/sub.
//!
//! Architecture:
//!   - Tokio thread runs Redis pub/sub
//!   - Publishes WorldStateV1 on `bong:world_state` every N seconds
//!   - Subscribes to `bong:agent_command` and `bong:agent_narrate`
//!   - Crossbeam channels bridge Tokio thread ↔ Bevy ECS main loop

use crossbeam_channel::{Receiver, Sender};
use std::time::Duration;

use crate::schema::agent_command::AgentCommandV1;
use crate::schema::channels::{CH_AGENT_COMMAND, CH_AGENT_NARRATE, CH_WORLD_STATE};
use crate::schema::narration::NarrationV1;
use crate::schema::world_state::WorldStateV1;

/// Messages from Redis → game loop
#[derive(Debug, Clone)]
pub enum RedisInbound {
    AgentCommand(AgentCommandV1),
    AgentNarration(NarrationV1),
}

/// Messages from game loop → Redis
#[derive(Debug, Clone)]
pub enum RedisOutbound {
    WorldState(WorldStateV1),
}


/// Spawn the Redis bridge daemon on a dedicated Tokio thread.
/// Returns a JoinHandle (drop-safe) and the channels for the game side.
pub fn spawn_redis_bridge(
    redis_url: &str,
) -> (
    std::thread::JoinHandle<()>,
    Sender<RedisOutbound>,
    Receiver<RedisInbound>,
) {
    let (tx_to_game, rx_inbound) = crossbeam_channel::unbounded::<RedisInbound>();
    let (tx_outbound, rx_from_game) = crossbeam_channel::unbounded::<RedisOutbound>();

    let url = redis_url.to_string();

    let handle = std::thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("[bong][redis] failed to create tokio runtime: {e}");
                return;
            }
        };

        rt.block_on(async move {
            tracing::info!("[bong][redis] connecting to {url}");

            let client = match redis::Client::open(url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("[bong][redis] failed to open client: {e}");
                    return;
                }
            };

            // Publisher connection
            let mut pub_conn = match client.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("[bong][redis] failed to get pub connection: {e}");
                    return;
                }
            };

            // Subscriber connection
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(ps) => ps,
                Err(e) => {
                    tracing::error!("[bong][redis] failed to get pubsub connection: {e}");
                    return;
                }
            };

            if let Err(e) = pubsub.subscribe(CH_AGENT_COMMAND).await {
                tracing::error!("[bong][redis] failed to subscribe to {CH_AGENT_COMMAND}: {e}");
                return;
            }
            if let Err(e) = pubsub.subscribe(CH_AGENT_NARRATE).await {
                tracing::error!("[bong][redis] failed to subscribe to {CH_AGENT_NARRATE}: {e}");
                return;
            }

            tracing::info!(
                "[bong][redis] subscribed to {CH_AGENT_COMMAND} and {CH_AGENT_NARRATE}"
            );

            let tx_to_game_clone = tx_to_game.clone();

            // Spawn subscriber task
            let sub_task = tokio::spawn(async move {
                use futures_util::StreamExt;
                let mut stream = pubsub.on_message();
                while let Some(msg) = stream.next().await {
                    let channel: String = match msg.get_channel() {
                        Ok(ch) => ch,
                        Err(_) => continue,
                    };
                    let payload: String = match msg.get_payload() {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    if channel == CH_AGENT_COMMAND {
                        match serde_json::from_str::<AgentCommandV1>(&payload) {
                            Ok(cmd) => {
                                tracing::info!(
                                    "[bong][redis] received agent command: {} ({} cmds)",
                                    cmd.id,
                                    cmd.commands.len()
                                );
                                let _ = tx_to_game_clone.send(RedisInbound::AgentCommand(cmd));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[bong][redis] failed to parse agent command: {e}"
                                );
                            }
                        }
                    } else if channel == CH_AGENT_NARRATE {
                        match serde_json::from_str::<NarrationV1>(&payload) {
                            Ok(narr) => {
                                tracing::info!(
                                    "[bong][redis] received narration ({} entries)",
                                    narr.narrations.len()
                                );
                                let _ =
                                    tx_to_game_clone.send(RedisInbound::AgentNarration(narr));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "[bong][redis] failed to parse narration: {e}"
                                );
                            }
                        }
                    }
                }
            });

            // Publisher loop: drain outbound channel and publish
            loop {
                // Non-blocking drain of outbound messages
                while let Ok(msg) = rx_from_game.try_recv() {
                    match msg {
                        RedisOutbound::WorldState(state) => {
                            match serde_json::to_string(&state) {
                                Ok(json) => {
                                    let result: Result<i64, _> = redis::cmd("PUBLISH")
                                        .arg(CH_WORLD_STATE)
                                        .arg(&json)
                                        .query_async(&mut pub_conn)
                                        .await;
                                    match result {
                                        Ok(n) => {
                                            tracing::debug!(
                                                "[bong][redis] published world_state (tick {}), {} subscribers",
                                                state.tick,
                                                n
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "[bong][redis] failed to publish world_state: {e}"
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "[bong][redis] failed to serialize world_state: {e}"
                                    );
                                }
                            }
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;

                if sub_task.is_finished() {
                    tracing::warn!("[bong][redis] subscriber task ended, stopping bridge");
                    break;
                }
            }
        });
    });

    (handle, tx_outbound, rx_inbound)
}
