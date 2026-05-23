use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::prelude::{bevy_ecs, Commands, Component, Entity, Position, Query, Res, With};

use crate::cultivation::tick::CultivationClock;
use crate::npc::lifecycle::PendingRetirement;
use crate::npc::lod::NpcLodTier;
use crate::npc::schedule::{
    nearest_poi_for_activity, NpcDailySchedule, ScheduleActivity, DAILY_POI_SEARCH_RADIUS,
};
use crate::npc::spawn::NpcMarker;
use crate::world::poi_novice::PoiNoviceRegistry;

use super::TRADE_STALL_BASELINE_SCORE;

// ---------------------------------------------------------------------------
// TradeStallScorer
// ---------------------------------------------------------------------------

/// Schedule-driven trade stall scorer.
#[derive(Clone, Copy, Debug, Component)]
pub struct TradeStallScorer;

impl ScorerBuilder for TradeStallScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("TradeStallScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn trade_stall_scorer_system(
    npcs: Query<
        (
            &Position,
            &NpcDailySchedule,
            Option<&PendingRetirement>,
            Option<&NpcLodTier>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<TradeStallScorer>>,
    clock: Option<Res<CultivationClock>>,
    pois: Option<Res<PoiNoviceRegistry>>,
) {
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    for (Actor(actor), mut score) in &mut scorers {
        let value = match npcs.get(*actor) {
            Ok((position, schedule, pending, tier)) => {
                let has_trade_spot = nearest_poi_for_activity(
                    pois.as_deref(),
                    position.get(),
                    ScheduleActivity::Trade,
                    DAILY_POI_SEARCH_RADIUS,
                )
                .is_some();
                if pending.is_some()
                    || !matches!(tier.copied().unwrap_or(NpcLodTier::Near), NpcLodTier::Near)
                    || !has_trade_spot
                {
                    0.0
                } else {
                    TRADE_STALL_BASELINE_SCORE
                        * schedule.weight(schedule.phase(tick), ScheduleActivity::Trade)
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}
