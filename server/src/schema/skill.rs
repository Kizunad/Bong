//! plan-skill-v1 §8 IPC schema。与 `agent/packages/schema/src/skill.ts` 1:1 对齐。
//!
//! 四条 channel payload：
//! - `bong:skill/xp_gain` → `SkillXpGainPayloadV1`
//! - `bong:skill/lv_up` → `SkillLvUpPayloadV1`
//! - `bong:skill/cap_changed` → `SkillCapChangedPayloadV1`
//! - `bong:skill/scroll_used` → `SkillScrollUsedPayloadV1`
//!
//! 对齐测试：samples `agent/packages/schema/samples/skill-*.sample.json` 走 TypeBox validate；
//! 本文件通过 `include_str!` 反序列化同一份 samples 做 roundtrip 双端校验（参考 `botany.rs` 模式）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::skill::components::{SkillEntry as RuntimeSkillEntry, SkillSet as RuntimeSkillSet};

/// plan §8 SkillId — snake_case 字符串枚举，与 TS 侧 Type.Union 对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillIdV1 {
    Herbalism,
    Alchemy,
    Forging,
    Combat,
    Mineral,
    Cultivation,
}

/// plan §8 XpGainSource — tagged union（tag="type"）。
///
/// - `action`：plan §3.1 做中学，`plan_id` + `action` 明示来源 plan 内的哪一触发点。
/// - `scroll`：plan §3.2 残卷顿悟。
/// - `realm_breakthrough`：占位变体，plan §2.2 境界突破本身不给 XP，保留给 v2+。
/// - `mentor`：plan §3.2 v2+ 师承。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XpGainSourceV1 {
    Action { plan_id: String, action: String },
    Scroll { scroll_id: String, xp_grant: u32 },
    RealmBreakthrough,
    Mentor { mentor_char: u64 },
}

/// plan §8 `SkillXpGain` event → channel payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillXpGainPayloadV1 {
    pub v: u8,
    pub char_id: u64,
    pub skill: SkillIdV1,
    pub amount: u32,
    pub source: XpGainSourceV1,
}

impl SkillXpGainPayloadV1 {
    pub fn new(char_id: u64, skill: SkillIdV1, amount: u32, source: XpGainSourceV1) -> Self {
        Self {
            v: 1,
            char_id,
            skill,
            amount,
            source,
        }
    }
}

/// plan §8 `SkillLvUp` → channel payload。narration 不在此，agent P5 独立生成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillLvUpPayloadV1 {
    pub v: u8,
    pub char_id: u64,
    pub skill: SkillIdV1,
    pub new_lv: u8,
}

impl SkillLvUpPayloadV1 {
    pub fn new(char_id: u64, skill: SkillIdV1, new_lv: u8) -> Self {
        Self {
            v: 1,
            char_id,
            skill,
            new_lv,
        }
    }
}

/// plan §4 境界软挂钩 cap 变化。突破上调 / 跌落下修均走此 payload。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillCapChangedPayloadV1 {
    pub v: u8,
    pub char_id: u64,
    pub skill: SkillIdV1,
    pub new_cap: u8,
}

impl SkillCapChangedPayloadV1 {
    pub fn new(char_id: u64, skill: SkillIdV1, new_cap: u8) -> Self {
        Self {
            v: 1,
            char_id,
            skill,
            new_cap,
        }
    }
}

/// plan §3.2 残卷使用结算 payload。`was_duplicate=true` 时 `xp_granted=0`（不消耗）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillScrollUsedPayloadV1 {
    pub v: u8,
    pub char_id: u64,
    pub scroll_id: String,
    pub skill: SkillIdV1,
    pub xp_granted: u32,
    pub was_duplicate: bool,
}

