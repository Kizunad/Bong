//! plan-woliu-path-v1 IPC schema — VoidErosionStateV1 / VoidErosionEventV1.
//!
//! server -> client / agent 同步虚蚀状态与虚蚀阶段推进事件。

use serde::{Deserialize, Serialize};

/// 虚蚀阶段枚举（wire format）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoidErosionStageV1 {
    None,
    LowPressure,
    VoidShadow,
    EchoBody,
    VoidEroded,
}

impl From<u8> for VoidErosionStageV1 {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::LowPressure,
            2 => Self::VoidShadow,
            3 => Self::EchoBody,
            4 => Self::VoidEroded,
            _ => Self::VoidEroded,
        }
    }
}

/// 虚蚀状态 payload（server -> client/agent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoidErosionStateV1 {
    pub entity: String,
    pub stage: VoidErosionStageV1,
    pub cumulative_erosion: f64,
    pub ambient_active: bool,
    pub contam_mult: f64,
    pub efficiency: f64,
    pub server_tick: u64,
}

/// 虚蚀阶段推进事件 payload（server -> client/agent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoidErosionEventV1 {
    pub entity: String,
    pub from_stage: VoidErosionStageV1,
    pub to_stage: VoidErosionStageV1,
    pub cumulative_erosion: f64,
    pub server_tick: u64,
}

#[cfg(test)]
mod woliu_erosion_schema_tests {
    use super::*;

    #[test]
    fn void_erosion_state_v1_serde_roundtrip() {
        let state = VoidErosionStateV1 {
            entity: "player_123".to_string(),
            stage: VoidErosionStageV1::EchoBody,
            cumulative_erosion: 250.0,
            ambient_active: true,
            contam_mult: 1.5,
            efficiency: 0.80,
            server_tick: 99999,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let back: VoidErosionStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            state, back,
            "VoidErosionStateV1 serde round-trip should not lose data"
        );
    }

    #[test]
    fn void_erosion_event_v1_serde_roundtrip() {
        let event = VoidErosionEventV1 {
            entity: "player_456".to_string(),
            from_stage: VoidErosionStageV1::LowPressure,
            to_stage: VoidErosionStageV1::VoidShadow,
            cumulative_erosion: 80.0,
            server_tick: 50000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: VoidErosionEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            event, back,
            "VoidErosionEventV1 serde round-trip should not lose data"
        );
    }

    #[test]
    fn void_erosion_stage_v1_from_u8_all_variants() {
        assert_eq!(VoidErosionStageV1::from(0), VoidErosionStageV1::None);
        assert_eq!(VoidErosionStageV1::from(1), VoidErosionStageV1::LowPressure);
        assert_eq!(VoidErosionStageV1::from(2), VoidErosionStageV1::VoidShadow);
        assert_eq!(VoidErosionStageV1::from(3), VoidErosionStageV1::EchoBody);
        assert_eq!(VoidErosionStageV1::from(4), VoidErosionStageV1::VoidEroded);
        assert_eq!(
            VoidErosionStageV1::from(255),
            VoidErosionStageV1::VoidEroded,
            "out-of-range value should clamp to VoidEroded"
        );
    }

    #[test]
    fn all_5_void_erosion_stage_v1_serde() {
        let stages = [
            VoidErosionStageV1::None,
            VoidErosionStageV1::LowPressure,
            VoidErosionStageV1::VoidShadow,
            VoidErosionStageV1::EchoBody,
            VoidErosionStageV1::VoidEroded,
        ];
        for stage in stages {
            let json = serde_json::to_string(&stage).expect("serialize");
            let back: VoidErosionStageV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                stage, back,
                "VoidErosionStageV1::{stage:?} serde round-trip failed"
            );
        }
    }

    #[test]
    fn void_erosion_state_v1_rejects_unknown_fields() {
        let json = r#"{"entity":"test","stage":"none","cumulative_erosion":0.0,"ambient_active":false,"contam_mult":1.0,"efficiency":1.0,"server_tick":0,"unknown_field":42}"#;
        let result = serde_json::from_str::<VoidErosionStateV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field"
        );
    }

    #[test]
    fn void_erosion_event_v1_rejects_unknown_fields() {
        let json = r#"{"entity":"test","from_stage":"none","to_stage":"low_pressure","cumulative_erosion":20.0,"server_tick":0,"bonus":42}"#;
        let result = serde_json::from_str::<VoidErosionEventV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field"
        );
    }

    #[test]
    fn void_erosion_event_v1_rejects_missing_required_field() {
        let json = r#"{"entity":"test","to_stage":"low_pressure","cumulative_erosion":20.0,"server_tick":0}"#;
        let result = serde_json::from_str::<VoidErosionEventV1>(json);
        assert!(
            result.is_err(),
            "missing required field from_stage should fail"
        );
    }

    #[test]
    fn void_erosion_stage_v1_rejects_invalid_string() {
        let result = serde_json::from_str::<VoidErosionStageV1>(r#""corrupted""#);
        assert!(
            result.is_err(),
            "invalid stage string 'corrupted' should fail deserialization"
        );
    }

    #[test]
    fn void_erosion_event_v1_stage_transitions() {
        let transitions = [
            (VoidErosionStageV1::None, VoidErosionStageV1::LowPressure),
            (
                VoidErosionStageV1::LowPressure,
                VoidErosionStageV1::VoidShadow,
            ),
            (VoidErosionStageV1::VoidShadow, VoidErosionStageV1::EchoBody),
            (VoidErosionStageV1::EchoBody, VoidErosionStageV1::VoidEroded),
        ];
        for (from, to) in transitions {
            let event = VoidErosionEventV1 {
                entity: "test".to_string(),
                from_stage: from,
                to_stage: to,
                cumulative_erosion: 100.0,
                server_tick: 0,
            };
            let json = serde_json::to_string(&event).expect("serialize");
            let back: VoidErosionEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(event, back);
        }
    }

    #[test]
    fn void_erosion_state_v1_snake_case_stage() {
        let json = serde_json::to_string(&VoidErosionStageV1::LowPressure).unwrap();
        assert!(
            json.contains("low_pressure"),
            "VoidErosionStageV1 should serialize as snake_case, got {}",
            json
        );
    }
}
