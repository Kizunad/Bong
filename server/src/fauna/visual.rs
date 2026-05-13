//! Fauna / TSY hostile visual shell contracts.

use valence::prelude::{bevy_ecs, Component, EntityKind};

use super::components::BeastKind;

pub const DEVOUR_RAT_ENTITY_KIND: EntityKind = EntityKind::new(126);
pub const ASH_SPIDER_ENTITY_KIND: EntityKind = EntityKind::new(127);
pub const HYBRID_BEAST_ENTITY_KIND: EntityKind = EntityKind::new(128);
pub const VOID_DISTORTED_ENTITY_KIND: EntityKind = EntityKind::new(129);
pub const DAOXIANG_ENTITY_KIND: EntityKind = EntityKind::new(130);
pub const ZHINIAN_ENTITY_KIND: EntityKind = EntityKind::new(131);
pub const TSY_SENTINEL_ENTITY_KIND: EntityKind = EntityKind::new(132);
pub const FUYA_ENTITY_KIND: EntityKind = EntityKind::new(133);
pub const SKULL_FIEND_ENTITY_KIND: EntityKind = EntityKind::new(134);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum FaunaVisualKind {
    DevourRat,
    AshSpider,
    HybridBeast,
    VoidDistorted,
    Daoxiang,
    Zhinian,
    TsySentinel,
    Fuya,
    SkullFiend,
}

impl FaunaVisualKind {
    pub const fn event_color(self) -> &'static str {
        match self {
            Self::DevourRat => "#FF4444",
            Self::AshSpider => "#B8D0C8",
            Self::HybridBeast => "#A07058",
            Self::VoidDistorted => "#5C3DAA",
            Self::Daoxiang => "#9C8A70",
            Self::Zhinian => "#343044",
            Self::TsySentinel => "#B89258",
            Self::Fuya => "#7A2FAD",
            Self::SkullFiend => "#C8D0FF",
        }
    }
}

pub const fn entity_kind_for_beast(kind: BeastKind) -> EntityKind {
    match kind {
        BeastKind::Rat => DEVOUR_RAT_ENTITY_KIND,
        BeastKind::Spider => ASH_SPIDER_ENTITY_KIND,
        BeastKind::HybridBeast => HYBRID_BEAST_ENTITY_KIND,
        BeastKind::VoidDistorted => VOID_DISTORTED_ENTITY_KIND,
        BeastKind::Whale => crate::npc::spawn_whale::WHALE_ENTITY_KIND,
    }
}

pub const fn visual_kind_for_beast(kind: BeastKind) -> Option<FaunaVisualKind> {
    match kind {
        BeastKind::Rat => Some(FaunaVisualKind::DevourRat),
        BeastKind::Spider => Some(FaunaVisualKind::AshSpider),
        BeastKind::HybridBeast => Some(FaunaVisualKind::HybridBeast),
        BeastKind::VoidDistorted => Some(FaunaVisualKind::VoidDistorted),
        BeastKind::Whale => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whale_keeps_existing_renderer_without_fauna_visual_shell() {
        assert_eq!(
            entity_kind_for_beast(BeastKind::Whale),
            crate::npc::spawn_whale::WHALE_ENTITY_KIND
        );
        assert_eq!(visual_kind_for_beast(BeastKind::Whale), None);
    }

    #[test]
    fn fauna_entity_kind_constants_match_client_raw_ids() {
        assert_eq!(DEVOUR_RAT_ENTITY_KIND.get(), 126);
        assert_eq!(ASH_SPIDER_ENTITY_KIND.get(), 127);
        assert_eq!(HYBRID_BEAST_ENTITY_KIND.get(), 128);
        assert_eq!(VOID_DISTORTED_ENTITY_KIND.get(), 129);
        assert_eq!(DAOXIANG_ENTITY_KIND.get(), 130);
        assert_eq!(ZHINIAN_ENTITY_KIND.get(), 131);
        assert_eq!(TSY_SENTINEL_ENTITY_KIND.get(), 132);
        assert_eq!(FUYA_ENTITY_KIND.get(), 133);
        assert_eq!(SKULL_FIEND_ENTITY_KIND.get(), 134);
    }
}
