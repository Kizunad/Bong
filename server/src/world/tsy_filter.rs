//! plan-tsy-zone-v1 §4 — TSY 入场过滤器。
//!
//! 玩家踏进 Entry 裂缝瞬间，inventory 内 `spirit_quality >= ENTRY_FILTER_THRESHOLD`
//! 的物品被负压剥离真元：`spirit_quality = 0` + 改名（"xxx（失灵）" 等）+ 清掉
//! `freshness`（保质期不再适用）。其他属性（grid 尺寸 / 重量 / 稀有度）保留。

use crate::inventory::{ItemInstance, PlayerInventory};

/// 灵质阈值 — 高于此值的物品被入场过滤剥离。
pub const ENTRY_FILTER_THRESHOLD: f64 = 0.3;

/// 单个被过滤物品的描述（emit 给 TsyEnterEvent）。
#[derive(Debug, Clone, PartialEq)]
pub struct FilteredItem {
    pub instance_id: u64,
    pub template_id: String,
    pub before_name: String,
    pub before_spirit_quality: f64,
}

/// 扫描整个 inventory（containers 内所有 PlacedItemState + equipped HashMap +
/// hotbar 9 槽），将 spirit_quality >= 阈值的物品**就地剥离**。返回被过滤
/// 物品的快照（caller 用于 emit `TsyEnterEvent.filtered_items`）。
pub fn apply_entry_filter(inv: &mut PlayerInventory) -> Vec<FilteredItem> {
    let mut filtered = Vec::new();

    // containers: Vec<ContainerState> { items: Vec<PlacedItemState { instance: ItemInstance }> }
    for container in inv.containers.iter_mut() {
        for placed in container.items.iter_mut() {
            try_strip(&mut placed.instance, &mut filtered);
        }
    }

    // equipped: HashMap<String, ItemInstance>
    for item in inv.equipped.values_mut() {
        try_strip(item, &mut filtered);
    }

    // hotbar: [Option<ItemInstance>; 9]
    for slot in inv.hotbar.iter_mut() {
        if let Some(item) = slot.as_mut() {
            try_strip(item, &mut filtered);
        }
    }

    filtered
}

fn try_strip(item: &mut ItemInstance, log: &mut Vec<FilteredItem>) {
    if item.spirit_quality < ENTRY_FILTER_THRESHOLD {
        return;
    }
    log.push(FilteredItem {
        instance_id: item.instance_id,
        template_id: item.template_id.clone(),
        before_name: item.display_name.clone(),
        before_spirit_quality: item.spirit_quality,
    });
    apply_spirit_strip(item);
}

fn apply_spirit_strip(item: &mut ItemInstance) {
    item.spirit_quality = 0.0;
    item.display_name = strip_name(&item.template_id, &item.display_name);
    // freshness = None 表示该物品已"死"——失去时间敏感性，不再走 shelflife decay。
    // 比 plan 文档原文 `*freshness = Freshness::Withered` 更合代码现状（Freshness
    // 是 struct 不是带 Withered 变体的 enum；冻结 / 重置内部 tick 等价语义）。
    item.freshness = None;
}

