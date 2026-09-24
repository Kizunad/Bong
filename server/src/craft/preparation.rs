//! 制作与锻造共用的材料托管。保留实例与原位置，开工前可原样返还。

use serde::{Deserialize, Serialize};

use crate::inventory::{
    add_existing_item_to_player_inventory, attach_at_location, bump_revision, validate_attach_fits,
    DroppedLootEntry, ItemInstance, ItemRegistry, PlayerInventory,
};
use crate::schema::inventory::InventoryLocationV1;
use crate::world::dimension::DimensionKind;

use super::{CraftRecipe, MAX_CRAFT_QUANTITY};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MaterialPreparation {
    pub recipe_id: Option<String>,
    #[serde(default)]
    pub station_pos: Option<(i32, i32, i32)>,
    pub materials: Vec<PreparedMaterial>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedMaterial {
    pub item: ItemInstance,
    pub origin: InventoryLocationV1,
}

impl MaterialPreparation {
    pub fn count(&self, recipe_id: &str, template_id: &str) -> u32 {
        if self.station_pos.is_some() || self.recipe_id.as_deref() != Some(recipe_id) {
            return 0;
        }
        self.materials
            .iter()
            .filter(|entry| entry.item.template_id == template_id)
            .map(|entry| entry.item.stack_count)
            .sum()
    }

    /// 调用方已验证配方和数量足够；不搜索背包补料。
    pub fn consume(&mut self, template_id: &str, mut count: u32) {
        for entry in &mut self.materials {
            if entry.item.template_id == template_id {
                let consumed = entry.item.stack_count.min(count);
                entry.item.stack_count -= consumed;
                count -= consumed;
            }
        }
        assert_eq!(count, 0, "制作只能消耗已校验充足的暂存材料");
        self.materials.retain(|entry| entry.item.stack_count > 0);
        self.clear_empty_recipe();
    }

    pub fn clear_empty_recipe(&mut self) {
        if self.materials.is_empty() {
            self.recipe_id = None;
            self.station_pos = None;
        }
    }
}

/// 整叠移入，不改实例属性；限制为当前配方最多一批所需的材料。
pub fn stage_material(
    inventory: &mut PlayerInventory,
    recipe: &CraftRecipe,
    instance_id: u64,
) -> Result<(), String> {
    stage_instance(
        inventory,
        recipe.id.as_str(),
        None,
        instance_id,
        |item, preparation| {
            let required = recipe
                .materials
                .iter()
                .filter(|(template, _)| template == &item.template_id)
                .map(|(_, count)| *count)
                .sum::<u32>();
            let staged = preparation.count(recipe.id.as_str(), &item.template_id);
            if required == 0
                || staged.saturating_add(item.stack_count)
                    > required.saturating_mul(MAX_CRAFT_QUANTITY)
            {
                return Err("材料不属于此配方或已超过一批制作所需数量".into());
            }
            Ok(())
        },
    )
}

/// 搬运完整实例；领域调用方负责材料资格，公共路径负责库存与来源一致性。
pub fn stage_instance(
    inventory: &mut PlayerInventory,
    recipe_id: &str,
    station_pos: Option<(i32, i32, i32)>,
    instance_id: u64,
    validate: impl FnOnce(&ItemInstance, &MaterialPreparation) -> Result<(), String>,
) -> Result<(), String> {
    if inventory
        .material_preparation
        .recipe_id
        .as_deref()
        .is_some_and(|id| {
            id != recipe_id || inventory.material_preparation.station_pos != station_pos
        })
    {
        return Err("请先取回上一配方的材料".into());
    }
    let source = inventory
        .containers
        .iter()
        .find_map(|container| {
            container
                .items
                .iter()
                .find(|entry| entry.instance.instance_id == instance_id)
                .map(|entry| PreparedMaterial {
                    item: entry.instance.clone(),
                    origin: InventoryLocationV1::Container {
                        container_id: container.id.clone(),
                        row: entry.row.into(),
                        col: entry.col.into(),
                    },
                })
        })
        .or_else(|| {
            inventory
                .hotbar
                .iter()
                .enumerate()
                .find_map(|(index, slot)| {
                    slot.as_ref()
                        .filter(|item| item.instance_id == instance_id)
                        .map(|item| PreparedMaterial {
                            item: item.clone(),
                            origin: InventoryLocationV1::Hotbar { index: index as u8 },
                        })
                })
        })
        .ok_or("材料已移动或不在背包中")?;

    validate(&source.item, &inventory.material_preparation)?;
    for container in &mut inventory.containers {
        container
            .items
            .retain(|entry| entry.instance.instance_id != instance_id);
    }
    for slot in &mut inventory.hotbar {
        if slot
            .as_ref()
            .is_some_and(|item| item.instance_id == instance_id)
        {
            *slot = None;
        }
    }
    inventory.material_preparation.recipe_id = Some(recipe_id.to_owned());
    inventory.material_preparation.station_pos = station_pos;
    inventory.material_preparation.materials.push(source);
    bump_revision(inventory);
    Ok(())
}

/// 原位置、其他背包空位、原实例落地。返回的掉落必须与库存一起持久化再发布。
pub fn return_materials(
    inventory: &mut PlayerInventory,
    registry: &ItemRegistry,
    instance_id: Option<u64>,
    world_pos: [f64; 3],
    dimension: DimensionKind,
) -> Result<Vec<DroppedLootEntry>, String> {
    if instance_id.is_some_and(|id| {
        !inventory
            .material_preparation
            .materials
            .iter()
            .any(|entry| entry.item.instance_id == id)
    }) {
        return Err("材料已取回".into());
    }
    let mut drops = Vec::new();
    let materials = std::mem::take(&mut inventory.material_preparation.materials);
    for entry in materials {
        if instance_id.is_some_and(|id| id != entry.item.instance_id) {
            inventory.material_preparation.materials.push(entry);
            continue;
        }
        if validate_attach_fits(inventory, &entry.item, &entry.origin).is_ok() {
            attach_at_location(inventory, entry.item, &entry.origin)
                .expect("已校验原位置可容纳材料");
        } else if add_existing_item_to_player_inventory(inventory, registry, entry.item.clone())
            .is_err()
        {
            drops.push(DroppedLootEntry {
                instance_id: entry.item.instance_id,
                source_container_id: "material_preparation".into(),
                source_row: 0,
                source_col: 0,
                world_pos,
                dimension,
                item: entry.item,
            });
        }
        bump_revision(inventory);
    }
    inventory.material_preparation.clear_empty_recipe();
    Ok(drops)
}
