use valence::prelude::{Entity, World};

use super::ToolKind;
use crate::inventory::{PlayerInventory, EQUIP_SLOT_MAIN_HAND};

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
    item_kind_to_tool(item.template_id.as_str())
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

    fn item(template_id: &str, instance_id: u64) -> ItemInstance {
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
            durability: 1.0,
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
            equipped.insert(EQUIP_SLOT_MAIN_HAND.to_string(), item(template_id, 42));
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
