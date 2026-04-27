//! plan-tsy-loot-v1 §1.2 — 上古遗物模板表。
//!
//! 定义"99/1 比例铁律"中的那 1%：捡到即用、不绑定、不激活，
//! 唯一代价是耐久极低 —— 用 `ItemInstance.charges` 的整数计数表达
//! （而非滥用 0..=1 的 `durability` 字段，避免触碰 `InventoryItemViewV1`
//! schema 边界，详见 Codex review 反馈）。
//!
//! Strength tier → charges：
//! - tier 1 = 1 次（一次性消耗：残卷 / 兽核）
//! - tier 2 = 3 次（三击 / 三次激活：轻量法宝）
//! - tier 3 = 5 次（五击 / 五次激活：重量法宝）
//!
//! `durability` 字段对 ancient 物品恒为 1.0（"全新但易碎"），由 `charges`
//! 单独跟踪剩余次数；归零时由消费系统从 inventory 移除。
//!
//! Spawn 入口在 `tsy_loot_spawn.rs`，本模块只负责数据 + `to_item_instance` 工厂。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Resource};

use super::{InventoryInstanceIdAllocator, ItemInstance, ItemRarity};

/// 上古遗物形态档次。决定客户端 tooltip 表现 + 后续 P3 商人系统能否交易。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AncientRelicKind {
    /// 法宝 / 剑 / 钩
    Weapon,
    /// 残卷（功法 / 丹方 / 阵图）
    Scroll,
    /// 异兽核（突破助力）
    BeastCore,
    /// 佩物（弟子遗物）
    Pendant,
}

/// 上古遗物来源类（来自 `worldview §十六.一` TSY 生命周期表）。
/// `tsy_loot_spawn.rs` 按 source_class 决定每座 TSY spawn 的总数与种类倾向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AncientRelicSource {
    /// 大能陨落类
    DaoLord,
    /// 宗门遗迹类
    SectRuins,
    /// 战场沉淀类
    BattleSediment,
}

/// 单条上古遗物模板。`AncientRelicPool::sample` 按 source 加权选取。
#[derive(Debug, Clone, PartialEq)]
pub struct AncientRelicTemplate {
    /// 模板 id，对应 `ItemInstance.template_id`（如 `"ancient_relic_sword_wuxiang"`）。
    pub template_id: String,
    pub display_name: String,
    pub kind: AncientRelicKind,
    pub source_class: AncientRelicSource,
    /// 1 / 2 / 3，决定 `to_item_instance` 写入的 durability。
    pub strength_tier: u8,
    pub description: String,
    /// 占用网格（与 `ItemInstance.grid_w/grid_h` 对齐）。
    pub grid_w: u8,
    pub grid_h: u8,
    pub weight: f64,
}

impl AncientRelicTemplate {
    /// 把模板实例化为 `ItemInstance`。charges 按 tier 映射为剩余使用次数：
    /// `1 / 3 / 5`。每次使用由对应系统 `-= 1`，归零销毁。
    /// `durability` 恒为 1.0（"全新但易碎"），保持在 schema `0..=1` 边界内。
    pub fn to_item_instance(
        &self,
        allocator: &mut InventoryInstanceIdAllocator,
    ) -> Result<ItemInstance, String> {
        let charges = strength_tier_to_charges(self.strength_tier);
        Ok(ItemInstance {
            instance_id: allocator.next_id()?,
            template_id: self.template_id.clone(),
            display_name: self.display_name.clone(),
            grid_w: self.grid_w,
            grid_h: self.grid_h,
            weight: self.weight,
            rarity: ItemRarity::Ancient,
            description: self.description.clone(),
            stack_count: 1,
            // §0 设计轴心 #2：上古遗物本身"无灵"，spirit_quality 恒为 0。
            spirit_quality: 0.0,
            // schema 约束 0..=1 — ancient 不参与磨损系统，恒满。
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: Some(charges),
        })
    }
}

