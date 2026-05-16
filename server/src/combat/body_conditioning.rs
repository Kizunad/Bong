use valence::prelude::{bevy_ecs, Entity, Event, EventReader, Query};

use crate::combat::armor::ARMOR_MITIGATION_CAP;
use crate::combat::components::{BodyPart, DerivedAttrs, WoundKind};
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

pub const GUANGBO_TICAO_ID: &str = "body.guangbo_ticao";

const MOVE_SPEED_BONUS_MAX: f32 = 0.05;
const JUMP_HEIGHT_BONUS_MAX: f32 = 0.05;
const LIMB_DEFENSE_BONUS_MAX: f32 = 0.005;

const LIMB_PARTS: [BodyPart; 4] = [
    BodyPart::ArmL,
    BodyPart::ArmR,
    BodyPart::LegL,
    BodyPart::LegR,
];

const ALL_WOUND_KINDS: [WoundKind; 5] = [
    WoundKind::Cut,
    WoundKind::Blunt,
    WoundKind::Pierce,
    WoundKind::Burn,
    WoundKind::Concussion,
];

pub fn guangbo_ticao_move_speed(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * MOVE_SPEED_BONUS_MAX
}

pub fn guangbo_ticao_jump_height(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * JUMP_HEIGHT_BONUS_MAX
}

pub fn guangbo_ticao_limb_defense(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * LIMB_DEFENSE_BONUS_MAX
}

#[derive(Debug, Clone, Event)]
pub struct GuangboTicaoPracticeEvent {
    pub entity: Entity,
}

pub fn guangbo_proficiency_gain(current: f32) -> f32 {
    if current < 0.5 { 0.01 } else { 0.005 }
}

pub fn record_guangbo_practice(known: &mut KnownTechniques) -> f32 {
    let entry = ensure_entry(known);
    let gain = guangbo_proficiency_gain(entry.proficiency);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
    entry.proficiency
}

fn ensure_entry(known: &mut KnownTechniques) -> &mut KnownTechnique {
    if let Some(idx) = known.entries.iter().position(|e| e.id == GUANGBO_TICAO_ID) {
        return &mut known.entries[idx];
    }
    known.entries.push(KnownTechnique {
        id: GUANGBO_TICAO_ID.to_string(),
        proficiency: 0.0,
        active: true,
    });
    known.entries.last_mut().expect("entry was just inserted")
}

pub fn consume_guangbo_practice_events(
    mut events: EventReader<GuangboTicaoPracticeEvent>,
    mut q: Query<&mut KnownTechniques>,
) {
    for event in events.read() {
        if let Ok(mut known) = q.get_mut(event.entity) {
            record_guangbo_practice(&mut known);
        }
    }
}

