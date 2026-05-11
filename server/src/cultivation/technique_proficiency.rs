use valence::prelude::{bevy_ecs, Entity, Event};

use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::{BiographyEntry, LifeRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProficiencySource {
    CombatCast,
    PracticeSession,
    BackfireSurvived,
}

#[derive(Debug, Clone, Event, PartialEq)]
pub struct TechniqueMasteredEvent {
    pub player: Entity,
    pub technique_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoliuProficiencyScalars {
    pub backfire_multiplier: f64,
    pub qi_cost_multiplier: f64,
    pub vortex_delta: f64,
    pub radius_multiplier: f32,
    pub cast_ticks_multiplier: f32,
}

pub fn proficiency_gain(
    current: f32,
    source: ProficiencySource,
    color_match: bool,
    meridian_health: f32,
) -> f32 {
    let base = match source {
        ProficiencySource::CombatCast => 0.008,
        ProficiencySource::PracticeSession => 0.003,
        ProficiencySource::BackfireSurvived => 0.015,
    };
    let color_mul = if color_match { 1.5 } else { 1.0 };
    let meridian_mul = 0.5 + meridian_health.clamp(0.0, 1.0) * 0.5;
    let diminish = 1.0 - current.clamp(0.0, 1.0) * 0.8;

    (base * color_mul * meridian_mul * diminish).max(0.001)
}

pub fn apply_proficiency_gain(
    known: &mut KnownTechniques,
    technique_id: &str,
    gain: f32,
) -> Option<TechniqueMasteredTransition> {
    let entry = known
        .entries
        .iter_mut()
        .find(|entry| entry.id == technique_id)?;
    let before = entry.proficiency.clamp(0.0, 1.0);
    entry.proficiency = (before + gain.max(0.0)).clamp(0.0, 1.0);
    Some(TechniqueMasteredTransition {
        before,
        after: entry.proficiency,
        mastered_now: before < 1.0 && entry.proficiency >= 1.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TechniqueMasteredTransition {
    pub before: f32,
    pub after: f32,
    pub mastered_now: bool,
}

pub fn woliu_scalars_for_proficiency(proficiency: f32) -> WoliuProficiencyScalars {
    let prof = proficiency.clamp(0.0, 1.0);
    WoliuProficiencyScalars {
        backfire_multiplier: 2.0 - 1.6 * f64::from(prof),
        qi_cost_multiplier: 1.3 - 0.45 * f64::from(prof),
        vortex_delta: 0.08 + 0.04 * f64::from(prof),
        radius_multiplier: 0.8 + 0.3 * prof,
        cast_ticks_multiplier: 1.2 - 0.3 * prof,
    }
}

pub fn record_mastered_life_event(
    life_record: &mut LifeRecord,
    technique_id: &str,
    region: &str,
    tick: u64,
) {
    life_record.biography.push(BiographyEntry::InsightTaken {
        trigger: "technique_mastered".to_string(),
        choice: technique_id.to_string(),
        alignment: Some(region.to_string()),
        cost_kind: None,
        tick,
    });
}

pub fn practice_session_gain(
    zone_qi: f64,
    current: f32,
    color_match: bool,
    meridian_health: f32,
) -> f32 {
    let zone_mul = if zone_qi < -0.5 {
        2.0
    } else if zone_qi < 0.0 {
        1.5
    } else {
        1.0
    };
    proficiency_gain(
        current,
        ProficiencySource::PracticeSession,
        color_match,
        meridian_health,
    ) * zone_mul
}

pub fn practice_session_qi_cost_per_tick() -> f64 {
    2.0
}

pub fn should_exit_practice_session(moved: bool, attacked: bool, qi_ratio: f64) -> bool {
    moved || attacked || qi_ratio < 0.10
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::known_techniques::KnownTechnique;

    #[test]
    fn gain_formula_combat() {
        let gain = proficiency_gain(0.0, ProficiencySource::CombatCast, true, 1.0);
        assert!((gain - 0.012).abs() < 1e-6);
    }

    #[test]
    fn gain_diminishing() {
        let low = proficiency_gain(0.0, ProficiencySource::CombatCast, false, 1.0);
        let high = proficiency_gain(0.9, ProficiencySource::CombatCast, false, 1.0);
        assert!(high < low);
    }

    #[test]
    fn color_match_bonus() {
        let normal = proficiency_gain(0.2, ProficiencySource::CombatCast, false, 1.0);
        let matched = proficiency_gain(0.2, ProficiencySource::CombatCast, true, 1.0);
        assert!((matched / normal - 1.5).abs() < 1e-5);
    }

    #[test]
    fn meridian_health_impact() {
        let weak = proficiency_gain(0.0, ProficiencySource::CombatCast, false, 0.0);
        let healthy = proficiency_gain(0.0, ProficiencySource::CombatCast, false, 1.0);
        assert!((weak / healthy - 0.5).abs() < 1e-6);
    }

    #[test]
    fn practice_session_gain_uses_zone_bonus() {
        let normal = practice_session_gain(0.9, 0.0, false, 1.0);
        let negative = practice_session_gain(-0.1, 0.0, false, 1.0);
        let deep = practice_session_gain(-0.6, 0.0, false, 1.0);
        assert!((negative / normal - 1.5).abs() < 1e-6);
        assert!((deep / normal - 2.0).abs() < 1e-6);
    }

    #[test]
    fn practice_session_qi_cost() {
        assert_eq!(practice_session_qi_cost_per_tick(), 2.0);
    }

    #[test]
    fn practice_session_exits_on_move() {
        assert!(should_exit_practice_session(true, false, 1.0));
        assert!(should_exit_practice_session(false, true, 1.0));
        assert!(should_exit_practice_session(false, false, 0.09));
        assert!(!should_exit_practice_session(false, false, 0.10));
    }

    #[test]
    fn backfire_chance_scales() {
        assert!((woliu_scalars_for_proficiency(0.0).backfire_multiplier - 2.0).abs() < 1e-9);
        assert!((woliu_scalars_for_proficiency(1.0).backfire_multiplier - 0.4).abs() < 1e-9);
    }

    #[test]
    fn qi_cost_scales() {
        assert!((woliu_scalars_for_proficiency(0.0).qi_cost_multiplier - 1.3).abs() < 1e-9);
        assert!((woliu_scalars_for_proficiency(1.0).qi_cost_multiplier - 0.85).abs() < 1e-9);
    }

    #[test]
    fn vortex_delta_scales() {
        assert!((woliu_scalars_for_proficiency(0.0).vortex_delta - 0.08).abs() < 1e-9);
        assert!((woliu_scalars_for_proficiency(1.0).vortex_delta - 0.12).abs() < 1e-9);
    }

    #[test]
    fn mastered_event_at_1_0() {
        let mut known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "woliu.vortex".to_string(),
                proficiency: 0.99,
                active: true,
            }],
        };
        let transition = apply_proficiency_gain(&mut known, "woliu.vortex", 0.02).unwrap();
        assert_eq!(transition.after, 1.0);
        assert!(transition.mastered_now);
    }
}
