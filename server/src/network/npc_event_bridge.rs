use valence::prelude::{EventReader, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::npc::faction::FactionEventNotice;
use crate::npc::lifecycle::{NpcDeathNotice, NpcSpawnNotice};
use crate::schema::npc::{FactionEventV1, NpcDeathV1, NpcSpawnedV1};

const NPC_EVENT_VERSION: u8 = 1;

pub fn publish_npc_spawn_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<NpcSpawnNotice>,
) {
    for ev in events.read() {
        let wire = NpcSpawnedV1 {
            v: NPC_EVENT_VERSION,
            kind: "npc_spawned".to_string(),
            npc_id: ev.npc_id.clone(),
            archetype: ev.archetype.as_str().to_string(),
            source: ev.source.as_str().to_string(),
            zone: ev.home_zone.clone(),
            pos: [ev.position.x, ev.position.y, ev.position.z],
            initial_age_ticks: ev.initial_age_ticks,
            at_tick: 0,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcSpawned(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcSpawned: {error}");
        }
    }
}

pub fn publish_npc_death_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<NpcDeathNotice>,
) {
    for ev in events.read() {
        let wire = NpcDeathV1 {
            v: NPC_EVENT_VERSION,
            kind: "npc_death".to_string(),
            npc_id: ev.npc_id.clone(),
            archetype: ev.archetype.as_str().to_string(),
            cause: ev.reason.as_str().to_string(),
            faction_id: ev.faction_id.map(|faction| faction.as_str().to_string()),
            life_record_snapshot: ev.life_record_snapshot.clone(),
            age_ticks: ev.age_ticks,
            max_age_ticks: ev.max_age_ticks,
            at_tick: 0,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcDeath(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcDeath: {error}");
        }
    }
}

pub fn publish_faction_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<FactionEventNotice>,
) {
    for ev in events.read() {
        let wire = FactionEventV1 {
            v: NPC_EVENT_VERSION,
            kind: "faction_event".to_string(),
            faction_id: ev.applied.faction_id.as_str().to_string(),
            event_kind: ev.applied.kind.as_str().to_string(),
            leader_id: ev.applied.leader_id.clone(),
            loyalty_bias: ev.applied.loyalty_bias,
            mission_queue_size: ev.applied.mission_queue_size,
            at_tick: 0,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::FactionEvent(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped FactionEvent: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::redis_bridge::RedisOutbound;
    use crate::npc::faction::{FactionEventApplied, FactionEventKind, FactionId};
    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, Update};

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        (app, rx_outbound)
    }

    #[test]
    fn publish_faction_events_uses_dedicated_outbound_variant() {
        let (mut app, rx) = setup_app();
        app.add_event::<FactionEventNotice>();
        app.add_systems(Update, publish_faction_events);

        app.world_mut().send_event(FactionEventNotice {
            applied: FactionEventApplied {
                faction_id: FactionId::Attack,
                kind: FactionEventKind::AdjustLoyaltyBias,
                leader_id: None,
                loyalty_bias: 0.7,
                mission_queue_size: 2,
            },
        });
        app.update();

        let outbound = rx.try_recv().expect("expected faction event outbound");
        let RedisOutbound::FactionEvent(payload) = outbound else {
            panic!("expected FactionEvent outbound");
        };
        assert_eq!(payload.faction_id, "attack");
        assert_eq!(payload.event_kind, "adjust_loyalty_bias");
    }
}
