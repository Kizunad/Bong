//! plan-lingtian-process-v1 P1 — 作物 / 加工产物 freshness game-tick 衰减。
//!
//! 这里提供运行时 Component 与纯函数计算。真正的 inventory NBT 仍由
//! `crate::shelflife::Freshness` 负责持久化；本组件是在线 tick 缓存层，按
//! plan 决策只在 server tick 推进，离线自然暂停。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Query, Res, Resource};

use crate::lingtian::{LingtianClock, BEVY_TICKS_PER_LINGTIAN_TICK};

pub const GAME_DAY_TICKS: u64 = 24_000;
pub const FRESH_HERB_TOTAL_TICKS: u64 = GAME_DAY_TICKS * 3;
pub const DRYING_HALF_LIFE_TICKS: u64 = GAME_DAY_TICKS * 14;
pub const GRINDING_HALF_LIFE_TICKS: u64 = GAME_DAY_TICKS * 7;
pub const FORGING_ALCHEMY_TOTAL_TICKS: u64 = GAME_DAY_TICKS * 30;
pub const EXTRACTION_HALF_LIFE_TICKS: u64 = GAME_DAY_TICKS * 3;
pub const ANQI_FRESHNESS_MULTIPLIER: f32 = 0.3;

#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreshnessTracker {
    pub profile_name: String,
    pub born_at_tick: u64,
    pub last_eval_tick: u64,
    pub initial_qi: f32,
    pub current_freshness: f32,
    pub frozen_until_tick: Option<u64>,
    #[serde(default)]
    pub in_anqi: bool,
    #[serde(default)]
    pub withered_item_id: Option<String>,
}

impl FreshnessTracker {
    #[allow(dead_code)]
    pub fn fresh(profile_name: impl Into<String>, born_at_tick: u64, initial_qi: f32) -> Self {
        Self {
            profile_name: profile_name.into(),
            born_at_tick,
            last_eval_tick: born_at_tick,
            initial_qi,
            current_freshness: 1.0,
            frozen_until_tick: None,
            in_anqi: false,
            withered_item_id: None,
        }
    }

    #[allow(dead_code)]
    pub fn freeze_until(mut self, tick: u64) -> Self {
        self.frozen_until_tick = Some(tick);
        self
    }

