use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

use crate::combat::components::{Lifecycle, RevivalDecision, REVIVE_WEAKENED_TICKS};
use crate::cultivation::lifespan::{DeathRegistry, ZoneDeathKind};
use crate::schema::death_cinematic::{
    DeathCinematicPhaseV1, DeathCinematicRollV1, DeathCinematicS2cV1, DeathCinematicZoneKindV1,
    DeathRollResultV1,
};

const PREDEATH_TICKS: u64 = 60;
const DEATH_MOMENT_TICKS: u64 = 20;
const FULL_ROLL_TICKS: u64 = 80;
const SHORT_ROLL_TICKS: u64 = 40;
const FULL_INSIGHT_TICKS: u64 = 120;
const SHORT_INSIGHT_TICKS: u64 = 60;
const DARKNESS_TICKS: u64 = 40;
const REBIRTH_TICKS: u64 = 60;

#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct DeathCinematic {
    pub character_id: String,
    pub started_at_tick: u64,
    pub roll: DeathCinematicRollV1,
    pub insight_text: Vec<String>,
    pub is_final: bool,
    pub death_number: u32,
    pub zone_kind: DeathCinematicZoneKindV1,
    pub tsy_death: bool,
    pub rebirth_weakened_ticks: u64,
    pub skip_predeath: bool,
    phase_durations: [u64; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeathCinematicInit {
    pub character_id: String,
    pub started_at_tick: u64,
    pub roll: DeathCinematicRollV1,
    pub insight_text: Vec<String>,
    pub is_final: bool,
    pub death_number: u32,
    pub zone_kind: DeathCinematicZoneKindV1,
    pub tsy_death: bool,
}

impl DeathCinematic {
    pub fn new(init: DeathCinematicInit) -> Self {
        let skip_predeath = init.death_number >= 5 && !init.is_final;
        let shortened = init.death_number >= 2 && !init.is_final;
        let phase_durations = [
            if skip_predeath { 0 } else { PREDEATH_TICKS },
            if skip_predeath { 0 } else { DEATH_MOMENT_TICKS },
            if shortened {
                SHORT_ROLL_TICKS
            } else {
                FULL_ROLL_TICKS
            },
            if shortened {
                SHORT_INSIGHT_TICKS
            } else {
                FULL_INSIGHT_TICKS
            },
            DARKNESS_TICKS,
            REBIRTH_TICKS,
        ];

        Self {
            character_id: init.character_id,
            started_at_tick: init.started_at_tick,
            roll: init.roll,
            insight_text: init.insight_text,
            is_final: init.is_final,
            death_number: init.death_number.max(1),
            zone_kind: init.zone_kind,
            tsy_death: init.tsy_death,
            rebirth_weakened_ticks: REVIVE_WEAKENED_TICKS,
            skip_predeath,
            phase_durations,
        }
    }

    pub fn total_duration_ticks(&self) -> u64 {
        self.phase_durations.iter().copied().sum()
    }

    pub fn phase_at(&self, now_tick: u64) -> (DeathCinematicPhaseV1, u64, u64) {
        let mut elapsed = now_tick.saturating_sub(self.started_at_tick);
        let phases = [
            DeathCinematicPhaseV1::Predeath,
            DeathCinematicPhaseV1::DeathMoment,
            DeathCinematicPhaseV1::Roll,
            DeathCinematicPhaseV1::InsightOverlay,
            DeathCinematicPhaseV1::Darkness,
            DeathCinematicPhaseV1::Rebirth,
        ];

        for (phase, duration) in phases.into_iter().zip(self.phase_durations) {
            if duration == 0 {
                continue;
            }
            if elapsed < duration {
                return (phase, elapsed, duration);
            }
            elapsed = elapsed.saturating_sub(duration);
        }

        (DeathCinematicPhaseV1::Rebirth, REBIRTH_TICKS, REBIRTH_TICKS)
    }

    pub fn snapshot(&self, now_tick: u64) -> DeathCinematicS2cV1 {
        let (phase, phase_tick, phase_duration_ticks) = self.phase_at(now_tick);
        DeathCinematicS2cV1 {
            v: 1,
            character_id: self.character_id.clone(),
            phase,
            phase_tick,
            phase_duration_ticks,
            total_elapsed_ticks: now_tick.saturating_sub(self.started_at_tick),
            total_duration_ticks: self.total_duration_ticks(),
            roll: self.roll,
            insight_text: self.insight_text.clone(),
            is_final: self.is_final,
            death_number: self.death_number,
            zone_kind: self.zone_kind,
            tsy_death: self.tsy_death,
            rebirth_weakened_ticks: self.rebirth_weakened_ticks,
            skip_predeath: self.skip_predeath,
        }
    }
}

pub fn build_death_cinematic(
    lifecycle: &Lifecycle,
    death_registry: Option<&DeathRegistry>,
    decision: Option<RevivalDecision>,
    zone_kind: ZoneDeathKind,
    cause: &str,
    insight_text: Vec<String>,
    started_at_tick: u64,
) -> DeathCinematic {
    let death_number = death_registry
        .map_or(lifecycle.death_count, |registry| {
            registry.death_count.max(lifecycle.death_count)
        })
        .max(1);
    let is_final = decision.is_none();
    let probability = decision
        .map_or(0.0, RevivalDecision::chance_shown)
        .clamp(0.0, 1.0);
    let result = match decision {
        None => DeathRollResultV1::Final,
        Some(RevivalDecision::Fortune { .. }) => DeathRollResultV1::Survive,
        Some(RevivalDecision::Tribulation { .. }) => DeathRollResultV1::Pending,
    };

    DeathCinematic::new(DeathCinematicInit {
        character_id: lifecycle.character_id.clone(),
        started_at_tick,
        roll: DeathCinematicRollV1 {
            probability,
            threshold: probability,
            luck_value: probability,
            result,
        },
        insight_text: if insight_text.is_empty() {
            vec![fallback_insight_line(cause, zone_kind)]
        } else {
            insight_text
        },
        is_final,
        death_number,
        zone_kind: map_zone_kind(zone_kind),
        tsy_death: is_tsy_death(cause, zone_kind),
    })
}

fn fallback_insight_line(cause: &str, zone_kind: ZoneDeathKind) -> String {
    match zone_kind {
        ZoneDeathKind::Death | ZoneDeathKind::Negative => "坍缩渊，概不赊欠。".to_string(),
        ZoneDeathKind::Ordinary if cause.contains("tribulation") => {
            "劫数记下了这一笔。".to_string()
        }
        ZoneDeathKind::Ordinary => "尘归尘，劫未尽。".to_string(),
    }
}

fn is_tsy_death(cause: &str, zone_kind: ZoneDeathKind) -> bool {
    matches!(zone_kind, ZoneDeathKind::Death | ZoneDeathKind::Negative)
        || cause.contains("tsy")
        || cause.contains("collapse")
}

pub fn map_zone_kind(zone_kind: ZoneDeathKind) -> DeathCinematicZoneKindV1 {
    match zone_kind {
        ZoneDeathKind::Ordinary => DeathCinematicZoneKindV1::Ordinary,
        ZoneDeathKind::Death => DeathCinematicZoneKindV1::Death,
        ZoneDeathKind::Negative => DeathCinematicZoneKindV1::Negative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cinematic_phase_sequence() {
        let cinematic = DeathCinematic::new(DeathCinematicInit {
            character_id: "offline:Azure".to_string(),
            started_at_tick: 100,
            roll: DeathCinematicRollV1 {
                probability: 0.65,
                threshold: 0.65,
                luck_value: 0.65,
                result: DeathRollResultV1::Pending,
            },
            insight_text: vec!["遗念".to_string()],
            is_final: false,
            death_number: 1,
            zone_kind: DeathCinematicZoneKindV1::Ordinary,
            tsy_death: false,
        });

        assert_eq!(
            cinematic.phase_at(100),
            (DeathCinematicPhaseV1::Predeath, 0, PREDEATH_TICKS)
        );
        assert_eq!(
            cinematic.phase_at(160),
            (DeathCinematicPhaseV1::DeathMoment, 0, DEATH_MOMENT_TICKS)
        );
        assert_eq!(
            cinematic.phase_at(180),
            (DeathCinematicPhaseV1::Roll, 0, FULL_ROLL_TICKS)
        );
        assert_eq!(
            cinematic.phase_at(260),
            (DeathCinematicPhaseV1::InsightOverlay, 0, FULL_INSIGHT_TICKS)
        );
        assert_eq!(
            cinematic.phase_at(380),
            (DeathCinematicPhaseV1::Darkness, 0, DARKNESS_TICKS)
        );
        assert_eq!(
            cinematic.phase_at(420),
            (DeathCinematicPhaseV1::Rebirth, 0, REBIRTH_TICKS)
        );
    }

    #[test]
    fn fifth_non_final_death_skips_predeath_and_death_moment() {
        let cinematic = build_death_cinematic(
            &Lifecycle {
                character_id: "offline:Azure".to_string(),
                death_count: 5,
                ..Default::default()
            },
            None,
            Some(RevivalDecision::Tribulation { chance: 0.25 }),
            ZoneDeathKind::Ordinary,
            "combat",
            Vec::new(),
            10,
        );

        assert!(cinematic.skip_predeath);
        assert_eq!(
            cinematic.phase_at(10),
            (DeathCinematicPhaseV1::Roll, 0, SHORT_ROLL_TICKS)
        );
    }

    #[test]
    fn tsy_death_adds_drop_warning_insight() {
        let cinematic = build_death_cinematic(
            &Lifecycle {
                character_id: "offline:Azure".to_string(),
                death_count: 1,
                ..Default::default()
            },
            None,
            None,
            ZoneDeathKind::Negative,
            "tsy_collapsed",
            Vec::new(),
            10,
        );

        assert!(cinematic.is_final);
        assert!(cinematic.tsy_death);
        assert_eq!(cinematic.roll.result, DeathRollResultV1::Final);
        assert!(cinematic.insight_text[0].contains("概不赊欠"));
    }
}
