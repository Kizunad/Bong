//! plan-combat-skill-feedback-bridges-v1 P0 — MeridianSeveredEventV1 serde struct。
//!
//! 与 agent `@bong/schema` `MeridianSeveredEventV1` TypeBox 定义 1:1 对应。
//! `SeveredSource` 直接 use 自 `cultivation::meridian::severed`（已 derive Serialize/Deserialize）。
//!
//! serde 外部标签（默认）：
//!
//!   - 6 个单元变体序列化为纯字符串 `"VoluntarySever"` 等
//!   - `Other(String)` 序列化为 `{"Other":"<reason>"}` 对象
//!
//! 与 TS `Type.Literal(...)` + `Type.Object({Other:Type.String(...)})` 完全对应。

use serde::{Deserialize, Serialize};

use crate::cultivation::meridian::severed::SeveredSource;

pub const MERIDIAN_SEVERED_EVENT_TYPE: &str = "meridian_severed";

/// Redis `bong:meridian_severed` 通道载荷。
///
/// 字段集与 agent `MeridianSeveredEventV1` TypeBox 对齐，
/// `deny_unknown_fields` 确保双端 schema 不悄悄漂移。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeridianSeveredEventV1 {
    /// schema version（恒为 1）。
    pub v: u8,
    /// 事件类型字面量（恒为 `"meridian_severed"`）。
    #[serde(rename = "type")]
    pub event_type: String,
    /// 玩家 wire id：有 Username 组件输出 `"offline:{username}"`，NPC 输出 `"char:{bits}"`。
    pub entity_id: String,
    /// 哪条经脉断了（MeridianId string name，如 `"Lung"`）。
    pub meridian_id: String,
    /// SEVERED 来源 7 类（serde 外部标签，wire 形态与 TS Union 对齐）。
    pub source: SeveredSource,
    /// 服务端 tick 时戳。
    pub at_tick: u64,
}

impl MeridianSeveredEventV1 {
    pub fn new(
        entity_id: impl Into<String>,
        meridian_id: impl Into<String>,
        source: SeveredSource,
        at_tick: u64,
    ) -> Self {
        Self {
            v: 1,
            event_type: MERIDIAN_SEVERED_EVENT_TYPE.to_string(),
            entity_id: entity_id.into(),
            meridian_id: meridian_id.into(),
            source,
            at_tick,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 七类 SeveredSource sample 文件（与 agent TS validateMeridianSeveredEventV1Contract 同源）
    // 逐条做 Rust serde from_str 校验，确保双端 wire 形态对齐。
    const SAMPLE_FILE: &str =
        include_str!("../../../agent/packages/schema/samples/meridian-severed-event.sample.json");

    fn load_samples() -> Vec<serde_json::Value> {
        serde_json::from_str(SAMPLE_FILE).expect("sample file should parse as JSON array")
    }

    /// 所有 7 条 sample 都能 Rust serde 反序列化
    #[test]
    fn all_seven_samples_deserialize() {
        let samples = load_samples();
        assert_eq!(
            samples.len(),
            7,
            "sample file should contain exactly 7 entries"
        );
        for (i, sample) in samples.iter().enumerate() {
            let result: Result<MeridianSeveredEventV1, _> = serde_json::from_value(sample.clone());
            assert!(
                result.is_ok(),
                "sample[{i}] should deserialize into MeridianSeveredEventV1; error: {:?}",
                result.err()
            );
        }
    }

    /// VoluntarySever 纯字符串 wire 形态
    #[test]
    fn voluntary_sever_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[0].clone()).expect("sample[0] VoluntarySever");
        assert_eq!(s.v, 1);
        assert_eq!(s.event_type, MERIDIAN_SEVERED_EVENT_TYPE);
        assert_eq!(s.entity_id, "offline:XiaoMing");
        assert_eq!(s.meridian_id, "Lung");
        assert_eq!(s.source, SeveredSource::VoluntarySever);
        // 序列化回去应是纯字符串 "VoluntarySever"
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "VoluntarySever");
    }

    /// BackfireOverload 纯字符串 wire 形态
    #[test]
    fn backfire_overload_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[1].clone()).expect("sample[1] BackfireOverload");
        assert_eq!(s.source, SeveredSource::BackfireOverload);
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "BackfireOverload");
    }

    /// OverloadTear 纯字符串 wire 形态
    #[test]
    fn overload_tear_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[2].clone()).expect("sample[2] OverloadTear");
        assert_eq!(s.source, SeveredSource::OverloadTear);
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "OverloadTear");
    }

    /// CombatWound 纯字符串 wire 形态
    #[test]
    fn combat_wound_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[3].clone()).expect("sample[3] CombatWound");
        assert_eq!(s.source, SeveredSource::CombatWound);
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "CombatWound");
    }

    /// TribulationFail 纯字符串 wire 形态
    #[test]
    fn tribulation_fail_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[4].clone()).expect("sample[4] TribulationFail");
        assert_eq!(s.source, SeveredSource::TribulationFail);
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "TribulationFail");
    }

    /// DuguDistortion 纯字符串 wire 形态
    #[test]
    fn dugu_distortion_is_plain_string() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[5].clone()).expect("sample[5] DuguDistortion");
        assert_eq!(s.source, SeveredSource::DuguDistortion);
        let json = serde_json::to_value(&s).expect("re-serialize");
        assert_eq!(json["source"], "DuguDistortion");
    }

    /// Other(String) 序列化为 `{"Other":"<reason>"}` 对象（serde 外部标签）
    #[test]
    fn other_source_is_object_wire_form() {
        let samples = load_samples();
        let s: MeridianSeveredEventV1 =
            serde_json::from_value(samples[6].clone()).expect("sample[6] Other");
        assert_eq!(s.source, SeveredSource::Other("上古遗物反噬".to_string()));
        let json = serde_json::to_value(&s).expect("re-serialize");
        // Other 变体序列化为 {"Other":"上古遗物反噬"} 对象，非纯字符串
        assert!(
            json["source"].is_object(),
            "Other should serialize as object"
        );
        assert_eq!(json["source"]["Other"], "上古遗物反噬");
    }

    /// deny_unknown_fields 应拒绝多余字段
    #[test]
    fn deny_unknown_fields_rejects_extra() {
        let bad = serde_json::json!({
            "v": 1,
            "type": "meridian_severed",
            "entity_id": "offline:Test",
            "meridian_id": "Lung",
            "source": "CombatWound",
            "at_tick": 100,
            "extra_field": "forbidden"
        });
        let result: Result<MeridianSeveredEventV1, _> = serde_json::from_value(bad);
        assert!(
            result.is_err(),
            "extra fields should be rejected by deny_unknown_fields"
        );
    }

    /// roundtrip：序列化再反序列化结果相同
    #[test]
    fn roundtrip_all_variants() {
        let variants = vec![
            SeveredSource::VoluntarySever,
            SeveredSource::BackfireOverload,
            SeveredSource::OverloadTear,
            SeveredSource::CombatWound,
            SeveredSource::TribulationFail,
            SeveredSource::DuguDistortion,
            SeveredSource::Other("测试扩展来源".to_string()),
        ];
        for source in variants {
            let event = MeridianSeveredEventV1::new("offline:Test", "Lung", source.clone(), 42);
            let json = serde_json::to_string(&event).expect("serialize");
            let back: MeridianSeveredEventV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                back.source, source,
                "roundtrip should preserve source variant {:?}",
                source
            );
            assert_eq!(back.v, 1);
            assert_eq!(back.event_type, MERIDIAN_SEVERED_EVENT_TYPE);
        }
    }
}
