use valence::prelude::{bevy_ecs, Client, Entity, Event, EventReader, EventWriter, Query};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::DamageTiltS2c;
use valence::protocol::VarInt;

use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::npc::spawn_rat::RatBlackboard;
use crate::schema::server_data::{
    CombatEventFloaterEntryV1, CombatEventFloaterV1, ServerDataPayloadV1, ServerDataV1,
};

#[derive(Debug, Clone, Copy, Event, PartialEq)]
pub struct RatBiteEvent {
    pub rat: Entity,
    pub target: Entity,
    pub qi_steal: u32,
}

pub fn apply_rat_bite_qi_drain(
    mut bites: EventReader<RatBiteEvent>,
    mut cultivators: Query<&mut Cultivation>,
    mut rats: Query<&mut RatBlackboard>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut clients: Query<&mut Client>,
) {
    for bite in bites.read() {
        if bite.qi_steal == 0 {
            continue;
        }
        let Ok(mut cultivation) = cultivators.get_mut(bite.target) else {
            continue;
        };
        if cultivation.qi_current <= 0.0 {
            continue;
        }

        let before = cultivation.qi_current;
        cultivation.qi_current =
            (cultivation.qi_current - f64::from(bite.qi_steal)).clamp(0.0, cultivation.qi_max);
        let drained = (before - cultivation.qi_current).max(0.0);
        if drained > 0.0 {
            if let Ok(mut rat) = rats.get_mut(bite.rat) {
                rat.drained_qi += drained;
            }
            send_bite_feedback(&mut clients, bite.target, drained as f32);
        }
        if before > 0.0 && cultivation.qi_current <= f64::EPSILON {
            deaths.send(CultivationDeathTrigger {
                entity: bite.target,
                cause: CultivationDeathCause::SwarmQiDrain,
                context: serde_json::json!({
                    "rat": format!("{:?}", bite.rat),
                    "qi_steal": bite.qi_steal,
                }),
            });
        }
    }
}

fn send_bite_feedback(clients: &mut Query<&mut Client>, target: Entity, drained: f32) {
    let Ok(mut client) = clients.get_mut(target) else {
        return;
    };

    let entry = CombatEventFloaterEntryV1 {
        kind: "qi_damage".to_string(),
        amount: drained,
        text: format!("-{}", drained.round() as i64),
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::CombatEventFloater(
        CombatEventFloaterV1 {
            events: vec![entry],
        },
    ));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());

    client.write_packet(&DamageTiltS2c {
        entity_id: VarInt(0),
        yaw: 0.0,
    });

    tracing::debug!(
        "[bong][network] sent {} {} (qi_damage) + DamageTilt for rat bite",
        SERVER_DATA_CHANNEL,
        payload_type,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Events, Update};

    use crate::cultivation::components::{Cultivation, Realm};

    fn cultivation(qi_current: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Induce,
            qi_current,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn rat_bite_drains_only_qi_no_hp_damage() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, apply_rat_bite_qi_drain);
        let rat = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(target).unwrap().qi_current,
            3.0,
            "Rat bite must drain Cultivation.qi_current directly"
        );
        assert!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .is_empty(),
            "nonlethal rat bites must not emit cultivation death"
        );
    }

    #[test]
    fn qi_drain_to_zero_emits_swarm_death_trigger() {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_systems(Update, apply_rat_bite_qi_drain);
        let rat = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn(cultivation(1.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        let deaths = app.world().resource::<Events<CultivationDeathTrigger>>();
        let event = deaths
            .iter_current_update_events()
            .next()
            .expect("rat bite to zero qi should emit death trigger");
        assert_eq!(event.entity, target);
        assert_eq!(event.cause, CultivationDeathCause::SwarmQiDrain);
    }
}