pub fn body_conditioning_aggregate(
    mut q: Query<(&mut DerivedAttrs, Option<&KnownTechniques>)>,
) {
    for (mut attrs, known) in &mut q {
        let Some(known) = known else { continue };
        let Some(entry) = known.entries.iter().find(|e| e.id == GUANGBO_TICAO_ID) else {
            continue;
        };
        if !entry.active {
            continue;
        }

        let prof = entry.proficiency;
        attrs.move_speed_multiplier *= 1.0 + guangbo_ticao_move_speed(prof);
        attrs.jump_height_multiplier *= 1.0 + guangbo_ticao_jump_height(prof);

        let limb_def = guangbo_ticao_limb_defense(prof);
        if limb_def > 0.0 {
            for &part in &LIMB_PARTS {
                for &kind in &ALL_WOUND_KINDS {
                    attrs
                        .defense_profile
                        .entry((part, kind))
                        .and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))
                        .or_insert(limb_def);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_proficiency_gives_no_bonus() {
        assert_eq!(guangbo_ticao_move_speed(0.0), 0.0);
        assert_eq!(guangbo_ticao_jump_height(0.0), 0.0);
        assert_eq!(guangbo_ticao_limb_defense(0.0), 0.0);
    }

    #[test]
    fn half_proficiency_gives_half_bonus() {
        let eps = 1e-6;
        assert!((guangbo_ticao_move_speed(0.5) - 0.025).abs() < eps);
        assert!((guangbo_ticao_jump_height(0.5) - 0.025).abs() < eps);
        assert!((guangbo_ticao_limb_defense(0.5) - 0.0025).abs() < eps);
    }

    #[test]
    fn full_proficiency_gives_max_bonus() {
        let eps = 1e-6;
        assert!((guangbo_ticao_move_speed(1.0) - MOVE_SPEED_BONUS_MAX).abs() < eps);
        assert!((guangbo_ticao_jump_height(1.0) - JUMP_HEIGHT_BONUS_MAX).abs() < eps);
        assert!((guangbo_ticao_limb_defense(1.0) - LIMB_DEFENSE_BONUS_MAX).abs() < eps);
    }

    #[test]
    fn proficiency_clamped_above_one() {
        assert_eq!(guangbo_ticao_move_speed(1.5), MOVE_SPEED_BONUS_MAX);
        assert_eq!(guangbo_ticao_jump_height(2.0), JUMP_HEIGHT_BONUS_MAX);
    }

    #[test]
    fn proficiency_clamped_below_zero() {
        assert_eq!(guangbo_ticao_move_speed(-0.5), 0.0);
    }

    #[test]
    fn limb_defense_covers_all_four_limbs_and_five_wound_kinds() {
        let mut attrs = DerivedAttrs::default();
        let limb_def = guangbo_ticao_limb_defense(1.0);

        for &part in &LIMB_PARTS {
            for &kind in &ALL_WOUND_KINDS {
                attrs
                    .defense_profile
                    .entry((part, kind))
                    .and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))
                    .or_insert(limb_def);
            }
        }

        assert_eq!(attrs.defense_profile.len(), 20);
        for &part in &LIMB_PARTS {
            for &kind in &ALL_WOUND_KINDS {
                let v = attrs.defense_profile[&(part, kind)];
                assert!((v - LIMB_DEFENSE_BONUS_MAX).abs() < 1e-6);
            }
        }

        assert!(!attrs.defense_profile.contains_key(&(BodyPart::Head, WoundKind::Cut)));
        assert!(!attrs.defense_profile.contains_key(&(BodyPart::Chest, WoundKind::Blunt)));
        assert!(!attrs.defense_profile.contains_key(&(BodyPart::Abdomen, WoundKind::Pierce)));
    }

    #[test]
    fn limb_defense_stacks_with_existing_armor() {
        let mut attrs = DerivedAttrs::default();
        attrs.defense_profile.insert((BodyPart::ArmL, WoundKind::Cut), 0.3);

        let limb_def = guangbo_ticao_limb_defense(1.0);
        attrs
            .defense_profile
            .entry((BodyPart::ArmL, WoundKind::Cut))
            .and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))
            .or_insert(limb_def);

        let expected = 0.3 + LIMB_DEFENSE_BONUS_MAX;
        assert!((attrs.defense_profile[&(BodyPart::ArmL, WoundKind::Cut)] - expected).abs() < 1e-6);
    }

    #[test]
    fn limb_defense_respects_mitigation_cap() {
        let mut attrs = DerivedAttrs::default();
        attrs.defense_profile.insert((BodyPart::LegR, WoundKind::Blunt), ARMOR_MITIGATION_CAP);

        let limb_def = guangbo_ticao_limb_defense(1.0);
        attrs
            .defense_profile
            .entry((BodyPart::LegR, WoundKind::Blunt))
            .and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))
            .or_insert(limb_def);

        assert_eq!(
            attrs.defense_profile[&(BodyPart::LegR, WoundKind::Blunt)],
            ARMOR_MITIGATION_CAP
        );
    }

    #[test]
    fn inactive_technique_gives_no_bonus() {
        use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: GUANGBO_TICAO_ID.to_string(),
                proficiency: 1.0,
                active: false,
            }],
        };
        let entry = known.entries.iter().find(|e| e.id == GUANGBO_TICAO_ID).unwrap();
        assert!(!entry.active);
    }

    #[test]
    fn unknown_technique_gives_no_effect() {
        let known = KnownTechniques { entries: vec![] };
        assert!(!known.entries.iter().any(|e| e.id == GUANGBO_TICAO_ID));
    }

    #[test]
    fn record_practice_increases_proficiency() {
        let mut known = KnownTechniques { entries: vec![] };
        let p1 = record_guangbo_practice(&mut known);
        assert!(p1 > 0.0);
        assert_eq!(known.entries.len(), 1);
        assert_eq!(known.entries[0].id, GUANGBO_TICAO_ID);

        let p2 = record_guangbo_practice(&mut known);
        assert!(p2 > p1);
    }

    #[test]
    fn proficiency_gain_diminishes_past_half() {
        let low_gain = guangbo_proficiency_gain(0.3);
        let high_gain = guangbo_proficiency_gain(0.7);
        assert!(low_gain > high_gain);
    }

    #[test]
    fn proficiency_never_exceeds_one() {
        let mut known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: GUANGBO_TICAO_ID.to_string(),
                proficiency: 0.999,
                active: true,
            }],
        };
        let p = record_guangbo_practice(&mut known);
        assert!(p <= 1.0);
    }
}
