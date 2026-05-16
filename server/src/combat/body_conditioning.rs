//! 广播体操功法（`body.guangbo_ticao`）：proficiency 0→1 线性缩放三类增益
//! （+5% 移速 / +5% 跳跃 / +0.5% 四肢防御），每次施放递减增长 proficiency。
//! `body_conditioning_aggregate` 在 Physics 阶段读 `KnownTechniques` 写入 `DerivedAttrs`。

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

pub fn apply_guangbo_ticao_bonuses(attrs: &mut DerivedAttrs, known: &KnownTechniques) {
    let Some(entry) = known.entries.iter().find(|e| e.id == GUANGBO_TICAO_ID) else {
        return;
    };
    if !entry.active {
        return;
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

pub fn body_conditioning_aggregate(
    mut q: Query<(&mut DerivedAttrs, Option<&KnownTechniques>)>,
) {
    for (mut attrs, known) in &mut q {
        let Some(known) = known else { continue };
        apply_guangbo_ticao_bonuses(&mut attrs, known);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{label}: expected {expected}, actual {actual} — check {label} scaling"
        );
    }

    fn make_known(proficiency: f32, active: bool) -> KnownTechniques {
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: GUANGBO_TICAO_ID.to_string(),
                proficiency,
                active,
            }],
        }
    }

    #[test]
    fn zero_proficiency_gives_no_bonus() {
        assert_near(guangbo_ticao_move_speed(0.0), 0.0, "guangbo_ticao_move_speed(0)");
        assert_near(guangbo_ticao_jump_height(0.0), 0.0, "guangbo_ticao_jump_height(0)");
        assert_near(guangbo_ticao_limb_defense(0.0), 0.0, "guangbo_ticao_limb_defense(0)");
    }

    #[test]
    fn half_proficiency_gives_half_bonus() {
        assert_near(guangbo_ticao_move_speed(0.5), 0.025, "guangbo_ticao_move_speed(0.5)");
        assert_near(guangbo_ticao_jump_height(0.5), 0.025, "guangbo_ticao_jump_height(0.5)");
        assert_near(guangbo_ticao_limb_defense(0.5), 0.0025, "guangbo_ticao_limb_defense(0.5)");
    }

    #[test]
    fn full_proficiency_gives_max_bonus() {
        assert_near(
            guangbo_ticao_move_speed(1.0),
            MOVE_SPEED_BONUS_MAX,
            "guangbo_ticao_move_speed(1.0) vs MOVE_SPEED_BONUS_MAX",
        );
        assert_near(
            guangbo_ticao_jump_height(1.0),
            JUMP_HEIGHT_BONUS_MAX,
            "guangbo_ticao_jump_height(1.0) vs JUMP_HEIGHT_BONUS_MAX",
        );
        assert_near(
            guangbo_ticao_limb_defense(1.0),
            LIMB_DEFENSE_BONUS_MAX,
            "guangbo_ticao_limb_defense(1.0) vs LIMB_DEFENSE_BONUS_MAX",
        );
    }

    #[test]
    fn proficiency_clamped_above_one() {
        assert_eq!(
            guangbo_ticao_move_speed(1.5),
            MOVE_SPEED_BONUS_MAX,
            "proficiency >1 should clamp to MOVE_SPEED_BONUS_MAX"
        );
        assert_eq!(
            guangbo_ticao_jump_height(2.0),
            JUMP_HEIGHT_BONUS_MAX,
            "proficiency >1 should clamp to JUMP_HEIGHT_BONUS_MAX"
        );
    }

    #[test]
    fn proficiency_clamped_below_zero() {
        assert_eq!(
            guangbo_ticao_move_speed(-0.5),
            0.0,
            "negative proficiency should clamp to 0"
        );
    }

    #[test]
    fn aggregate_applies_all_bonuses_at_full_proficiency() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_near(
            attrs.move_speed_multiplier,
            1.0 + MOVE_SPEED_BONUS_MAX,
            "move_speed after full-prof aggregate",
        );
        assert_near(
            attrs.jump_height_multiplier,
            1.0 + JUMP_HEIGHT_BONUS_MAX,
            "jump_height after full-prof aggregate",
        );
        assert_eq!(
            attrs.defense_profile.len(),
            20,
            "expected 4 limbs × 5 wound kinds = 20 defense_profile entries"
        );
        for &part in &LIMB_PARTS {
            for &kind in &ALL_WOUND_KINDS {
                let v = attrs.defense_profile[&(part, kind)];
                assert_near(v, LIMB_DEFENSE_BONUS_MAX, "defense_profile limb entry");
            }
        }
        assert!(
            !attrs.defense_profile.contains_key(&(BodyPart::Head, WoundKind::Cut)),
            "Head should not get limb defense"
        );
        assert!(
            !attrs.defense_profile.contains_key(&(BodyPart::Chest, WoundKind::Blunt)),
            "Chest should not get limb defense"
        );
        assert!(
            !attrs.defense_profile.contains_key(&(BodyPart::Abdomen, WoundKind::Pierce)),
            "Abdomen should not get limb defense"
        );
    }

    #[test]
    fn aggregate_stacks_with_existing_armor() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        attrs.defense_profile.insert((BodyPart::ArmL, WoundKind::Cut), 0.3);
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        let expected = 0.3 + LIMB_DEFENSE_BONUS_MAX;
        assert_near(
            attrs.defense_profile[&(BodyPart::ArmL, WoundKind::Cut)],
            expected,
            "armor 0.3 + LIMB_DEFENSE_BONUS_MAX stacking",
        );
    }

    #[test]
    fn aggregate_respects_mitigation_cap() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        attrs.defense_profile.insert((BodyPart::LegR, WoundKind::Blunt), ARMOR_MITIGATION_CAP);
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.defense_profile[&(BodyPart::LegR, WoundKind::Blunt)],
            ARMOR_MITIGATION_CAP,
            "defense_profile should not exceed ARMOR_MITIGATION_CAP ({ARMOR_MITIGATION_CAP})"
        );
    }

    #[test]
    fn inactive_technique_leaves_attrs_unchanged() {
        let known = make_known(1.0, false);
        let mut attrs = DerivedAttrs::default();
        let before_speed = attrs.move_speed_multiplier;
        let before_jump = attrs.jump_height_multiplier;
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.move_speed_multiplier, before_speed,
            "inactive technique should not change move_speed_multiplier"
        );
        assert_eq!(
            attrs.jump_height_multiplier, before_jump,
            "inactive technique should not change jump_height_multiplier"
        );
        assert!(
            attrs.defense_profile.is_empty(),
            "inactive technique should not add defense_profile entries"
        );
    }

    #[test]
    fn unknown_technique_leaves_attrs_unchanged() {
        let known = KnownTechniques { entries: vec![] };
        let mut attrs = DerivedAttrs::default();
        let before_speed = attrs.move_speed_multiplier;
        let before_jump = attrs.jump_height_multiplier;
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.move_speed_multiplier, before_speed,
            "empty KnownTechniques should not change move_speed_multiplier"
        );
        assert_eq!(
            attrs.jump_height_multiplier, before_jump,
            "empty KnownTechniques should not change jump_height_multiplier"
        );
        assert!(
            attrs.defense_profile.is_empty(),
            "empty KnownTechniques should not add defense_profile entries"
        );
    }

    #[test]
    fn record_practice_increases_proficiency() {
        let mut known = KnownTechniques { entries: vec![] };
        let p1 = record_guangbo_practice(&mut known);
        assert!(
            p1 > 0.0,
            "first record_guangbo_practice should yield positive proficiency, got {p1}"
        );
        assert_eq!(
            known.entries.len(),
            1,
            "record_guangbo_practice should auto-create entry"
        );
        assert_eq!(
            known.entries[0].id, GUANGBO_TICAO_ID,
            "auto-created entry should have id={GUANGBO_TICAO_ID}"
        );

        let p2 = record_guangbo_practice(&mut known);
        assert!(
            p2 > p1,
            "second practice should increase proficiency: {p2} > {p1}"
        );
    }

    #[test]
    fn proficiency_gain_diminishes_past_half() {
        let low_gain = guangbo_proficiency_gain(0.3);
        let high_gain = guangbo_proficiency_gain(0.7);
        assert!(
            low_gain > high_gain,
            "gain at 0.3 ({low_gain}) should exceed gain at 0.7 ({high_gain}) — diminishing returns"
        );
    }

    #[test]
    fn proficiency_never_exceeds_one() {
        let mut known = make_known(0.999, true);
        let p = record_guangbo_practice(&mut known);
        assert!(
            p <= 1.0,
            "proficiency should never exceed 1.0 after practice, got {p}"
        );
    }
}
