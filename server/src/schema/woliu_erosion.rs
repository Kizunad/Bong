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

/// P3: 虚蚀视觉同步 payload（server -> client `bong:void_erosion_visual`）。
///
/// 当虚蚀阶段变化或常驻涡流切换时，服务端向客户端发送此 payload
/// 以驱动半透明渲染、回响粒子重播、声音扭曲 HUD overlay。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoidErosionVisualSyncPayloadV1 {
    pub entity_id: String,
    pub stage: u8,
    pub cumulative_erosion: f64,
    pub ambient_active: bool,
    /// 玩家模型 alpha = `1.0 - stage * 0.15`（阶段 4 = 0.4 半透明）。
    pub model_alpha: f32,
    /// 声音扭曲 HUD overlay 是否激活（阶段 3+）。
    pub sound_distortion_active: bool,
    pub server_tick: u64,
}

/// P4: 天道感知修正 payload（server -> agent IPC）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoidErosionTiandaoModifierV1 {
    pub entity: String,
    pub stage: VoidErosionStageV1,
    /// 天道感知概率乘数：1.0=无修正, 0.6=阶段3(-40%), 0.0=阶段4(放弃追踪)。
    pub detection_modifier: f64,
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

    // ────────────────────────────────────────────────────────
    // P3: VoidErosionVisualSyncPayloadV1
    // ────────────────────────────────────────────────────────

    #[test]
    fn visual_sync_payload_serde_roundtrip() {
        let payload = VoidErosionVisualSyncPayloadV1 {
            entity_id: "player_789".to_string(),
            stage: 3,
            cumulative_erosion: 250.0,
            ambient_active: true,
            model_alpha: 0.55,
            sound_distortion_active: true,
            server_tick: 12345,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: VoidErosionVisualSyncPayloadV1 =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            payload, back,
            "VoidErosionVisualSyncPayloadV1 serde round-trip should preserve all fields"
        );
    }

    #[test]
    fn visual_sync_payload_rejects_unknown_fields() {
        let json = r#"{"entity_id":"test","stage":0,"cumulative_erosion":0.0,"ambient_active":false,"model_alpha":1.0,"sound_distortion_active":false,"server_tick":0,"extra":42}"#;
        let result = serde_json::from_str::<VoidErosionVisualSyncPayloadV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field"
        );
    }

    #[test]
    fn visual_sync_payload_all_stages() {
        for stage in 0..=4u8 {
            let payload = VoidErosionVisualSyncPayloadV1 {
                entity_id: format!("player_{stage}"),
                stage,
                cumulative_erosion: stage as f64 * 100.0,
                ambient_active: stage >= 1,
                model_alpha: 1.0 - stage as f32 * 0.15,
                sound_distortion_active: stage >= 3,
                server_tick: 0,
            };
            let json = serde_json::to_string(&payload).expect("serialize");
            let back: VoidErosionVisualSyncPayloadV1 =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                payload, back,
                "stage {stage} visual sync serde round-trip failed"
            );
        }
    }

    // ────────────────────────────────────────────────────────
    // P4: VoidErosionTiandaoModifierV1
    // ────────────────────────────────────────────────────────

    #[test]
    fn tiandao_modifier_serde_roundtrip() {
        let payload = VoidErosionTiandaoModifierV1 {
            entity: "player_void".to_string(),
            stage: VoidErosionStageV1::EchoBody,
            detection_modifier: 0.6,
            server_tick: 55555,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: VoidErosionTiandaoModifierV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            payload, back,
            "VoidErosionTiandaoModifierV1 serde round-trip should preserve all fields"
        );
    }

    #[test]
    fn tiandao_modifier_rejects_unknown_fields() {
        let json = r#"{"entity":"test","stage":"none","detection_modifier":1.0,"server_tick":0,"foo":"bar"}"#;
        let result = serde_json::from_str::<VoidErosionTiandaoModifierV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field"
        );
    }

    #[test]
    fn tiandao_modifier_all_stages() {
        let cases = [
            (VoidErosionStageV1::None, 1.0),
            (VoidErosionStageV1::LowPressure, 1.0),
            (VoidErosionStageV1::VoidShadow, 1.0),
            (VoidErosionStageV1::EchoBody, 0.6),
            (VoidErosionStageV1::VoidEroded, 0.0),
        ];
        for (stage, expected_modifier) in cases {
            let payload = VoidErosionTiandaoModifierV1 {
                entity: "test".to_string(),
                stage,
                detection_modifier: expected_modifier,
                server_tick: 0,
            };
            let json = serde_json::to_string(&payload).expect("serialize");
            let back: VoidErosionTiandaoModifierV1 =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                payload, back,
                "stage {stage:?} tiandao modifier round-trip failed"
            );
        }
    }
}