/// strength_tier → 初始剩余次数（`ItemInstance.charges`）。位于模块根方便 testing。
pub fn strength_tier_to_charges(tier: u8) -> u32 {
    match tier {
        1 => 1,
        2 => 3,
        3 => 5,
        _ => 1,
    }
}

/// 上古遗物模板池（资源）。`tsy_loot_spawn.rs` 在 zone 激活时按 source 抽取。
#[derive(Debug, Default, Resource)]
pub struct AncientRelicPool {
    templates: Vec<AncientRelicTemplate>,
}

impl AncientRelicPool {
    pub fn from_seed() -> Self {
        Self {
            templates: seed_ancient_relics(),
        }
    }

    #[allow(dead_code)]
    pub fn templates(&self) -> &[AncientRelicTemplate] {
        &self.templates
    }

    /// 按 source 抽取候选。回退：source 无命中时退回任意一条（保证 spawn 不失败）。
    /// `seed` 用于确定性选取，方便 save/load 一致 + 单测可控。
    pub fn sample(&self, source: AncientRelicSource, seed: u64) -> Option<&AncientRelicTemplate> {
        if self.templates.is_empty() {
            return None;
        }
        let by_source: Vec<&AncientRelicTemplate> = self
            .templates
            .iter()
            .filter(|t| t.source_class == source)
            .collect();
        let pool: &[&AncientRelicTemplate] = if by_source.is_empty() {
            // fallback：源不匹配 → 用全池；但仍要返回 &T，所以走另一条路径。
            return self.templates.first();
        } else {
            &by_source
        };
        let idx = (seed as usize) % pool.len();
        Some(pool[idx])
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }
}

