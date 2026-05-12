use std::collections::HashMap;

use valence::prelude::Entity;

use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{ColorKind, Cultivation, MeridianSystem, QiColor};
use crate::cultivation::insight_apply::InsightModifiers;
use crate::cultivation::known_techniques::{technique_definition, KnownTechniques};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::technique_scroll::{can_learn_technique, ScrollReadOutcome};

pub const OBSERVE_RANGE_BLOCKS: f64 = 16.0;
pub const OBSERVE_COOLDOWN_TICKS: u64 = 20 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechniqueGrade {
    Yellow,
    Profound,
    Earth,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObserveOutcome {
    Eligible { chance: f64 },
    OutOfRange,
    NoLineOfSight,
    OnCooldown,
    EarthGradeBlocked,
    LearnBlocked(ScrollReadOutcome),
    UnknownTechnique,
}

#[derive(Debug, Default)]
pub struct ObserveCooldowns {
    last_roll_at: HashMap<(Entity, Entity, String), u64>,
}

impl ObserveCooldowns {
    pub fn can_roll(
        &self,
        observer: Entity,
        caster: Entity,
        technique_id: &str,
        now_tick: u64,
    ) -> bool {
        self.last_roll_at
            .get(&(observer, caster, technique_id.to_string()))
            .is_none_or(|last| now_tick.saturating_sub(*last) >= OBSERVE_COOLDOWN_TICKS)
    }

    pub fn record_roll(
        &mut self,
        observer: Entity,
        caster: Entity,
        technique_id: impl Into<String>,
        now_tick: u64,
    ) {
        self.last_roll_at
            .insert((observer, caster, technique_id.into()), now_tick);
    }
}

pub fn observe_learn_chance(
    technique_grade: TechniqueGrade,
    observer_color: &QiColor,
    practice_log: Option<&PracticeLog>,
    insight_modifiers: &InsightModifiers,
) -> f64 {
    let base = match technique_grade {
        TechniqueGrade::Yellow => 0.05,
        TechniqueGrade::Profound => 0.01,
        TechniqueGrade::Earth | TechniqueGrade::Other => 0.0,
    };
    let color_bonus = if observer_color.main == ColorKind::Intricate {
        1.5
    } else {
        1.0
    };
    let practice_bonus = practice_log
        .and_then(|log| log.weights.get(&ColorKind::Intricate).copied())
        .map(|weight| (weight / 100.0).min(0.5) + 1.0)
        .unwrap_or(1.0);
    let insight_bonus = 1.0 + insight_modifiers.observe_chance_bonus.max(0.0);

    (base * color_bonus * practice_bonus * insight_bonus).min(0.15)
}

pub fn evaluate_observe_attempt(
    known: &KnownTechniques,
    cultivation: &Cultivation,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    learner: ObserveLearnerContext,
    ctx: ObserveAttemptContext,
) -> ObserveOutcome {
    if ctx.distance_blocks > OBSERVE_RANGE_BLOCKS {
        return ObserveOutcome::OutOfRange;
    }
    if !ctx.has_line_of_sight {
        return ObserveOutcome::NoLineOfSight;
    }
    if !ctx.cooldown_ready {
        return ObserveOutcome::OnCooldown;
    }
    let Some(definition) = technique_definition(ctx.technique_id) else {
        return ObserveOutcome::UnknownTechnique;
    };
    let grade = parse_grade(definition.grade);
    if grade == TechniqueGrade::Earth {
        return ObserveOutcome::EarthGradeBlocked;
    }
    let learn = can_learn_technique(known, cultivation, meridians, severed, ctx.technique_id);
    if !matches!(learn, ScrollReadOutcome::Learned) {
        return ObserveOutcome::LearnBlocked(learn);
    }
    ObserveOutcome::Eligible {
        chance: observe_learn_chance(
            grade,
            learner.observer_color,
            learner.practice_log,
            learner.insight_modifiers,
        ),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ObserveLearnerContext<'a> {
    pub observer_color: &'a QiColor,
    pub practice_log: Option<&'a PracticeLog>,
    pub insight_modifiers: &'a InsightModifiers,
}

#[derive(Debug, Clone, Copy)]
pub struct ObserveAttemptContext<'a> {
    pub technique_id: &'a str,
    pub distance_blocks: f64,
    pub has_line_of_sight: bool,
    pub cooldown_ready: bool,
}

pub fn parse_grade(raw: &str) -> TechniqueGrade {
    match raw {
        "yellow" => TechniqueGrade::Yellow,
        "profound" => TechniqueGrade::Profound,
        "earth" => TechniqueGrade::Earth,
        _ => TechniqueGrade::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{MeridianId, Realm};

    fn intricate_color() -> QiColor {
        QiColor {
            main: ColorKind::Intricate,
            ..Default::default()
        }
    }

    #[test]
    fn observe_learn_chance_yellow() {
        let chance = observe_learn_chance(
            TechniqueGrade::Yellow,
            &intricate_color(),
            None,
            &InsightModifiers::new(),
        );
        assert!((chance - 0.075).abs() < 1e-9);
    }

    #[test]
    fn observe_learn_chance_profound() {
        let chance = observe_learn_chance(
            TechniqueGrade::Profound,
            &QiColor::default(),
            None,
            &InsightModifiers::new(),
        );
        assert!((chance - 0.01).abs() < 1e-9);
    }

    #[test]
    fn observe_learn_chance_earth_zero() {
        assert_eq!(
            observe_learn_chance(
                TechniqueGrade::Earth,
                &intricate_color(),
                None,
                &InsightModifiers::new()
            ),
            0.0
        );
    }

    #[test]
    fn observe_learn_chance_cap() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Intricate, 10_000.0);
        let mut modifiers = InsightModifiers::new();
        modifiers.observe_chance_bonus = 20.0;

        let chance = observe_learn_chance(
            TechniqueGrade::Yellow,
            &intricate_color(),
            Some(&log),
            &modifiers,
        );

        assert_eq!(chance, 0.15);
    }

    #[test]
    fn observe_cooldown_60s() {
        let observer = Entity::from_raw(1);
        let caster = Entity::from_raw(2);
        let mut cooldowns = ObserveCooldowns::default();

        assert!(cooldowns.can_roll(observer, caster, "woliu.burst", 10));
        cooldowns.record_roll(observer, caster, "woliu.burst", 10);
        assert!(!cooldowns.can_roll(observer, caster, "woliu.burst", 10 + 20 * 59));
        assert!(cooldowns.can_roll(observer, caster, "woliu.burst", 10 + 20 * 60));
    }

    #[test]
    fn observe_requires_line_of_sight() {
        let outcome = evaluate_observe_attempt(
            &KnownTechniques::default(),
            &Cultivation::default(),
            &MeridianSystem::default(),
            None,
            ObserveLearnerContext {
                observer_color: &QiColor::default(),
                practice_log: None,
                insight_modifiers: &InsightModifiers::new(),
            },
            ObserveAttemptContext {
                technique_id: "woliu.burst",
                distance_blocks: 4.0,
                has_line_of_sight: false,
                cooldown_ready: true,
            },
        );
        assert_eq!(outcome, ObserveOutcome::NoLineOfSight);
    }

    #[test]
    fn observe_requires_realm() {
        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;
        meridians.get_mut(MeridianId::Lung).integrity = 1.0;

        let outcome = evaluate_observe_attempt(
            &KnownTechniques::default(),
            &Cultivation {
                realm: Realm::Awaken,
                ..Default::default()
            },
            &meridians,
            None,
            ObserveLearnerContext {
                observer_color: &QiColor::default(),
                practice_log: None,
                insight_modifiers: &InsightModifiers::new(),
            },
            ObserveAttemptContext {
                technique_id: "woliu.vortex",
                distance_blocks: 4.0,
                has_line_of_sight: true,
                cooldown_ready: true,
            },
        );

        assert!(matches!(
            outcome,
            ObserveOutcome::LearnBlocked(ScrollReadOutcome::RealmTooLow { .. })
        ));
    }
}
