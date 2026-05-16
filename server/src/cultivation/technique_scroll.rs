use valence::prelude::{bevy_ecs, Entity, Event};

use crate::cultivation::components::{Cultivation, MeridianId, MeridianSystem, Realm};
use crate::cultivation::known_techniques::{
    technique_definition, KnownTechnique, KnownTechniques, TechniqueDefinition,
};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::inventory::ItemTemplate;

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueScrollReadEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source_item: String,
    pub outcome: ScrollReadOutcome,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueLearnedEvent {
    pub player: Entity,
    pub technique_id: String,
    pub source: LearnSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LearnSource {
    Scroll { item_id: String },
    Observe { observed_entity: Entity },
    Mentor { npc_entity: Entity },
    DyingMaster { npc_entity: Entity },
    DevCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScrollReadOutcome {
    Learned,
    AlreadyKnown,
    RealmTooLow { required: Realm, current: Realm },
    MeridianSevered { channel: MeridianId },
    MeridianMissing { channel: MeridianId },
    InvalidScroll,
}

pub fn read_combat_technique_scroll(
    known: &mut KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    template: &ItemTemplate,
) -> ScrollReadOutcome {
    let Some(spec) = template.technique_scroll_spec.as_ref() else {
        return ScrollReadOutcome::InvalidScroll;
    };
    if spec.kind != "combat_technique" {
        return ScrollReadOutcome::InvalidScroll;
    }
    learn_technique_if_allowed(
        known,
        cultivation,
        meridians,
        severed,
        spec.skill_id.as_str(),
        0.0,
    )
}

pub fn learn_technique_if_allowed(
    known: &mut KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    technique_id: &str,
    initial_proficiency: f32,
) -> ScrollReadOutcome {
    let Some(definition) = technique_definition(technique_id) else {
        return ScrollReadOutcome::InvalidScroll;
    };
    if known.entries.iter().any(|entry| entry.id == technique_id) {
        return ScrollReadOutcome::AlreadyKnown;
    }
    if let Some(required) = required_realm(definition) {
        if realm_rank(cultivation.realm) < realm_rank(required) {
            return ScrollReadOutcome::RealmTooLow {
                required,
                current: cultivation.realm,
            };
        }
    } else {
        return ScrollReadOutcome::InvalidScroll;
    }
    if let Err(outcome) = check_required_meridians(definition, meridians, severed) {
        return outcome;
    }

    known.entries.push(KnownTechnique {
        id: technique_id.to_string(),
        proficiency: initial_proficiency.clamp(0.0, 1.0),
        active: true,
    });
    ScrollReadOutcome::Learned
}

pub fn can_learn_technique(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    technique_id: &str,
) -> ScrollReadOutcome {
    let mut probe = known.clone();
    learn_technique_if_allowed(
        &mut probe,
        cultivation,
        meridians,
        severed,
        technique_id,
        0.0,
    )
}

fn check_required_meridians(
    definition: &TechniqueDefinition,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
) -> Result<(), ScrollReadOutcome> {
    for required in definition.required_meridians {
        let Some(channel) = parse_meridian_id(required.channel) else {
            return Err(ScrollReadOutcome::InvalidScroll);
        };
        if severed.is_some_and(|severed| severed.is_severed(channel)) {
            return Err(ScrollReadOutcome::MeridianSevered { channel });
        }
        let state = meridians.get(channel);
        if !state.opened || state.integrity < f64::from(required.min_health) {
            return Err(ScrollReadOutcome::MeridianMissing { channel });
        }
    }
    Ok(())
}

fn required_realm(definition: &TechniqueDefinition) -> Option<Realm> {
    match definition.required_realm {
        "Awaken" => Some(Realm::Awaken),
        "Induce" => Some(Realm::Induce),
        "Condense" => Some(Realm::Condense),
        "Solidify" => Some(Realm::Solidify),
        "Spirit" => Some(Realm::Spirit),
        "Void" => Some(Realm::Void),
        _ => None,
    }
}

pub fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

pub fn parse_meridian_id(raw: &str) -> Option<MeridianId> {
    match raw {
        "Lung" => Some(MeridianId::Lung),
        "LargeIntestine" => Some(MeridianId::LargeIntestine),
        "Stomach" => Some(MeridianId::Stomach),
        "Spleen" => Some(MeridianId::Spleen),
        "Heart" => Some(MeridianId::Heart),
        "SmallIntestine" => Some(MeridianId::SmallIntestine),
        "Bladder" => Some(MeridianId::Bladder),
        "Kidney" => Some(MeridianId::Kidney),
        "Pericardium" => Some(MeridianId::Pericardium),
        "TripleEnergizer" => Some(MeridianId::TripleEnergizer),
        "GallBladder" | "Gallbladder" => Some(MeridianId::Gallbladder),
        "Liver" => Some(MeridianId::Liver),
        "Ren" => Some(MeridianId::Ren),
        "Du" => Some(MeridianId::Du),
        "Chong" => Some(MeridianId::Chong),
        "Dai" => Some(MeridianId::Dai),
        "YinQiao" => Some(MeridianId::YinQiao),
        "YangQiao" => Some(MeridianId::YangQiao),
        "YinWei" => Some(MeridianId::YinWei),
        "YangWei" => Some(MeridianId::YangWei),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ItemCategory, ItemRarity, TechniqueScrollSpec, DEFAULT_CAST_DURATION_MS,
        DEFAULT_COOLDOWN_MS,
    };

    fn template(id: &str, skill_id: &str) -> ItemTemplate {
        ItemTemplate {
            id: id.to_string(),
            display_name: "涡流残卷".to_string(),
            category: ItemCategory::Scroll,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Uncommon,
            spirit_quality_initial: 0.5,
            description: "test".to_string(),
            effect: None,
            cast_duration_ms: DEFAULT_CAST_DURATION_MS,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: Some(TechniqueScrollSpec {
                kind: "combat_technique".to_string(),
                skill_id: skill_id.to_string(),
            }),
            container_spec: None,
        }
    }

    fn open_required_meridians(meridians: &mut MeridianSystem, skill_id: &str) {
        let definition = technique_definition(skill_id).unwrap();
        for required in definition.required_meridians {
            let id = parse_meridian_id(required.channel).unwrap();
            let channel = meridians.get_mut(id);
            channel.opened = true;
            channel.integrity = 1.0;
        }
    }

    #[test]
    fn read_scroll_success() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
        );

        assert_eq!(outcome, ScrollReadOutcome::Learned);
        assert_eq!(known.entries.len(), 1);
        assert_eq!(known.entries[0].id, "woliu.vortex");
        assert_eq!(known.entries[0].proficiency, 0.0);
        assert!(known.entries[0].active);
    }

    #[test]
    fn read_scroll_realm_too_low() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation::default();
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::RealmTooLow {
                required: Realm::Condense,
                current: Realm::Awaken
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_meridian_severed() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");
        let mut severed = MeridianSeveredPermanent::default();
        severed.severed_meridians.insert(MeridianId::Lung);

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            Some(&severed),
            &template("scroll_woliu_vortex", "woliu.vortex"),
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::MeridianSevered {
                channel: MeridianId::Lung
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_meridian_missing() {
        let mut known = KnownTechniques::default();
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &MeridianSystem::default(),
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
        );

        assert_eq!(
            outcome,
            ScrollReadOutcome::MeridianMissing {
                channel: MeridianId::Lung
            }
        );
        assert!(known.entries.is_empty());
    }

    #[test]
    fn read_scroll_already_known() {
        let mut known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "woliu.vortex".to_string(),
                proficiency: 0.2,
                active: true,
            }],
        };
        let cultivation = Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        };
        let mut meridians = MeridianSystem::default();
        open_required_meridians(&mut meridians, "woliu.vortex");

        let outcome = read_combat_technique_scroll(
            &mut known,
            &cultivation,
            &meridians,
            None,
            &template("scroll_woliu_vortex", "woliu.vortex"),
        );

        assert_eq!(outcome, ScrollReadOutcome::AlreadyKnown);
        assert_eq!(known.entries.len(), 1);
        assert_eq!(known.entries[0].proficiency, 0.2);
    }

    #[test]
    fn read_scroll_invalid() {
        let mut known = KnownTechniques::default();
        let mut invalid = template("scroll_bad", "woliu.vortex");
        invalid.technique_scroll_spec = None;

        let outcome = read_combat_technique_scroll(
            &mut known,
            &Cultivation::default(),
            &MeridianSystem::default(),
            None,
            &invalid,
        );

        assert_eq!(outcome, ScrollReadOutcome::InvalidScroll);
        assert!(known.entries.is_empty());
    }
}
