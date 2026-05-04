use serde::{Deserialize, Serialize};

use crate::cultivation::components::ColorKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyleTelemetryColorSnapshotV1 {
    pub main: ColorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<ColorKind>,
    pub is_chaotic: bool,
    pub is_hunyuan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StyleBalanceTelemetryEventV1 {
    pub v: u8,
    pub attacker_player_id: String,
    pub defender_player_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attacker_color: Option<StyleTelemetryColorSnapshotV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defender_color: Option<StyleTelemetryColorSnapshotV1>,
    pub cause: String,
    pub resolved_at_tick: u64,
}
