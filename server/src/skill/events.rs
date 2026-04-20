//! plan-skill-v1 §8 Events：四种 skill 生命周期事件。
//!
//! - `SkillXpGain` —— 做中学 / 残卷顿悟 / 师承输入，amount 可为 0（失败给 0 也要记一次 source）。
//! - `SkillLvUp` —— 每升一级一条（跨多级拆多条）。narration **不放这里** —— P5 由 agent 按 channel
//!   生成后通过独立通道回推（见 plan §2.3）。
//! - `SkillCapChanged` —— 境界突破 / 跌落触发（plan §4）。
//! - `SkillScrollUsed` —— 残卷拖入槽的结算；`was_duplicate=true` 时 `xp_granted=0`（plan §3.2）。
//!
//! `XpGainSource` 为 tagged union（`#[serde(tag="type")]`），与 TS 侧
//! `agent/packages/schema/src/skill.ts` 的 Type.Union 对齐。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Entity, Event};

use super::components::{ScrollId, SkillId};

/// plan §8 XP 来源枚举，agent 消费来区分"动作 / 残卷 / 突破 / 师承"四路。
///
/// 与 TS 侧 Type.Union 保持 tag/field 名一致：tag = "type"，`plan`/`action` snake_case。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XpGainSource {
    /// plan §3.1 做中学：`plan_id` 指向触发 plan 名（例 `"lingtian"`），`action` 是
    /// 具体子动作（例 `"harvest_auto"`）。plan §7 汇总表 为 source of truth。
    Action {
        plan_id: &'static str,
        action: &'static str,
    },
    /// plan §3.2 残卷顿悟：携带 scroll_id + 设计态 xp_grant（与 SkillScroll.xp_grant 一致）。
    Scroll { scroll_id: ScrollId, xp_grant: u32 },
    /// 境界突破本身**不给 XP**（plan §2.2 / §3.3），此变体保留给 v2+ 特殊境界解锁场景。
    RealmBreakthrough,
    /// plan §3.2 v2+ 师承：师父消耗 qi 传功；mentor_char 引用师父 entity。
    Mentor { mentor_char: u64 },
}

/// plan §8 做中学 / 顿悟 入 XP 事件。`char_entity` 为玩家 Bevy entity；`amount` 可为 0。
#[derive(Debug, Clone, Event)]
pub struct SkillXpGain {
    pub char_entity: Entity,
    pub skill: SkillId,
    pub amount: u32,
    pub source: XpGainSource,
}

/// plan §8 升级事件。narration 由 agent 在 P5 生成，不在此携带。
#[derive(Debug, Clone, Copy, Event)]
pub struct SkillLvUp {
    pub char_entity: Entity,
    pub skill: SkillId,
    pub new_lv: u8,
}

/// plan §4 境界软挂钩变更事件：境界突破 → cap 上调；境界跌落 → cap 下修。
#[derive(Debug, Clone, Copy, Event)]
pub struct SkillCapChanged {
    pub char_entity: Entity,
    pub skill: SkillId,
    pub new_cap: u8,
}

/// plan §3.2 残卷使用结算。`was_duplicate=true` 时 `xp_granted=0`，scroll **不消耗**（client 侧 retreat）。
#[derive(Debug, Clone, Event)]
pub struct SkillScrollUsed {
    pub char_entity: Entity,
    pub scroll_id: ScrollId,
    pub skill: SkillId,
    pub xp_granted: u32,
    pub was_duplicate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_gain_source_action_serializes_with_type_tag() {
        let src = XpGainSource::Action {
            plan_id: "lingtian",
            action: "harvest_auto",
        };
        let json = serde_json::to_string(&src).unwrap();
        assert_eq!(
            json,
            r#"{"type":"action","plan_id":"lingtian","action":"harvest_auto"}"#
        );
    }

    #[test]
    fn xp_gain_source_scroll_serializes_fields() {
        let src = XpGainSource::Scroll {
            scroll_id: ScrollId::new("scroll:bai_cao_tu_kao_can"),
            xp_grant: 500,
        };
        let json = serde_json::to_string(&src).unwrap();
        assert!(json.contains(r#""type":"scroll""#));
        assert!(json.contains(r#""scroll_id":"scroll:bai_cao_tu_kao_can""#));
        assert!(json.contains(r#""xp_grant":500"#));
    }

    #[test]
    fn xp_gain_source_realm_breakthrough_has_only_type() {
        let json = serde_json::to_string(&XpGainSource::RealmBreakthrough).unwrap();
        assert_eq!(json, r#"{"type":"realm_breakthrough"}"#);
    }
}
