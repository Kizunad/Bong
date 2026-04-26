use serde::{Deserialize, Serialize};

use super::{common::EventKind, narration::Narration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClientPayloadType {
    Welcome,
    Heartbeat,
    Narration,
    ZoneInfo,
    EventAlert,
    PlayerState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneInfoPayload {
    pub zone: String,
    pub spirit_qi: f64,
    pub danger_level: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_events: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventAlertSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAlertPayload {
    pub kind: EventKind,
    pub title: String,
    pub detail: String,
    pub severity: EventAlertSeverity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStatePayload {
    pub realm: String,
    pub spirit_qi: f64,
    pub spirit_qi_max: f64,
    pub karma: f64,
    pub composite_power: f64,
    pub zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientPayloadV1 {
    Welcome {
        v: u8,
        message: String,
    },
    Heartbeat {
        v: u8,
        message: String,
    },
    Narration {
        v: u8,
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        v: u8,
        zone_info: ZoneInfoPayload,
    },
    EventAlert {
        v: u8,
        event_alert: EventAlertPayload,
    },
    PlayerState {
        v: u8,
        player_state: PlayerStatePayload,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    const WELCOME_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/client-payload-welcome.sample.json");
    const HEARTBEAT_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/client-payload-heartbeat.sample.json");
    const NARRATION_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/client-payload-narration.sample.json");
    const ZONE_INFO_SAMPLE: &str =
        include_str!("../../../agent/packages/schema/samples/client-payload-zone-info.sample.json");
    const EVENT_ALERT_SAMPLE: &str = include_str!(
        "../../../agent/packages/schema/samples/client-payload-event-alert.sample.json"
    );
    const PLAYER_STATE_SAMPLE: &str = include_str!(
        "../../../agent/packages/schema/samples/client-payload-player-state.sample.json"
    );

    #[test]
    fn deserialize_welcome_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(WELCOME_SAMPLE)
            .expect("client-payload-welcome.sample.json should deserialize into ClientPayloadV1");

        match payload {
            ClientPayloadV1::Welcome { v, message } => {
                assert_eq!(v, 1);
                assert!(message.contains("天道"));
            }
            other => panic!("expected welcome payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_heartbeat_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(HEARTBEAT_SAMPLE)
            .expect("client-payload-heartbeat.sample.json should deserialize into ClientPayloadV1");

        match payload {
            ClientPayloadV1::Heartbeat { v, message } => {
                assert_eq!(v, 1);
                assert_eq!(message, "server tick 84000");
            }
            other => panic!("expected heartbeat payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_narration_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(NARRATION_SAMPLE)
            .expect("client-payload-narration.sample.json should deserialize into ClientPayloadV1");

        match payload {
            ClientPayloadV1::Narration { v, narrations } => {
                assert_eq!(v, 1);
                assert_eq!(narrations.len(), 1);
                assert!(narrations[0].text.contains("天道震怒"));
            }
            other => panic!("expected narration payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_zone_info_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(ZONE_INFO_SAMPLE)
            .expect("client-payload-zone-info.sample.json should deserialize into ClientPayloadV1");

        match payload {
            ClientPayloadV1::ZoneInfo { v, zone_info } => {
                assert_eq!(v, 1);
                assert_eq!(zone_info.zone, "blood_valley");
                assert_eq!(zone_info.danger_level, 3);
                assert_eq!(
                    zone_info.active_events,
                    Some(vec!["beast_tide_warning".to_string()])
                );
            }
            other => panic!("expected zone_info payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_event_alert_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(EVENT_ALERT_SAMPLE).expect(
            "client-payload-event-alert.sample.json should deserialize into ClientPayloadV1",
        );

        match payload {
            ClientPayloadV1::EventAlert { v, event_alert } => {
                assert_eq!(v, 1);
                assert_eq!(event_alert.kind, EventKind::ThunderTribulation);
                assert_eq!(event_alert.severity, EventAlertSeverity::Critical);
                assert_eq!(event_alert.zone.as_deref(), Some("blood_valley"));
            }
            other => panic!("expected event_alert payload, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_player_state_sample() {
        let payload: ClientPayloadV1 = serde_json::from_str(PLAYER_STATE_SAMPLE).expect(
            "client-payload-player-state.sample.json should deserialize into ClientPayloadV1",
        );

        match payload {
            ClientPayloadV1::PlayerState { v, player_state } => {
                assert_eq!(v, 1);
                assert_eq!(player_state.realm, "Induce");
                assert_eq!(player_state.spirit_qi, 78.0);
                assert_eq!(player_state.spirit_qi_max, 100.0);
                assert_eq!(player_state.zone, "blood_valley");
            }
            other => panic!("expected player_state payload, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_all_client_payload_samples() {
        let samples = [
            WELCOME_SAMPLE,
            HEARTBEAT_SAMPLE,
            NARRATION_SAMPLE,
            ZONE_INFO_SAMPLE,
            EVENT_ALERT_SAMPLE,
            PLAYER_STATE_SAMPLE,
        ];

        for sample in samples {
            let payload: ClientPayloadV1 = serde_json::from_str(sample).unwrap();
            let re_json = serde_json::to_string(&payload).unwrap();
            let _: ClientPayloadV1 = serde_json::from_str(&re_json).unwrap();
        }
    }

    #[test]
    fn deserialize_client_payload_type_literals() {
        let welcome: ClientPayloadType = serde_json::from_str("\"welcome\"").unwrap();
        let heartbeat: ClientPayloadType = serde_json::from_str("\"heartbeat\"").unwrap();
        let narration: ClientPayloadType = serde_json::from_str("\"narration\"").unwrap();
        let zone_info: ClientPayloadType = serde_json::from_str("\"zone_info\"").unwrap();
        let event_alert: ClientPayloadType = serde_json::from_str("\"event_alert\"").unwrap();
        let player_state: ClientPayloadType = serde_json::from_str("\"player_state\"").unwrap();

        assert_eq!(welcome, ClientPayloadType::Welcome);
        assert_eq!(heartbeat, ClientPayloadType::Heartbeat);
        assert_eq!(narration, ClientPayloadType::Narration);
        assert_eq!(zone_info, ClientPayloadType::ZoneInfo);
        assert_eq!(event_alert, ClientPayloadType::EventAlert);
        assert_eq!(player_state, ClientPayloadType::PlayerState);
    }
}
