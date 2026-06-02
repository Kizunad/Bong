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

/// Version literal default for serde.
fn v1_literal() -> u8 {
    1
}

/// 变异推进事件 payload（server → agent），plan-dandao-runtime-wiring-v1 P2。
///
/// **Wire schema** 与 agent `MutationEventV1` TypeBox 严格对齐：
/// `{ v:1, entity_id:u64, from_stage, to_stage, cumulative_toxin, at_tick }`
///
/// - `entity_id` = `Entity::to_bits()`（u64 整数，对齐 agent `EntityId: Integer`）
/// - `v` 序列化为 1，agent `additionalProperties:false` 严格校验
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationEventV1 {
    /// Schema 版本，固定为 1，对齐 agent TypeBox `Type.Literal(1)`。
    #[serde(default = "v1_literal")]
    pub v: u8,
    /// Entity bits（`Entity::to_bits()`），对齐 agent `EntityId: Integer`。
    pub entity_id: u64,
    pub from_stage: MutationStageV1,
    pub to_stage: MutationStageV1,
    pub cumulative_toxin: f64,
    /// 对齐 agent `at_tick: Integer`。
    pub at_tick: u64,
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
        // 对齐后字段：v:1, entity_id:u64, from_stage, to_stage, cumulative_toxin, at_tick。
        let event = MutationEventV1 {
            v: 1,
            entity_id: 0x0001_0002_0003_0004u64,
            from_stage: MutationStageV1::Subtle,
            to_stage: MutationStageV1::Visible,
            cumulative_toxin: 105.0,
            at_tick: 50000,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let back: MutationEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(event, back, "MutationEventV1 serde round-trip 不应丢失数据");
    }

    #[test]
    fn mutation_event_v1_wire_fields_match_agent_typebox() {
        // 验证序列化 JSON 字段名与 agent MutationEventV1 TypeBox 完全一致。
        let event = MutationEventV1 {
            v: 1,
            entity_id: 42,
            from_stage: MutationStageV1::None,
            to_stage: MutationStageV1::Subtle,
            cumulative_toxin: 10.0,
            at_tick: 9999,
        };
        let json = serde_json::to_string(&event).expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse");
        // 必须含 agent 期望的全部字段。
        assert_eq!(value["v"], 1, "v 字段必须为 1");
        assert_eq!(value["entity_id"], 42, "entity_id 必须为 u64 整数");
        assert_eq!(value["from_stage"], "none");
        assert_eq!(value["to_stage"], "subtle");
        assert_eq!(value["cumulative_toxin"], 10.0);
        assert_eq!(value["at_tick"], 9999);
        // 不应含旧字段。
        assert!(value.get("entity").is_none(), "旧 entity 字段不应出现");
        assert!(
            value.get("new_meridian_penalty").is_none(),
            "旧 new_meridian_penalty 不应出现"
        );
        assert!(
            value.get("server_tick").is_none(),
            "旧 server_tick 不应出现"
        );
    }

    #[test]
    fn mutation_event_v1_shared_sample_roundtrip() {
        // 与 agent/packages/schema/samples/mutation_event_v1.json 同一 sample 正反对拍。
        let sample_json = r#"{"v":1,"entity_id":281474976710657,"from_stage":"subtle","to_stage":"visible","cumulative_toxin":105.0,"at_tick":50000}"#;
        let parsed: MutationEventV1 =
            serde_json::from_str(sample_json).expect("sample JSON 应能反序列化");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.entity_id, 281474976710657u64);
        assert_eq!(parsed.from_stage, MutationStageV1::Subtle);
        assert_eq!(parsed.to_stage, MutationStageV1::Visible);
        assert_eq!(parsed.cumulative_toxin, 105.0);
        assert_eq!(parsed.at_tick, 50000);
        // 回序列化应产出相同 JSON（字段顺序由 serde 保证稳定）。
        let back_json = serde_json::to_string(&parsed).expect("serialize");
        let back: MutationEventV1 = serde_json::from_str(&back_json).expect("deserialize");
        assert_eq!(parsed, back);
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
        // 验证各阶段跃迁的合法性（对齐后字段）。
        let transitions = [
            (MutationStageV1::None, MutationStageV1::Subtle),
            (MutationStageV1::Subtle, MutationStageV1::Visible),
            (MutationStageV1::Visible, MutationStageV1::Heavy),
            (MutationStageV1::Heavy, MutationStageV1::Bestial),
        ];
        for (from, to) in transitions {
            let event = MutationEventV1 {
                v: 1,
                entity_id: 1234567890u64,
                from_stage: from,
                to_stage: to,
                cumulative_toxin: 100.0,
                at_tick: 0,
            };
            let json = serde_json::to_string(&event).expect("serialize");
            let back: MutationEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(event, back);
        }
    }

    // ─── negative pinning tests ─────────────────────────────────────────

    #[test]
    fn mutation_stage_v1_rejects_invalid_string() {
        let result = serde_json::from_str::<MutationStageV1>(r#""corrupted""#);
        assert!(
            result.is_err(),
            "invalid stage string 'corrupted' should fail deserialization"
        );
    }

    #[test]
    fn mutation_kind_v1_rejects_invalid_string() {
        let result = serde_json::from_str::<MutationKindV1>(r#""laser_eyes""#);
        assert!(
            result.is_err(),
            "invalid kind string 'laser_eyes' should fail deserialization"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_unknown_fields() {
        // 对齐后字段集，多出 bonus_field 应被 deny_unknown_fields 拒绝。
        let json = r#"{"v":1,"entity_id":42,"from_stage":"none","to_stage":"subtle","cumulative_toxin":0.0,"at_tick":0,"bonus_field":42}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field in MutationEventV1"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_old_entity_field() {
        // 旧 wire 格式（entity String），不应被新 struct 接受。
        let json = r#"{"v":1,"entity":"player_old","from_stage":"none","to_stage":"subtle","cumulative_toxin":0.0,"at_tick":0}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "旧 entity 字段应被 deny_unknown_fields 拒绝（已替换为 entity_id）"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_old_server_tick_field() {
        // 旧 server_tick 字段（已替换为 at_tick）。
        let json = r#"{"v":1,"entity_id":42,"from_stage":"none","to_stage":"subtle","cumulative_toxin":0.0,"server_tick":0}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "旧 server_tick 字段应被 deny_unknown_fields 拒绝（已替换为 at_tick）"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_new_meridian_penalty_field() {
        // 旧 new_meridian_penalty 字段（已从 wire 移除）。
        let json = r#"{"v":1,"entity_id":42,"from_stage":"none","to_stage":"subtle","cumulative_toxin":0.0,"at_tick":0,"new_meridian_penalty":0.1}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "旧 new_meridian_penalty 字段应被 deny_unknown_fields 拒绝"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_missing_required_field() {
        // Missing entity_id（必须字段）。
        let json = r#"{"v":1,"from_stage":"subtle","to_stage":"visible","cumulative_toxin":0.0,"at_tick":0}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "missing entity_id should fail for MutationEventV1"
        );
    }

    #[test]
    fn mutation_event_v1_rejects_missing_at_tick() {
        // Missing at_tick（必须字段）。
        let json = r#"{"v":1,"entity_id":42,"from_stage":"subtle","to_stage":"visible","cumulative_toxin":0.0}"#;
        let result = serde_json::from_str::<MutationEventV1>(json);
        assert!(
            result.is_err(),
            "missing at_tick should fail for MutationEventV1"
        );
    }

    #[test]
    fn dandao_style_v1_rejects_unknown_fields() {
        let json = r#"{"entity":"test","brew_count":0,"pill_intake_count":0,"cumulative_toxin":0.0,"mutation_stage":"none","mastery_ticks":0,"server_tick":0,"secret_field":"hack"}"#;
        let result = serde_json::from_str::<DandaoStyleV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field in DandaoStyleV1"
        );
    }

    #[test]
    fn dandao_style_v1_rejects_invalid_mutation_stage() {
        let json = r#"{"entity":"test","brew_count":0,"pill_intake_count":0,"cumulative_toxin":0.0,"mutation_stage":"mega","mastery_ticks":0,"server_tick":0}"#;
        let result = serde_json::from_str::<DandaoStyleV1>(json);
        assert!(
            result.is_err(),
            "invalid mutation_stage 'mega' should fail for DandaoStyleV1"
        );
    }

    #[test]
    fn active_mutation_v1_rejects_unknown_fields() {
        let json = r#"{"kind":"horns","body_slot":"head","level":1,"acquired_tick":0,"phantom_field":true}"#;
        let result = serde_json::from_str::<ActiveMutationV1>(json);
        assert!(
            result.is_err(),
            "deny_unknown_fields should reject unknown field in ActiveMutationV1"
        );
    }

    #[test]
    fn all_5_mutation_stage_v1_serde() {
        let stages = [
            MutationStageV1::None,
            MutationStageV1::Subtle,
            MutationStageV1::Visible,
            MutationStageV1::Heavy,
            MutationStageV1::Bestial,
        ];
        for stage in stages {
            let json = serde_json::to_string(&stage).expect("serialize");
            let back: MutationStageV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                stage, back,
                "MutationStageV1::{stage:?} serde round-trip failed"
            );
        }
    }
}
