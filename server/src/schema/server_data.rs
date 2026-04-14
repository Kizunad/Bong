use serde::{Deserialize, Serialize};

use super::common::{EventKind, MAX_PAYLOAD_BYTES};
use super::narration::Narration;
use super::world_state::PlayerPowerBreakdown;

pub const SERVER_DATA_VERSION: u8 = 1;
pub const WELCOME_MESSAGE: &str = "Bong server connected";
pub const HEARTBEAT_MESSAGE: &str = "mock agent tick";

#[derive(Debug)]
pub enum ServerDataBuildError {
    Json(serde_json::Error),
    Oversize { size: usize, max: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServerDataType {
    Welcome,
    Heartbeat,
    Narration,
    ZoneInfo,
    EventAlert,
    PlayerState,
    UiOpen,
    CultivationDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerDataPayloadV1 {
    Welcome {
        message: String,
    },
    Heartbeat {
        message: String,
    },
    Narration {
        narrations: Vec<Narration>,
    },
    ZoneInfo {
        zone: String,
        spirit_qi: f64,
        danger_level: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_events: Option<Vec<String>>,
    },
    EventAlert {
        event: EventKind,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        zone: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ticks: Option<u64>,
    },
    PlayerState {
        #[serde(skip_serializing_if = "Option::is_none")]
        player: Option<String>,
        realm: String,
        spirit_qi: f64,
        karma: f64,
        composite_power: f64,
        breakdown: PlayerPowerBreakdown,
        zone: String,
    },
    UiOpen {
        #[serde(skip_serializing_if = "Option::is_none")]
        ui: Option<String>,
        xml: String,
    },
    /// 经脉详细快照。20 条经脉以 SoA(parallel arrays) 布局，顺序与 `MeridianId` 判别式一致
    /// (Lung=0..Liver=11, Ren=12..YangWei=19)。保持 ≤ MAX_PAYLOAD_BYTES 预算。
    CultivationDetail {
        /// 境界字面量（Awaken/Induce/Condense/Solidify/Spirit/Void，与 `Realm` 判别式对齐）。
        realm: String,
        opened: Vec<bool>,
        flow_rate: Vec<f64>,
        flow_capacity: Vec<f64>,
        integrity: Vec<f64>,
        /// 每条经脉未打通时的累积进度 0..=1（已打通恒为 1.0）。
        open_progress: Vec<f64>,
        /// 每条经脉当前裂痕条目数（0..=255，饱和）。UI 用于渲染裂痕图标密度。
        cracks_count: Vec<u8>,
        /// 整个实体的污染总量（所有 `Contamination.entries.amount` 求和）。
        contamination_total: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDataV1 {
    pub v: u8,
    #[serde(flatten)]
    pub payload: ServerDataPayloadV1,
}

impl ServerDataV1 {
    pub fn new(payload: ServerDataPayloadV1) -> Self {
        Self {
            v: SERVER_DATA_VERSION,
            payload,
        }
    }

    pub fn welcome(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Welcome {
            message: message.into(),
        })
    }

    pub fn heartbeat(message: impl Into<String>) -> Self {
        Self::new(ServerDataPayloadV1::Heartbeat {
            message: message.into(),
        })
    }

    pub fn payload_type(&self) -> ServerDataType {
        self.payload.payload_type()
    }

    pub fn to_json_bytes_checked(&self) -> Result<Vec<u8>, ServerDataBuildError> {
        let bytes = serde_json::to_vec(self).map_err(ServerDataBuildError::Json)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(ServerDataBuildError::Oversize {
                size: bytes.len(),
                max: MAX_PAYLOAD_BYTES,
            });
        }

        Ok(bytes)
    }
}

impl ServerDataPayloadV1 {
    pub fn payload_type(&self) -> ServerDataType {
        match self {
            Self::Welcome { .. } => ServerDataType::Welcome,
            Self::Heartbeat { .. } => ServerDataType::Heartbeat,
            Self::Narration { .. } => ServerDataType::Narration,
            Self::ZoneInfo { .. } => ServerDataType::ZoneInfo,
            Self::EventAlert { .. } => ServerDataType::EventAlert,
            Self::PlayerState { .. } => ServerDataType::PlayerState,
            Self::UiOpen { .. } => ServerDataType::UiOpen,
            Self::CultivationDetail { .. } => ServerDataType::CultivationDetail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cultivation_detail_roundtrip_and_size_budget() {
        let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
            realm: "Induce".to_string(),
            opened: vec![true; 20],
            flow_rate: vec![1.5; 20],
            flow_capacity: vec![10.25; 20],
            integrity: vec![0.87; 20],
            open_progress: vec![1.0; 20],
            cracks_count: vec![0; 20],
            contamination_total: 0.0,
        });
        let bytes = payload
            .to_json_bytes_checked()
            .expect("cultivation_detail must fit MAX_PAYLOAD_BYTES");
        assert!(
            bytes.len() <= super::super::common::MAX_PAYLOAD_BYTES,
            "over budget: {} bytes",
            bytes.len()
        );
        let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
        match back.payload {
            ServerDataPayloadV1::CultivationDetail { opened, flow_rate, .. } => {
                assert_eq!(opened.len(), 20);
                assert_eq!(flow_rate.len(), 20);
                assert_eq!(flow_rate[0], 1.5);
            }
            other => panic!("expected CultivationDetail, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_server_data_samples() {
        let samples = [
            include_str!("../../../agent/packages/schema/samples/server-data.welcome.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.heartbeat.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.narration.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.zone-info.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.event-alert.sample.json"
            ),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.player-state.sample.json"
            ),
            include_str!("../../../agent/packages/schema/samples/server-data.ui-open.sample.json"),
            include_str!(
                "../../../agent/packages/schema/samples/server-data.cultivation-detail.sample.json"
            ),
        ];

        for json in samples {
            let payload: ServerDataV1 =
                serde_json::from_str(json).expect("sample should deserialize into ServerDataV1");

            let reserialized = serde_json::to_string(&payload)
                .expect("deserialized ServerDataV1 should serialize back to JSON");
            let roundtrip: ServerDataV1 = serde_json::from_str(&reserialized)
                .expect("serialized ServerDataV1 should deserialize again");

            let payload_value =
                serde_json::to_value(&payload).expect("payload should convert to JSON value");
            let roundtrip_value =
                serde_json::to_value(&roundtrip).expect("roundtrip should convert to JSON value");

            assert_eq!(
                payload_value, roundtrip_value,
                "roundtrip must preserve typed payload content"
            );
        }
    }
}
