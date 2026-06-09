use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Query, Res, With};

use crate::cultivation::components::Cultivation;
use crate::cultivation::tick::CultivationClock;
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{
    NpcAgingConfig, NpcArchetype, NpcLifespan, NpcRegistry, PendingRetirement,
};
use crate::npc::lod::{lod_gated_score, NpcLodConfig, NpcLodTick, NpcLodTier};
use crate::npc::schedule::{
    schedule_multiplier, scheduled_wander_score, NpcDailySchedule, NpcHomeBase, ScheduleActivity,
};
use crate::npc::spawn::NpcMarker;
use crate::world::tiandao_hunt::{TiandaoAttention, TiandaoResponseLevel};

use super::WANDER_BASELINE_SCORE;

// ---------------------------------------------------------------------------
// AgeingScorer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct AgeingScorer;

impl ScorerBuilder for AgeingScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("AgeingScorer")
    }
}

pub(crate) fn ageing_scorer_system(
    npcs: Query<(&NpcLifespan, &NpcArchetype, Option<&PendingRetirement>), With<NpcMarker>>,
    registry: Option<Res<NpcRegistry>>,
    config: Option<Res<NpcAgingConfig>>,
    mut scorers: Query<(&Actor, &mut Score), With<AgeingScorer>>,
) {
    let aging_enabled = config.as_deref().map(|cfg| cfg.enabled).unwrap_or(true);
    let should_reduce_population = registry
        .as_deref()
        .map(NpcRegistry::should_reduce_population)
        .unwrap_or(false);

    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((lifespan, archetype, pending_retirement)) = npcs.get(*actor) {
            if pending_retirement.is_some()
                || !aging_enabled
                || *archetype == NpcArchetype::GuardianRelic
            {
                0.0
            } else if lifespan.is_expired() {
                1.0
            } else if should_reduce_population && lifespan.age_ratio() >= 0.8 {
                0.8
            } else {
                0.0
            }
        } else {
            0.0
        };

        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// HungerScorer
// ---------------------------------------------------------------------------

/// Commoner/Beast hunger scorer: lower `Hunger` -> higher score.
#[derive(Clone, Copy, Debug, Component)]
pub struct HungerScorer;

impl ScorerBuilder for HungerScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("HungerScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn hunger_scorer_system(
    npcs: Query<(&Hunger, Option<&NpcDailySchedule>, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<HungerScorer>>,
    clock: Option<Res<CultivationClock>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((h, schedule, tier)) => {
                match lod_gated_score(tier, tick, &cfg, || h.hunger_pressure()) {
                    Some(value) => {
                        let multiplier = schedule_multiplier(
                            schedule,
                            tier,
                            clock_tick,
                            ScheduleActivity::Forage,
                        )
                        .unwrap_or(1.0);
                        value * multiplier
                    }
                    None => continue,
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// WanderScorer
// ---------------------------------------------------------------------------

/// Commoner daily wander scorer (fallback baseline, always > picker threshold).
#[derive(Clone, Copy, Debug, Component)]
pub struct WanderScorer;

impl ScorerBuilder for WanderScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("WanderScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn wander_scorer_system(
    npcs: Query<
        (
            Option<&PendingRetirement>,
            Option<&NpcDailySchedule>,
            Option<&NpcLodTier>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<WanderScorer>>,
    clock: Option<Res<CultivationClock>>,
    lod_config: Option<Res<NpcLodConfig>>,
    lod_tick: Option<Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    let clock_tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((pending, schedule, tier)) => match lod_gated_score(tier, tick, &cfg, || {
                if pending.is_some() {
                    0.0
                } else {
                    WANDER_BASELINE_SCORE
                }
            }) {
                Some(value) => scheduled_wander_score(
                    schedule,
                    tier,
                    clock_tick,
                    schedule
                        .map(|schedule| schedule.seed)
                        .unwrap_or_else(|| actor.index() as u64),
                    value,
                )
                .unwrap_or(value),
                None => continue,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// ReturnHomeScorer
// ---------------------------------------------------------------------------

/// Night, low qi, or low hunger -> return home.
#[derive(Clone, Copy, Debug, Component)]
pub struct ReturnHomeScorer;

impl ScorerBuilder for ReturnHomeScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ReturnHomeScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn return_home_scorer_system(
    npcs: Query<
        (
            &NpcDailySchedule,
            &NpcHomeBase,
            Option<&Hunger>,
            Option<&Cultivation>,
            Option<&PendingRetirement>,
            Option<&NpcLodTier>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<ReturnHomeScorer>>,
    clock: Option<Res<CultivationClock>>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((schedule, _home, hunger, cultivation, pending, tier)) => {
                if pending.is_some()
                    || !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Near)
                {
                    0.0
                } else {
                    return_home_score(schedule, hunger, cultivation, tick)
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

pub(crate) fn return_home_score(
    schedule: &NpcDailySchedule,
    hunger: Option<&Hunger>,
    cultivation: Option<&Cultivation>,
    tick: u64,
) -> f32 {
    let phase = schedule.phase(tick);
    let night_rest = if matches!(phase, crate::npc::schedule::DayPhase::Night) {
        0.6_f32 * schedule.weight(phase, ScheduleActivity::Rest)
    } else {
        0.0
    };
    let low_qi = cultivation
        .filter(|cultivation| {
            cultivation.qi_max > f64::EPSILON && cultivation.qi_current / cultivation.qi_max < 0.2
        })
        .map(|_| 0.8_f32)
        .unwrap_or(0.0);
    let low_hunger = hunger
        .filter(|hunger| hunger.value < 0.3)
        .map(|_| 0.5_f32)
        .unwrap_or(0.0);
    night_rest.max(low_qi).max(low_hunger)
}

// ---------------------------------------------------------------------------
// FearCultivatorScorer
// ---------------------------------------------------------------------------

use crate::cultivation::components::Realm;
use valence::client::ClientMarker;

use super::FEAR_CULTIVATOR_RANGE;

/// Mortal seeing a cultivator (Realm > Awaken) fear scorer.
#[derive(Clone, Copy, Debug, Component)]
pub struct FearCultivatorScorer;

impl ScorerBuilder for FearCultivatorScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("FearCultivatorScorer")
    }
}

/// Realm -> fear weight (Awaken = 0).
fn realm_fear_weight(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 0.25,
        Realm::Condense => 0.5,
        Realm::Solidify => 0.75,
        Realm::Spirit => 0.9,
        Realm::Void => 1.0,
    }
}

/// Distance falloff: 0 -> 1; `max_range` and beyond -> 0.
fn fear_distance_falloff(distance: f32, max_range: f32) -> f32 {
    if !distance.is_finite() || distance >= max_range {
        0.0
    } else {
        (1.0 - distance / max_range).clamp(0.0, 1.0)
    }
}

pub(crate) fn fear_cultivator_score(distance: f32, realm: Realm) -> f32 {
    realm_fear_weight(realm) * fear_distance_falloff(distance, FEAR_CULTIVATOR_RANGE)
}

fn tiandao_watch_fear_bonus(attention: Option<&TiandaoAttention>) -> f32 {
    match attention.map(|attention| attention.response) {
        Some(TiandaoResponseLevel::Watch) => 0.2,
        Some(TiandaoResponseLevel::Pressure) => 0.5,
        Some(TiandaoResponseLevel::Tribulation | TiandaoResponseLevel::Annihilate) => 1.0,
        _ => 0.0,
    }
}

pub(crate) fn fear_cultivator_scorer_system(
    npcs: Query<&NpcBlackboard, With<NpcMarker>>,
    players: Query<(&Cultivation, Option<&TiandaoAttention>), With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<FearCultivatorScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok(bb) => match bb.nearest_player.and_then(|e| players.get(e).ok()) {
                Some((cult, attention)) => (fear_cultivator_score(bb.player_distance, cult.realm)
                    + tiandao_watch_fear_bonus(attention))
                .clamp(0.0, 1.0),
                None => 0.0,
            },
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

use crate::npc::spawn::NpcBlackboard;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::schedule::NpcDailySchedule;
    use big_brain::prelude::BigBrainSet;
    use valence::prelude::{App, DVec3, IntoSystemConfigs, Position, PreUpdate};

    #[test]
    fn fear_score_is_zero_for_awaken_realm_regardless_of_distance() {
        assert_eq!(fear_cultivator_score(0.0, Realm::Awaken), 0.0);
        assert_eq!(fear_cultivator_score(5.0, Realm::Awaken), 0.0);
        assert_eq!(fear_cultivator_score(49.0, Realm::Awaken), 0.0);
    }

    #[test]
    fn fear_score_scales_with_realm_and_proximity() {
        let near_void = fear_cultivator_score(1.0, Realm::Void);
        let far_void = fear_cultivator_score(45.0, Realm::Void);
        let near_induce = fear_cultivator_score(1.0, Realm::Induce);

        assert!(
            near_void > far_void,
            "higher proximity -> higher fear, got near={near_void} far={far_void}"
        );
        assert!(
            near_void > near_induce,
            "higher realm at same distance -> higher fear"
        );
        assert_eq!(
            fear_cultivator_score(FEAR_CULTIVATOR_RANGE, Realm::Void),
            0.0,
            "fear at or beyond range must drop to 0"
        );
    }

    #[test]
    fn hunger_scorer_is_inverse_of_hunger_value() {
        let mut app = App::new();
        app.add_systems(PreUpdate, hunger_scorer_system.in_set(BigBrainSet::Scorers));

        let npc = app.world_mut().spawn((NpcMarker, Hunger::new(0.25))).id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), HungerScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!((score - 0.75).abs() < 1e-5);
    }

    #[test]
    fn wander_scorer_is_zero_when_pending_retirement() {
        let mut app = App::new();
        app.add_systems(PreUpdate, wander_scorer_system.in_set(BigBrainSet::Scorers));

        let npc = app.world_mut().spawn((NpcMarker, PendingRetirement)).id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), WanderScorer))
            .id();

        app.update();

        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn return_home_high_score_at_night() {
        let mut schedule = NpcDailySchedule::for_archetype(NpcArchetype::Rogue, 0);
        schedule.phase_offset_ticks = 0;
        let hunger = Hunger::new(0.9);
        let cultivation = Cultivation {
            qi_current: 90.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let score = return_home_score(&schedule, Some(&hunger), Some(&cultivation), 20_000);
        assert!(
            score >= 0.36,
            "night Rest weight should drive return-home, got {score}"
        );
    }

    #[test]
    fn fear_cultivator_scorer_reads_realm_from_nearest_player() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            fear_cultivator_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Solidify,
                    ..Cultivation::default()
                },
            ))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 10.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), FearCultivatorScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            score > 0.4,
            "solidify-realm player at 10 blocks should score above 0.4, got {score}"
        );
    }

    #[test]
    fn fear_cultivator_scorer_adds_watch_bonus_from_tiandao_attention() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            fear_cultivator_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app
            .world_mut()
            .spawn((
                ClientMarker,
                Position::new([0.0, 66.0, 0.0]),
                Cultivation {
                    realm: Realm::Solidify,
                    ..Cultivation::default()
                },
                TiandaoAttention {
                    response: TiandaoResponseLevel::Watch,
                    ..TiandaoAttention::default()
                },
            ))
            .id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 10.0,
                    target_position: Some(DVec3::new(0.0, 66.0, 0.0)),
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), FearCultivatorScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        let baseline = fear_cultivator_score(10.0, Realm::Solidify);
        assert!(
            score > baseline,
            "Watch 级天道注视必须提升 NPC flee/fear scorer，baseline={baseline} score={score}"
        );
        assert!((score - (baseline + 0.2)).abs() < 1e-5);
    }
}
