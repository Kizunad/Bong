use valence::prelude::{Entity, EventWriter, World};

use super::ToolKind;
use crate::inventory::{
    inventory_item_by_instance_borrow, set_item_instance_durability,
    InventoryDurabilityChangedEvent, InventoryDurabilityUpdate, PlayerInventory,
    EQUIP_SLOT_MAIN_HAND,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolDurabilityUseOutcome {
    pub kind: ToolKind,
    pub instance_id: u64,
    pub update: InventoryDurabilityUpdate,
}

pub fn item_kind_to_tool(item_id: &str) -> Option<ToolKind> {
    match item_id {
        "cai_yao_dao" => Some(ToolKind::CaiYaoDao),
        "bao_chu" => Some(ToolKind::BaoChu),
        "cao_lian" => Some(ToolKind::CaoLian),
        "dun_qi_jia" => Some(ToolKind::DunQiJia),
        "gua_dao" => Some(ToolKind::GuaDao),
        "gu_hai_qian" => Some(ToolKind::GuHaiQian),
        "bing_jia_shou_tao" => Some(ToolKind::BingJiaShouTao),
        _ => None,
    }
}

pub fn main_hand_tool(player: Entity, world: &World) -> Option<ToolKind> {
    let inventory = world.get::<PlayerInventory>(player)?;
    main_hand_tool_in_inventory(inventory)
}

pub fn main_hand_tool_in_inventory(inventory: &PlayerInventory) -> Option<ToolKind> {
    let item = inventory.equipped.get(EQUIP_SLOT_MAIN_HAND)?;
    if item.durability <= 0.0 {
        return None;
    }
    item_kind_to_tool(item.template_id.as_str())
}

pub fn main_hand_tool_instance_in_inventory(
    inventory: &PlayerInventory,
) -> Option<(ToolKind, u64, f64)> {
    let item = inventory.equipped.get(EQUIP_SLOT_MAIN_HAND)?;
    let kind = item_kind_to_tool(item.template_id.as_str())?;
    (item.durability > 0.0).then_some((kind, item.instance_id, item.durability))
}

pub fn damage_main_hand_tool(
    entity: Entity,
    inventory: &mut PlayerInventory,
    durability_events: &mut EventWriter<InventoryDurabilityChangedEvent>,
    cost_ratio: f64,
) -> Option<ToolDurabilityUseOutcome> {
    if !cost_ratio.is_finite() || cost_ratio <= 0.0 {
        return None;
    }
    let (kind, instance_id, current) = main_hand_tool_instance_in_inventory(inventory)?;
    let next = (current - cost_ratio).clamp(0.0, 1.0);
    if next >= current {
        return None;
    }
    match set_item_instance_durability(inventory, instance_id, next) {
        Ok(update) => {
            durability_events.send(InventoryDurabilityChangedEvent {
                entity,
                revision: update.revision,
                instance_id: update.instance_id,
                durability: update.durability,
            });
            Some(ToolDurabilityUseOutcome {
                kind,
                instance_id,
                update,
            })
        }
        Err(error) => {
            tracing::warn!(
                "[bong][tools] failed to persist durability for tool instance {}: {}",
                instance_id,
                error
            );
            None
        }
    }
}

pub fn damage_tool_instance(
    entity: Entity,
    inventory: &mut PlayerInventory,
    instance_id: u64,
    durability_events: &mut EventWriter<InventoryDurabilityChangedEvent>,
    cost_ratio: f64,
) -> Option<ToolDurabilityUseOutcome> {
    if !cost_ratio.is_finite() || cost_ratio <= 0.0 {
        return None;
    }
    let item = inventory_item_by_instance_borrow(inventory, instance_id)?;
    let kind = item_kind_to_tool(item.template_id.as_str())?;
    if item.durability <= 0.0 {
        return None;
    }
    let next = (item.durability - cost_ratio).clamp(0.0, 1.0);
    if next >= item.durability {
        return None;
    }
    match set_item_instance_durability(inventory, instance_id, next) {
        Ok(update) => {
            durability_events.send(InventoryDurabilityChangedEvent {
                entity,
                revision: update.revision,
                instance_id: update.instance_id,
                durability: update.durability,
            });
            Some(ToolDurabilityUseOutcome {
                kind,
                instance_id,
                update,
            })
        }
        Err(error) => {
            tracing::warn!(
                "[bong][tools] failed to persist durability for tool instance {}: {}",
                instance_id,
                error
            );
            None
        }
    }
}

pub fn has_required_tool(actual: Option<ToolKind>, required: Option<ToolKind>) -> bool {
    match required {
        Some(required) => actual == Some(required),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{InventoryRevision, ItemInstance, ItemRarity, PlayerInventory};
    use std::collections::HashMap;
    use valence::prelude::App;

    fn item(template_id: &str, instance_id: u64, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
        }
    }

    fn inventory_with_main_hand(template_id: Option<&str>) -> PlayerInventory {
        let mut equipped = HashMap::new();
        if let Some(template_id) = template_id {
            equipped.insert(EQUIP_SLOT_MAIN_HAND.to_string(), item(template_id, 42, 1.0));
        }
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: Vec::new(),
            equipped,
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 10.0,
        }
    }

    #[test]
    fn maps_all_tool_item_ids() {
        for kind in crate::tools::ALL_TOOL_KINDS {
            assert_eq!(item_kind_to_tool(kind.item_id()), Some(kind));
        }
        assert_eq!(item_kind_to_tool("iron_sword"), None);
    }

    #[test]
    fn detects_main_hand_tool_from_inventory() {
        let inventory = inventory_with_main_hand(Some("dun_qi_jia"));
        assert_eq!(
            main_hand_tool_in_inventory(&inventory),
            Some(ToolKind::DunQiJia)
        );
    }

    #[test]
    fn non_tool_main_hand_returns_none() {
        let inventory = inventory_with_main_hand(Some("iron_sword"));
        assert_eq!(main_hand_tool_in_inventory(&inventory), None);
    }

    #[test]
    fn broken_main_hand_tool_returns_none() {
        let mut inventory = inventory_with_main_hand(Some("cao_lian"));
        inventory
            .equipped
            .get_mut(EQUIP_SLOT_MAIN_HAND)
            .expect("tool should be equipped")
            .durability = 0.0;

        assert_eq!(main_hand_tool_in_inventory(&inventory), None);
        assert_eq!(main_hand_tool_instance_in_inventory(&inventory), None);
    }

    #[test]
    fn world_query_detects_main_hand_tool() {
        let mut app = App::new();
        let player = app
            .world_mut()
            .spawn(inventory_with_main_hand(Some("cai_yao_dao")))
            .id();

        assert_eq!(
            main_hand_tool(player, app.world()),
            Some(ToolKind::CaiYaoDao)
        );
    }
}