    #[allow(dead_code)]
    pub fn with_withered_item(mut self, item_id: impl Into<String>) -> Self {
        self.withered_item_id = Some(item_id.into());
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Summer,
    SummerToWinter,
    Winter,
    WinterToSummer,
}

impl Season {
    pub fn freshness_multiplier(self, tide_roll_0_1: f32) -> f32 {
        match self {
            Season::Summer => 1.5,
            Season::Winter => 0.7,
            Season::SummerToWinter | Season::WinterToSummer => {
                0.7 + tide_roll_0_1.clamp(0.0, 1.0) * 0.6
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreshnessProfileKind {
    Linear { total_ticks: u64 },
    Exponential { half_life_ticks: u64 },
}

pub fn profile_kind(profile_name: &str) -> Option<FreshnessProfileKind> {
    match profile_name {
        "fresh_herb_v1" => Some(FreshnessProfileKind::Linear {
            total_ticks: FRESH_HERB_TOTAL_TICKS,
        }),
        "drying_v1" => Some(FreshnessProfileKind::Exponential {
            half_life_ticks: DRYING_HALF_LIFE_TICKS,
        }),
        "grinding_v1" => Some(FreshnessProfileKind::Exponential {
            half_life_ticks: GRINDING_HALF_LIFE_TICKS,
        }),
        "forging_alchemy_v1" => Some(FreshnessProfileKind::Linear {
            total_ticks: FORGING_ALCHEMY_TOTAL_TICKS,
        }),
        "extraction_v1" => Some(FreshnessProfileKind::Exponential {
            half_life_ticks: EXTRACTION_HALF_LIFE_TICKS,
        }),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreshnessContext {
    pub now_tick: u64,
    pub last_eval_tick: u64,
    pub season: Season,
    pub tide_roll_0_1: f32,
    pub in_anqi: bool,
}

impl FreshnessContext {
    pub fn effective_multiplier(self) -> f32 {
        let anqi = if self.in_anqi {
            ANQI_FRESHNESS_MULTIPLIER
        } else {
            1.0
        };
        self.season.freshness_multiplier(self.tide_roll_0_1) * anqi
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Resource)]
pub struct FreshnessEnvironment {
    pub season: Season,
    pub tide_roll_0_1: f32,
}

impl Default for FreshnessEnvironment {
    fn default() -> Self {
        Self {
            season: Season::Summer,
            tide_roll_0_1: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FreshnessTransition {
    Alive {
        freshness: f32,
    },
    Withered {
        freshness: f32,
        item_id: Option<String>,
        quality: f32,
    },
    Frozen {
        freshness: f32,
    },
}

pub fn freshness_tick_system(
    clock: Option<Res<LingtianClock>>,
    environment: Option<Res<FreshnessEnvironment>>,
    mut trackers: Query<&mut FreshnessTracker>,
) {
    let Some(clock) = clock else {
        return;
    };
    let environment = environment.as_deref().copied().unwrap_or_default();
    let now_tick = clock
        .lingtian_tick
        .saturating_mul(BEVY_TICKS_PER_LINGTIAN_TICK as u64);
    for mut tracker in &mut trackers {
        let in_anqi = tracker.in_anqi;
        advance_tracker_to_tick(
            &mut tracker,
            now_tick,
            environment.season,
            environment.tide_roll_0_1,
            in_anqi,
        );
    }
}

pub fn advance_tracker_to_tick(
    tracker: &mut FreshnessTracker,
    now_tick: u64,
    season: Season,
    tide_roll_0_1: f32,
    in_anqi: bool,
) -> FreshnessTransition {
    if now_tick <= tracker.last_eval_tick {
        return FreshnessTransition::Alive {
            freshness: tracker.current_freshness,
        };
    }
    if tracker
        .frozen_until_tick
        .is_some_and(|frozen_until| frozen_until > now_tick)
    {
        tracker.last_eval_tick = now_tick;
        return FreshnessTransition::Frozen {
            freshness: tracker.current_freshness,
        };
    }
    if now_tick.saturating_sub(tracker.last_eval_tick) < GAME_DAY_TICKS {
        return FreshnessTransition::Alive {
            freshness: tracker.current_freshness,
        };
    }
    let transition = advance_freshness(
        tracker,
        FreshnessContext {
            now_tick,
            last_eval_tick: tracker.last_eval_tick,
            season,
            tide_roll_0_1,
            in_anqi,
        },
    );
    match &transition {
        FreshnessTransition::Alive { freshness }
        | FreshnessTransition::Withered { freshness, .. }
        | FreshnessTransition::Frozen { freshness } => {
            tracker.current_freshness = *freshness;
        }
    }
    tracker.last_eval_tick = now_tick;
    transition
}

pub fn advance_freshness(tracker: &FreshnessTracker, ctx: FreshnessContext) -> FreshnessTransition {
    if tracker
        .frozen_until_tick
        .is_some_and(|frozen_until| frozen_until > ctx.now_tick)
    {
        return FreshnessTransition::Frozen {
            freshness: tracker.current_freshness,
        };
    }

    let Some(profile) = profile_kind(tracker.profile_name.as_str()) else {
        return FreshnessTransition::Alive {
            freshness: tracker.current_freshness,
        };
    };
    let elapsed = ctx.now_tick.saturating_sub(ctx.last_eval_tick);
    let effective_elapsed = ((elapsed as f64) * ctx.effective_multiplier() as f64).round() as u64;
    let next = match profile {
        FreshnessProfileKind::Linear { total_ticks } => linear_decay(
            tracker.current_freshness,
            effective_elapsed,
            total_ticks.max(1),
        ),
        FreshnessProfileKind::Exponential { half_life_ticks } => exponential_decay(
            tracker.current_freshness,
            effective_elapsed,
            half_life_ticks.max(1),
        ),
    };

    if next <= 0.0 {
        FreshnessTransition::Withered {
            freshness: 0.0,
            item_id: tracker.withered_item_id.clone(),
            quality: tracker.initial_qi * 0.3,
        }
    } else {
        FreshnessTransition::Alive { freshness: next }
    }
}

fn linear_decay(current: f32, elapsed_ticks: u64, total_ticks: u64) -> f32 {
    let delta = elapsed_ticks as f32 / total_ticks as f32;
    (current - delta).clamp(0.0, 1.0)
}

fn exponential_decay(current: f32, elapsed_ticks: u64, half_life_ticks: u64) -> f32 {
    let n = elapsed_ticks as f32 / half_life_ticks as f32;
    (current * 0.5_f32.powf(n)).clamp(0.0, 1.0)
}

#[allow(dead_code)]
pub fn freshness_profile_for_processed_item(item_id: &str) -> Option<&'static str> {
    if item_id.starts_with("dry_") {
        Some("drying_v1")
    } else if item_id.starts_with("powder_") {
        Some("grinding_v1")
    } else if item_id.starts_with("processed_") {
        Some("forging_alchemy_v1")
    } else if item_id.starts_with("extract_") {
        Some("extraction_v1")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(season: Season) -> FreshnessContext {
        FreshnessContext {
            now_tick: GAME_DAY_TICKS,
            last_eval_tick: 0,
            season,
            tide_roll_0_1: 0.5,
            in_anqi: false,
        }
    }

    #[test]
    fn freshness_tracker_default_initial_value_1_0() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.2);
        assert_eq!(tracker.current_freshness, 1.0);
        assert_eq!(tracker.initial_qi, 1.2);
    }

    #[test]
    fn freshness_tick_decreases_per_game_day() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.0);
        let next = advance_freshness(&tracker, ctx(Season::Winter));
        assert_eq!(
            next,
            FreshnessTransition::Alive {
                freshness: 1.0 - (0.7 / 3.0)
            }
        );
    }

    #[test]
    fn freshness_tick_skips_frozen_entries() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.0).freeze_until(100);
        let next = advance_freshness(
            &tracker,
            FreshnessContext {
                now_tick: 99,
                ..ctx(Season::Summer)
            },
        );
        assert_eq!(next, FreshnessTransition::Frozen { freshness: 1.0 });
    }

    #[test]
    fn freshness_tick_offline_pauses() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.0);
        let next = advance_freshness(
            &tracker,
            FreshnessContext {
                now_tick: 0,
                last_eval_tick: 0,
                ..ctx(Season::Summer)
            },
        );
        assert_eq!(next, FreshnessTransition::Alive { freshness: 1.0 });
    }

    #[test]
    fn freshness_tracker_waits_until_full_game_day() {
        let mut tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.0);
        advance_tracker_to_tick(
            &mut tracker,
            GAME_DAY_TICKS - 1,
            Season::WinterToSummer,
            0.5,
            false,
        );
        assert_eq!(tracker.current_freshness, 1.0);
        assert_eq!(tracker.last_eval_tick, 0);

        advance_tracker_to_tick(
            &mut tracker,
            GAME_DAY_TICKS,
            Season::WinterToSummer,
            0.5,
            false,
        );
        assert!((tracker.current_freshness - (1.0 - 1.0 / 3.0)).abs() < 1e-6);
        assert_eq!(tracker.last_eval_tick, GAME_DAY_TICKS);
    }

