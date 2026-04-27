//! plan-tsy-zone-followup-v1 §2 — server 侧 TsyEnter/TsyExit V1 wire serde 结构。
//!
//! 与 `agent/packages/schema/src/tsy.ts` TypeBox 定义双端对齐：字段名、类型、`kind`
//! 字面量、`v` 字面量、`pos` 数组形态、`reason` literal 必须 1:1 对齐。
//!
//! 通过 `agent/packages/schema/samples/tsy-{enter,exit}-event.sample.json` 双端校验
//! （inline test 反序列化 sample → assert 字段）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsyDimensionAnchorV1 {
    pub dimension: String,
    /// `[x, y, z]`，f64 精度。与 schema/src/tsy.ts `pos` 一致。
    pub pos: [f64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsyFilteredItemV1 {
    pub instance_id: u64,
    pub template_id: String,
    /// 当前唯一可能值：`"spirit_quality_too_high"`。如未来扩多原因，此处用 enum + literal union。
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsyEnterEventV1 {
    pub v: u8,
    pub kind: String,
    pub tick: u64,
    pub player_id: String,
    pub family_id: String,
    pub return_to: TsyDimensionAnchorV1,
    pub filtered_items: Vec<TsyFilteredItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsyExitEventV1 {
    pub v: u8,
    pub kind: String,
    pub tick: u64,
    pub player_id: String,
    pub family_id: String,
    pub duration_ticks: u64,
    pub qi_drained_total: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_tsy_enter_event_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/tsy-enter-event.sample.json");
        let ev: TsyEnterEventV1 = serde_json::from_str(json)
            .expect("tsy-enter-event.sample.json should deserialize into TsyEnterEventV1");
        assert_eq!(ev.v, 1);
        assert_eq!(ev.kind, "tsy_enter");
        assert_eq!(ev.player_id, "kiz");
        assert_eq!(ev.family_id, "tsy_lingxu_01");
        assert_eq!(ev.return_to.dimension, "minecraft:overworld");
        assert_eq!(ev.return_to.pos, [0.0, 65.0, 0.0]);
        assert_eq!(ev.filtered_items.len(), 1);
        assert_eq!(ev.filtered_items[0].instance_id, 7);
        assert_eq!(ev.filtered_items[0].template_id, "bone_coin");
        assert_eq!(ev.filtered_items[0].reason, "spirit_quality_too_high");
    }

    #[test]
    fn deserialize_tsy_exit_event_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/tsy-exit-event.sample.json");
        let ev: TsyExitEventV1 = serde_json::from_str(json)
            .expect("tsy-exit-event.sample.json should deserialize into TsyExitEventV1");
        assert_eq!(ev.v, 1);
        assert_eq!(ev.kind, "tsy_exit");
        assert_eq!(ev.family_id, "tsy_lingxu_01");
        assert_eq!(ev.duration_ticks, 12000);
        assert!((ev.qi_drained_total - 350.5).abs() < 1e-9);
    }

    #[test]
    fn round_trip_tsy_enter_event_through_serde() {
        let ev = TsyEnterEventV1 {
            v: 1,
            kind: "tsy_enter".to_string(),
            tick: 12345,
            player_id: "kiz".to_string(),
            family_id: "tsy_lingxu_01".to_string(),
            return_to: TsyDimensionAnchorV1 {
                dimension: "minecraft:overworld".to_string(),
                pos: [0.0, 65.0, 0.0],
            },
            filtered_items: vec![TsyFilteredItemV1 {
                instance_id: 7,
                template_id: "bone_coin".to_string(),
                reason: "spirit_quality_too_high".to_string(),
            }],
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let parsed: TsyEnterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ev);
    }

    #[test]
    fn round_trip_tsy_exit_event_through_serde() {
        let ev = TsyExitEventV1 {
            v: 1,
            kind: "tsy_exit".to_string(),
            tick: 99999,
            player_id: "kiz".to_string(),
            family_id: "tsy_lingxu_01".to_string(),
            duration_ticks: 12000,
            qi_drained_total: 0.0,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let parsed: TsyExitEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ev);
    }
}