/// MVP 种子表。每个 `AncientRelicSource` 至少 2 件（§8.1 unit test）。
pub fn seed_ancient_relics() -> Vec<AncientRelicTemplate> {
    vec![
        // —— DaoLord —— 大能陨落
        AncientRelicTemplate {
            template_id: "ancient_relic_sword_wuxiang".into(),
            display_name: "无相剑残骸".into(),
            kind: AncientRelicKind::Weapon,
            source_class: AncientRelicSource::DaoLord,
            strength_tier: 3,
            description: "上古剑修无相真人的佩剑残骸。无灵但仍有锋。五击即碎。".into(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.5,
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_pendant_yuanfeng".into(),
            display_name: "渊锋玉佩".into(),
            kind: AncientRelicKind::Pendant,
            source_class: AncientRelicSource::DaoLord,
            strength_tier: 2,
            description: "陨落道君随身玉佩，仍残留三息护体禁制。三次激活即散。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.4,
        },
        // —— SectRuins —— 宗门遗迹
        AncientRelicTemplate {
            template_id: "ancient_relic_scroll_kaimai".into(),
            display_name: "《开脉残卷》".into(),
            kind: AncientRelicKind::Scroll,
            source_class: AncientRelicSource::SectRuins,
            strength_tier: 1,
            description: "失落宗门灵墟的开脉功法残页。一次性消耗。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_scroll_danfang".into(),
            display_name: "《九转金丹方残页》".into(),
            kind: AncientRelicKind::Scroll,
            source_class: AncientRelicSource::SectRuins,
            strength_tier: 1,
            description: "上古丹方残卷，可教导一次九转金丹的炼制要诀。看后即焚。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.2,
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_weapon_zongmen_blade".into(),
            display_name: "宗门戒刀".into(),
            kind: AncientRelicKind::Weapon,
            source_class: AncientRelicSource::SectRuins,
            strength_tier: 2,
            description: "宗门弟子佩刀的残骸，尚可三斩。".into(),
            grid_w: 1,
            grid_h: 2,
            weight: 1.2,
        },
        // —— BattleSediment —— 战场沉淀
        AncientRelicTemplate {
            template_id: "ancient_relic_core_yibian".into(),
            display_name: "异变兽核（干涸）".into(),
            kind: AncientRelicKind::BeastCore,
            source_class: AncientRelicSource::BattleSediment,
            strength_tier: 1,
            description: "通灵境突破所需的异兽核心，已干涸但仍可一次性引用。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_core_yaoxue".into(),
            display_name: "妖血凝晶".into(),
            kind: AncientRelicKind::BeastCore,
            source_class: AncientRelicSource::BattleSediment,
            strength_tier: 1,
            description: "战场凝结的妖血晶体，可作突破催化剂一次。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.3,
        },
        AncientRelicTemplate {
            template_id: "ancient_relic_pendant_warring_sigil".into(),
            display_name: "战阵符印".into(),
            kind: AncientRelicKind::Pendant,
            source_class: AncientRelicSource::BattleSediment,
            strength_tier: 2,
            description: "古战场遗留的护身符印，三次激活后符纹溃散。".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.3,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_pool_size_at_least_eight() {
        // §8.1：≥10 件偏理想；MVP 8 件已足以覆盖 source class 分布。
        assert!(seed_ancient_relics().len() >= 8, "MVP 至少 8 件遗物模板");
    }

    #[test]
    fn each_source_class_has_at_least_two_templates() {
        let pool = seed_ancient_relics();
        for src in [
            AncientRelicSource::DaoLord,
            AncientRelicSource::SectRuins,
            AncientRelicSource::BattleSediment,
        ] {
            let count = pool.iter().filter(|t| t.source_class == src).count();
            assert!(count >= 2, "source {src:?} 至少 2 件模板，实际 {count}");
        }
    }

    #[test]
    fn strength_tier_charges_mapping() {
        assert_eq!(strength_tier_to_charges(1), 1);
        assert_eq!(strength_tier_to_charges(2), 3);
        assert_eq!(strength_tier_to_charges(3), 5);
        // 兜底：未知 tier 视作一次性。
        assert_eq!(strength_tier_to_charges(0), 1);
        assert_eq!(strength_tier_to_charges(99), 1);
    }

    #[test]
    fn to_item_instance_writes_ancient_rarity_durability_and_charges() {
        let mut allocator = InventoryInstanceIdAllocator::default();
        let template = &seed_ancient_relics()[0];
        let item = template.to_item_instance(&mut allocator).expect("alloc");
        assert_eq!(item.rarity, ItemRarity::Ancient);
        assert_eq!(item.spirit_quality, 0.0, "ancient relic 必须无灵");
        assert_eq!(item.template_id, template.template_id);
        assert_eq!(
            item.durability, 1.0,
            "ancient durability 恒为 1.0（schema 0..=1 边界）"
        );
        assert_eq!(
            item.charges,
            Some(strength_tier_to_charges(template.strength_tier)),
            "charges 跟 strength_tier 1/3/5 映射"
        );
        assert_eq!(item.stack_count, 1);
        assert!(item.freshness.is_none());
        assert!(item.mineral_id.is_none());
    }

    #[test]
    fn pool_sample_filters_by_source() {
        let pool = AncientRelicPool::from_seed();
        for seed in 0..32u64 {
            let picked = pool
                .sample(AncientRelicSource::DaoLord, seed)
                .expect("DaoLord 池有内容");
            assert_eq!(picked.source_class, AncientRelicSource::DaoLord);
        }
    }

    #[test]
    fn pool_sample_deterministic_for_same_seed() {
        let pool = AncientRelicPool::from_seed();
        let a = pool.sample(AncientRelicSource::SectRuins, 7).unwrap();
        let b = pool.sample(AncientRelicSource::SectRuins, 7).unwrap();
        assert_eq!(a.template_id, b.template_id);
    }

    #[test]
    fn empty_pool_sample_returns_none() {
        let pool = AncientRelicPool::default();
        assert!(pool.sample(AncientRelicSource::DaoLord, 0).is_none());
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }
}
