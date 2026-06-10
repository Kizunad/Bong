//! plan-territory-v1 P3 — 领地霸主变动叙事请求 IPC schema。
//!
//! 与 `agent/packages/schema/src/channels.ts` TERRITORY_NARRATION_REQUEST 通道对齐。
//!
//! 注意：agent 侧暂无订阅此通道的 runtime（P3 是纯 server workflow）。
//! server 已留 channel + schema 契约，同时通过 PendingGameplayNarrations.push_zone
//! 做兜底，保证 narration 不依赖 agent 即 in-game 可见。
//! 未来独立 agent PR 加 runtime 即升级为 LLM 渲染。

use serde::{Deserialize, Serialize};

/// 领地事件类型（三时刻）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerritoryEventKindV1 {
    /// 新霸主确立（旧主=None 或换主）。
    Established,
    /// 霸主被驱逐（霸主 influence 跌破 DOMINANCE_DROP 且无继任）。
    Ousted,
    /// 区域灵气耗尽（spirit_qi 从 >0 跌至 ≤0）。
    QiDepleted,
}

/// 领地叙事请求 IPC payload。
///
/// 与 `tiandao_hunt_narration.rs` 同构——deny_unknown_fields + v:u8 版本号 + new() 构造。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerritoryDominanceNarrationRequestV1 {
    pub v: u8,
    /// 事件所在 zone 名称。
    pub zone: String,
    /// 事件类型（三时刻：Established / Ousted / QiDepleted）。
    pub event_kind: TerritoryEventKindV1,
    /// 匿名境界段：Low / Mid / High（不传 char_id / 玩家名）。
    pub realm_band: String,
    /// 事件发生时的 zone spirit_qi（供叙事参考）。
    pub spirit_qi: f64,
}

impl TerritoryDominanceNarrationRequestV1 {
    pub fn new(
        zone: impl Into<String>,
        event_kind: TerritoryEventKindV1,
        realm_band: impl Into<String>,
        spirit_qi: f64,
    ) -> Self {
        Self {
            v: 1,
            zone: zone.into(),
            event_kind,
            realm_band: realm_band.into(),
            spirit_qi,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_established_wire_shape() {
        let payload = TerritoryDominanceNarrationRequestV1::new(
            "qingyun_peaks",
            TerritoryEventKindV1::Established,
            "mid",
            0.8,
        );

        let wire = serde_json::to_value(&payload).expect("payload should serialize");

        assert_eq!(
            wire,
            json!({
                "v": 1,
                "zone": "qingyun_peaks",
                "event_kind": "established",
                "realm_band": "mid",
                "spirit_qi": 0.8
            }),
            "Established wire shape 应与预期完全一致"
        );
    }

    #[test]
    fn serializes_ousted_wire_shape() {
        let payload = TerritoryDominanceNarrationRequestV1::new(
            "blood_valley",
            TerritoryEventKindV1::Ousted,
            "high",
            0.3,
        );
        let wire = serde_json::to_value(&payload).expect("payload should serialize");
        assert_eq!(
            wire["event_kind"],
            json!("ousted"),
            "Ousted wire value 应为 ousted"
        );
    }

    #[test]
    fn serializes_qi_depleted_wire_shape() {
        let payload = TerritoryDominanceNarrationRequestV1::new(
            "north_wastes",
            TerritoryEventKindV1::QiDepleted,
            "low",
            0.0,
        );
        let wire = serde_json::to_value(&payload).expect("payload should serialize");
        assert_eq!(
            wire["event_kind"],
            json!("qi_depleted"),
            "QiDepleted wire value 应为 qi_depleted"
        );
    }

    #[test]
    fn v_field_is_always_1() {
        let payload = TerritoryDominanceNarrationRequestV1::new(
            "spawn",
            TerritoryEventKindV1::Established,
            "low",
            0.5,
        );
        let wire = serde_json::to_value(&payload).expect("serialize ok");
        assert_eq!(wire["v"], json!(1), "v 字段应为 1（版本号）");
    }

    #[test]
    fn deny_unknown_fields_rejects_extra_fields() {
        // deny_unknown_fields 拒绝反序列化时出现未知字段
        let err = serde_json::from_value::<TerritoryDominanceNarrationRequestV1>(json!({
            "v": 1,
            "zone": "spawn",
            "event_kind": "established",
            "realm_band": "low",
            "spirit_qi": 0.5,
            "unknown_extra": "should_fail"
        }))
        .expect_err("未知字段应被拒绝 (deny_unknown_fields)");
        assert!(
            err.to_string().contains("unknown field"),
            "预期 unknown field 错误，实际={err}"
        );
    }

    #[test]
    fn deserializes_valid_payload() {
        let payload: TerritoryDominanceNarrationRequestV1 = serde_json::from_value(json!({
            "v": 1,
            "zone": "spawn",
            "event_kind": "qi_depleted",
            "realm_band": "high",
            "spirit_qi": 0.0
        }))
        .expect("合法 payload 应反序列化成功");
        assert_eq!(payload.event_kind, TerritoryEventKindV1::QiDepleted);
        assert_eq!(payload.realm_band, "high");
        assert_eq!(payload.zone, "spawn");
    }

    #[test]
    fn event_kind_variants_all_serialize() {
        let variants = [
            (TerritoryEventKindV1::Established, "established"),
            (TerritoryEventKindV1::Ousted, "ousted"),
            (TerritoryEventKindV1::QiDepleted, "qi_depleted"),
        ];
        for (variant, expected) in variants {
            let wire = serde_json::to_value(variant).expect("variant should serialize");
            assert_eq!(
                wire,
                json!(expected),
                "TerritoryEventKindV1::{variant:?} 应序列化为 {expected}"
            );
        }
    }

    #[test]
    fn deserializes_all_event_kind_variants() {
        assert_eq!(
            serde_json::from_value::<TerritoryEventKindV1>(json!("established")).unwrap(),
            TerritoryEventKindV1::Established
        );
        assert_eq!(
            serde_json::from_value::<TerritoryEventKindV1>(json!("ousted")).unwrap(),
            TerritoryEventKindV1::Ousted
        );
        assert_eq!(
            serde_json::from_value::<TerritoryEventKindV1>(json!("qi_depleted")).unwrap(),
            TerritoryEventKindV1::QiDepleted
        );
    }

    #[test]
    fn rejects_unknown_event_kind() {
        let err = serde_json::from_value::<TerritoryEventKindV1>(json!("unknown_event"))
            .expect_err("未知 event_kind 应被拒绝");
        assert!(
            err.to_string().contains("unknown variant"),
            "预期 unknown variant 错误，实际={err}"
        );
    }
}
