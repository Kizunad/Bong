# Season Rhythm API

`server/src/world/season/mod.rs` is the shared rhythm boundary for
`plan-jiezeq-v1`.

## Query Contract

- Call `query_season(zone, tick)` from downstream systems.
- The returned `SeasonState` contains `season`, `tick_into_phase`,
  `phase_total_ticks`, and `year_index`.
- `zone` is accepted for future compatibility, but the current rhythm is
  intentionally global and synchronized across the whole server.
- Tests and mocks should pass an explicit tick instead of reading
  `WorldSeasonState` directly.

## Runtime State

- `WorldSeasonState` stores the current effective rhythm state.
- `season_tick` advances it from `CultivationClock`.
- `SeasonChangedEvent` is emitted only when the phase changes.
- `/season query`, `/season set <phase>`, and `/season advance <N>[h|d|y|t]`
  are operator-only dev commands and must not broadcast player-facing labels.

## Downstream Hooks

- Cultivation breakthrough should multiply success by
  `season_success_modifier(query_season(zone, tick).season)`.
- Shelflife readers should use the `*_with_season` compute helpers so summer
  dispersal, winter condensation, tide-turn chaos, and frozen-container
  exceptions stay centralized.
- Karma calamity rolls should call `targeted_calamity_roll_with_season`.
- Lifespan aging should apply `season_aging_modifier`.
- Terrain follow-up plans can use `query_season(zone, tick).season.is_xizhuan()`
  to double ancient array-core activation and pseudo-vein refresh rates.
- Botany and weather follow-up plans should consume this API instead of
  defining their own season clock.
