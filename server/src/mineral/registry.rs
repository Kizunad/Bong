//! plan-mineral-v1 §7 — `MineralRegistry` resource。
//!
//! 静态登记 18 个 mineral 的元数据（tier / vanilla_block / category /
//! forge_tier_min / 默认 lifetime / lingshi 灵气区间）。alchemy / forge / ipc
//! 由此查 mineral_id → 元数据。
//!
//! Registry 在 server 启动时由 [`build_default_registry`] 一次性构造。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

use super::types::{MineralCategory, MineralId, MineralRarity};

/// plan §1.4 — 灵石灵气初始区间。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LingShiQiRange {
    pub min: f32,
    pub max: f32,
}

/// 单条 mineral 的 runtime 元数据。
#[derive(Debug, Clone)]
pub struct MineralEntry {
    pub id: MineralId,
    pub canonical_name: &'static str,
    pub display_name_zh: &'static str,
    pub rarity: MineralRarity,
    pub category: MineralCategory,
    pub vanilla_block: &'static str,
    /// 0 = 不入炉。
    pub forge_tier_min: u8,
    /// 灵石专用：初始灵气区间。其他 mineral 为 None。
    pub ling_shi_qi_range: Option<LingShiQiRange>,
    /// shelflife profile id（仅灵石），其他 mineral 为 None。
    pub decay_profile: Option<&'static str>,
}

#[derive(Debug, Default, Resource)]
pub struct MineralRegistry {
    by_id: HashMap<MineralId, MineralEntry>,
}

impl MineralRegistry {
    pub fn insert(&mut self, entry: MineralEntry) {
        self.by_id.insert(entry.id, entry);
    }

    pub fn get(&self, id: MineralId) -> Option<&MineralEntry> {
        self.by_id.get(&id)
    }

    pub fn get_by_str(&self, s: &str) -> Option<&MineralEntry> {
        MineralId::from_str(s).and_then(|id| self.by_id.get(&id))
    }

