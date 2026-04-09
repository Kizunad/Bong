use crossbeam_channel::{Receiver, Sender};
use std::{thread, time::Duration};
use valence::prelude::Resource;

use crate::schema::server_data::{
    ServerDataBuildError, ServerDataType, ServerDataV1,
};

pub const SERVER_DATA_CHANNEL: &str = "bong:server_data";

pub type PayloadBuildError = ServerDataBuildError;

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

pub fn serialize_server_data_payload(payload: &ServerDataV1) -> Result<Vec<u8>, PayloadBuildError> {
    payload.to_json_bytes_checked()
}

pub fn payload_type_label(payload_type: ServerDataType) -> &'static str {
    match payload_type {
        ServerDataType::Welcome => "welcome",
        ServerDataType::Heartbeat => "heartbeat",
        ServerDataType::Narration => "narration",
        ServerDataType::ZoneInfo => "zone_info",
        ServerDataType::EventAlert => "event_alert",
        ServerDataType::PlayerState => "player_state",
        ServerDataType::UiOpen => "ui_open",
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipientSelector {
    Broadcast,
    Player(String),
    Zone(String),
}

#[allow(dead_code)]
impl RecipientSelector {
    pub fn player(username_or_alias: impl Into<String>) -> Self {
        Self::Player(username_or_alias.into())
    }

    pub fn zone(zone_name: impl Into<String>) -> Self {
        Self::Zone(zone_name.into())
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecipientMetadata {
    pub username: Option<String>,
    pub zone: Option<String>,
}

#[allow(dead_code)]
pub type ZoneRouteHook<'a> = &'a dyn Fn(&str, &RecipientMetadata) -> bool;

#[allow(dead_code)]
pub fn normalize_player_target(target: &str) -> Option<String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }

    let logical_name = strip_offline_alias_prefix(trimmed).trim();
    if logical_name.is_empty() {
        return None;
    }

    Some(logical_name.to_ascii_lowercase())
}

#[allow(dead_code)]
pub fn route_recipient_indices(
    selector: &RecipientSelector,
    recipients: &[RecipientMetadata],
    zone_hook: Option<ZoneRouteHook<'_>>,
) -> Vec<usize> {
    match selector {
        RecipientSelector::Broadcast => (0..recipients.len()).collect(),
        RecipientSelector::Player(username_or_alias) => {
            let Some(target_key) = normalize_player_target(username_or_alias) else {
                return Vec::new();
            };

            recipients
                .iter()
                .enumerate()
                .filter_map(|(index, recipient)| {
                    let username_key = recipient
                        .username
                        .as_deref()
                        .and_then(normalize_player_target);

                    (username_key.as_deref() == Some(target_key.as_str())).then_some(index)
                })
                .collect()
        }
        RecipientSelector::Zone(zone_name) => {
            let Some(matches_zone) = zone_hook else {
                return Vec::new();
            };

            recipients
                .iter()
                .enumerate()
                .filter_map(|(index, recipient)| matches_zone(zone_name, recipient).then_some(index))
                .collect()
        }
    }
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

#[allow(dead_code)]
fn strip_offline_alias_prefix(value: &str) -> &str {
    match value.split_once(':') {
        Some((prefix, logical_name)) if prefix.eq_ignore_ascii_case("offline") => logical_name,
        _ => value,
    }
}

#[cfg(test)]
mod server_data_tests {
    use super::*;
    use crate::schema::common::{EventKind, MAX_PAYLOAD_BYTES, NarrationScope, NarrationStyle};
    use crate::schema::narration::Narration;
    use crate::schema::server_data::{
        ServerDataPayloadV1, HEARTBEAT_MESSAGE, SERVER_DATA_VERSION, WELCOME_MESSAGE,
    };
    use crate::schema::world_state::PlayerPowerBreakdown;
    use serde_json::json;
    use std::collections::BTreeSet;

    fn sample_player_breakdown() -> PlayerPowerBreakdown {
        PlayerPowerBreakdown {
            combat: 0.2,
            wealth: 0.4,
            social: 0.65,
            karma: 0.2,
            territory: 0.1,
        }
    }

    #[test]
    fn serializes_known_payloads() {
        let payloads: Vec<(ServerDataV1, serde_json::Value)> = vec![
            (
                ServerDataV1::welcome(WELCOME_MESSAGE),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "welcome",
                    "message": WELCOME_MESSAGE,
                }),
            ),
            (
                ServerDataV1::heartbeat(HEARTBEAT_MESSAGE),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "heartbeat",
                    "message": HEARTBEAT_MESSAGE,
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::Narration {
                    narrations: vec![Narration {
                        scope: NarrationScope::Broadcast,
                        target: None,
                        text: "血谷上空乌云翻涌，一道紫雷正在酝酿。".to_string(),
                        style: NarrationStyle::SystemWarning,
                    }],
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "narration",
                    "narrations": [{
                        "scope": "broadcast",
                        "text": "血谷上空乌云翻涌，一道紫雷正在酝酿。",
                        "style": "system_warning",
                    }],
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
                    zone: "blood_valley".to_string(),
                    spirit_qi: 0.42,
                    danger_level: 3,
                    active_events: Some(vec!["beast_tide".to_string()]),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "zone_info",
                    "zone": "blood_valley",
                    "spirit_qi": 0.42,
                    "danger_level": 3,
                    "active_events": ["beast_tide"],
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::EventAlert {
                    event: EventKind::ThunderTribulation,
                    message: "天劫将至，请于三十息内离开血谷中央。".to_string(),
                    zone: Some("blood_valley".to_string()),
                    duration_ticks: Some(600),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "event_alert",
                    "event": "thunder_tribulation",
                    "message": "天劫将至，请于三十息内离开血谷中央。",
                    "zone": "blood_valley",
                    "duration_ticks": 600,
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::PlayerState {
                    player: Some("offline:Steve".to_string()),
                    realm: "qi_refining_3".to_string(),
                    spirit_qi: 78.0,
                    karma: 0.2,
                    composite_power: 0.35,
                    breakdown: sample_player_breakdown(),
                    zone: "blood_valley".to_string(),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "player_state",
                    "player": "offline:Steve",
                    "realm": "qi_refining_3",
                    "spirit_qi": 78.0,
                    "karma": 0.2,
                    "composite_power": 0.35,
                    "breakdown": {
                        "combat": 0.2,
                        "wealth": 0.4,
                        "social": 0.65,
                        "karma": 0.2,
                        "territory": 0.1,
                    },
                    "zone": "blood_valley",
                }),
            ),
            (
                ServerDataV1::new(ServerDataPayloadV1::UiOpen {
                    ui: Some("cultivation_panel".to_string()),
                    xml: "<flow-layout><label text=\"修仙面板\"/></flow-layout>".to_string(),
                }),
                json!({
                    "v": SERVER_DATA_VERSION,
                    "type": "ui_open",
                    "ui": "cultivation_panel",
                    "xml": "<flow-layout><label text=\"修仙面板\"/></flow-layout>",
                }),
            ),
        ];

        let mut observed_payload_types = BTreeSet::new();

        for (payload, expected_json) in payloads {
            let payload_type = payload.payload_type();
            let bytes =
                serialize_server_data_payload(&payload).expect("known payload should serialize");
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("serialized payload should be JSON");

            assert_eq!(value, expected_json);
            assert_eq!(
                value.get("type"),
                Some(&json!(payload_type_label(payload_type.clone())))
            );

            observed_payload_types.insert(payload_type_label(payload_type));
        }

        assert_eq!(observed_payload_types.len(), 7);
        assert_eq!(
            observed_payload_types,
            BTreeSet::from([
                "welcome",
                "heartbeat",
                "narration",
                "zone_info",
                "event_alert",
                "player_state",
                "ui_open",
            ])
        );
    }

    #[test]
    fn routes_player_and_zone_targets() {
        let recipients = vec![
            RecipientMetadata {
                username: Some("Steve".to_string()),
                zone: Some("blood_valley".to_string()),
            },
            RecipientMetadata {
                username: Some("offline:Alex".to_string()),
                zone: Some("spawn".to_string()),
            },
            RecipientMetadata {
                username: None,
                zone: Some("blood_valley".to_string()),
            },
        ];

        let broadcast_matches =
            route_recipient_indices(&RecipientSelector::Broadcast, &recipients, None);
        assert_eq!(broadcast_matches, vec![0, 1, 2]);

        let steve_plain =
            route_recipient_indices(&RecipientSelector::player("Steve"), &recipients, None);
        let steve_alias = route_recipient_indices(
            &RecipientSelector::player("offline:Steve"),
            &recipients,
            None,
        );
        assert_eq!(steve_plain, vec![0]);
        assert_eq!(steve_alias, vec![0]);

        let alex_plain =
            route_recipient_indices(&RecipientSelector::player("Alex"), &recipients, None);
        let alex_alias =
            route_recipient_indices(&RecipientSelector::player("offline:Alex"), &recipients, None);
        assert_eq!(alex_plain, vec![1]);
        assert_eq!(alex_alias, vec![1]);

        let zone_matches = route_recipient_indices(
            &RecipientSelector::zone("blood_valley"),
            &recipients,
            Some(&|zone_name, recipient| {
                recipient
                    .zone
                    .as_deref()
                    .is_some_and(|zone| zone.eq_ignore_ascii_case(zone_name))
            }),
        );
        assert_eq!(zone_matches, vec![0, 2]);

        let zone_without_hook =
            route_recipient_indices(&RecipientSelector::zone("blood_valley"), &recipients, None);
        assert!(zone_without_hook.is_empty());
    }

    #[test]
    fn rejects_oversize_payloads() {
        let payload = ServerDataV1::welcome("x".repeat(MAX_PAYLOAD_BYTES * 2));

        let error =
            serialize_server_data_payload(&payload).expect_err("oversized payload should fail");

        match error {
            PayloadBuildError::Oversize { size, max } => {
                assert!(size > max);
                assert_eq!(max, MAX_PAYLOAD_BYTES);
            }
            PayloadBuildError::Json(err) => {
                panic!("unexpected json error while testing oversize rejection: {err}");
            }
        }
    }
}
