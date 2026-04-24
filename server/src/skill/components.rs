//! plan-skill-v1 §8 子技能数据契约：SkillId / SkillEntry / SkillSet / ScrollId。
//!
//! 世界观锚点：`worldview.md §三` 材料/丹药/器物为辅助，技艺再熟也不能替代境界。
//! 因此 SkillSet 只挂玩家 entity（plan §8），BlockEntity 不参与。
//!
//! 曲线 / 升级算子见 [`super::curve`]。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// plan §8 游戏内 tick，与 `player::gameplay::GameplayTick` 对齐。
pub type Tick = u64;

/// plan §1 首批 skill 列表（MVP 三种）。v2+ 的战斗武学 / 阵法 / 师承均待定（见 plan §11）。
///
/// serde 落盘为 snake_case 字符串，与 `agent/packages/schema/src/skill.ts` 对齐。
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillId {
    Herbalism,
    Alchemy,
    Forging,
}

impl SkillId {
    /// plan §7 汇总表 source-of-truth string id，供 XpGainSource.Action::plan / Redis channel 派生使用。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Herbalism => "herbalism",
            Self::Alchemy => "alchemy",
            Self::Forging => "forging",
        }
    }
}

/// plan §3.2 残卷唯一标识（非仅 skill_id）。`consumed_scrolls.insert(scroll_id)` 判"已学"。
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScrollId(pub String);

impl ScrollId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// plan §8 单条 skill 状态快照。`lv` 是 real_lv（不含 cap 压制），展示层再做 effective_lv = min(lv, cap)。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillEntry {
    /// real_lv，0-10；超 cap 不扣，只影响 effective_lv。
    pub lv: u8,
    /// 当前 lv 内累积 XP；跨级后归零。
    pub xp: u32,
    /// 终身总 XP 快照（统计用，不因升级而清零）。
    pub total_xp: u64,
    /// 最近一次上交 XP 的 tick，用于 UI 行的"最近 +XP"窗口（plan §5.1 左列）。
    pub last_action_at: Tick,
    /// plan §3.1 多样性奖励去重计数：连续重复同一动作 +1，换动作归零。
    pub recent_repeat_count: u8,
}

/// plan §8 玩家的 skill 集合。挂玩家 entity；`consumed_scrolls` 一生累积不清零。
///
/// 死透重生（plan-death-lifecycle §4/§5）→ 新角色 `SkillSet::default()` 全新实例，
/// 旧 `consumed_scrolls` 不迁移（worldview §十二：经验在玩家脑子里而非角色身上）。
#[derive(Debug, Default, Clone, Component, Serialize, Deserialize)]
pub struct SkillSet {
    pub skills: HashMap<SkillId, SkillEntry>,
    pub consumed_scrolls: HashSet<ScrollId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_id_stringifies_to_snake_case() {
        assert_eq!(SkillId::Herbalism.as_str(), "herbalism");
        assert_eq!(SkillId::Alchemy.as_str(), "alchemy");
        assert_eq!(SkillId::Forging.as_str(), "forging");

        let json = serde_json::to_string(&SkillId::Herbalism).unwrap();
        assert_eq!(json, "\"herbalism\"");
    }

    #[test]
    fn scroll_id_serializes_as_transparent_string() {
        let id = ScrollId::new("scroll:bai_cao_tu_kao_can");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"scroll:bai_cao_tu_kao_can\"");
        let back: ScrollId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn skill_set_default_is_empty() {
        let set = SkillSet::default();
        assert!(set.skills.is_empty());
        assert!(set.consumed_scrolls.is_empty());
    }

    #[test]
    fn skill_entry_roundtrip() {
        let entry = SkillEntry {
            lv: 4,
            xp: 700,
            total_xp: 3_700,
            last_action_at: 20_480,
            recent_repeat_count: 2,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: SkillEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back, entry);
    }
}