    pub fn is_valid_mineral_id(&self, s: &str) -> bool {
        self.get_by_str(s).is_some()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MineralEntry> {
        self.by_id.values()
    }
}

/// 构造默认 registry — 18 个 mineral 全部登记，按 plan §1 四表 + §1.4 灵石数据。
pub fn build_default_registry() -> MineralRegistry {
    let mut reg = MineralRegistry::default();

    // 金属系
    reg.insert(metal(MineralId::FanTie, "凡铁"));
    reg.insert(metal(MineralId::CuTie, "粗铁"));
    reg.insert(metal(MineralId::ZaGang, "杂钢"));
    reg.insert(metal(MineralId::LingTie, "灵铁"));
    reg.insert(metal(MineralId::SuiTie, "髓铁"));
    reg.insert(metal(MineralId::CanTie, "残铁"));
    reg.insert(metal(MineralId::KuJin, "枯金"));

    // 灵晶系
    reg.insert(crystal(MineralId::LingJing, "灵晶"));
    reg.insert(crystal(MineralId::YuSui, "玉髓"));
    reg.insert(crystal(MineralId::WuYao, "乌曜石"));

    // 丹砂辅料
    reg.insert(alchemy_aux(MineralId::DanSha, "丹砂"));
    reg.insert(alchemy_aux(MineralId::ZhuSha, "朱砂"));
    reg.insert(alchemy_aux(MineralId::XiongHuang, "雄黄"));
    reg.insert(alchemy_aux(MineralId::XiePhen, "邪粉"));

    // 灵石燃料层 — half_life 接 plan §1.4 表（real-day）
    reg.insert(ling_shi(
        MineralId::LingShiFan,
        "凡品灵石",
        LingShiQiRange { min: 5.0, max: 15.0 },
        "ling_shi_fan_v1",
    ));
    reg.insert(ling_shi(
        MineralId::LingShiZhong,
        "中品灵石",
        LingShiQiRange { min: 30.0, max: 60.0 },
        "ling_shi_zhong_v1",
    ));
    reg.insert(ling_shi(
        MineralId::LingShiShang,
        "上品灵石",
        LingShiQiRange { min: 120.0, max: 200.0 },
        "ling_shi_shang_v1",
    ));
    reg.insert(ling_shi(
        MineralId::LingShiYi,
        "遗品灵石",
        LingShiQiRange { min: 500.0, max: 800.0 },
        "ling_shi_yi_v1",
    ));

    reg
}

fn metal(id: MineralId, zh: &'static str) -> MineralEntry {
    MineralEntry {
        id,
        canonical_name: id.as_str(),
        display_name_zh: zh,
        rarity: id.rarity(),
        category: MineralCategory::Metal,
        vanilla_block: id.vanilla_block(),
        forge_tier_min: id.forge_tier_min(),
        ling_shi_qi_range: None,
        decay_profile: None,
    }
}

fn crystal(id: MineralId, zh: &'static str) -> MineralEntry {
    MineralEntry {
        id,
        canonical_name: id.as_str(),
        display_name_zh: zh,
        rarity: id.rarity(),
        category: MineralCategory::Crystal,
        vanilla_block: id.vanilla_block(),
        forge_tier_min: 0,
        ling_shi_qi_range: None,
        decay_profile: None,
    }
}

fn alchemy_aux(id: MineralId, zh: &'static str) -> MineralEntry {
    MineralEntry {
        id,
        canonical_name: id.as_str(),
        display_name_zh: zh,
        rarity: id.rarity(),
        category: MineralCategory::AlchemyAux,
        vanilla_block: id.vanilla_block(),
        forge_tier_min: 0,
        ling_shi_qi_range: None,
        decay_profile: None,
    }
}

fn ling_shi(
    id: MineralId,
    zh: &'static str,
    qi_range: LingShiQiRange,
    profile: &'static str,
) -> MineralEntry {
    MineralEntry {
        id,
        canonical_name: id.as_str(),
        display_name_zh: zh,
        rarity: id.rarity(),
        category: MineralCategory::LingShi,
        vanilla_block: id.vanilla_block(),
        forge_tier_min: 0,
        ling_shi_qi_range: Some(qi_range),
        decay_profile: Some(profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_all_18_minerals() {
        let reg = build_default_registry();
        assert_eq!(reg.len(), MineralId::ALL.len());
        for &id in MineralId::ALL {
            assert!(reg.get(id).is_some(), "missing entry for {id}");
        }
    }

    #[test]
    fn registry_lookup_by_canonical_str() {
        let reg = build_default_registry();
        let entry = reg.get_by_str("sui_tie").expect("sui_tie should resolve");
        assert_eq!(entry.id, MineralId::SuiTie);
        assert_eq!(entry.display_name_zh, "髓铁");
        assert_eq!(entry.vanilla_block, "ancient_debris");
        assert_eq!(entry.forge_tier_min, 3);
    }

    #[test]
    fn registry_rejects_unknown_id() {
        let reg = build_default_registry();
        assert!(!reg.is_valid_mineral_id("xuan_tie"));
        assert!(!reg.is_valid_mineral_id("yi_beast_bone"));
        assert!(reg.is_valid_mineral_id("fan_tie"));
    }

    #[test]
    fn ling_shi_entries_carry_qi_range_and_decay_profile() {
        let reg = build_default_registry();
        for &id in &[
            MineralId::LingShiFan,
            MineralId::LingShiZhong,
            MineralId::LingShiShang,
            MineralId::LingShiYi,
        ] {
            let entry = reg.get(id).expect("ling_shi entry should exist");
            assert!(entry.ling_shi_qi_range.is_some());
            assert!(entry.decay_profile.is_some());
        }
        let fan = reg.get(MineralId::LingShiFan).unwrap();
        assert_eq!(fan.decay_profile, Some("ling_shi_fan_v1"));
        assert_eq!(fan.ling_shi_qi_range.unwrap().min, 5.0);
    }

    #[test]
    fn non_lingshi_entries_have_no_qi_or_decay() {
        let reg = build_default_registry();
        for &id in &[MineralId::FanTie, MineralId::DanSha, MineralId::LingJing] {
            let entry = reg.get(id).unwrap();
            assert!(entry.ling_shi_qi_range.is_none());
            assert!(entry.decay_profile.is_none());
        }
    }
}
