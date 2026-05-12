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
    let left = worst_wound_grade(wounds, BodyPart::LegL);
    let right = worst_wound_grade(wounds, BodyPart::LegR);
    leg_wound_to_speed(left).min(leg_wound_to_speed(right))
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
