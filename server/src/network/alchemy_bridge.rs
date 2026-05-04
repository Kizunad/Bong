//! 炼丹事件 → Redis outbound 桥。

use valence::prelude::{EventReader, Query, Res};

use super::cast_emit::current_unix_millis;
use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::alchemy::{
    danxin::AlchemyInsightEvent, AlchemyFurnace, AlchemyOutcomeEvent, ResolvedOutcome,
};
use crate::schema::alchemy::{AlchemyInsightV1, AlchemySessionEndV1};

pub fn publish_alchemy_session_end_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut reader: EventReader<AlchemyOutcomeEvent>,
    furnaces: Query<&AlchemyFurnace>,
) {
    let Some(redis) = redis else {
        reader.clear();
        return;
    };

    for event in reader.read() {
        let Ok(furnace) = furnaces.get(event.furnace) else {
            continue;
        };
        let Some(furnace_pos) = furnace.pos else {
            continue;
        };
        let payload = session_end_payload(event, furnace_pos, furnace.tier);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AlchemySessionEnd(payload));
    }
}

pub fn publish_alchemy_insight_events(
    redis: Option<Res<RedisBridgeResource>>,
    mut reader: EventReader<AlchemyInsightEvent>,
) {
    let Some(redis) = redis else {
        reader.clear();
        return;
    };

    for event in reader.read() {
        let payload = AlchemyInsightV1 {
            v: 1,
            player_id: event.player_id.clone(),
            source_pill: event.hint.source_pill.clone(),
            recipe_id: event.hint.recipe_id.clone(),
            accuracy: event.hint.accuracy,
            ingredients: event.hint.ingredients.clone(),
            ts: current_unix_millis(),
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::AlchemyInsight(payload));
    }
}

pub(crate) fn alchemy_session_id(
    furnace_pos: (i32, i32, i32),
    caster_id: &str,
    recipe_id: &str,
) -> String {
    format!(
        "alchemy:{}:{}:{}:{}:{}",
        furnace_pos.0, furnace_pos.1, furnace_pos.2, caster_id, recipe_id
    )
}

pub(crate) fn alchemy_outcome_recipe_id(outcome: &ResolvedOutcome) -> Option<String> {
    match outcome {
        ResolvedOutcome::Pill { recipe_id, .. } => Some(recipe_id.clone()),
        ResolvedOutcome::Waste { recipe_id } => recipe_id.clone(),
        ResolvedOutcome::Explode { .. } | ResolvedOutcome::Mismatch => None,
    }
}

pub(crate) fn alchemy_outcome_pill(outcome: &ResolvedOutcome) -> Option<String> {
    match outcome {
        ResolvedOutcome::Pill { pill, .. } => Some(pill.clone()),
        _ => None,
    }
}

pub(crate) fn alchemy_outcome_quality(outcome: &ResolvedOutcome) -> Option<f64> {
    match outcome {
        ResolvedOutcome::Pill { quality, .. } => Some(*quality),
        _ => None,
    }
}

pub(crate) fn alchemy_outcome_damage(outcome: &ResolvedOutcome) -> Option<f64> {
    match outcome {
        ResolvedOutcome::Explode { damage, .. } => Some(*damage),
        _ => None,
    }
}

pub(crate) fn alchemy_outcome_meridian_crack(outcome: &ResolvedOutcome) -> Option<f64> {
    match outcome {
        ResolvedOutcome::Explode { meridian_crack, .. } => Some(*meridian_crack),
        _ => None,
    }
}

fn session_end_payload(
    event: &AlchemyOutcomeEvent,
    furnace_pos: (i32, i32, i32),
    furnace_tier: u8,
) -> AlchemySessionEndV1 {
    let recipe_id = alchemy_outcome_recipe_id(&event.outcome).or_else(|| event.recipe_id.clone());
    let session_recipe = recipe_id.as_deref().unwrap_or("unknown");
    AlchemySessionEndV1 {
        v: 1,
        session_id: alchemy_session_id(furnace_pos, event.caster_id.as_str(), session_recipe),
        recipe_id,
        furnace_pos,
        furnace_tier,
        caster_id: event.caster_id.clone(),
        bucket: event.bucket.into(),
        pill: alchemy_outcome_pill(&event.outcome),
        quality: alchemy_outcome_quality(&event.outcome),
        damage: alchemy_outcome_damage(&event.outcome),
        meridian_crack: alchemy_outcome_meridian_crack(&event.outcome),
        elapsed_ticks: event.elapsed_ticks,
        ts: current_unix_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::outcome::OutcomeBucket;
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Update};

    #[test]
    fn publish_alchemy_session_end_queues_payload() {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<AlchemyOutcomeEvent>();
        app.add_systems(Update, publish_alchemy_session_end_events);

        let furnace = app
            .world_mut()
            .spawn(AlchemyFurnace {
                tier: 1,
                pos: Some((-12, 64, 38)),
                ..Default::default()
            })
            .id();
        app.world_mut().send_event(AlchemyOutcomeEvent {
            furnace,
            caster_id: "offline:Azure".to_string(),
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            bucket: OutcomeBucket::Explode,
            outcome: ResolvedOutcome::Explode {
                damage: 12.0,
                meridian_crack: 0.2,
            },
            elapsed_ticks: 120,
        });

        app.update();

        let payload = match rx_outbound
            .try_recv()
            .expect("alchemy session end should publish")
        {
            RedisOutbound::AlchemySessionEnd(payload) => payload,
            other => panic!("expected AlchemySessionEnd, got {other:?}"),
        };
        assert_eq!(payload.furnace_pos, (-12, 64, 38));
        assert_eq!(payload.furnace_tier, 1);
        assert_eq!(payload.caster_id, "offline:Azure");
        assert_eq!(payload.recipe_id, Some("kai_mai_pill_v0".to_string()));
        assert_eq!(
            payload.session_id,
            "alchemy:-12:64:38:offline:Azure:kai_mai_pill_v0"
        );
        assert_eq!(payload.damage, Some(12.0));
        assert_eq!(payload.elapsed_ticks, 120);
    }
}
