use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, App, Event, EventWriter, Res, ResMut, Resource, Update};

use crate::cultivation::tick::CultivationClock;

pub const YEAR_TICKS: u64 = 48 * 3600 * 20;
pub const SUMMER_TICKS: u64 = YEAR_TICKS * 40 / 100;
pub const XIZHUAN_TICKS: u64 = YEAR_TICKS * 10 / 100;
pub const WINTER_TICKS: u64 = YEAR_TICKS * 40 / 100;
pub const VANILLA_DAY_TICKS: u64 = 24_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Summer,
    SummerToWinter,
    Winter,
    WinterToSummer,
}

impl Season {
    pub const fn is_xizhuan(self) -> bool {
        matches!(self, Self::SummerToWinter | Self::WinterToSummer)
    }

    pub const fn phase_total_ticks(self) -> u64 {
        match self {
            Self::Summer => SUMMER_TICKS,
            Self::SummerToWinter | Self::WinterToSummer => XIZHUAN_TICKS,
            Self::Winter => WINTER_TICKS,
        }
    }

    pub const fn phase_start_tick(self) -> u64 {
        match self {
            Self::Summer => 0,
            Self::SummerToWinter => SUMMER_TICKS,
            Self::Winter => SUMMER_TICKS + XIZHUAN_TICKS,
            Self::WinterToSummer => SUMMER_TICKS + XIZHUAN_TICKS + WINTER_TICKS,
        }
    }

    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Summer => "summer",
            Self::SummerToWinter => "summer_to_winter",
            Self::Winter => "winter",
            Self::WinterToSummer => "winter_to_summer",
        }
    }

    /// plan-lingtian-weather-v1 §2 — `plot_qi_cap` 的稳定季节修饰（绝对增量）。
    /// 夏散 -0.2、冬聚 +0.2；汐转返回 0（基线无偏移，由调用方乘
    /// [`xizhuan_qi_cap_amplitude`] 注入 jitter）。
    pub const fn plot_qi_cap_modifier(self) -> f32 {
        match self {
            Self::Summer => -0.2,
            Self::Winter => 0.2,
            Self::SummerToWinter | Self::WinterToSummer => 0.0,
        }
    }

    /// plan-lingtian-weather-v1 §2 — `natural_supply` 的稳定季节修饰（相对增量）。
    /// 夏 -10%、冬 +10%、汐转 0（基线无偏移）。
    pub const fn natural_supply_modifier(self) -> f32 {
        match self {
            Self::Summer => -0.10,
            Self::Winter => 0.10,
            Self::SummerToWinter | Self::WinterToSummer => 0.0,
        }
    }

    /// plan-lingtian-weather-v1 §2 — plot ↔ zone qi 流速倍率。
    /// 夏 ×1.3（缚力外散）、冬 ×0.7（缚力内收）、汐转基线 1.0（具体由
    /// [`xizhuan_zone_flow_jitter_max_delta`] 派生 1.0–1.5 RNG）。
    pub const fn zone_flow_multiplier(self) -> f32 {
        match self {
            Self::Summer => 1.3,
            Self::Winter => 0.7,
            Self::SummerToWinter | Self::WinterToSummer => 1.0,
        }
    }

    /// plan-lingtian-weather-v1 §2 — 汐转期 `plot_qi_cap` jitter 振幅。
    /// 调用方传 jitter ∈ [-1, 1]，乘以振幅即可得"反复 ±0.3"。
    /// 非汐转季节返回 0（无 jitter）。
    pub const fn xizhuan_qi_cap_amplitude(self) -> f32 {
        if self.is_xizhuan() {
            0.3
        } else {
            0.0
        }
    }

    /// plan-lingtian-weather-v1 §2 — 汐转期 `natural_supply` jitter 振幅（±20%）。
    pub const fn xizhuan_supply_amplitude(self) -> f32 {
        if self.is_xizhuan() {
            0.20
        } else {
            0.0
        }
    }

    /// plan-lingtian-weather-v1 §2 — 汐转期 `zone_flow_multiplier` jitter 上限（最大额外 +0.5）。
    /// 实际倍率 = 基线 1.0 + jitter_normalized_[0,1] × 0.5（即 1.0–1.5 RNG）。
    /// 非汐转返回 0（无 jitter，使用稳定 1.3 / 0.7）。
    pub const fn xizhuan_zone_flow_jitter_max_delta(self) -> f32 {
        if self.is_xizhuan() {
            0.5
        } else {
            0.0
        }
    }

    /// plan-lingtian-weather-v1 §0 / §3 — 汐转期天气事件 RNG 翻倍倍率。
    ///
    /// 由 P2 `weather_generator_system` 在每 game-day RNG roll 时乘到基线触发
    /// 概率上；P0 引入但调用点在 P2，先暂 `#[allow(dead_code)]`。
    #[allow(dead_code)]
    pub const fn weather_rng_multiplier(self) -> f32 {
        if self.is_xizhuan() {
            2.0
        } else {
            1.0
        }
    }
}

