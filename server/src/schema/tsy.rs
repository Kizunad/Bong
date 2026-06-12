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

/// plan-agent-ui-data-v1 server 栈修复 — 坍缩渊首次激活事件 wire 结构。
///
/// 与 `agent/packages/schema/src/tsy.ts` `TsyZoneActivatedV1` TypeBox 1:1 对齐：
/// - `source_class` 使用 snake_case 字面量（"dao_lord" / "sect_ruins" / "battle_sediment"）。
/// - `player_id`：触发 first-enter 的玩家 canonical_player_id（"offline:<name>"），
///   由 tsy_event_bridge 在 publish 时从 `TsyZoneActivated.triggering_player_entity`
///   解析写入；agent 直接用作 `target_player` 选人，不再靠 zone 名瞎匹配。
/// - 通过 `agent/packages/schema/samples/tsy-zone-activated.sample.json` 双端校验。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TsyZoneActivatedEventV1 {
    pub v: u8,
    pub kind: String,
    pub tick: u64,
    pub family_id: String,
    /// snake_case 字面量：`"dao_lord"` / `"sect_ruins"` / `"battle_sediment"`。
    pub source_class: String,
    /// 触发首次进入的玩家 canonical_player_id（"offline:<name>" 格式）。
    /// agent 直接用此字段找目标玩家，无需再靠 zone 名猜测。
    pub player_id: String,
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

    // ── TsyZoneActivatedEventV1 ──────────────────────────────────────────────

    /// 跨端契约 pin 测试：tsy-zone-activated.sample.json 双端对齐
    /// （server Rust ↔ agent TypeBox TsyZoneActivatedV1）。
    #[test]
    fn deserialize_tsy_zone_activated_sample() {
        let json =
            include_str!("../../../agent/packages/schema/samples/tsy-zone-activated.sample.json");
        let ev: TsyZoneActivatedEventV1 =
            serde_json::from_str(json).expect("tsy-zone-activated.sample.json should deserialize");
        assert_eq!(ev.v, 1, "v 应为 1");
        assert_eq!(
            ev.kind, "tsy_zone_activated",
            "kind 应为 tsy_zone_activated"
        );
        assert_eq!(ev.tick, 12345, "tick 应为 12345");
        assert_eq!(ev.family_id, "tsy_lingxu_01", "family_id 对齐");
        assert_eq!(ev.source_class, "dao_lord", "source_class 应为 dao_lord");
        assert_eq!(
            ev.player_id, "offline:Kiz",
            "player_id 应为 offline:Kiz（触发玩家）"
        );
    }

    /// round-trip：每个 source_class 变体均序列化为正确 snake_case 字面量。
    #[test]
    fn tsy_zone_activated_source_class_literals() {
        for (source_class, expected) in [
            ("dao_lord", "\"dao_lord\""),
            ("sect_ruins", "\"sect_ruins\""),
            ("battle_sediment", "\"battle_sediment\""),
        ] {
            let ev = TsyZoneActivatedEventV1 {
                v: 1,
                kind: "tsy_zone_activated".to_string(),
                tick: 0,
                family_id: "tsy_test".to_string(),
                source_class: source_class.to_string(),
                player_id: "offline:TestPlayer".to_string(),
            };
            let json = serde_json::to_string(&ev).expect("serialize");
            assert!(
                json.contains(expected),
                "source_class={source_class} 序列化后应含 {expected}，实为 {json}"
            );
            // 反序列化 round-trip
            let parsed: TsyZoneActivatedEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                parsed.source_class, source_class,
                "round-trip source_class 应为 {source_class}"
            );
        }
    }

    /// 含 player_id — 与 TS 端对齐（bridge 解析触发玩家 entity → canonical_player_id 写入）。
    #[test]
    fn tsy_zone_activated_has_player_id_field() {
        let ev = TsyZoneActivatedEventV1 {
            v: 1,
            kind: "tsy_zone_activated".to_string(),
            tick: 500,
            family_id: "tsy_lingxu_01".to_string(),
            source_class: "sect_ruins".to_string(),
            player_id: "offline:Kiz".to_string(),
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(
            json.contains("player_id"),
            "TsyZoneActivatedEventV1 应含 player_id 字段，实为 {json}"
        );
        assert!(
            json.contains("offline:Kiz"),
            "player_id 值应为 'offline:Kiz'，实为 {json}"
        );
    }

    /// player_id round-trip：序列化后反序列化回完全一致。
    #[test]
    fn tsy_zone_activated_player_id_round_trip() {
        let ev = TsyZoneActivatedEventV1 {
            v: 1,
            kind: "tsy_zone_activated".to_string(),
            tick: 500,
            family_id: "tsy_lingxu_01".to_string(),
            source_class: "sect_ruins".to_string(),
            player_id: "offline:WanLingFeng".to_string(),
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let parsed: TsyZoneActivatedEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed.player_id, "offline:WanLingFeng",
            "player_id round-trip 应保持原值"
        );
    }
}
