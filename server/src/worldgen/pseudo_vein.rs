use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::schema::pseudo_vein::{
    PseudoVeinDissipateEventV1, PseudoVeinQiRedistributionV1, PseudoVeinSeasonV1,
    PseudoVeinSnapshotV1,
};
use crate::schema::tribulation::{TribulationEventV1, TribulationPhaseV1};

pub const TICKS_PER_SECOND: u64 = 20;
pub const TICKS_PER_MINUTE: u64 = TICKS_PER_SECOND * 60;
pub const TICKS_PER_HOUR: u64 = TICKS_PER_MINUTE * 60;
pub const PSEUDO_VEIN_INITIAL_QI: f64 = 0.60;
pub const PSEUDO_VEIN_HUNGRY_RING_BASE_QI: f64 = 0.08;
pub const PSEUDO_VEIN_HUNGRY_RING_REFILL_MAX: f64 = 0.08;
pub const PSEUDO_VEIN_TRIBULATION_BONUS: f64 = 0.30;
pub const PSEUDO_VEIN_TRIBULATION_MARK_TICKS: u64 = 24 * TICKS_PER_HOUR;
pub const PSEUDO_VEIN_QI_TURBULENCE_KIND: u8 = 2;
pub const PSEUDO_VEIN_NEG_PRESSURE_MIN: f64 = 0.4;
pub const PSEUDO_VEIN_NEG_PRESSURE_MAX: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoVeinLifecycle {
    pub spawned_at: u64,
    pub decay_rate: f64,
    pub occupant_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinRuntimeState {
    pub id: String,
    pub center_xz: [f64; 2],
    pub season_at_spawn: PseudoVeinSeasonV1,
    pub lifecycle: PseudoVeinLifecycle,
    pub last_tick: u64,
    pub qi_current: f64,
    pub total_qi_consumed: f64,
    pub warning_sent: bool,
    pub dissipated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinAdvance {
    pub snapshot: PseudoVeinSnapshotV1,
    pub warning_threshold_crossed: bool,
    pub dissipate_event: Option<PseudoVeinDissipateEventV1>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoVeinStormHotspot {
    pub center_xz: [f64; 2],
    pub neg_pressure: f64,
    pub anomaly_kind: u8,
    pub anomaly_intensity: f64,
    pub active_until_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OccupantRealm {
    pub player_id: String,
    pub realm_rank: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PseudoVeinTribulationMark {
    pub target_player_id: String,
    pub expires_at_tick: u64,
    pub bonus_probability: f64,
}

impl PseudoVeinRuntimeState {
    pub fn new(
        id: impl Into<String>,
        center_xz: [f64; 2],
        spawned_at: u64,
        season_at_spawn: PseudoVeinSeasonV1,
    ) -> Self {
        let occupant_count = 0;
        Self {
            id: id.into(),
            center_xz,
            season_at_spawn,
            lifecycle: PseudoVeinLifecycle {
                spawned_at,
                decay_rate: decay_rate_per_tick(occupant_count),
                occupant_count,
            },
            last_tick: spawned_at,
            qi_current: PSEUDO_VEIN_INITIAL_QI,
            total_qi_consumed: 0.0,
            warning_sent: false,
            dissipated: false,
        }
    }

    pub fn advance(&mut self, current_tick: u64, occupants: Vec<String>) -> PseudoVeinAdvance {
        let occupant_count = occupants.len();
        let elapsed = current_tick.saturating_sub(self.last_tick);
        self.lifecycle.occupant_count = occupant_count;
        self.lifecycle.decay_rate = decay_rate_per_tick(occupant_count);

        if !self.dissipated && elapsed > 0 {
            let consumed = (elapsed as f64 * self.lifecycle.decay_rate).min(self.qi_current);
            self.qi_current = (self.qi_current - consumed).max(0.0);
            self.total_qi_consumed += consumed;
            self.last_tick = current_tick;
        }

        let warning_threshold_crossed = !self.warning_sent && self.qi_current <= 0.30;
        if warning_threshold_crossed {
            self.warning_sent = true;
        }

        let dissipate_event = if !self.dissipated && self.qi_current <= 0.0 {
            self.dissipated = true;
            Some(build_dissipate_event(
                &self.id,
                self.center_xz,
                current_tick,
            ))
        } else {
            None
        };

        PseudoVeinAdvance {
            snapshot: self.snapshot(current_tick, occupants),
            warning_threshold_crossed,
            dissipate_event,
        }
    }

    pub fn snapshot(&self, current_tick: u64, occupants: Vec<String>) -> PseudoVeinSnapshotV1 {
        PseudoVeinSnapshotV1 {
            v: 1,
            id: self.id.clone(),
            center_xz: self.center_xz,
            spirit_qi_current: round3(self.qi_current),
            occupants,
            spawned_at_tick: self.lifecycle.spawned_at,
            estimated_decay_at_tick: current_tick
                + estimated_ticks_until_decay(self.qi_current, self.lifecycle.occupant_count),
            season_at_spawn: self.season_at_spawn,
        }
    }
}

pub fn decay_multiplier_for_occupants(occupant_count: usize) -> f64 {
    match occupant_count {
        0 | 1 => 1.0,
        2 => 1.4,
        3 => 1.8,
        4 => 2.5,
        _ => 3.5,
    }
}

pub fn decay_lifetime_minutes_for_occupants(occupant_count: usize) -> f64 {
    if occupant_count <= 1 {
        90.0
    } else {
        54.0 / decay_multiplier_for_occupants(occupant_count)
    }
}

pub fn decay_rate_per_tick(occupant_count: usize) -> f64 {
    let lifetime_ticks =
        decay_lifetime_minutes_for_occupants(occupant_count) * TICKS_PER_MINUTE as f64;
    PSEUDO_VEIN_INITIAL_QI / lifetime_ticks
}

pub fn estimated_ticks_until_decay(qi_current: f64, occupant_count: usize) -> u64 {
    if qi_current <= 0.0 {
        return 0;
    }
    (qi_current / decay_rate_per_tick(occupant_count)).ceil() as u64
}

pub fn season_spawn_rate_multiplier(season: PseudoVeinSeasonV1) -> f64 {
    match season {
        PseudoVeinSeasonV1::SummerToWinter | PseudoVeinSeasonV1::WinterToSummer => 2.0,
        PseudoVeinSeasonV1::Summer | PseudoVeinSeasonV1::Winter => 1.0,
    }
}

pub fn build_dissipate_event(
    id: &str,
    center_xz: [f64; 2],
    current_tick: u64,
) -> PseudoVeinDissipateEventV1 {
    let storm_duration_ticks = storm_duration_ticks(id, current_tick);
    PseudoVeinDissipateEventV1 {
        v: 1,
        id: id.to_string(),
        center_xz,
        storm_anchors: storm_anchors(id, center_xz, current_tick),
        storm_duration_ticks,
        qi_redistribution: PseudoVeinQiRedistributionV1 {
            refill_to_hungry_ring: 0.7,
            collected_by_tiandao: 0.3,
        },
    }
}

pub fn storm_hotspots_from_event(
    event: &PseudoVeinDissipateEventV1,
    started_at_tick: u64,
) -> Vec<PseudoVeinStormHotspot> {
    let active_until_tick = started_at_tick + event.storm_duration_ticks;
    event
        .storm_anchors
        .iter()
        .enumerate()
        .map(|(idx, center_xz)| PseudoVeinStormHotspot {
            center_xz: *center_xz,
            neg_pressure: round3(PSEUDO_VEIN_NEG_PRESSURE_MIN + (idx as f64 % 3.0) * 0.1)
                .min(PSEUDO_VEIN_NEG_PRESSURE_MAX),
            anomaly_kind: PSEUDO_VEIN_QI_TURBULENCE_KIND,
            anomaly_intensity: 0.75,
            active_until_tick,
        })
        .collect()
}

pub fn active_storm_hotspots(
    hotspots: &[PseudoVeinStormHotspot],
    current_tick: u64,
) -> Vec<PseudoVeinStormHotspot> {
    hotspots
        .iter()
        .copied()
        .filter(|hotspot| current_tick < hotspot.active_until_tick)
        .collect()
}

pub fn hungry_ring_refill_delta(dissipated_at_tick: u64, current_tick: u64) -> f64 {
    let elapsed = current_tick.saturating_sub(dissipated_at_tick);
    if elapsed >= TICKS_PER_HOUR {
        return 0.0;
    }
    let remaining = 1.0 - (elapsed as f64 / TICKS_PER_HOUR as f64);
    round3(PSEUDO_VEIN_HUNGRY_RING_REFILL_MAX * remaining)
}

pub fn hungry_ring_qi_after_refill(dissipated_at_tick: u64, current_tick: u64) -> f64 {
    round3(
        PSEUDO_VEIN_HUNGRY_RING_BASE_QI
            + hungry_ring_refill_delta(dissipated_at_tick, current_tick),
    )
}

pub fn shrine_rejection_message() -> &'static str {
    "此地灵脉飘忽，龛石不立"
}

pub fn can_place_shrine_in_pseudo_vein(state: &PseudoVeinRuntimeState) -> Result<(), &'static str> {
    if state.dissipated {
        Ok(())
    } else {
        Err(shrine_rejection_message())
    }
}

pub fn highest_realm_occupant(occupants: &[OccupantRealm]) -> Option<&OccupantRealm> {
    occupants.iter().max_by_key(|occupant| occupant.realm_rank)
}

pub fn build_tribulation_bait_mark(
    occupants: &[OccupantRealm],
    qi_consumed: f64,
    current_tick: u64,
) -> Option<PseudoVeinTribulationMark> {
    if qi_consumed < PSEUDO_VEIN_INITIAL_QI {
        return None;
    }
    let target = highest_realm_occupant(occupants)?;
    Some(PseudoVeinTribulationMark {
        target_player_id: target.player_id.clone(),
        expires_at_tick: current_tick + PSEUDO_VEIN_TRIBULATION_MARK_TICKS,
        bonus_probability: PSEUDO_VEIN_TRIBULATION_BONUS,
    })
}

pub fn effective_tribulation_probability(
    base_probability: f64,
    mark: Option<&PseudoVeinTribulationMark>,
    current_tick: u64,
) -> f64 {
    let bonus = mark
        .filter(|mark| current_tick <= mark.expires_at_tick)
        .map(|mark| mark.bonus_probability)
        .unwrap_or(0.0);
    (base_probability + bonus).clamp(0.0, 1.0)
}

pub fn tribulation_bait_event(
    zone_id: &str,
    center_xz: [f64; 2],
    mark: &PseudoVeinTribulationMark,
) -> TribulationEventV1 {
    let mut event = TribulationEventV1::targeted(
        TribulationPhaseV1::Omen,
        Some(zone_id.to_string()),
        Some([center_xz[0], 64.0, center_xz[1]]),
    );
    event.char_id = Some(mark.target_player_id.clone());
    event
}

fn storm_anchors(id: &str, center_xz: [f64; 2], current_tick: u64) -> Vec<[f64; 2]> {
    let seed = hash_seed(&(id, current_tick));
    let count = 1 + (seed % 3) as usize;
    (0..count)
        .map(|idx| {
            let angle_degrees = ((seed >> (idx * 9)) % 360) as f64;
            let angle = angle_degrees.to_radians();
            let radius = 100.0 + (((seed >> (idx * 7)) % 101) as f64);
            [
                round3(center_xz[0] + angle.cos() * radius),
                round3(center_xz[1] + angle.sin() * radius),
            ]
        })
        .collect()
}

fn storm_duration_ticks(id: &str, current_tick: u64) -> u64 {
    6000 + (hash_seed(&(id, current_tick, "duration")) % 6001)
}

fn hash_seed<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::tribulation::TribulationKindV1;

    #[test]
    fn decay_multiplier_pins_all_occupant_bands() {
        assert_eq!(decay_multiplier_for_occupants(0), 1.0);
        assert_eq!(decay_multiplier_for_occupants(1), 1.0);
        assert_eq!(decay_multiplier_for_occupants(2), 1.4);
        assert_eq!(decay_multiplier_for_occupants(3), 1.8);
        assert_eq!(decay_multiplier_for_occupants(4), 2.5);
        assert_eq!(decay_multiplier_for_occupants(5), 3.5);
        assert_eq!(decay_multiplier_for_occupants(99), 3.5);
    }

    #[test]
    fn three_players_dissipate_in_thirty_minute_window() {
        let mut state = PseudoVeinRuntimeState::new(
            "pseudo_vein_unit",
            [0.0, 0.0],
            0,
            PseudoVeinSeasonV1::Summer,
        );

        let before = state.advance(
            25 * TICKS_PER_MINUTE,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert!(before.snapshot.spirit_qi_current > 0.0);
        assert!(before.dissipate_event.is_none());

        let after = state.advance(
            30 * TICKS_PER_MINUTE,
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(after.snapshot.spirit_qi_current, 0.0);
        assert!(after.dissipate_event.is_some());
    }

    #[test]
    fn lone_player_dissipates_by_ninety_minutes() {
        let mut state = PseudoVeinRuntimeState::new(
            "pseudo_vein_solo",
            [0.0, 0.0],
            0,
            PseudoVeinSeasonV1::Winter,
        );

        let before = state.advance(80 * TICKS_PER_MINUTE, vec!["solo".to_string()]);
        assert!(before.snapshot.spirit_qi_current > 0.0);
        let after = state.advance(90 * TICKS_PER_MINUTE, vec!["solo".to_string()]);
        assert_eq!(after.snapshot.spirit_qi_current, 0.0);
    }

    #[test]
    fn warning_threshold_fires_at_qi_point_three_once() {
        let mut state = PseudoVeinRuntimeState::new(
            "pseudo_vein_warn",
            [0.0, 0.0],
            0,
            PseudoVeinSeasonV1::Summer,
        );

        let first = state.advance(45 * TICKS_PER_MINUTE, vec!["solo".to_string()]);
        assert!(first.warning_threshold_crossed);
        let second = state.advance(46 * TICKS_PER_MINUTE, vec!["solo".to_string()]);
        assert!(!second.warning_threshold_crossed);
    }

    #[test]
    fn dissipate_event_spawns_one_to_three_qi_turbulence_storms() {
        let event = build_dissipate_event("pseudo_vein_42", [1280.0, -640.0], 30000);

        assert!((1..=3).contains(&event.storm_anchors.len()));
        assert!((6000..=12000).contains(&event.storm_duration_ticks));
        assert_eq!(event.qi_redistribution.refill_to_hungry_ring, 0.7);
        assert_eq!(event.qi_redistribution.collected_by_tiandao, 0.3);

        let hotspots = storm_hotspots_from_event(&event, 30000);
        assert_eq!(hotspots.len(), event.storm_anchors.len());
        assert!(hotspots.iter().all(|hotspot| {
            hotspot.anomaly_kind == PSEUDO_VEIN_QI_TURBULENCE_KIND
                && hotspot.neg_pressure >= PSEUDO_VEIN_NEG_PRESSURE_MIN
                && hotspot.neg_pressure <= PSEUDO_VEIN_NEG_PRESSURE_MAX
        }));
    }

    #[test]
    fn storm_hotspot_override_clears_after_duration() {
        let event = build_dissipate_event("pseudo_vein_clear", [0.0, 0.0], 100);
        let hotspots = storm_hotspots_from_event(&event, 100);

        assert!(!active_storm_hotspots(&hotspots, 100 + event.storm_duration_ticks - 1).is_empty());
        assert!(active_storm_hotspots(&hotspots, 100 + event.storm_duration_ticks).is_empty());
    }

    #[test]
    fn hungry_ring_refill_decays_back_to_baseline_in_one_game_hour() {
        let dissipated_at = 1_000;

        assert_eq!(
            hungry_ring_qi_after_refill(dissipated_at, dissipated_at),
            PSEUDO_VEIN_HUNGRY_RING_BASE_QI + PSEUDO_VEIN_HUNGRY_RING_REFILL_MAX
        );
        assert!(
            hungry_ring_refill_delta(dissipated_at, dissipated_at + 30 * TICKS_PER_MINUTE) > 0.0
        );
        assert_eq!(
            hungry_ring_qi_after_refill(dissipated_at, dissipated_at + TICKS_PER_HOUR),
            PSEUDO_VEIN_HUNGRY_RING_BASE_QI
        );
    }

    #[test]
    fn shrine_placement_is_rejected_while_pseudo_vein_is_active() {
        let mut state = PseudoVeinRuntimeState::new(
            "pseudo_vein_shrine",
            [0.0, 0.0],
            0,
            PseudoVeinSeasonV1::Summer,
        );

        assert_eq!(
            can_place_shrine_in_pseudo_vein(&state),
            Err("此地灵脉飘忽，龛石不立")
        );
        state.dissipated = true;
        assert_eq!(can_place_shrine_in_pseudo_vein(&state), Ok(()));
    }

    #[test]
    fn tide_turn_seasons_double_spawn_rate() {
        assert_eq!(
            season_spawn_rate_multiplier(PseudoVeinSeasonV1::Summer),
            1.0
        );
        assert_eq!(
            season_spawn_rate_multiplier(PseudoVeinSeasonV1::Winter),
            1.0
        );
        assert_eq!(
            season_spawn_rate_multiplier(PseudoVeinSeasonV1::SummerToWinter),
            2.0
        );
        assert_eq!(
            season_spawn_rate_multiplier(PseudoVeinSeasonV1::WinterToSummer),
            2.0
        );
    }

    #[test]
    fn tribulation_bait_marks_highest_realm_occupant_for_twenty_four_hours() {
        let occupants = vec![
            OccupantRealm {
                player_id: "low".to_string(),
                realm_rank: 2,
            },
            OccupantRealm {
                player_id: "high".to_string(),
                realm_rank: 5,
            },
        ];
        let mark = build_tribulation_bait_mark(&occupants, PSEUDO_VEIN_INITIAL_QI, 500)
            .expect("consumed pseudo vein qi should mark highest realm occupant");

        assert_eq!(mark.target_player_id, "high");
        assert_eq!(
            mark.expires_at_tick,
            500 + PSEUDO_VEIN_TRIBULATION_MARK_TICKS
        );
        assert_eq!(mark.bonus_probability, 0.30);
        assert_eq!(
            effective_tribulation_probability(0.10, Some(&mark), 501),
            0.40
        );
        assert_eq!(
            effective_tribulation_probability(0.10, Some(&mark), mark.expires_at_tick + 1),
            0.10
        );
    }

    #[test]
    fn tribulation_bait_event_uses_targeted_omen_for_pseudo_vein_zone() {
        let mark = PseudoVeinTribulationMark {
            target_player_id: "offline:Azure".to_string(),
            expires_at_tick: 999,
            bonus_probability: 0.30,
        };
        let event = tribulation_bait_event("pseudo_vein_42", [1280.0, -640.0], &mark);

        assert_eq!(event.kind, TribulationKindV1::Targeted);
        assert_eq!(event.phase, TribulationPhaseV1::Omen);
        assert_eq!(event.zone.as_deref(), Some("pseudo_vein_42"));
        assert_eq!(event.char_id.as_deref(), Some("offline:Azure"));
        assert_eq!(event.epicenter, Some([1280.0, 64.0, -640.0]));
    }
}
