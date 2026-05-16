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
pub const GREEN_SPIDER_ENTITY_KIND: EntityKind = EntityKind::new(135);
pub const JUNGLE_SCORPION_ENTITY_KIND: EntityKind = EntityKind::new(136);
pub const COCKADE_SNAKE_ENTITY_KIND: EntityKind = EntityKind::new(137);
pub const BLUE_SPIDER_ENTITY_KIND: EntityKind = EntityKind::new(138);
pub const ICE_SCORPION_ENTITY_KIND: EntityKind = EntityKind::new(139);
pub const MANDRAKE_SNAKE_ENTITY_KIND: EntityKind = EntityKind::new(140);
pub const DARK_TIGER_ENTITY_KIND: EntityKind = EntityKind::new(141);
pub const LIVING_PILLAR_ENTITY_KIND: EntityKind = EntityKind::new(142);
pub const POISON_DRAGON_ENTITY_KIND: EntityKind = EntityKind::new(143);
pub const BONE_DRAGON_ENTITY_KIND: EntityKind = EntityKind::new(144);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum FaunaVisualKind {
    DevourRat,
    AshSpider,
    GreenSpider,
    JungleScorpion,
    CockadeSnake,
    BlueSpider,
    IceScorpion,
    MandrakeSnake,
    HybridBeast,
    VoidDistorted,
    DarkTiger,
    LivingPillar,
    PoisonDragon,
    BoneDragon,
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
            Self::GreenSpider => "#44CC66",
            Self::JungleScorpion => "#8B6914",
            Self::CockadeSnake => "#DD4422",
            Self::BlueSpider => "#4488DD",
            Self::IceScorpion => "#88CCEE",
            Self::MandrakeSnake => "#AA44BB",
            Self::HybridBeast => "#A07058",
            Self::VoidDistorted => "#5C3DAA",
            Self::DarkTiger => "#332211",
            Self::LivingPillar => "#2A1A3A",
            Self::PoisonDragon => "#33AA22",
            Self::BoneDragon => "#DDDDAA",
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
        BeastKind::GreenSpider => GREEN_SPIDER_ENTITY_KIND,
        BeastKind::JungleScorpion => JUNGLE_SCORPION_ENTITY_KIND,
        BeastKind::CockadeSnake => COCKADE_SNAKE_ENTITY_KIND,
        BeastKind::BlueSpider => BLUE_SPIDER_ENTITY_KIND,
        BeastKind::IceScorpion => ICE_SCORPION_ENTITY_KIND,
        BeastKind::MandrakeSnake => MANDRAKE_SNAKE_ENTITY_KIND,
        BeastKind::HybridBeast => HYBRID_BEAST_ENTITY_KIND,
        BeastKind::VoidDistorted => VOID_DISTORTED_ENTITY_KIND,
        BeastKind::DarkTiger => DARK_TIGER_ENTITY_KIND,
        BeastKind::LivingPillar => LIVING_PILLAR_ENTITY_KIND,
        BeastKind::PoisonDragon => POISON_DRAGON_ENTITY_KIND,
        BeastKind::BoneDragon => BONE_DRAGON_ENTITY_KIND,
        BeastKind::Whale => crate::npc::spawn_whale::WHALE_ENTITY_KIND,
    }
}

pub const fn visual_kind_for_beast(kind: BeastKind) -> Option<FaunaVisualKind> {
    match kind {
        BeastKind::Rat => Some(FaunaVisualKind::DevourRat),
        BeastKind::Spider => Some(FaunaVisualKind::AshSpider),
        BeastKind::GreenSpider => Some(FaunaVisualKind::GreenSpider),
        BeastKind::JungleScorpion => Some(FaunaVisualKind::JungleScorpion),
        BeastKind::CockadeSnake => Some(FaunaVisualKind::CockadeSnake),
        BeastKind::BlueSpider => Some(FaunaVisualKind::BlueSpider),
        BeastKind::IceScorpion => Some(FaunaVisualKind::IceScorpion),
        BeastKind::MandrakeSnake => Some(FaunaVisualKind::MandrakeSnake),
        BeastKind::HybridBeast => Some(FaunaVisualKind::HybridBeast),
        BeastKind::VoidDistorted => Some(FaunaVisualKind::VoidDistorted),
        BeastKind::DarkTiger => Some(FaunaVisualKind::DarkTiger),
        BeastKind::LivingPillar => Some(FaunaVisualKind::LivingPillar),
        BeastKind::PoisonDragon => Some(FaunaVisualKind::PoisonDragon),
        BeastKind::BoneDragon => Some(FaunaVisualKind::BoneDragon),
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
        assert_eq!(GREEN_SPIDER_ENTITY_KIND.get(), 135);
        assert_eq!(JUNGLE_SCORPION_ENTITY_KIND.get(), 136);
        assert_eq!(COCKADE_SNAKE_ENTITY_KIND.get(), 137);
        assert_eq!(BLUE_SPIDER_ENTITY_KIND.get(), 138);
        assert_eq!(ICE_SCORPION_ENTITY_KIND.get(), 139);
        assert_eq!(MANDRAKE_SNAKE_ENTITY_KIND.get(), 140);
        assert_eq!(DARK_TIGER_ENTITY_KIND.get(), 141);
        assert_eq!(LIVING_PILLAR_ENTITY_KIND.get(), 142);
        assert_eq!(POISON_DRAGON_ENTITY_KIND.get(), 143);
        assert_eq!(BONE_DRAGON_ENTITY_KIND.get(), 144);
    }

    #[test]
    fn all_new_beasts_have_visual_kind() {
        let new_kinds = [
            BeastKind::GreenSpider,
            BeastKind::JungleScorpion,
            BeastKind::CockadeSnake,
            BeastKind::BlueSpider,
            BeastKind::IceScorpion,
            BeastKind::MandrakeSnake,
            BeastKind::DarkTiger,
            BeastKind::LivingPillar,
            BeastKind::PoisonDragon,
            BeastKind::BoneDragon,
        ];
        for kind in new_kinds {
            assert!(
                visual_kind_for_beast(kind).is_some(),
                "{kind:?} should have a FaunaVisualKind"
            );
        }
    }
}
