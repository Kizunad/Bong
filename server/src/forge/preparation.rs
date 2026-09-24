//! 锻造只消费已暂存的实例；通用材料区承担返还、存档、负重与死亡掉落。

use std::collections::HashMap;

use super::blueprint::{Blueprint, StepSpec};
use crate::craft::preparation::stage_instance;
use crate::inventory::{bump_revision, ForgeMaterialDeficit, ItemInstance, PlayerInventory};

pub fn material_key(item: &ItemInstance) -> &str {
    item.mineral_id.as_deref().unwrap_or(&item.template_id)
}

pub fn stage(
    inventory: &mut PlayerInventory,
    blueprint: &Blueprint,
    station: (i32, i32, i32),
    instance: u64,
) -> Result<(), String> {
    stage_instance(
        inventory,
        &blueprint.id,
        Some(station),
        instance,
        |item, _| {
            let key = material_key(item);
            let recognized = blueprint.steps.iter().any(|step| match step {
                StepSpec::Billet { profile } => {
                    profile.required.iter().any(|value| value.material == key)
                        || profile
                            .optional_carriers
                            .iter()
                            .any(|value| value.material == key)
                }
                _ => false,
            });
            if item.mineral_id.is_none() && !recognized {
                return Err("此物品不能作为锻造投料".into());
            }
            Ok(())
        },
    )
}

pub fn consume(
    inventory: &mut PlayerInventory,
    needed: &HashMap<String, u32>,
) -> Result<(), Vec<ForgeMaterialDeficit>> {
    let mut deficits = Vec::new();
    for (material, need) in needed {
        let have = inventory
            .material_preparation
            .materials
            .iter()
            .filter(|entry| material_key(&entry.item) == material)
            .map(|entry| entry.item.stack_count)
            .sum();
        if have < *need {
            deficits.push(ForgeMaterialDeficit {
                material: material.clone(),
                have,
                need: *need,
            });
        }
    }
    if !deficits.is_empty() {
        return Err(deficits);
    }
    for (material, need) in needed {
        let mut remaining = *need;
        for entry in &mut inventory.material_preparation.materials {
            if material_key(&entry.item) == material {
                let consumed = entry.item.stack_count.min(remaining);
                entry.item.stack_count -= consumed;
                remaining -= consumed;
            }
        }
    }
    inventory
        .material_preparation
        .materials
        .retain(|entry| entry.item.stack_count > 0);
    inventory.material_preparation.clear_empty_recipe();
    bump_revision(inventory);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::preparation::{return_materials, stage_instance};
    use crate::inventory::{ContainerState, InventoryRevision, ItemRegistry, PlacedItemState};
    use crate::world::dimension::DimensionKind;

    fn inventory() -> PlayerInventory {
        let item = ItemInstance {
            instance_id: 11,
            template_id: "fan_tie".into(),
            display_name: "凡铁".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 1.0,
            rarity: crate::inventory::ItemRarity::Common,
            description: String::new(),
            stack_count: 5,
            spirit_quality: 1.0,
            durability: 0.7,
            freshness: None,
            mineral_id: Some("fan_tie".into()),
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        };
        let mut inventory = PlayerInventory {
            revision: InventoryRevision(1),
            material_preparation: Default::default(),
            containers: vec![ContainerState {
                id: "pack".into(),
                name: "背包".into(),
                rows: 5,
                cols: 7,
                items: vec![PlacedItemState {
                    row: 1,
                    col: 2,
                    instance: item,
                }],
                quick_access: false,
                owner_instance_id: None,
            }],
            equipped: Default::default(),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
            triggered_treasures: Vec::new(),
        };
        inventory.containers[0].items[0].instance.stack_count = 5;
        inventory
    }

    #[test]
    fn custody_roundtrip_preserves_instance_and_prevents_cross_workstation_use() {
        let mut inventory = inventory();
        let original = inventory.containers[0].items[0].clone();
        stage_instance(&mut inventory, "iron", Some((1, 64, 0)), 11, |_, _| Ok(())).unwrap();
        assert!(inventory.containers[0].items.is_empty());
        assert_eq!(
            inventory.material_preparation.count("iron", "fan_tie"),
            0,
            "制作不能消费锻造准备材料，即使配方 ID 一样"
        );
        inventory = serde_json::from_str(&serde_json::to_string(&inventory).unwrap()).unwrap();
        for station in [None, Some((2, 64, 0))] {
            assert!(stage_instance(&mut inventory, "iron", station, 11, |_, _| Ok(())).is_err());
        }
        let drops = return_materials(
            &mut inventory,
            &ItemRegistry::default(),
            None,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        )
        .unwrap();
        assert!(drops.is_empty());
        assert_eq!(
            inventory.containers[0].items,
            vec![original],
            "取回完整实例及原格子，不重造物品"
        );
    }

    #[test]
    fn shortage_is_atomic_and_unstaged_inventory_cannot_cover_it() {
        let mut inventory = inventory();
        assert!(consume(&mut inventory, &HashMap::from([("fan_tie".into(), 3)])).is_err());
        stage_instance(&mut inventory, "iron", Some((1, 64, 0)), 11, |_, _| Ok(())).unwrap();
        let before = inventory.material_preparation.clone();
        assert!(consume(
            &mut inventory,
            &HashMap::from([("fan_tie".into(), 3), ("za_gang".into(), 1)])
        )
        .is_err());
        assert_eq!(
            inventory.material_preparation, before,
            "一种料不足时足量材料也不能被扣除"
        );
        consume(&mut inventory, &HashMap::from([("fan_tie".into(), 3)])).unwrap();
        return_materials(
            &mut inventory,
            &ItemRegistry::default(),
            None,
            [0.0, 64.0, 0.0],
            DimensionKind::Overworld,
        )
        .unwrap();
        assert_eq!(
            inventory.containers[0].items[0].instance.stack_count, 2,
            "只能取回余料，不能撤回已开炉的投料"
        );
    }
}
