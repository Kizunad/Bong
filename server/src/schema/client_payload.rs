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
    LocustSwarmWarning,
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
    LocustSwarmWarning {
        v: u8,
        zone: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        direction: Option<String>,
    },
    PlayerState {
        v: u8,
        player_state: PlayerStatePayload,
    },
}
