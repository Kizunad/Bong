//! plan-tsy-loot-v1 §3 — 秘境内死亡分流。
//!
//! 玩家在 TSY 内死亡时：
//! - **秘境所得**（入场后捡到的物品）→ 100% 掉在死亡点
//! - **原带物**（入场 snapshot 里的物品）→ 50% 掉在死亡点（与 §十二 主世界规则一致）
//! - 灵龛重生时玩家身上仅剩"原带物的 50% 保留部分"
//!
//! 本模块只做"分流 + 删 inventory + 产出 outcome"。把 outcome 转换成 DroppedLoot 注册 +
//! spawn 干尸 + 发事件由调用方 (`apply_death_drop_on_revive`) 完成。

use std::collections::HashSet;

use valence::math::DVec3;

use super::{select_drop_instance_ids, DroppedItemRecord, PlayerInventory};
use crate::world::tsy::TsyPresence;

/// 分流结果。`apply_death_drop_on_revive` 据此 spawn DroppedLoot + CorpseEmbalmed。
///
/// **不包含** corpse Entity（spawn 由调用方做，因为需要 Commands）。
#[derive(Debug, Clone, PartialEq)]
pub struct TsyDeathDropOutcome {
    /// 原带物按 50% 掉落的部分（带 source 元信息以便 DroppedLootEntry 引用）。
    pub entry_carry_dropped: Vec<DroppedItemRecord>,
    /// 原带物保留的 50%（重生时随玩家带回）。MVP 仍留在 inventory 里 — 本结构只
    /// 列 instance_id 给观察用，不实际从 inventory 取走。
    pub entry_carry_kept_ids: Vec<u64>,
    /// 秘境所得 100% 掉落。
    pub tsy_acquired_dropped: Vec<DroppedItemRecord>,
    /// 死亡坐标，spawn DroppedLoot / CorpseEmbalmed 都用此点。
    pub corpse_pos: DVec3,
    /// 干尸标记。MVP 总是 true（凡 TSY 内死亡都干尸化）；P3 可能引入"成丝"等
    /// 不同遗骸形态时再分支。
    pub is_embalmed: bool,
}

impl TsyDeathDropOutcome {
    /// 总掉落数（entry 50% + tsy 100%）。
    pub fn total_dropped(&self) -> usize {
        self.entry_carry_dropped.len() + self.tsy_acquired_dropped.len()
    }
}

