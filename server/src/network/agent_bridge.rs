use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;
use std::{thread, time::Duration};
use valence::prelude::Resource;

pub const SERVER_DATA_CHANNEL: &str = "bong:server_data";

const SERVER_PAYLOAD_VERSION: u8 = 1;
const SERVER_PAYLOAD_MAX_BYTES: usize = 1024;
const WELCOME_PAYLOAD_TYPE: &str = "welcome";
const WELCOME_PAYLOAD_MESSAGE: &str = "Bong server connected";
const HEARTBEAT_PAYLOAD_TYPE: &str = "heartbeat";
const HEARTBEAT_PAYLOAD_MESSAGE: &str = "mock agent tick";

#[derive(Debug)]
pub enum PayloadBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
}

#[derive(Serialize)]
struct ServerPayloadV1<'a> {
    v: u8,
    #[serde(rename = "type")]
    payload_type: &'a str,
    message: &'a str,
}

#[derive(Debug, Clone)]
pub enum AgentCommand {
    Heartbeat,
}

#[derive(Debug, Clone)]
pub enum GameEvent {
    Placeholder,
}

pub struct NetworkBridgeResource {
    pub tx_to_agent: Sender<GameEvent>,
    pub rx_from_agent: Receiver<AgentCommand>,
}

impl Resource for NetworkBridgeResource {}

pub fn build_welcome_payload() -> Result<Vec<u8>, PayloadBuildError> {
    serialize_payload_v1(WELCOME_PAYLOAD_TYPE, WELCOME_PAYLOAD_MESSAGE)
}

pub fn build_heartbeat_payload() -> Result<Vec<u8>, PayloadBuildError> {
    serialize_payload_v1(HEARTBEAT_PAYLOAD_TYPE, HEARTBEAT_PAYLOAD_MESSAGE)
}

impl NetworkBridgeResource {
    pub fn new(tx_to_agent: Sender<GameEvent>, rx_from_agent: Receiver<AgentCommand>) -> Self {
        Self {
            tx_to_agent,
            rx_from_agent,
        }
    }
}

pub fn spawn_mock_bridge_daemon(
    tx_to_game: Sender<AgentCommand>,
    _rx_from_game: Receiver<GameEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(
                    "[bong][bridge] failed to create tokio runtime for mock bridge daemon: {error}"
                );
                return;
            }
        };

        tracing::info!("[bong][bridge] tokio runtime started");

        runtime.block_on(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;

                if tx_to_game.send(AgentCommand::Heartbeat).is_err() {
                    tracing::warn!("[bong][bridge] channel to game closed; stopping daemon");
                    break;
                }
            }
        });
    })
}

fn serialize_payload_v1(payload_type: &str, message: &str) -> Result<Vec<u8>, PayloadBuildError> {
    let payload = ServerPayloadV1 {
        v: SERVER_PAYLOAD_VERSION,
        payload_type,
        message,
    };

    let bytes = serde_json::to_vec(&payload).map_err(PayloadBuildError::Json)?;
    if bytes.len() > SERVER_PAYLOAD_MAX_BYTES {
        return Err(PayloadBuildError::Oversize {
            size: bytes.len(),
            max: SERVER_PAYLOAD_MAX_BYTES,
        });
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_schema_v1_is_valid_utf8_json() {
        let welcome_bytes = build_welcome_payload().expect("welcome payload should serialize");
        let heartbeat_bytes =
            build_heartbeat_payload().expect("heartbeat payload should serialize");

        let welcome_utf8 =
            std::str::from_utf8(&welcome_bytes).expect("welcome payload should be valid UTF-8");
        let heartbeat_utf8 =
            std::str::from_utf8(&heartbeat_bytes).expect("heartbeat payload should be valid UTF-8");

        assert_eq!(
            welcome_utf8,
            r#"{"v":1,"type":"welcome","message":"Bong server connected"}"#
        );
        assert_eq!(
            heartbeat_utf8,
            r#"{"v":1,"type":"heartbeat","message":"mock agent tick"}"#
        );

        let welcome_json: serde_json::Value =
            serde_json::from_slice(&welcome_bytes).expect("welcome bytes should parse as JSON");
        let heartbeat_json: serde_json::Value =
            serde_json::from_slice(&heartbeat_bytes).expect("heartbeat bytes should parse as JSON");

        assert_eq!(
            welcome_json,
            serde_json::json!({
                "v": 1,
                "type": "welcome",
                "message": "Bong server connected"
            })
        );
        assert_eq!(
            heartbeat_json,
            serde_json::json!({
                "v": 1,
                "type": "heartbeat",
                "message": "mock agent tick"
            })
        );
    }

    #[test]
    fn payload_schema_rejects_oversize_messages() {
        let oversized_message = "x".repeat(SERVER_PAYLOAD_MAX_BYTES * 2);

        let error = serialize_payload_v1(WELCOME_PAYLOAD_TYPE, &oversized_message)
            .expect_err("oversized payload should be rejected");

        match error {
            PayloadBuildError::Oversize { size, max } => {
                assert!(size > max, "oversized payload should exceed max size");
                assert_eq!(max, SERVER_PAYLOAD_MAX_BYTES);
            }
            PayloadBuildError::Json(err) => {
                panic!("unexpected json error while testing oversize rejection: {err}");
            }
        }
    }
}