    #[test]
    fn freshness_tracker_uses_environment_and_anqi_multiplier() {
        let env = FreshnessEnvironment {
            season: Season::Summer,
            tide_roll_0_1: 0.0,
        };
        let mut tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.0);
        tracker.in_anqi = true;
        let in_anqi = tracker.in_anqi;
        advance_tracker_to_tick(
            &mut tracker,
            GAME_DAY_TICKS,
            env.season,
            env.tide_roll_0_1,
            in_anqi,
        );
        assert!(
            (tracker.current_freshness - 0.85).abs() < 1e-6,
            "summer 1.5 * anqi 0.3 should decay one fresh-herb day by 0.15"
        );
    }

    #[test]
    fn freshness_with_summer_multiplier_1_5() {
        assert_eq!(Season::Summer.freshness_multiplier(0.0), 1.5);
    }

    #[test]
    fn freshness_with_winter_multiplier_0_7() {
        assert_eq!(Season::Winter.freshness_multiplier(1.0), 0.7);
    }

    #[test]
    fn freshness_with_tide_multiplier_random_0_7_to_1_3() {
        assert_eq!(Season::SummerToWinter.freshness_multiplier(0.0), 0.7);
        assert_eq!(Season::WinterToSummer.freshness_multiplier(1.0), 1.3);
        assert_eq!(Season::SummerToWinter.freshness_multiplier(0.5), 1.0);
    }

    #[test]
    fn freshness_in_anqi_multiplier_0_3_combines_with_season() {
        let ctx = FreshnessContext {
            in_anqi: true,
            ..ctx(Season::Summer)
        };
        assert!((ctx.effective_multiplier() - 0.45).abs() < 1e-6);
    }

    #[test]
    fn freshness_zero_transitions_to_withered_item() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 1.1)
            .with_withered_item("withered_ci_she_hao");
        let next = advance_freshness(
            &tracker,
            FreshnessContext {
                now_tick: GAME_DAY_TICKS * 10,
                last_eval_tick: 0,
                season: Season::Summer,
                tide_roll_0_1: 0.5,
                in_anqi: false,
            },
        );
        match next {
            FreshnessTransition::Withered {
                item_id, quality, ..
            } => {
                assert_eq!(item_id, Some("withered_ci_she_hao".to_string()));
                assert!((quality - 0.33).abs() < 1e-6);
            }
            other => panic!("expected withered transition, got {other:?}"),
        }
    }

    #[test]
    fn freshness_withered_item_has_quality_0_3() {
        let tracker = FreshnessTracker::fresh("fresh_herb_v1", 0, 2.0);
        let next = advance_freshness(
            &tracker,
            FreshnessContext {
                now_tick: GAME_DAY_TICKS * 10,
                last_eval_tick: 0,
                season: Season::Summer,
                tide_roll_0_1: 0.5,
                in_anqi: false,
            },
        );
        match next {
            FreshnessTransition::Withered { quality, .. } => assert_eq!(quality, 0.6),
            other => panic!("expected withered transition, got {other:?}"),
        }
    }
}
