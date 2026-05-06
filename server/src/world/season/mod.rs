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
}
