use valence::prelude::{bevy_ecs, Component, Entity, Event};

use crate::cultivation::components::{Cultivation, MeridianSystem};
use crate::cultivation::known_techniques::{technique_definition, KnownTechniques};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::technique_observe::{parse_grade, TechniqueGrade};
use crate::cultivation::technique_scroll::{
    learn_technique_if_allowed, LearnSource, ScrollReadOutcome, TechniqueLearnedEvent,
};
use crate::inventory::PlayerInventory;
use crate::npc::lifecycle::NpcArchetype;

pub const WOLIU_STYLE_TAG: &str = "woliu";
pub const MENTOR_MIN_REPUTATION: i32 = 50;
pub const MENTOR_REPUTATION_COST: i32 = 10;

#[derive(Debug, Clone, Default, Component, PartialEq, Eq)]
pub struct CombatStyleTags {
    pub tags: Vec<String>,
}

impl CombatStyleTags {
    pub fn has(&self, tag: &str) -> bool {
        self.tags.iter().any(|candidate| candidate == tag)
    }
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueMentorTaughtEvent {
    pub player: Entity,
    pub npc_entity: Entity,
    pub technique_id: String,
    pub bone_coin_cost: u64,
    pub reputation_cost: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MentorOutcome {
    Taught {
        learned: TechniqueLearnedEvent,
        bone_coin_cost: u64,
    },
    MissingStyleTag,
    LowReputation,
    UnsupportedArchetype,
    EarthGradeRefused,
    NotEnoughBoneCoins {
        required: u64,
        current: u64,
    },
    LearnBlocked(ScrollReadOutcome),
    UnknownTechnique,
}

pub fn mentor_dialog_option_appears(
    archetype: NpcArchetype,
    tags: Option<&CombatStyleTags>,
    reputation_to_player: i32,
) -> bool {
    matches!(archetype, NpcArchetype::Rogue | NpcArchetype::Disciple)
        && reputation_to_player >= MENTOR_MIN_REPUTATION
        && tags.is_some_and(|tags| tags.has(WOLIU_STYLE_TAG))
}

pub fn mentor_teaches_technique(
    inventory: &mut PlayerInventory,
    known: &mut KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    ctx: MentorTeachContext,
) -> MentorOutcome {
    if !matches!(ctx.archetype, NpcArchetype::Rogue | NpcArchetype::Disciple) {
        return MentorOutcome::UnsupportedArchetype;
    }
    if !ctx.tags.has(WOLIU_STYLE_TAG) {
        return MentorOutcome::MissingStyleTag;
    }
    if ctx.reputation_to_player < MENTOR_MIN_REPUTATION {
        return MentorOutcome::LowReputation;
    }
    let Some(definition) = technique_definition(ctx.technique_id) else {
        return MentorOutcome::UnknownTechnique;
    };
    if parse_grade(definition.grade) == TechniqueGrade::Earth {
        return MentorOutcome::EarthGradeRefused;
    }
    let cost = mentor_cost_for_grade(parse_grade(definition.grade));
    if inventory.bone_coins < cost {
        return MentorOutcome::NotEnoughBoneCoins {
            required: cost,
            current: inventory.bone_coins,
        };
    }
    let outcome = learn_technique_if_allowed(
        known,
        cultivation,
        meridians,
        severed,
        ctx.technique_id,
        0.0,
    );
    if !matches!(outcome, ScrollReadOutcome::Learned) {
        return MentorOutcome::LearnBlocked(outcome);
    }
    inventory.bone_coins -= cost;
    MentorOutcome::Taught {
        learned: TechniqueLearnedEvent {
            player: ctx.player,
            technique_id: ctx.technique_id.to_string(),
            source: LearnSource::Mentor {
                npc_entity: ctx.npc_entity,
            },
        },
        bone_coin_cost: cost,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MentorTeachContext<'a> {
    pub player: Entity,
    pub npc_entity: Entity,
    pub archetype: NpcArchetype,
    pub tags: &'a CombatStyleTags,
    pub reputation_to_player: i32,
    pub technique_id: &'a str,
}

pub fn mentor_cost_for_grade(grade: TechniqueGrade) -> u64 {
    match grade {
        TechniqueGrade::Yellow => 20,
        TechniqueGrade::Profound => 50,
        TechniqueGrade::Earth | TechniqueGrade::Other => u64::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{MeridianId, Realm};
    use crate::inventory::{ContainerState, InventoryRevision, MAIN_PACK_CONTAINER_ID};
    use std::collections::HashMap;

    fn tags() -> CombatStyleTags {
        CombatStyleTags {
            tags: vec![WOLIU_STYLE_TAG.to_string()],
        }
    }

    fn inventory(coins: u64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 4,
                cols: 9,
                items: Vec::new(),
            }],
            equipped: HashMap::new(),
            hotbar: Default::default(),
            bone_coins: coins,
            max_weight: 99.0,
        }
    }

    fn opened_lung_heart() -> MeridianSystem {
        let mut meridians = MeridianSystem::default();
        for id in [MeridianId::Lung, MeridianId::Heart] {
            let m = meridians.get_mut(id);
            m.opened = true;
            m.integrity = 1.0;
        }
        meridians
    }

    #[test]
    fn mentor_dialog_option_visible() {
        assert!(super::mentor_dialog_option_appears(
            NpcArchetype::Rogue,
            Some(&tags()),
            50
        ));
    }

    #[test]
    fn mentor_teaches_woliu_technique() {
        let player = Entity::from_raw(1);
        let npc = Entity::from_raw(2);
        let mut inventory = inventory(50);
        let mut known = KnownTechniques::default();

        let outcome = super::mentor_teaches_technique(
            &mut inventory,
            &mut known,
            &Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
            &opened_lung_heart(),
            None,
            MentorTeachContext {
                player,
                npc_entity: npc,
                archetype: NpcArchetype::Rogue,
                tags: &tags(),
                reputation_to_player: 60,
                technique_id: "woliu.burst",
            },
        );

        assert!(matches!(
            outcome,
            MentorOutcome::Taught {
                bone_coin_cost: 20,
                ..
            }
        ));
        assert_eq!(inventory.bone_coins, 30);
        assert!(known.entries.iter().any(|entry| entry.id == "woliu.burst"));
    }

    #[test]
    fn mentor_cost_deducted() {
        let mut inventory = inventory(50);
        let mut known = KnownTechniques::default();

        let _ = super::mentor_teaches_technique(
            &mut inventory,
            &mut known,
            &Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
            &opened_lung_heart(),
            None,
            MentorTeachContext {
                player: Entity::from_raw(1),
                npc_entity: Entity::from_raw(2),
                archetype: NpcArchetype::Disciple,
                tags: &tags(),
                reputation_to_player: 50,
                technique_id: "woliu.vacuum_palm",
            },
        );

        assert_eq!(inventory.bone_coins, 30);
    }

    #[test]
    fn mentor_refuses_low_affinity() {
        assert!(!super::mentor_dialog_option_appears(
            NpcArchetype::Rogue,
            Some(&tags()),
            49
        ));
    }

    #[test]
    fn mentor_refuses_earth_grade() {
        let mut inventory = inventory(100);
        let mut known = KnownTechniques::default();

        let outcome = super::mentor_teaches_technique(
            &mut inventory,
            &mut known,
            &Cultivation {
                realm: Realm::Condense,
                ..Default::default()
            },
            &opened_lung_heart(),
            None,
            MentorTeachContext {
                player: Entity::from_raw(1),
                npc_entity: Entity::from_raw(2),
                archetype: NpcArchetype::Rogue,
                tags: &tags(),
                reputation_to_player: 70,
                technique_id: "woliu.heart",
            },
        );

        assert_eq!(outcome, MentorOutcome::EarthGradeRefused);
    }

    #[test]
    fn mentor_refuses_severed_meridian() {
        let mut inventory = inventory(100);
        let mut known = KnownTechniques::default();
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Lung);

        let outcome = super::mentor_teaches_technique(
            &mut inventory,
            &mut known,
            &Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
            &opened_lung_heart(),
            Some(&severed),
            MentorTeachContext {
                player: Entity::from_raw(1),
                npc_entity: Entity::from_raw(2),
                archetype: NpcArchetype::Rogue,
                tags: &tags(),
                reputation_to_player: 70,
                technique_id: "woliu.burst",
            },
        );

        assert!(matches!(
            outcome,
            MentorOutcome::LearnBlocked(ScrollReadOutcome::MeridianSevered { .. })
        ));
    }
}