impl Default for Season {
    fn default() -> Self {
        Self::Summer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonState {
    pub season: Season,
    pub tick_into_phase: u64,
    pub phase_total_ticks: u64,
    pub year_index: u64,
}

impl Default for SeasonState {
    fn default() -> Self {
        query_season("", 0)
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct WorldSeasonState {
    pub current: SeasonState,
    pub last_phase_change_tick: u64,
    tick_offset: i128,
}

impl WorldSeasonState {
    pub fn effective_tick(&self, clock_tick: u64) -> u64 {
        add_offset(clock_tick, self.tick_offset)
    }

    pub fn set_phase(&mut self, phase: Season, clock_tick: u64) -> SeasonState {
        let current_effective = self.effective_tick(clock_tick);
        let current_year = query_season("", current_effective).year_index;
        let target_effective = current_year
            .saturating_mul(YEAR_TICKS)
            .saturating_add(phase.phase_start_tick());
        self.tick_offset = target_effective as i128 - clock_tick as i128;
        self.current = query_season("", target_effective);
        self.last_phase_change_tick = clock_tick;
        self.current
    }

    pub fn advance_by_ticks(&mut self, ticks: u64, clock_tick: u64) -> SeasonState {
        let old_effective = self.effective_tick(clock_tick);
        let new_effective = old_effective.saturating_add(ticks);
        self.tick_offset = new_effective as i128 - clock_tick as i128;
        self.current = query_season("", new_effective);
        self.current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Event)]
pub struct SeasonChangedEvent {
    pub from: Season,
    pub to: Season,
    pub tick: u64,
}

pub fn register(app: &mut App) {
    app.insert_resource(WorldSeasonState::default())
        .add_event::<SeasonChangedEvent>()
        .add_systems(Update, season_tick);
}

pub fn query_season(_zone: &str, tick: u64) -> SeasonState {
    let year_index = tick / YEAR_TICKS;
    let tick_in_year = tick % YEAR_TICKS;

    if tick_in_year < SUMMER_TICKS {
        return SeasonState {
            season: Season::Summer,
            tick_into_phase: tick_in_year,
            phase_total_ticks: Season::Summer.phase_total_ticks(),
            year_index,
        };
    }

    let after_summer = tick_in_year - SUMMER_TICKS;
    if after_summer < XIZHUAN_TICKS {
        return SeasonState {
            season: Season::SummerToWinter,
            tick_into_phase: after_summer,
            phase_total_ticks: Season::SummerToWinter.phase_total_ticks(),
            year_index,
        };
    }

    let after_first_xizhuan = after_summer - XIZHUAN_TICKS;
    if after_first_xizhuan < WINTER_TICKS {
        return SeasonState {
            season: Season::Winter,
            tick_into_phase: after_first_xizhuan,
            phase_total_ticks: Season::Winter.phase_total_ticks(),
            year_index,
        };
    }

    SeasonState {
        season: Season::WinterToSummer,
        tick_into_phase: after_first_xizhuan - WINTER_TICKS,
        phase_total_ticks: Season::WinterToSummer.phase_total_ticks(),
        year_index,
    }
}

pub fn season_tick(
    clock: Option<Res<CultivationClock>>,
    mut state: ResMut<WorldSeasonState>,
    mut events: EventWriter<SeasonChangedEvent>,
) {
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let effective_tick = state.effective_tick(clock_tick);
    let next = query_season("", effective_tick);
    let previous = state.current;
    if previous.season != next.season {
        events.send(SeasonChangedEvent {
            from: previous.season,
            to: next.season,
            tick: clock_tick,
        });
        state.last_phase_change_tick = clock_tick;
    }
    state.current = next;
}

fn add_offset(base: u64, offset: i128) -> u64 {
    if offset >= 0 {
        base.saturating_add(offset as u64)
    } else {
        base.saturating_sub((-offset) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    #[test]
    fn season_state_at_tick_zero_is_summer() {
        let state = query_season("spawn", 0);
        assert_eq!(state.season, Season::Summer);
        assert_eq!(state.tick_into_phase, 0);
        assert_eq!(state.phase_total_ticks, SUMMER_TICKS);
        assert_eq!(state.year_index, 0);
    }

    #[test]
    fn season_state_at_boundaries_covers_all_phases() {
        assert_eq!(
            query_season("spawn", SUMMER_TICKS - 1).season,
            Season::Summer
        );
        assert_eq!(
            query_season("spawn", SUMMER_TICKS).season,
            Season::SummerToWinter
        );
        assert_eq!(
            query_season("spawn", SUMMER_TICKS + XIZHUAN_TICKS).season,
            Season::Winter
        );
        assert_eq!(
            query_season("spawn", SUMMER_TICKS + XIZHUAN_TICKS + WINTER_TICKS).season,
            Season::WinterToSummer
        );
    }

    #[test]
    fn season_state_at_year_ticks_minus_one_is_winter_to_summer() {
        let state = query_season("spawn", YEAR_TICKS - 1);
        assert_eq!(state.season, Season::WinterToSummer);
        assert_eq!(state.tick_into_phase, XIZHUAN_TICKS - 1);
        assert_eq!(state.year_index, 0);
    }

    #[test]
    fn season_state_at_year_ticks_wraps_to_summer_year_index_plus_1() {
        let state = query_season("spawn", YEAR_TICKS);
        assert_eq!(state.season, Season::Summer);
        assert_eq!(state.tick_into_phase, 0);
        assert_eq!(state.year_index, 1);
    }

    #[test]
    fn query_season_returns_consistent_result_for_same_tick() {
        let left = query_season("spawn", SUMMER_TICKS + 42);
        let right = query_season("unknown_zone", SUMMER_TICKS + 42);
        assert_eq!(left, right);
    }

    #[test]
    fn season_tick_emits_on_phase_boundary() {
        let mut app = App::new();
        app.insert_resource(CultivationClock {
            tick: SUMMER_TICKS - 1,
        });
        app.add_event::<SeasonChangedEvent>();
        app.insert_resource(WorldSeasonState {
            current: query_season("", SUMMER_TICKS - 1),
            last_phase_change_tick: 0,
            tick_offset: 0,
        });
        app.add_systems(Update, season_tick);

        app.world_mut().resource_mut::<CultivationClock>().tick = SUMMER_TICKS;
        app.update();

        let events = app.world().resource::<Events<SeasonChangedEvent>>();
        let mut reader = events.get_reader();
        let collected = reader.read(events).copied().collect::<Vec<_>>();
        assert_eq!(
            collected,
            vec![SeasonChangedEvent {
                from: Season::Summer,
                to: Season::SummerToWinter,
                tick: SUMMER_TICKS,
            }]
        );
    }

    #[test]
    fn world_season_state_set_phase_offsets_effective_clock() {
        let mut state = WorldSeasonState::default();
        let current_clock = 5_000;

        let result = state.set_phase(Season::Winter, current_clock);

        assert_eq!(result.season, Season::Winter);
        assert_eq!(result.tick_into_phase, 0);
        assert_eq!(
            query_season("", state.effective_tick(current_clock)).season,
            Season::Winter
        );
    }

    #[test]
    fn world_season_state_advance_crosses_full_year() {
        let mut state = WorldSeasonState::default();
        let result = state.advance_by_ticks(YEAR_TICKS + SUMMER_TICKS, 0);

        assert_eq!(result.season, Season::SummerToWinter);
        assert_eq!(result.tick_into_phase, 0);
        assert_eq!(result.year_index, 1);
    }

    // -------- plan-lingtian-weather-v1 §2 modifier 单测 --------

    #[test]
    fn season_default_is_summer() {
        // 与 query_season("", 0) 在 jiezeq-v1 下保持一致：tick 0 → Summer。
        assert_eq!(Season::default(), Season::Summer);
    }

    #[test]
    fn plot_qi_cap_modifier_summer_minus_0_2() {
        assert!(
            (Season::Summer.plot_qi_cap_modifier() + 0.2).abs() < 1e-6,
            "Summer 夏散：plot_qi_cap_modifier 应当 -0.2，实际 {}",
            Season::Summer.plot_qi_cap_modifier()
        );
    }

    #[test]
    fn plot_qi_cap_modifier_winter_plus_0_2() {
        assert!(
            (Season::Winter.plot_qi_cap_modifier() - 0.2).abs() < 1e-6,
            "Winter 冬聚：plot_qi_cap_modifier 应当 +0.2，实际 {}",
            Season::Winter.plot_qi_cap_modifier()
        );
    }

    #[test]
    fn plot_qi_cap_modifier_xizhuan_zero_base() {
        assert_eq!(Season::SummerToWinter.plot_qi_cap_modifier(), 0.0);
        assert_eq!(Season::WinterToSummer.plot_qi_cap_modifier(), 0.0);
    }

    #[test]
    fn natural_supply_modifier_summer_minus_10_percent() {
        assert!((Season::Summer.natural_supply_modifier() + 0.10).abs() < 1e-6);
    }

    #[test]
    fn natural_supply_modifier_winter_plus_10_percent() {
        assert!((Season::Winter.natural_supply_modifier() - 0.10).abs() < 1e-6);
    }

    #[test]
    fn natural_supply_modifier_xizhuan_zero_base() {
        assert_eq!(Season::SummerToWinter.natural_supply_modifier(), 0.0);
        assert_eq!(Season::WinterToSummer.natural_supply_modifier(), 0.0);
    }

    #[test]
    fn zone_flow_multiplier_summer_1_3() {
        assert!((Season::Summer.zone_flow_multiplier() - 1.3).abs() < 1e-6);
    }

    #[test]
    fn zone_flow_multiplier_winter_0_7() {
        assert!((Season::Winter.zone_flow_multiplier() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn zone_flow_multiplier_xizhuan_1_0_base() {
        assert!((Season::SummerToWinter.zone_flow_multiplier() - 1.0).abs() < 1e-6);
        assert!((Season::WinterToSummer.zone_flow_multiplier() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn xizhuan_qi_cap_amplitude_only_in_xizhuan() {
        assert_eq!(Season::Summer.xizhuan_qi_cap_amplitude(), 0.0);
        assert_eq!(Season::Winter.xizhuan_qi_cap_amplitude(), 0.0);
        assert!((Season::SummerToWinter.xizhuan_qi_cap_amplitude() - 0.3).abs() < 1e-6);
        assert!((Season::WinterToSummer.xizhuan_qi_cap_amplitude() - 0.3).abs() < 1e-6);
    }

    #[test]
    fn xizhuan_supply_amplitude_only_in_xizhuan() {
        assert_eq!(Season::Summer.xizhuan_supply_amplitude(), 0.0);
        assert_eq!(Season::Winter.xizhuan_supply_amplitude(), 0.0);
        assert!((Season::SummerToWinter.xizhuan_supply_amplitude() - 0.20).abs() < 1e-6);
        assert!((Season::WinterToSummer.xizhuan_supply_amplitude() - 0.20).abs() < 1e-6);
    }

    #[test]
    fn xizhuan_zone_flow_jitter_max_delta_only_in_xizhuan() {
        assert_eq!(Season::Summer.xizhuan_zone_flow_jitter_max_delta(), 0.0);
        assert_eq!(Season::Winter.xizhuan_zone_flow_jitter_max_delta(), 0.0);
        assert!((Season::SummerToWinter.xizhuan_zone_flow_jitter_max_delta() - 0.5).abs() < 1e-6);
        assert!((Season::WinterToSummer.xizhuan_zone_flow_jitter_max_delta() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn weather_rng_multiplier_doubles_in_xizhuan() {
        // §0 / §3 — 汐转期"全部事件 RNG ×2"
        assert_eq!(Season::Summer.weather_rng_multiplier(), 1.0);
        assert_eq!(Season::Winter.weather_rng_multiplier(), 1.0);
        assert_eq!(Season::SummerToWinter.weather_rng_multiplier(), 2.0);
        assert_eq!(Season::WinterToSummer.weather_rng_multiplier(), 2.0);
    }
}
