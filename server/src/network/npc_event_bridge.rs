use valence::prelude::{EventReader, Res};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::npc::faction::FactionEventNotice;
use crate::npc::lifecycle::{NpcDeathNotice, NpcSpawnNotice};
use crate::npc::movement::GameTick;
use crate::schema::npc::{FactionEventV1, NpcDeathV1, NpcSpawnedV1};

const NPC_EVENT_VERSION: u8 = 1;

pub fn publish_npc_spawn_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<NpcSpawnNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
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
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcSpawned(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcSpawned: {error}");
        }
    }
}

pub fn publish_npc_death_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<NpcDeathNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
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
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::NpcDeath(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped NpcDeath: {error}");
        }
    }
}

pub fn publish_faction_events(
    redis: Res<RedisBridgeResource>,
    game_tick: Option<Res<GameTick>>,
    mut events: EventReader<FactionEventNotice>,
) {
    let at_tick = current_game_tick(game_tick.as_deref());
    for ev in events.read() {
        let wire = FactionEventV1 {
            v: NPC_EVENT_VERSION,
            kind: "faction_event".to_string(),
            faction_id: ev.applied.faction_id.as_str().to_string(),
            event_kind: ev.applied.kind.as_str().to_string(),
            leader_id: ev.applied.leader_id.clone(),
            loyalty_bias: ev.applied.loyalty_bias,
            mission_queue_size: ev.applied.mission_queue_size,
            at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::FactionEvent(wire)) {
            tracing::warn!("[bong][npc_event_bridge] dropped FactionEvent: {error}");
        }
    }
}

fn current_game_tick(game_tick: Option<&GameTick>) -> u64 {
    game_tick.map(|tick| u64::from(tick.0)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::redis_bridge::RedisOutbound;
    use crate::npc::faction::{FactionEventApplied, FactionEventKind, FactionId};
    use crate::npc::lifecycle::{NpcArchetype, NpcDeathReason};
    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, DVec3, Update};

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
        app.insert_resource(GameTick(321));
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
        assert_eq!(payload.at_tick, 321);
    }

    #[test]
    fn publish_spawn_and_death_events_use_game_tick() {
        let (mut app, rx) = setup_app();
        app.add_event::<NpcSpawnNotice>();
        app.add_event::<NpcDeathNotice>();
        app.insert_resource(GameTick(654));
        app.add_systems(Update, (publish_npc_spawn_events, publish_npc_death_events));

        app.world_mut().send_event(NpcSpawnNotice {
            npc_id: "npc_1v1".to_string(),
            archetype: NpcArchetype::Rogue,
            source: crate::npc::lifecycle::NpcSpawnSource::AgentCommand,
            home_zone: "green_cloud_peak".to_string(),
            position: DVec3::new(1.0, 64.0, 2.0),
            initial_age_ticks: 0.0,
        });
        app.world_mut().send_event(NpcDeathNotice {
            npc_id: "npc_2v1".to_string(),
            archetype: NpcArchetype::Commoner,
            reason: NpcDeathReason::Combat,
            faction_id: Some(FactionId::Neutral),
            life_record_snapshot: None,
            age_ticks: 10.0,
            max_age_ticks: 20.0,
        });
        app.update();

        let outbounds = [
            rx.try_recv().expect("expected first NPC event outbound"),
            rx.try_recv().expect("expected second NPC event outbound"),
        ];
        assert!(outbounds.iter().any(|outbound| matches!(
            outbound,
            RedisOutbound::NpcSpawned(payload) if payload.at_tick == 654
        )));
        assert!(outbounds.iter().any(|outbound| matches!(
            outbound,
            RedisOutbound::NpcDeath(payload) if payload.at_tick == 654
        )));
    }
}