impl SkillScrollUsedPayloadV1 {
    pub fn new(
        char_id: u64,
        scroll_id: impl Into<String>,
        skill: SkillIdV1,
        xp_granted: u32,
        was_duplicate: bool,
    ) -> Self {
        Self {
            v: 1,
            char_id,
            scroll_id: scroll_id.into(),
            skill,
            xp_granted,
            was_duplicate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillEntrySnapshotV1 {
    pub lv: u8,
    pub xp: u32,
    pub xp_to_next: u32,
    pub total_xp: u64,
    pub cap: u8,
    pub recent_gain_xp: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillSnapshotPayloadV1 {
    pub v: u8,
    pub char_id: u64,
    pub skills: BTreeMap<String, SkillEntrySnapshotV1>,
    pub consumed_scrolls: Vec<String>,
}

impl SkillEntrySnapshotV1 {
    pub fn from_runtime(entry: &RuntimeSkillEntry, cap: u8) -> Self {
        Self {
            lv: entry.lv,
            xp: entry.xp,
            xp_to_next: crate::skill::curve::xp_to_next(entry.lv),
            total_xp: entry.total_xp,
            cap,
            recent_gain_xp: 0,
        }
    }
}

impl SkillSnapshotPayloadV1 {
    pub fn new(
        char_id: u64,
        skills: BTreeMap<String, SkillEntrySnapshotV1>,
        consumed_scrolls: Vec<String>,
    ) -> Self {
        Self {
            v: 1,
            char_id,
            skills,
            consumed_scrolls,
        }
    }

    pub fn from_runtime(
        char_id: u64,
        skill_set: &RuntimeSkillSet,
        cap_for: impl Fn(crate::skill::components::SkillId) -> u8,
    ) -> Self {
        let mut skills = BTreeMap::new();
        for skill in crate::skill::components::SkillId::ALL {
            let entry = skill_set.skills.get(&skill).cloned().unwrap_or_default();
            skills.insert(
                skill.as_str().to_string(),
                SkillEntrySnapshotV1::from_runtime(&entry, cap_for(skill)),
            );
        }
        let mut consumed_scrolls = skill_set
            .consumed_scrolls
            .iter()
            .map(|scroll| scroll.as_str().to_string())
            .collect::<Vec<_>>();
        consumed_scrolls.sort();
        Self::new(char_id, skills, consumed_scrolls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Samples 定义在 agent/packages/schema/samples —— 双端共享同一份字节，避免 drift。
    const SAMPLE_XP_GAIN: &str =
        include_str!("../../../agent/packages/schema/samples/skill-xp-gain.sample.json");
    const SAMPLE_LV_UP: &str =
        include_str!("../../../agent/packages/schema/samples/skill-lv-up.sample.json");
    const SAMPLE_CAP_CHANGED: &str =
        include_str!("../../../agent/packages/schema/samples/skill-cap-changed.sample.json");
    const SAMPLE_SCROLL_USED: &str =
        include_str!("../../../agent/packages/schema/samples/skill-scroll-used.sample.json");
    const SAMPLE_SNAPSHOT: &str =
        include_str!("../../../agent/packages/schema/samples/skill-snapshot.sample.json");

    /// samples 是 JSON array（多案例），每一条都能反序列化成 Payload 并 roundtrip。
    fn assert_array_roundtrip<T>(raw: &str)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let values: Vec<serde_json::Value> =
            serde_json::from_str(raw).expect("sample file must be a JSON array of payloads");
        assert!(!values.is_empty(), "sample array must not be empty");
        for (i, value) in values.iter().enumerate() {
            let payload: T = serde_json::from_value(value.clone())
                .unwrap_or_else(|e| panic!("sample[{i}] failed to deserialize: {e}"));
            let back = serde_json::to_value(&payload).unwrap();
            let parsed_back: T = serde_json::from_value(back.clone()).unwrap();
            assert_eq!(parsed_back, payload, "sample[{i}] roundtrip mismatch");
        }
    }

    #[test]
    fn skill_xp_gain_samples_roundtrip() {
        assert_array_roundtrip::<SkillXpGainPayloadV1>(SAMPLE_XP_GAIN);
    }

    #[test]
    fn skill_lv_up_samples_roundtrip() {
        assert_array_roundtrip::<SkillLvUpPayloadV1>(SAMPLE_LV_UP);
    }

    #[test]
    fn skill_cap_changed_samples_roundtrip() {
        assert_array_roundtrip::<SkillCapChangedPayloadV1>(SAMPLE_CAP_CHANGED);
    }

    #[test]
    fn skill_scroll_used_samples_roundtrip() {
        assert_array_roundtrip::<SkillScrollUsedPayloadV1>(SAMPLE_SCROLL_USED);
    }

    #[test]
    fn skill_snapshot_samples_roundtrip() {
        assert_array_roundtrip::<SkillSnapshotPayloadV1>(SAMPLE_SNAPSHOT);
    }

    #[test]
    fn xp_gain_source_tagged_union_parses_all_variants() {
        let action: XpGainSourceV1 = serde_json::from_str(
            r#"{"type":"action","plan_id":"lingtian","action":"harvest_auto"}"#,
        )
        .unwrap();
        assert!(matches!(action, XpGainSourceV1::Action { .. }));

        let scroll: XpGainSourceV1 = serde_json::from_str(
            r#"{"type":"scroll","scroll_id":"scroll:bai_cao_tu_kao_can","xp_grant":500}"#,
        )
        .unwrap();
        assert!(matches!(scroll, XpGainSourceV1::Scroll { .. }));

        let rb: XpGainSourceV1 = serde_json::from_str(r#"{"type":"realm_breakthrough"}"#).unwrap();
        assert_eq!(rb, XpGainSourceV1::RealmBreakthrough);

        let mentor: XpGainSourceV1 =
            serde_json::from_str(r#"{"type":"mentor","mentor_char":42}"#).unwrap();
        assert!(matches!(mentor, XpGainSourceV1::Mentor { .. }));
    }

    #[test]
    fn skill_snapshot_from_runtime_fills_missing_skills() {
        let mut set = RuntimeSkillSet::default();
        set.skills.insert(
            crate::skill::components::SkillId::Alchemy,
            RuntimeSkillEntry {
                lv: 3,
                xp: 40,
                total_xp: 1_440,
                last_action_at: 99,
                recent_repeat_count: 0,
            },
        );

        let payload = SkillSnapshotPayloadV1::from_runtime(1001, &set, |_| 5);
        assert_eq!(payload.char_id, 1001);
        assert_eq!(
            payload.skills.len(),
            crate::skill::components::SkillId::ALL.len()
        );
        assert_eq!(payload.skills.get("alchemy").unwrap().lv, 3);
        assert_eq!(payload.skills.get("alchemy").unwrap().cap, 5);
        assert_eq!(payload.skills.get("forging").unwrap().lv, 0);
        assert_eq!(payload.skills.get("combat").unwrap().lv, 0);
        assert_eq!(payload.skills.get("mineral").unwrap().lv, 0);
        assert_eq!(payload.skills.get("cultivation").unwrap().lv, 0);
    }
}
