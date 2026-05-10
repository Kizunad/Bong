use valence::prelude::{Entity, EventReader, Query, Res, Username};

use crate::combat::baomai_v3::{BaomaiSkillEvent, BaomaiSkillId};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::baomai_v3::{BaomaiSkillEventV1, BaomaiSkillIdV1};
use crate::schema::cultivation::meridian_id_to_string;

pub fn publish_baomai_v3_skill_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<BaomaiSkillEvent>,
    usernames: Query<&Username>,
) {
    for event in events.read() {
        let mut payload = BaomaiSkillEventV1::new(
            baomai_skill_payload(event.skill),
            entity_wire_id(usernames.get(event.caster).ok(), event.caster),
            event.tick,
        );
        payload.target_id = event
            .target
            .map(|target| entity_wire_id(usernames.get(target).ok(), target));
        payload.qi_invested = event.qi_invested;
        payload.damage = event.damage;
        payload.radius_blocks = event.radius_blocks;
        payload.blood_multiplier = event.blood_multiplier;
        payload.flow_rate_multiplier = event.flow_rate_multiplier;
        payload.meridian_ids = event
            .meridian_dependencies
            .iter()
            .map(|id| meridian_id_to_string(*id).to_string())
            .collect();

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV3SkillEvent(payload))
        {
            tracing::warn!("[bong][baomai-v3] failed to queue skill event: {error}");
        }
    }
}

fn baomai_skill_payload(skill: BaomaiSkillId) -> BaomaiSkillIdV1 {
    match skill {
        BaomaiSkillId::BengQuan => BaomaiSkillIdV1::BengQuan,
        BaomaiSkillId::FullPowerCharge => BaomaiSkillIdV1::FullPowerCharge,
        BaomaiSkillId::FullPowerRelease => BaomaiSkillIdV1::FullPowerRelease,
        BaomaiSkillId::MountainShake => BaomaiSkillIdV1::MountainShake,
        BaomaiSkillId::BloodBurn => BaomaiSkillIdV1::BloodBurn,
        BaomaiSkillId::Disperse => BaomaiSkillIdV1::Disperse,
    }
}

fn entity_wire_id(username: Option<&Username>, entity: Entity) -> String {
    username
        .map(|username| format!("offline:{}", username.0))
        .unwrap_or_else(|| format!("char:{}", entity.to_bits()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Events, Update};

    use crate::cultivation::components::MeridianId;

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<BaomaiSkillEvent>();
        app.add_systems(Update, publish_baomai_v3_skill_events);
        (app, rx_outbound)
    }

    #[test]
    fn publishes_baomai_skill_event_on_plan_channel() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<BaomaiSkillEvent>>()
            .send(BaomaiSkillEvent {
                skill: BaomaiSkillId::Disperse,
                caster,
                target: None,
                tick: 42,
                qi_invested: 5350.0,
                damage: 0.0,
                radius_blocks: None,
                blood_multiplier: 1.0,
                flow_rate_multiplier: 10.0,
                meridian_dependencies: vec![MeridianId::Ren, MeridianId::Du],
            });

        app.update();

        match rx_outbound.try_recv().expect("baomai event should publish") {
            RedisOutbound::BaomaiV3SkillEvent(payload) => {
                assert_eq!(payload.skill_id, BaomaiSkillIdV1::Disperse);
                assert_eq!(payload.flow_rate_multiplier, 10.0);
                assert_eq!(payload.meridian_ids, vec!["Ren", "Du"]);
                assert!(payload.caster_id.starts_with("char:"));
            }
            other => panic!("expected baomai skill outbound, got {other:?}"),
        }
    }
}
