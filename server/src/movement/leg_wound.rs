//! plan-race-system-v1 P0b —— `combined_leg_factor` 的"哪些部位算腿"判定改为查询
//! [`crate::body_plan::humanoid_plan_static`] 的 `PartConsequence::Locomotion` 标签
//! （`assets/body_plans/plans/humanoid.json`），不再硬编码假设"`LegL`/`LegR` 这两个
//! enum 变体就是双腿"。humanoid plan 目前恰好只标了这两个部位为 `Locomotion`，故行为
//! bit-for-bit 不变；泛化后天然支持未来非人形 plan（如飞鲸尾鳍）声明任意数量的
//! locomotion 部位取最劣值。本文件其余公开函数签名不变（外部既有调用点
//! `movement/mod.rs` 不在本 work item 声明范围内）。

use crate::body_plan::PartConsequence;
use crate::combat::components::{BodyPart, Wound, Wounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegWoundGrade {
    Intact,
    Bruise,
    Abrasion,
    Laceration,
    Fracture,
    Severed,
}

pub fn leg_wound_to_speed(wound: LegWoundGrade) -> f32 {
    match wound {
        LegWoundGrade::Intact | LegWoundGrade::Bruise => 1.0,
        LegWoundGrade::Abrasion => 0.9,
        LegWoundGrade::Laceration => 0.7,
        LegWoundGrade::Fracture => 0.4,
        LegWoundGrade::Severed => 0.0,
    }
}

pub fn combined_leg_factor_from_optional(wounds: Option<&Wounds>) -> f32 {
    wounds.map(combined_leg_factor).unwrap_or(1.0)
}

pub fn combined_leg_factor(wounds: &Wounds) -> f32 {
    let plan = crate::body_plan::humanoid_plan_static();
    crate::body_plan::legacy_body_parts_matching(plan, |consequence| {
        matches!(consequence, PartConsequence::Locomotion)
    })
    .map(|part| leg_wound_to_speed(worst_wound_grade(wounds, part)))
    .fold(1.0_f32, f32::min)
}

pub fn leg_strain_magnitude(leg_wound_factor: f32) -> f32 {
    ((1.0 - leg_wound_factor.clamp(0.0, 1.0)) / 0.15).clamp(0.0, 1.0)
}

pub fn worst_wound_grade(wounds: &Wounds, part: BodyPart) -> LegWoundGrade {
    wounds
        .entries
        .iter()
        .filter(|wound| wound.location == part)
        .map(wound_grade)
        .max_by_key(|grade| grade_rank(*grade))
        .unwrap_or(LegWoundGrade::Intact)
}

fn wound_grade(wound: &Wound) -> LegWoundGrade {
    wound_severity_to_grade(wound.severity)
}

pub fn wound_severity_to_grade(severity: f32) -> LegWoundGrade {
    if severity >= 70.0 {
        LegWoundGrade::Severed
    } else if severity >= 35.0 {
        LegWoundGrade::Fracture
    } else if severity >= 15.0 {
        LegWoundGrade::Laceration
    } else if severity >= 5.0 {
        LegWoundGrade::Abrasion
    } else if severity > 0.0 {
        LegWoundGrade::Bruise
    } else {
        LegWoundGrade::Intact
    }
}

const fn grade_rank(grade: LegWoundGrade) -> u8 {
    match grade {
        LegWoundGrade::Intact => 0,
        LegWoundGrade::Bruise => 1,
        LegWoundGrade::Abrasion => 2,
        LegWoundGrade::Laceration => 3,
        LegWoundGrade::Fracture => 4,
        LegWoundGrade::Severed => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::components::{WoundKind, Wounds};

    fn wound(location: BodyPart, severity: f32) -> Wound {
        Wound {
            location,
            kind: WoundKind::Blunt,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: 0,
            inflicted_by: None,
        }
    }

    #[test]
    fn canonical_leg_wound_speed_table() {
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Intact), 1.0);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Bruise), 1.0);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Abrasion), 0.9);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Laceration), 0.7);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Fracture), 0.4);
        assert_eq!(leg_wound_to_speed(LegWoundGrade::Severed), 0.0);
    }

    // ── plan-race-system-v1 P0b：LegL/LegR 必须在 humanoid.json 标 Locomotion，否则
    // combined_leg_factor 的 PartConsequence 分发会静默漏掉腿伤 ──────────────────

    #[test]
    fn leg_l_and_leg_r_are_tagged_locomotion_in_humanoid_plan() {
        let plan = crate::body_plan::humanoid_plan_static();
        for part in [BodyPart::LegL, BodyPart::LegR] {
            let id = crate::body_plan::legacy_body_part_to_id(part);
            let def = plan
                .parts
                .iter()
                .find(|def| def.id == id)
                .unwrap_or_else(|| panic!("humanoid.json must declare part {id}"));
            assert_eq!(
                def.consequence,
                PartConsequence::Locomotion,
                "{part:?} (id={id}) must be tagged Locomotion in humanoid.json for combined_leg_factor to see it"
            );
        }
    }

    #[test]
    fn combined_leg_factor_ignores_non_locomotion_wounds_even_when_severed() {
        // Chest/Arm 伤势即便判 Severed 也不该压低腿部速度系数。
        let wounds = Wounds {
            entries: vec![wound(BodyPart::Chest, 99.0), wound(BodyPart::ArmR, 99.0)],
            ..Default::default()
        };
        assert_eq!(combined_leg_factor(&wounds), 1.0);
    }

    #[test]
    fn combined_factor_takes_worst_leg() {
        let wounds = Wounds {
            entries: vec![
                wound(BodyPart::LegL, 18.0),
                wound(BodyPart::LegR, 42.0),
                wound(BodyPart::ArmL, 99.0),
            ],
            ..Default::default()
        };

        assert_eq!(combined_leg_factor(&wounds), 0.4);
    }

    #[test]
    fn healed_legs_return_to_normal() {
        assert_eq!(combined_leg_factor(&Wounds::default()), 1.0);
    }
}
