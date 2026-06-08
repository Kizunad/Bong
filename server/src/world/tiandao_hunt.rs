use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::{Season, WorldSeasonState};
use crate::world::zone::ZoneRegistry;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, Entity, Position, Query, Res, Update, With, Without,
};

const ATTENTION_MIN: f64 = 0.0;
const ATTENTION_MAX: f64 = 100.0;
pub const TIANDAO_HUNT_EVAL_INTERVAL_TICKS: u64 = 10 * 20;

#[derive(Debug, Clone, Component, PartialEq)]
pub struct TiandaoAttention {
    pub level: f64,
    pub response: TiandaoResponseLevel,
    pub last_eval_tick: u64,
    pub accumulation_rate: f64,
    pub peak_level: f64,
}

impl Default for TiandaoAttention {
    fn default() -> Self {
        Self {
            level: 0.0,
            response: TiandaoResponseLevel::None,
            last_eval_tick: 0,
            accumulation_rate: 0.0,
            peak_level: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiandaoResponseLevel {
    None,
    Watch,
    Pressure,
    Tribulation,
    Annihilate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TiandaoActivity {
    #[cfg(test)]
    Meditating,
    #[cfg(test)]
    Combat,
    #[cfg(test)]
    Moving,
    Standing,
    #[cfg(test)]
    InNiche,
}

type AttentionAttachFilter = (With<Client>, With<Cultivation>, Without<TiandaoAttention>);

impl Default for TiandaoActivity {
    fn default() -> Self {
        Self::Standing
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoAttentionInput {
    pub realm: Realm,
    pub zone_spirit_qi: f64,
    pub activity: TiandaoActivity,
    pub season: Season,
}

pub fn register(app: &mut App) {
    app.add_systems(Update, (attach_tiandao_attention, tiandao_hunt_tick));
}

pub fn attach_tiandao_attention(
    mut commands: Commands,
    players: Query<Entity, AttentionAttachFilter>,
) {
    for entity in players.iter() {
        commands.entity(entity).insert(TiandaoAttention::default());
    }
}

pub fn tiandao_hunt_tick(
    clock: Option<Res<CultivationClock>>,
    zones: Option<Res<ZoneRegistry>>,
    season: Option<Res<WorldSeasonState>>,
    mut players: Query<
        (
            &Cultivation,
            &Position,
            Option<&CurrentDimension>,
            &mut TiandaoAttention,
        ),
        With<Client>,
    >,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;
    let season = season
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_default();
    for (cultivation, position, dimension, mut attention) in players.iter_mut() {
        apply_attention_eval(
            cultivation,
            position,
            dimension,
            &mut attention,
            zones.as_deref(),
            season,
            now_tick,
        );
    }
}

pub fn apply_attention_eval(
    cultivation: &Cultivation,
    position: &Position,
    dimension: Option<&CurrentDimension>,
    attention: &mut TiandaoAttention,
    zones: Option<&ZoneRegistry>,
    season: Season,
    now_tick: u64,
) {
    let dimension = dimension
        .map(|dim| dim.0)
        .unwrap_or(DimensionKind::Overworld);
    let zone_spirit_qi = zones
        .and_then(|registry| registry.find_zone(dimension, position.0))
        .map(|zone| zone.spirit_qi)
        .unwrap_or_default();
    let input = TiandaoAttentionInput {
        realm: cultivation.realm,
        zone_spirit_qi,
        activity: TiandaoActivity::Standing,
        season,
    };
    if should_evaluate_attention(now_tick, attention.last_eval_tick) {
        advance_attention(attention, input, now_tick);
    }
}

pub fn should_evaluate_attention(now_tick: u64, last_eval_tick: u64) -> bool {
    now_tick >= last_eval_tick
        && now_tick.saturating_sub(last_eval_tick) >= TIANDAO_HUNT_EVAL_INTERVAL_TICKS
}

pub fn advance_attention(
    attention: &mut TiandaoAttention,
    input: TiandaoAttentionInput,
    eval_tick: u64,
) {
    let previous_response = attention.response;
    let accumulation_rate = accumulation_rate(input);
    let decay = decay_rate(previous_response, input.zone_spirit_qi);
    let next_level =
        (attention.level + accumulation_rate - decay).clamp(ATTENTION_MIN, ATTENTION_MAX);

    attention.level = next_level;
    attention.accumulation_rate = accumulation_rate;
    attention.peak_level = attention.peak_level.max(next_level);
    attention.response = response_for_level(previous_response, next_level);
    attention.last_eval_tick = eval_tick;
}

pub fn accumulation_rate(input: TiandaoAttentionInput) -> f64 {
    realm_base_rate(input.realm)
        * zone_qi_factor(input.zone_spirit_qi)
        * activity_factor(input.activity)
        * season_factor(input.season)
}

pub const fn realm_base_rate(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken | Realm::Induce => 0.0,
        Realm::Condense => 0.01,
        Realm::Solidify => 0.05,
        Realm::Spirit => 0.15,
        Realm::Void => 0.40,
    }
}

pub fn zone_qi_factor(spirit_qi: f64) -> f64 {
    if spirit_qi <= 0.1 {
        0.3
    } else if spirit_qi <= 0.3 {
        0.6
    } else if spirit_qi <= 0.6 {
        1.0
    } else {
        1.8
    }
}

pub const fn activity_factor(activity: TiandaoActivity) -> f64 {
    match activity {
        #[cfg(test)]
        TiandaoActivity::Meditating => 1.5,
        #[cfg(test)]
        TiandaoActivity::Combat => 1.2,
        #[cfg(test)]
        TiandaoActivity::Moving => 0.8,
        TiandaoActivity::Standing => 1.0,
        #[cfg(test)]
        TiandaoActivity::InNiche => 0.5,
    }
}

pub const fn season_factor(season: Season) -> f64 {
    if season.is_xizhuan() {
        1.5
    } else {
        1.0
    }
}

pub fn decay_rate(response: TiandaoResponseLevel, zone_spirit_qi: f64) -> f64 {
    let base = match response {
        TiandaoResponseLevel::None => 0.08,
        TiandaoResponseLevel::Watch => 0.05,
        TiandaoResponseLevel::Pressure => 0.03,
        TiandaoResponseLevel::Tribulation => 0.01,
        TiandaoResponseLevel::Annihilate => 0.0,
    };
    base * zone_decay_multiplier(zone_spirit_qi)
}

pub fn zone_decay_multiplier(spirit_qi: f64) -> f64 {
    if spirit_qi < 0.0 {
        5.0
    } else if spirit_qi == 0.0 {
        3.0
    } else {
        1.0
    }
}

pub fn response_for_level(previous: TiandaoResponseLevel, level: f64) -> TiandaoResponseLevel {
    match previous {
        TiandaoResponseLevel::None => {
            if level >= 15.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::None
            }
        }
        TiandaoResponseLevel::Watch => {
            if level >= 40.0 {
                TiandaoResponseLevel::Pressure
            } else if level < 10.0 {
                TiandaoResponseLevel::None
            } else {
                TiandaoResponseLevel::Watch
            }
        }
        TiandaoResponseLevel::Pressure => {
            if level >= 70.0 {
                TiandaoResponseLevel::Tribulation
            } else if level < 30.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::Pressure
            }
        }
        TiandaoResponseLevel::Tribulation => {
            if level >= 90.0 {
                TiandaoResponseLevel::Annihilate
            } else if level < 60.0 {
                TiandaoResponseLevel::Pressure
            } else {
                TiandaoResponseLevel::Tribulation
            }
        }
        TiandaoResponseLevel::Annihilate => {
            if level < 80.0 {
                TiandaoResponseLevel::Tribulation
            } else {
                TiandaoResponseLevel::Annihilate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::season::Season;
    use valence::prelude::DVec3;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn input(realm: Realm) -> TiandaoAttentionInput {
        TiandaoAttentionInput {
            realm,
            zone_spirit_qi: 0.6,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
        }
    }

    #[test]
    fn realm_base_rates_match_plan_scale() {
        assert_eq!(realm_base_rate(Realm::Awaken), 0.0);
        assert_eq!(realm_base_rate(Realm::Induce), 0.0);
        assert_eq!(realm_base_rate(Realm::Condense), 0.01);
        assert_eq!(realm_base_rate(Realm::Solidify), 0.05);
        assert_eq!(realm_base_rate(Realm::Spirit), 0.15);
        assert_eq!(realm_base_rate(Realm::Void), 0.40);
    }

    #[test]
    fn low_realms_never_accumulate_attention() {
        for realm in [Realm::Awaken, Realm::Induce] {
            let mut attention = TiandaoAttention {
                level: 10.0,
                ..TiandaoAttention::default()
            };
            advance_attention(&mut attention, input(realm), 200);
            assert!(attention.level < 10.0);
            assert_eq!(attention.accumulation_rate, 0.0);
            assert_eq!(attention.response, TiandaoResponseLevel::None);
        }
    }

    #[test]
    fn zone_qi_factor_has_all_plan_thresholds() {
        assert_eq!(zone_qi_factor(-0.5), 0.3);
        assert_eq!(zone_qi_factor(0.1), 0.3);
        assert_eq!(zone_qi_factor(0.1001), 0.6);
        assert_eq!(zone_qi_factor(0.3), 0.6);
        assert_eq!(zone_qi_factor(0.3001), 1.0);
        assert_eq!(zone_qi_factor(0.6), 1.0);
        assert_eq!(zone_qi_factor(0.6001), 1.8);
    }

    #[test]
    fn activity_factor_matches_plan_actions() {
        assert_eq!(activity_factor(TiandaoActivity::Meditating), 1.5);
        assert_eq!(activity_factor(TiandaoActivity::Combat), 1.2);
        assert_eq!(activity_factor(TiandaoActivity::Moving), 0.8);
        assert_eq!(activity_factor(TiandaoActivity::Standing), 1.0);
        assert_eq!(activity_factor(TiandaoActivity::InNiche), 0.5);
    }

    #[test]
    fn xizhuan_season_increases_accumulation() {
        let normal = accumulation_rate(TiandaoAttentionInput {
            season: Season::Summer,
            ..input(Realm::Void)
        });
        let xizhuan = accumulation_rate(TiandaoAttentionInput {
            season: Season::SummerToWinter,
            ..input(Realm::Void)
        });
        assert!((xizhuan - normal * 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn response_upgrade_thresholds_are_inclusive() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::None, 15.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 40.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 70.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 90.0),
            TiandaoResponseLevel::Annihilate
        );
    }

    #[test]
    fn response_downgrade_uses_hysteresis() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 10.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 9.99),
            TiandaoResponseLevel::None
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 30.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 29.99),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 60.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 59.99),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 80.0),
            TiandaoResponseLevel::Annihilate
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 79.99),
            TiandaoResponseLevel::Tribulation
        );
    }

    #[test]
    fn advance_attention_clamps_and_tracks_peak() {
        let mut attention = TiandaoAttention {
            level: 99.9,
            response: TiandaoResponseLevel::Annihilate,
            last_eval_tick: u64::MAX,
            accumulation_rate: 0.0,
            peak_level: 50.0,
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Void,
                zone_spirit_qi: 1.0,
                activity: TiandaoActivity::Meditating,
                season: Season::SummerToWinter,
            },
            u64::MAX,
        );
        assert_eq!(attention.level, 100.0);
        assert_eq!(attention.peak_level, 100.0);
        assert_eq!(attention.last_eval_tick, u64::MAX);
        assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
    }

    #[test]
    fn decay_never_underflows_attention() {
        let mut attention = TiandaoAttention {
            level: 0.01,
            response: TiandaoResponseLevel::None,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: -0.5,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            200,
        );
        assert_eq!(attention.level, 0.0);
    }

    #[test]
    fn dead_and_negative_zones_accelerate_decay_without_qi_transfer() {
        assert_eq!(zone_decay_multiplier(0.0), 3.0);
        assert_eq!(zone_decay_multiplier(-0.01), 5.0);
        assert_eq!(zone_decay_multiplier(0.01), 1.0);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, 0.0), 0.15);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, -0.1), 0.25);
    }

    #[test]
    fn void_high_qi_meditation_reaches_watch_in_plan_timeframe() {
        let mut attention = TiandaoAttention::default();
        let input = TiandaoAttentionInput {
            realm: Realm::Void,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        for _ in 0..28 {
            advance_attention(&mut attention, input, 200);
        }
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
        assert!(attention.level >= 15.0);
    }

    #[test]
    fn evaluation_respects_ten_second_interval() {
        let cultivation = Cultivation {
            realm: Realm::Void,
            ..Cultivation::default()
        };
        let position = Position(DVec3::new(0.0, 65.0, 0.0));
        let dimension = CurrentDimension(DimensionKind::Overworld);
        let zones = ZoneRegistry::fallback();
        let mut attention = TiandaoAttention::default();

        apply_attention_eval(
            &cultivation,
            &position,
            Some(&dimension),
            &mut attention,
            Some(&zones),
            Season::Summer,
            199,
        );
        assert_eq!(attention.last_eval_tick, 0);
        assert_eq!(attention.level, 0.0);

        apply_attention_eval(
            &cultivation,
            &position,
            Some(&dimension),
            &mut attention,
            Some(&zones),
            Season::Summer,
            200,
        );
        assert_eq!(attention.last_eval_tick, 200);
        assert!(attention.level > 0.0);
    }
}
