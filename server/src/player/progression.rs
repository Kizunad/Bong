use super::state::PlayerState;

#[cfg_attr(not(test), allow(dead_code))]
const MAX_EVENT_RESOLUTION_TICKS: u64 = 1_200;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct ProgressionInput {
    pub experience_gain: i64,
    pub karma_delta: f64,
    pub spirit_qi_delta: f64,
}

impl ProgressionInput {
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn new(experience_gain: i64, karma_delta: f64, spirit_qi_delta: f64) -> Self {
        Self {
            experience_gain,
            karma_delta,
            spirit_qi_delta,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
struct RealmRule {
    name: &'static str,
    min_experience: u64,
    min_karma: f64,
    spirit_qi_max: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
const REALM_LADDER: [RealmRule; 15] = [
    RealmRule {
        name: "mortal",
        min_experience: 0,
        min_karma: -1.0,
        spirit_qi_max: 100.0,
    },
    RealmRule {
        name: "qi_refining_1",
        min_experience: 100,
        min_karma: -1.0,
        spirit_qi_max: 120.0,
    },
    RealmRule {
        name: "qi_refining_2",
        min_experience: 260,
        min_karma: -0.95,
        spirit_qi_max: 140.0,
    },
    RealmRule {
        name: "qi_refining_3",
        min_experience: 480,
        min_karma: -0.9,
        spirit_qi_max: 160.0,
    },
    RealmRule {
        name: "qi_refining_4",
        min_experience: 760,
        min_karma: -0.85,
        spirit_qi_max: 180.0,
    },
    RealmRule {
        name: "qi_refining_5",
        min_experience: 1_100,
        min_karma: -0.8,
        spirit_qi_max: 205.0,
    },
    RealmRule {
        name: "qi_refining_6",
        min_experience: 1_500,
        min_karma: -0.75,
        spirit_qi_max: 230.0,
    },
    RealmRule {
        name: "qi_refining_7",
        min_experience: 1_950,
        min_karma: -0.7,
        spirit_qi_max: 255.0,
    },
    RealmRule {
        name: "qi_refining_8",
        min_experience: 2_450,
        min_karma: -0.65,
        spirit_qi_max: 280.0,
    },
    RealmRule {
        name: "qi_refining_9",
        min_experience: 3_000,
        min_karma: -0.6,
        spirit_qi_max: 310.0,
    },
    RealmRule {
        name: "foundation_establishment_1",
        min_experience: 3_800,
        min_karma: -0.5,
        spirit_qi_max: 360.0,
    },
    RealmRule {
        name: "foundation_establishment_2",
        min_experience: 4_700,
        min_karma: -0.35,
        spirit_qi_max: 420.0,
    },
    RealmRule {
        name: "foundation_establishment_3",
        min_experience: 5_700,
        min_karma: -0.2,
        spirit_qi_max: 490.0,
    },
    RealmRule {
        name: "golden_core",
        min_experience: 7_600,
        min_karma: 0.0,
        spirit_qi_max: 620.0,
    },
    RealmRule {
        name: "nascent_soul",
        min_experience: 9_800,
        min_karma: 0.25,
        spirit_qi_max: 780.0,
    },
];

#[cfg_attr(not(test), allow(dead_code))]
pub fn synthetic_gain_input(
    experience_gain: i64,
    karma_delta: f64,
    spirit_qi_delta: f64,
) -> ProgressionInput {
    ProgressionInput::new(experience_gain, karma_delta, spirit_qi_delta)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn cultivation_tick_input(zone_spirit_qi: f64) -> ProgressionInput {
    let zone_qi = clamp_unit_finite(zone_spirit_qi);
    let experience_gain = (zone_qi * 24.0).round() as i64 + 1;
    let karma_delta = (zone_qi - 0.5) * 0.01;
    let spirit_qi_delta = 0.5 + zone_qi * 3.5;

    ProgressionInput::new(experience_gain, karma_delta, spirit_qi_delta)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn active_event_resolution_input(
    intensity: f64,
    duration_ticks: u64,
    resolved_successfully: bool,
) -> ProgressionInput {
    let bounded_intensity = clamp_unit_finite(intensity);
    let bounded_duration_ratio =
        duration_ticks.min(MAX_EVENT_RESOLUTION_TICKS) as f64 / MAX_EVENT_RESOLUTION_TICKS as f64;
    let event_factor = (bounded_intensity * bounded_duration_ratio).clamp(0.0, 1.0);
    let magnitude = (event_factor * 120.0).round() as i64;

    let experience_gain = if resolved_successfully {
        magnitude.max(1)
    } else {
        -((magnitude / 2).max(1))
    };
    let karma_delta = if resolved_successfully {
        0.08 * bounded_intensity
    } else {
        -0.12 * bounded_intensity
    };
    let spirit_qi_delta = if resolved_successfully {
        6.0 * event_factor
    } else {
        -4.0 * event_factor
    };

    ProgressionInput::new(experience_gain, karma_delta, spirit_qi_delta)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn apply_progression(state: &PlayerState, input: ProgressionInput) -> PlayerState {
    let normalized = state.normalized();
    let next_experience = apply_experience_gain(normalized.experience, input.experience_gain);
    let next_karma = apply_karma_delta(normalized.karma, input.karma_delta);

    let current_realm_index = realm_index(normalized.realm.as_str());
    let promoted_realm_index =
        resolve_realm_promotion(current_realm_index, next_experience, next_karma);
    let promoted_rule = REALM_LADDER[promoted_realm_index];

    let next_spirit_qi_max = normalized
        .spirit_qi_max
        .max(promoted_rule.spirit_qi_max)
        .max(1.0);
    let next_spirit_qi = apply_spirit_qi_delta(
        normalized.spirit_qi,
        input.spirit_qi_delta,
        next_spirit_qi_max,
    );

    PlayerState {
        realm: promoted_rule.name.to_string(),
        spirit_qi: next_spirit_qi,
        spirit_qi_max: next_spirit_qi_max,
        karma: next_karma,
        experience: next_experience,
        inventory_score: normalized.inventory_score,
    }
    .normalized()
}

#[allow(dead_code)]
pub fn apply_progression_in_place(state: &mut PlayerState, input: ProgressionInput) {
    *state = apply_progression(state, input);
}

#[cfg_attr(not(test), allow(dead_code))]
fn apply_experience_gain(experience: u64, gain: i64) -> u64 {
    if gain >= 0 {
        experience.saturating_add(gain as u64)
    } else {
        let reduction = gain
            .checked_abs()
            .map(|value| value as u64)
            .unwrap_or(u64::MAX);
        experience.saturating_sub(reduction)
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn apply_karma_delta(current_karma: f64, karma_delta: f64) -> f64 {
    if !karma_delta.is_finite() {
        return current_karma.clamp(-1.0, 1.0);
    }

    (current_karma + karma_delta).clamp(-1.0, 1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
fn apply_spirit_qi_delta(current_qi: f64, qi_delta: f64, qi_max: f64) -> f64 {
    if !qi_delta.is_finite() {
        return current_qi.clamp(0.0, qi_max);
    }

    (current_qi + qi_delta).clamp(0.0, qi_max)
}

#[cfg_attr(not(test), allow(dead_code))]
fn clamp_unit_finite(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }

    value.clamp(0.0, 1.0)
}

#[cfg_attr(not(test), allow(dead_code))]
fn resolve_realm_promotion(current_index: usize, experience: u64, karma: f64) -> usize {
    let mut promoted_index = current_index;

    while promoted_index + 1 < REALM_LADDER.len() {
        let next_rule = REALM_LADDER[promoted_index + 1];
        if experience >= next_rule.min_experience && karma >= next_rule.min_karma {
            promoted_index += 1;
        } else {
            break;
        }
    }

    promoted_index
}

#[cfg_attr(not(test), allow(dead_code))]
fn realm_index(realm: &str) -> usize {
    let normalized = realm.trim().to_ascii_lowercase();

    if normalized == REALM_LADDER[0].name {
        return 0;
    }

    if let Some(stage) = normalized
        .strip_prefix("qi_refining_")
        .and_then(|value| value.parse::<usize>().ok())
    {
        if (1..=9).contains(&stage) {
            return stage;
        }
    }

    if let Some(stage) = normalized
        .strip_prefix("foundation_establishment_")
        .or_else(|| normalized.strip_prefix("foundation_"))
        .and_then(|value| value.parse::<usize>().ok())
    {
        if (1..=3).contains(&stage) {
            return 9 + stage;
        }
    }

    match normalized.as_str() {
        "golden_core" => 13,
        "nascent_soul" => 14,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{build_player_state_payload, collect_players_for_world_state};
    use crate::schema::server_data::ServerDataV1;
    use crate::world::zone::{default_spawn_bounds, ZoneRegistry};
    use valence::prelude::{DVec3, Uuid};

    #[test]
    fn threshold_crossing_promotes_from_mortal_to_qi_refining_1() {
        let state = PlayerState {
            experience: 95,
            ..PlayerState::default()
        };

        let progressed = apply_progression(&state, synthetic_gain_input(10, 0.0, 2.0));

        assert_eq!(progressed.realm, "qi_refining_1");
        assert_eq!(progressed.experience, 105);
        assert_eq!(progressed.spirit_qi_max, 120.0);
        assert!(progressed.spirit_qi >= 0.0);
    }

    #[test]
    fn karma_is_clamped_for_oversized_positive_and_negative_inputs() {
        let positive =
            apply_progression(&PlayerState::default(), synthetic_gain_input(0, 8.0, 0.0));
        let negative =
            apply_progression(&PlayerState::default(), synthetic_gain_input(0, -8.0, 0.0));

        assert_eq!(positive.karma, 1.0);
        assert_eq!(negative.karma, -1.0);
    }

    #[test]
    fn karma_gate_blocks_higher_realm_even_with_high_experience() {
        let state = PlayerState {
            realm: "mortal".to_string(),
            spirit_qi: 10.0,
            spirit_qi_max: 100.0,
            karma: -0.45,
            experience: 8_000,
            inventory_score: 0.0,
        };

        let progressed = apply_progression(&state, synthetic_gain_input(0, 0.0, 0.0));

        assert_eq!(progressed.realm, "foundation_establishment_1");
        assert!(progressed.experience >= 8_000);
    }

    #[test]
    fn promotion_updates_qi_max_and_clamps_qi_to_new_cap() {
        let state = PlayerState {
            realm: "qi_refining_1".to_string(),
            spirit_qi: 500.0,
            spirit_qi_max: 120.0,
            karma: -0.4,
            experience: 3_690,
            inventory_score: 0.0,
        };

        let progressed = apply_progression(&state, synthetic_gain_input(150, 0.0, 500.0));

        assert_eq!(progressed.realm, "foundation_establishment_1");
        assert_eq!(progressed.spirit_qi_max, 360.0);
        assert_eq!(progressed.spirit_qi, 360.0);
    }

    #[test]
    fn experience_accumulation_is_saturating_and_non_negative() {
        let near_max = PlayerState {
            experience: u64::MAX - 3,
            ..PlayerState::default()
        };
        let saturated = apply_progression(&near_max, synthetic_gain_input(i64::MAX, 0.0, 0.0));
        assert_eq!(saturated.experience, u64::MAX);

        let zeroed = apply_progression(&saturated, synthetic_gain_input(i64::MIN, 0.0, 0.0));
        assert_eq!(zeroed.experience, 0);
    }

    #[test]
    fn invalid_non_finite_inputs_do_not_create_nan_or_invalid_state() {
        let state = PlayerState {
            spirit_qi: 50.0,
            ..PlayerState::default()
        };

        let progressed =
            apply_progression(&state, synthetic_gain_input(12, f64::NAN, f64::INFINITY));

        assert!(progressed.spirit_qi.is_finite());
        assert!(progressed.spirit_qi_max.is_finite());
        assert!(progressed.karma.is_finite());
        assert!(progressed.spirit_qi >= 0.0);
        assert!(progressed.spirit_qi_max >= 1.0);
        assert_eq!(progressed.karma, 0.0);
    }

    #[test]
    fn cultivation_and_event_inputs_are_deterministic_and_safe() {
        let low = cultivation_tick_input(-5.0);
        let high = cultivation_tick_input(5.0);

        assert!(low.experience_gain >= 1);
        assert!(high.experience_gain > low.experience_gain);
        assert!(high.spirit_qi_delta > low.spirit_qi_delta);
        assert!(high.karma_delta.is_finite());

        let event_a = active_event_resolution_input(0.7, 600, true);
        let event_b = active_event_resolution_input(0.7, 600, true);
        let failed = active_event_resolution_input(0.7, 600, false);

        assert_eq!(event_a, event_b);
        assert!(event_a.experience_gain > 0);
        assert!(failed.experience_gain < 0);
        assert!(failed.karma_delta < 0.0);
    }

    #[test]
    fn progression_changes_are_visible_through_existing_projection_seam() {
        let before = PlayerState::default();
        let after = apply_progression(&before, synthetic_gain_input(180, 0.15, 30.0));

        let payload = build_player_state_payload(&after, "spawn")
            .expect("progressed state should serialize in player_state payload");
        let decoded: ServerDataV1 =
            serde_json::from_slice(&payload).expect("player_state payload should decode");

        let player_state = match decoded.payload {
            crate::schema::server_data::ServerDataPayloadV1::PlayerState {
                realm,
                spirit_qi,
                composite_power,
                ..
            } => (realm, spirit_qi, composite_power),
            _ => unreachable!("expected player_state payload"),
        };
        assert_eq!(player_state.0, "qi_refining_1");
        assert!(player_state.1 > 0.0);
        assert!(player_state.2 > 0.0);

        let registry = ZoneRegistry::fallback();
        let (spawn_min, spawn_max) = default_spawn_bounds();
        let (profiles, _) = collect_players_for_world_state(
            [(
                "Azure",
                Uuid::nil(),
                DVec3::new(
                    spawn_min.x.min(spawn_max.x),
                    spawn_min.y.min(spawn_max.y),
                    spawn_min.z.min(spawn_max.z),
                ),
                Some(&after),
            )],
            &registry,
        );

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].realm, "qi_refining_1");
        assert!(profiles[0].composite_power > 0.0);
        assert!(profiles[0].breakdown.combat > 0.0);
    }
}
