pub mod components;
pub mod kinds;
pub mod registry;

use valence::prelude::App;

#[allow(unused_imports)]
pub use components::ToolTag;
#[allow(unused_imports)]
pub use kinds::{ToolKind, ALL_TOOL_KINDS};
#[allow(unused_imports)]
pub use registry::{
    damage_main_hand_tool, has_required_tool, item_kind_to_tool, main_hand_tool,
    main_hand_tool_in_inventory, ToolDurabilityUseOutcome,
};

pub fn register(_app: &mut App) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_tags_keep_inventory_instance_back_reference() {
        let tag = ToolTag {
            kind: ToolKind::BaoChu,
            instance_id: 7,
        };

        assert_eq!(tag.kind, ToolKind::BaoChu);
        assert_eq!(tag.instance_id, 7);
    }

    #[test]
    fn required_tool_check_rejects_wrong_tool() {
        assert!(has_required_tool(
            Some(ToolKind::DunQiJia),
            Some(ToolKind::DunQiJia)
        ));
        assert!(!has_required_tool(
            Some(ToolKind::CaiYaoDao),
            Some(ToolKind::DunQiJia)
        ));
        assert!(has_required_tool(None, None));
    }

    #[test]
    fn tools_have_low_combat_multipliers_below_entry_sword() {
        for kind in ALL_TOOL_KINDS {
            let multiplier = kind.combat_damage_multiplier();
            assert!(multiplier > 1.0, "{kind:?} should beat bare hands");
            assert!(multiplier < 1.2, "{kind:?} should stay below iron sword");
        }
    }

    #[test]
    fn tools_have_standardized_durability_costs() {
        for kind in ALL_TOOL_KINDS {
            assert_eq!(kind.durability_cost_basis_points_per_use(), 100);
            assert_eq!(kind.durability_cost_ratio_per_use(), 0.01);
        }
    }
}
