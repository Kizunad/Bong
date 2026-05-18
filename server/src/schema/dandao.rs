//! plan-dandao-path-v1 IPC schema — MutationStateV1 / MutationEventV1。
//!
//! server → client / agent 同步变异状态与变异推进事件。

use serde::{Deserialize, Serialize};

/// 变异阶段枚举（wire format）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStageV1 {
    None,
    Subtle,
    Visible,
    Heavy,
    Bestial,
}

impl From<u8> for MutationStageV1 {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Subtle,
            2 => Self::Visible,
            3 => Self::Heavy,
            4 => Self::Bestial,
            _ => Self::Bestial,
        }
    }
}

/// 变异类型枚举（wire format）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKindV1 {
    GoldenIris,
    HardenedNails,
    ToughSkin,
    BoneRidge,
    ForearmScales,
    SpineSpurs,
    Horns,
    Tail,
    BackCarapace,
    ExtraArms,
    BodyEnlarge,
    BeastFace,
}

/// 已激活的单个变异 slot（wire format）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveMutationV1 {
    pub kind: MutationKindV1,
    pub body_slot: String,
    pub level: u8,
    pub acquired_tick: u64,
}

/// 变异状态 payload（server → client/agent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationStateV1 {
    pub entity: String,
    pub stage: MutationStageV1,
    pub slots: Vec<ActiveMutationV1>,
    pub meridian_penalty: f64,
    pub cumulative_toxin: f64,
    pub social_penalty: i32,
    pub server_tick: u64,
}

/// 变异推进事件 payload（server → client/agent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationEventV1 {
    pub entity: String,
    pub from_stage: MutationStageV1,
    pub to_stage: MutationStageV1,
    pub cumulative_toxin: f64,
    pub new_meridian_penalty: f64,
    pub server_tick: u64,
}

/// 丹道样式状态 payload（server → client/agent）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DandaoStyleV1 {
    pub entity: String,
    pub brew_count: u32,
    pub pill_intake_count: u32,
    pub cumulative_toxin: f64,
    pub mutation_stage: MutationStageV1,
    pub mastery_ticks: u64,
    pub server_tick: u64,
}

#[cfg(test)]
mod dandao_schema_tests {
    use super::*;

    #[test]
    fn mutation_state_v1_serde_roundtrip() {
        let state = MutationStateV1 {
            entity: "player_123".to_string(),
            stage: MutationStageV1::Heavy,
            slots: vec![ActiveMutationV1 {
                kind: MutationKindV1::Horns,
                body_slot: "head".to_string(),
                level: 2,
                acquired_tick: 12345,
            }],
            meridian_penalty: 0.15,
            cumulative_toxin: 280.0,
            social_penalty: -50,
            server_tick: 99999,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let back: MutationStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back, "MutationStateV1 serde round-trip 不应丢失数据");
    }

    #[test]
    fn mutation_event_v1_serde_roundtrip() {
        let event = MutationEventV1 {
            entity: "player_456".to_string(),
            from_stage: MutationStageV1::Subtle,
            to_stage: MutationStageV1::Visible,
            cumulative_toxin: 105.0,
            new_meridian_penalty: 0.08,
            server_tick: 50000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: MutationEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, back, "MutationEventV1 serde round-trip 不应丢失数据");
    }

    #[test]
    fn dandao_style_v1_serde_roundtrip() {
        let style = DandaoStyleV1 {
            entity: "player_789".to_string(),
            brew_count: 42,
            pill_intake_count: 100,
            cumulative_toxin: 123.456,
            mutation_stage: MutationStageV1::Visible,
            mastery_ticks: 99999,
            server_tick: 10000,
        };
        let json = serde_json::to_string(&style).expect("serialize");
        let back: DandaoStyleV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(style, back, "DandaoStyleV1 serde round-trip 不应丢失数据");
    }

    #[test]
    fn mutation_stage_v1_from_u8_all_variants() {
        assert_eq!(MutationStageV1::from(0), MutationStageV1::None);
        assert_eq!(MutationStageV1::from(1), MutationStageV1::Subtle);
        assert_eq!(MutationStageV1::from(2), MutationStageV1::Visible);
        assert_eq!(MutationStageV1::from(3), MutationStageV1::Heavy);
        assert_eq!(MutationStageV1::from(4), MutationStageV1::Bestial);
        assert_eq!(
            MutationStageV1::from(255),
            MutationStageV1::Bestial,
            "越界值应 clamp 到 Bestial"
        );
    }

    #[test]
    fn mutation_state_v1_empty_slots() {
        let state = MutationStateV1 {
            entity: "test".to_string(),
            stage: MutationStageV1::None,
            slots: vec![],
            meridian_penalty: 0.0,
            cumulative_toxin: 0.0,
            social_penalty: 0,
            server_tick: 0,
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let back: MutationStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back);
        assert!(back.slots.is_empty());
    }

    #[test]
    fn mutation_state_v1_rejects_unknown_fields() {
        let json = r#"{"entity":"test","stage":"none","slots":[],"meridian_penalty":0.0,"cumulative_toxin":0.0,"social_penalty":0,"server_tick":0,"unknown_field":42}"#;
        let result = serde_json::from_str::<MutationStateV1>(json);
        assert!(result.is_err(), "deny_unknown_fields 应拒绝未知字段");
    }

    #[test]
    fn all_12_mutation_kind_v1_serde() {
        let kinds = [
            MutationKindV1::GoldenIris,
            MutationKindV1::HardenedNails,
            MutationKindV1::ToughSkin,
            MutationKindV1::BoneRidge,
            MutationKindV1::ForearmScales,
            MutationKindV1::SpineSpurs,
            MutationKindV1::Horns,
            MutationKindV1::Tail,
            MutationKindV1::BackCarapace,
            MutationKindV1::ExtraArms,
            MutationKindV1::BodyEnlarge,
            MutationKindV1::BeastFace,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).expect("serialize");
            let back: MutationKindV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(kind, back, "MutationKindV1::{kind:?} serde round-trip 失败");
        }
    }

    #[test]
    fn mutation_event_v1_stage_transition_values() {
        // 验证各阶段跃迁的合法性
        let transitions = [
            (MutationStageV1::None, MutationStageV1::Subtle),
            (MutationStageV1::Subtle, MutationStageV1::Visible),
            (MutationStageV1::Visible, MutationStageV1::Heavy),
            (MutationStageV1::Heavy, MutationStageV1::Bestial),
        ];
        for (from, to) in transitions {
            let event = MutationEventV1 {
                entity: "test".to_string(),
                from_stage: from,
                to_stage: to,
                cumulative_toxin: 100.0,
                new_meridian_penalty: 0.03,
                server_tick: 0,
            };
            let json = serde_json::to_string(&event).expect("serialize");
            let back: MutationEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(event, back);
        }
    }
}