/// 应用秘境死亡分流：mutate inventory，移除将要掉落的 item，返回 outcome。
///
/// `seed` 控制 50% Roll 的伪随机；调用方通常用 `death_drop_seed(entity, revision)`。
pub fn apply_tsy_death_drop(
    inventory: &mut PlayerInventory,
    presence: &TsyPresence,
    corpse_pos: DVec3,
    seed: u64,
) -> TsyDeathDropOutcome {
    let snapshot: HashSet<u64> = presence.entry_inventory_snapshot.iter().copied().collect();

    // ----- 分类阶段 -----
    // 走一遍 inventory，记下每个 instance 的 (location, is_snapshot)；秘境所得直接掉，
    // 原带物先 collect 出 candidate_ids 喂给 select_drop_instance_ids 做 50% Roll。
    let mut entry_carry_ids: Vec<u64> = Vec::new();
    let mut tsy_acquired_records: Vec<DroppedItemRecord> = Vec::new();

    for container in &inventory.containers {
        for placed in &container.items {
            let instance_id = placed.instance.instance_id;
            if snapshot.contains(&instance_id) {
                entry_carry_ids.push(instance_id);
            } else {
                tsy_acquired_records.push(DroppedItemRecord {
                    container_id: container.id.clone(),
                    row: placed.row,
                    col: placed.col,
                    instance: placed.instance.clone(),
                });
            }
        }
    }
    for (slot, item) in &inventory.equipped {
        let instance_id = item.instance_id;
        if snapshot.contains(&instance_id) {
            entry_carry_ids.push(instance_id);
        } else {
            tsy_acquired_records.push(DroppedItemRecord {
                container_id: slot.clone(),
                row: 0,
                col: 0,
                instance: item.clone(),
            });
        }
    }
    for (slot_idx, slot) in inventory.hotbar.iter().enumerate() {
        if let Some(item) = slot {
            let instance_id = item.instance_id;
            if snapshot.contains(&instance_id) {
                entry_carry_ids.push(instance_id);
            } else {
                tsy_acquired_records.push(DroppedItemRecord {
                    container_id: "hotbar".to_string(),
                    row: 0,
                    col: slot_idx as u8,
                    instance: item.clone(),
                });
            }
        }
    }

    // ----- 50% Roll：原带物 -----
    let entry_drop_count = entry_carry_ids.len() / 2;
    let entry_kept_ids: Vec<u64> = if entry_drop_count == 0 {
        // 原带物 < 2 件 → 不掉。但如果只有 1 件，仍按"少于 2 件不掉"处理（与
        // apply_death_drop_to_inventory 主世界规则一致）。
        entry_carry_ids.clone()
    } else {
        let dropped_set: HashSet<u64> =
            select_drop_instance_ids(entry_carry_ids.clone(), entry_drop_count, seed)
                .into_iter()
                .collect();
        entry_carry_ids
            .iter()
            .copied()
            .filter(|id| !dropped_set.contains(id))
            .collect()
    };
    let entry_dropped_set: HashSet<u64> = entry_carry_ids
        .iter()
        .copied()
        .filter(|id| !entry_kept_ids.contains(id))
        .collect();

    // ----- 物理移除阶段：从 inventory 抽走所有"要掉"的 instance（秘境所得 + 原带 50%）-----
    let tsy_acquired_ids: HashSet<u64> = tsy_acquired_records
        .iter()
        .map(|r| r.instance.instance_id)
        .collect();
    let mut all_dropped_ids = entry_dropped_set.clone();
    all_dropped_ids.extend(tsy_acquired_ids.iter().copied());

    let mut entry_carry_dropped: Vec<DroppedItemRecord> = Vec::new();
    for container in &mut inventory.containers {
        let container_id = container.id.clone();
        let mut kept = Vec::with_capacity(container.items.len());
        for placed in container.items.drain(..) {
            if entry_dropped_set.contains(&placed.instance.instance_id) {
                entry_carry_dropped.push(DroppedItemRecord {
                    container_id: container_id.clone(),
                    row: placed.row,
                    col: placed.col,
                    instance: placed.instance,
                });
            } else if tsy_acquired_ids.contains(&placed.instance.instance_id) {
                // 已经在 tsy_acquired_records 里 collect 过了，这里只移除。
            } else {
                kept.push(placed);
            }
        }
        container.items = kept;
    }

    let equipped_to_remove: Vec<String> = inventory
        .equipped
        .iter()
        .filter_map(|(slot, item)| {
            all_dropped_ids
                .contains(&item.instance_id)
                .then(|| slot.clone())
        })
        .collect();
    for slot in equipped_to_remove {
        if let Some(item) = inventory.equipped.remove(&slot) {
            if entry_dropped_set.contains(&item.instance_id) {
                entry_carry_dropped.push(DroppedItemRecord {
                    container_id: slot,
                    row: 0,
                    col: 0,
                    instance: item,
                });
            }
            // tsy_acquired 在 records 里已经记好。
        }
    }

    for slot_idx in 0..inventory.hotbar.len() {
        let should_remove = inventory.hotbar[slot_idx]
            .as_ref()
            .is_some_and(|item| all_dropped_ids.contains(&item.instance_id));
        if !should_remove {
            continue;
        }
        if let Some(item) = inventory.hotbar[slot_idx].take() {
            if entry_dropped_set.contains(&item.instance_id) {
                entry_carry_dropped.push(DroppedItemRecord {
                    container_id: "hotbar".to_string(),
                    row: 0,
                    col: slot_idx as u8,
                    instance: item,
                });
            }
        }
    }

    if !all_dropped_ids.is_empty() {
        super::bump_revision(inventory);
    }

    TsyDeathDropOutcome {
        entry_carry_dropped,
        entry_carry_kept_ids: entry_kept_ids,
        tsy_acquired_dropped: tsy_acquired_records,
        corpse_pos,
        is_embalmed: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        PlayerInventory,
    };
    use crate::world::dimension::DimensionKind;
    use crate::world::tsy::DimensionAnchor;
    use std::collections::HashMap;

    fn item(id: u64) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: format!("test_item_{id}"),
            display_name: format!("test {id}"),
            grid_w: 1,
            grid_h: 1,
            weight: 0.5,
            rarity: ItemRarity::Common,
            description: "test".into(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
        }
    }

    fn make_inventory(items: Vec<ItemInstance>) -> PlayerInventory {
        let placed = items
            .into_iter()
            .enumerate()
            .map(|(i, instance)| PlacedItemState {
                row: 0,
                col: i as u8,
                instance,
            })
            .collect();
        PlayerInventory {
            revision: InventoryRevision(1),
            containers: vec![ContainerState {
                id: "main_pack".into(),
                name: "main".into(),
                rows: 1,
                cols: 16,
                items: placed,
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 45.0,
        }
    }

    fn presence_with_snapshot(snapshot: Vec<u64>) -> TsyPresence {
        TsyPresence {
            family_id: "tsy_test".into(),
            entered_at_tick: 0,
            entry_inventory_snapshot: snapshot,
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
        }
    }

    #[test]
    fn empty_snapshot_drops_all_acquired() {
        // 入场空 → 所有都是秘境所得 → 100% 掉
        let mut inv = make_inventory(vec![item(1), item(2), item(3)]);
        let presence = presence_with_snapshot(vec![]);
        let outcome = apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 42);
        assert_eq!(outcome.entry_carry_dropped.len(), 0);
        assert_eq!(outcome.tsy_acquired_dropped.len(), 3);
        assert_eq!(outcome.entry_carry_kept_ids.len(), 0);
        assert!(inv.containers[0].items.is_empty(), "全部秘境所得应被移除");
    }

    #[test]
    fn full_snapshot_drops_half_kept_half() {
        // 入场即全部 → 没有秘境所得 → 50% Roll
        let mut inv = make_inventory(vec![item(1), item(2), item(3), item(4)]);
        let presence = presence_with_snapshot(vec![1, 2, 3, 4]);
        let outcome = apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 7);
        assert_eq!(outcome.tsy_acquired_dropped.len(), 0);
        assert_eq!(
            outcome.entry_carry_dropped.len(),
            2,
            "4 件原带 → 50% 掉 = 2 件"
        );
        assert_eq!(outcome.entry_carry_kept_ids.len(), 2);
        assert_eq!(
            inv.containers[0].items.len(),
            2,
            "保留的 2 件还在 inventory"
        );
    }

    #[test]
    fn mixed_inventory_splits_correctly() {
        // 10 件原带 + 5 件秘境所得 → 5 件 entry 掉 + 5 件 acquired 掉
        let mut items: Vec<ItemInstance> = (1..=15).map(item).collect();
        items.shuffle_in_place(); // 模拟乱序
        let mut inv = make_inventory(items);
        let presence = presence_with_snapshot((1..=10).collect());
        let outcome = apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 1234);
        assert_eq!(outcome.entry_carry_dropped.len(), 5);
        assert_eq!(outcome.tsy_acquired_dropped.len(), 5);
        assert_eq!(outcome.entry_carry_kept_ids.len(), 5);
        assert_eq!(outcome.total_dropped(), 10);
        assert_eq!(
            inv.containers[0].items.len(),
            5,
            "保留的 5 件原带还在 inventory"
        );
    }

    #[test]
    fn single_entry_carry_does_not_drop() {
        // 1 件原带 / 2 = 0.5 截断 = 0 → 不掉
        let mut inv = make_inventory(vec![item(1)]);
        let presence = presence_with_snapshot(vec![1]);
        let outcome = apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 42);
        assert_eq!(outcome.entry_carry_dropped.len(), 0);
        assert_eq!(outcome.entry_carry_kept_ids.len(), 1);
        assert_eq!(inv.containers[0].items.len(), 1);
    }

    #[test]
    fn corpse_pos_recorded() {
        let mut inv = make_inventory(vec![item(1), item(2)]);
        let presence = presence_with_snapshot(vec![]);
        let pos = DVec3::new(123.0, 64.5, -45.0);
        let outcome = apply_tsy_death_drop(&mut inv, &presence, pos, 0);
        assert_eq!(outcome.corpse_pos, pos);
        assert!(outcome.is_embalmed);
    }

    #[test]
    fn equipped_and_hotbar_partition_by_snapshot() {
        let mut inv = make_inventory(vec![item(1)]);
        inv.equipped.insert("main_hand".into(), item(2));
        inv.hotbar[0] = Some(item(3));
        // snapshot 只含 1 → 2、3 都是秘境所得 → 100% 掉
        let presence = presence_with_snapshot(vec![1]);
        let outcome = apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 0);
        assert_eq!(outcome.tsy_acquired_dropped.len(), 2);
        // 1 件原带 → 50%/2 = 0 → 不掉
        assert_eq!(outcome.entry_carry_dropped.len(), 0);
        assert!(inv.equipped.is_empty(), "equipped 秘境所得应清空");
        assert!(inv.hotbar[0].is_none(), "hotbar 秘境所得应清空");
        assert_eq!(inv.containers[0].items.len(), 1);
    }

    #[test]
    fn revision_bumps_when_anything_dropped() {
        let mut inv = make_inventory(vec![item(1), item(2)]);
        let original = inv.revision.0;
        let presence = presence_with_snapshot(vec![]);
        apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 0);
        assert!(inv.revision.0 > original, "revision 应在掉落后 bump");
    }

    #[test]
    fn revision_unchanged_when_nothing_dropped() {
        // 1 件原带 → 50%=0 不掉，没有秘境所得 → 不应 bump revision
        let mut inv = make_inventory(vec![item(1)]);
        let original = inv.revision.0;
        let presence = presence_with_snapshot(vec![1]);
        apply_tsy_death_drop(&mut inv, &presence, DVec3::ZERO, 0);
        assert_eq!(inv.revision.0, original);
    }

    /// 测试用 trait：让 Vec<ItemInstance> shuffle_in_place 一下，避免引入 rand。
    trait ShuffleInPlace {
        fn shuffle_in_place(&mut self);
    }
    impl ShuffleInPlace for Vec<ItemInstance> {
        fn shuffle_in_place(&mut self) {
            // 简单确定性 shuffle，足够把"按 instance_id 排序"打散。
            for i in (1..self.len()).rev() {
                let swap = (i * 31 + 7) % (i + 1);
                self.swap(i, swap);
            }
        }
    }
}