/// plan §4.2 改名表 — 按 template_id 前缀分流。
///
/// **注意**：改名后字符串带有"（失灵）/（无灵）/（枯）"等后缀显式标记被剥离。
/// 后续 plan-tsy-loot-v1 死亡结算可通过 `template_id` 仍然识别物品类型。
pub fn strip_name(template_id: &str, original: &str) -> String {
    if template_id == "bone_coin" {
        return "枯骨残片".to_string();
    }
    if template_id.starts_with("spirit_herb_") {
        return format!("{original}（枯）");
    }
    if template_id.starts_with("weapon_") {
        return format!("{original}（失灵）");
    }
    format!("{original}（无灵）")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ContainerState, InventoryRevision, ItemRarity, PlacedItemState};
    use std::collections::HashMap;

    fn item(
        instance_id: u64,
        template_id: &str,
        display_name: &str,
        spirit_quality: f64,
    ) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: display_name.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
        }
    }

    fn empty_inv() -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: Vec::new(),
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 100.0,
        }
    }

    #[test]
    fn empty_inventory_returns_no_filtered_items() {
        let mut inv = empty_inv();
        let filtered = apply_entry_filter(&mut inv);
        assert!(filtered.is_empty());
    }

    #[test]
    fn low_spirit_quality_items_pass_through_untouched() {
        let mut inv = empty_inv();
        inv.containers.push(ContainerState {
            id: "bag".to_string(),
            name: "Bag".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(1, "bone_coin", "骨币", 0.1),
            }],
        });
        let filtered = apply_entry_filter(&mut inv);
        assert!(filtered.is_empty(), "0.1 < 0.3 should not be filtered");
        let still_there = &inv.containers[0].items[0].instance;
        assert_eq!(still_there.spirit_quality, 0.1);
        assert_eq!(still_there.display_name, "骨币");
    }

    #[test]
    fn bone_coin_with_high_spirit_quality_becomes_dry_bone() {
        let mut inv = empty_inv();
        inv.containers.push(ContainerState {
            id: "bag".to_string(),
            name: "Bag".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(7, "bone_coin", "满灵骨币", 0.8),
            }],
        });
        let filtered = apply_entry_filter(&mut inv);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].instance_id, 7);
        assert_eq!(filtered[0].before_spirit_quality, 0.8);
        let stripped = &inv.containers[0].items[0].instance;
        assert_eq!(stripped.spirit_quality, 0.0);
        assert_eq!(stripped.display_name, "枯骨残片");
        assert_eq!(stripped.template_id, "bone_coin", "template_id 必须保留");
    }

    #[test]
    fn equipped_weapon_loses_spirit_and_gets_failed_suffix() {
        let mut inv = empty_inv();
        inv.equipped.insert(
            "main_hand".to_string(),
            item(11, "weapon_jade_sword", "玉灵剑", 0.5),
        );
        let filtered = apply_entry_filter(&mut inv);
        assert_eq!(filtered.len(), 1);
        let stripped = inv.equipped.get("main_hand").unwrap();
        assert_eq!(stripped.spirit_quality, 0.0);
        assert_eq!(stripped.display_name, "玉灵剑（失灵）");
    }

    #[test]
    fn hotbar_slot_with_high_quality_gets_stripped() {
        let mut inv = empty_inv();
        inv.hotbar[3] = Some(item(22, "spirit_herb_lingcao", "鲜采灵草", 0.6));
        let filtered = apply_entry_filter(&mut inv);
        assert_eq!(filtered.len(), 1);
        let stripped = inv.hotbar[3].as_ref().unwrap();
        assert_eq!(stripped.spirit_quality, 0.0);
        assert_eq!(stripped.display_name, "鲜采灵草（枯）");
    }

    #[test]
    fn unknown_template_falls_through_to_wuling_suffix() {
        let mut inv = empty_inv();
        inv.containers.push(ContainerState {
            id: "bag".to_string(),
            name: "Bag".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(99, "mystery_artifact", "无名之物", 0.4),
            }],
        });
        let filtered = apply_entry_filter(&mut inv);
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            inv.containers[0].items[0].instance.display_name,
            "无名之物（无灵）"
        );
    }

    #[test]
    fn at_threshold_exactly_is_filtered_inclusive() {
        // ENTRY_FILTER_THRESHOLD = 0.3 严格 >=，所以 0.3 必须被过滤。
        let mut inv = empty_inv();
        inv.containers.push(ContainerState {
            id: "bag".to_string(),
            name: "Bag".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item(33, "weapon_iron", "铁剑", 0.3),
            }],
        });
        let filtered = apply_entry_filter(&mut inv);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].before_spirit_quality, 0.3);
    }

    #[test]
    fn freshness_is_cleared_when_item_is_stripped() {
        // 鲜灵草带 freshness 字段，过滤后 freshness = None。
        let mut inv = empty_inv();
        let mut herb = item(44, "spirit_herb_lingcao", "鲜灵草", 0.6);
        // 用 mock freshness（在 worldgen 现网走 DecayProfile 注入）。本测只关心 .freshness.is_none()。
        herb.freshness = Some(crate::shelflife::Freshness {
            created_at_tick: 0,
            initial_qi: 1.0,
            track: crate::shelflife::DecayTrack::Decay,
            profile: crate::shelflife::DecayProfileId::new("test_profile"),
            frozen_accumulated: 0,
            frozen_since_tick: None,
        });
        inv.containers.push(ContainerState {
            id: "bag".to_string(),
            name: "Bag".to_string(),
            rows: 4,
            cols: 4,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: herb,
            }],
        });
        apply_entry_filter(&mut inv);
        assert!(inv.containers[0].items[0].instance.freshness.is_none());
    }
}
