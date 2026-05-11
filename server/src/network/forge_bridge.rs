//! 炼器（武器）事件 → Redis outbound 桥。

use valence::prelude::{Entity, EventReader, Query, Res, Username};

use super::cast_emit::current_unix_millis;
use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::forge::events::{ForgeOutcomeEvent, ForgeStartAccepted};
use crate::player::state::canonical_player_id;
use crate::schema::forge::ForgeOutcomeBucketV1;
use crate::schema::forge_bridge::{
    ForgeMaterialStackV1, ForgeOutcomePayloadV1, ForgeStartPayloadV1,
};

pub fn publish_forge_start_on_session_create(
    redis: Option<Res<RedisBridgeResource>>,
    mut reader: EventReader<ForgeStartAccepted>,
    names: Query<&Username>,
) {
    let Some(redis) = redis else {
        reader.clear();
        return;
    };

    for event in reader.read() {
        let payload = ForgeStartPayloadV1 {
            v: 1,
            session_id: event.session.0,
            blueprint_id: event.blueprint.clone(),
            station_id: station_wire_id(event.station),
            caster_id: caster_wire_id(event.caster, &names),
            materials: event
                .materials
                .iter()
                .map(|(material, count)| ForgeMaterialStackV1 {
                    material: material.clone(),
                    count: *count,
                })
                .collect(),
            ts: current_unix_millis(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::ForgeStart(payload));
    }
}

pub fn publish_forge_outcome(
    redis: Option<Res<RedisBridgeResource>>,
    mut reader: EventReader<ForgeOutcomeEvent>,
    names: Query<&Username>,
) {
    let Some(redis) = redis else {
        reader.clear();
        return;
    };

    for event in reader.read() {
        let payload = ForgeOutcomePayloadV1 {
            v: 1,
            session_id: event.session.0,
            blueprint_id: event.blueprint.clone(),
            bucket: ForgeOutcomeBucketV1::from(event.bucket),
            weapon_item: event.weapon_item.clone(),
            quality: event.quality,
            color: event.color,
            side_effects: event.side_effects.clone(),
            achieved_tier: event.achieved_tier,
            caster_id: caster_wire_id(event.caster, &names),
            ts: current_unix_millis(),
        };
        let _ = redis.tx_outbound.send(RedisOutbound::ForgeOutcome(payload));
    }
}

fn station_wire_id(station: Entity) -> String {
    format!("forge_station_{}", station.to_bits())
}

fn caster_wire_id(caster: Entity, names: &Query<&Username>) -> String {
    names
        .get(caster)
        .map(|username| canonical_player_id(username.0.as_str()))
        .unwrap_or_else(|_| format!("entity:{}", caster.to_bits()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::events::{ForgeBucket, ForgeOutcomeEvent, ForgeStartAccepted};
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, Update};

    #[test]
    fn publish_forge_start_on_session_create_queues_payload() {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<ForgeStartAccepted>();
        app.add_systems(Update, publish_forge_start_on_session_create);

        let caster = app.world_mut().spawn(Username("Azure".to_string())).id();
        let station = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ForgeStartAccepted {
            session: crate::forge::session::ForgeSessionId(7),
            station,
            caster,
            blueprint: "qing_feng_v0".to_string(),
            materials: vec![("fan_tie".to_string(), 3)],
        });

        app.update();

        let payload = match rx_outbound.try_recv().expect("forge start should publish") {
            RedisOutbound::ForgeStart(payload) => payload,
            other => panic!("expected ForgeStart, got {other:?}"),
        };
        assert_eq!(payload.session_id, 7);
        assert_eq!(payload.blueprint_id, "qing_feng_v0");
        assert_eq!(payload.caster_id, "offline:Azure");
        assert_eq!(payload.materials[0].material, "fan_tie");
    }

    #[test]
    fn publish_forge_outcome_queues_payload() {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<ForgeOutcomeEvent>();
        app.add_systems(Update, publish_forge_outcome);

        let caster = app.world_mut().spawn(Username("Azure".to_string())).id();
        app.world_mut().send_event(ForgeOutcomeEvent {
            session: crate::forge::session::ForgeSessionId(8),
            caster,
            blueprint: "qing_feng_v0".to_string(),
            bucket: ForgeBucket::Flawed,
            weapon_item: Some("iron_sword".to_string()),
            quality: 0.42,
            color: None,
            side_effects: vec!["brittle_edge".to_string()],
            achieved_tier: 1,
            consecration_qi_amount: 0.0,
        });

        app.update();

        let payload = match rx_outbound
            .try_recv()
            .expect("forge outcome should publish")
        {
            RedisOutbound::ForgeOutcome(payload) => payload,
            other => panic!("expected ForgeOutcome, got {other:?}"),
        };
        assert_eq!(payload.session_id, 8);
        assert_eq!(payload.bucket, ForgeOutcomeBucketV1::Flawed);
        assert_eq!(payload.side_effects, vec!["brittle_edge"]);
    }
}
