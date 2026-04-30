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
    has_required_tool, item_kind_to_tool, main_hand_tool, main_hand_tool_in_inventory,
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
}
