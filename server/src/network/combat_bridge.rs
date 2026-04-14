use std::collections::HashMap;

use valence::prelude::{Entity, EventReader, Query, Res, ResMut, Resource, Username, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use super::WORLD_STATE_PUBLISH_INTERVAL_TICKS;
use crate::combat::components::Lifecycle;
use crate::combat::events::{CombatEvent, DeathEvent};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::schema::combat_event::{CombatRealtimeEventV1, CombatRealtimeKindV1, CombatSummaryV1};

#[derive(Debug, Default)]
pub struct CombatSummaryAccumulator {
    pub window_start_tick: Option<u64>,
    pub combat_event_count: u64,
    pub death_event_count: u64,
}

impl Resource for CombatSummaryAccumulator {}

pub fn publish_combat_realtime_events(
    redis: Res<RedisBridgeResource>,
    mut summary: ResMut<CombatSummaryAccumulator>,
    mut combat_reader: EventReader<CombatEvent>,
    mut death_reader: EventReader<DeathEvent>,
    lifecycle_q: Query<&Lifecycle>,
    client_q: Query<&Username, With<valence::prelude::Client>>,
    npc_q: Query<(), With<NpcMarker>>,
) {
    let mut identity_cache = HashMap::new();

    for ev in combat_reader.read() {
        summary.window_start_tick.get_or_insert(ev.resolved_at_tick);
        summary.combat_event_count = summary.combat_event_count.saturating_add(1);

        let Some(target_id) = resolve_canonical_id(
            ev.target,
            &lifecycle_q,
            &client_q,
            &npc_q,
            &mut identity_cache,
        ) else {
            continue;
        };
        let attacker_id = resolve_canonical_id(
            ev.attacker,
            &lifecycle_q,
            &client_q,
            &npc_q,
            &mut identity_cache,
        );

        let payload = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::CombatEvent,
            tick: ev.resolved_at_tick,
            target_id,
            attacker_id,
            description: Some(ev.description.clone()),
            cause: None,
        };

        let _ = redis
            .tx_outbound
            .send(RedisOutbound::CombatRealtime(payload));
    }

    for ev in death_reader.read() {
        summary.window_start_tick.get_or_insert(ev.at_tick);
        summary.death_event_count = summary.death_event_count.saturating_add(1);

        let Some(target_id) = resolve_canonical_id(
            ev.target,
            &lifecycle_q,
            &client_q,
            &npc_q,
            &mut identity_cache,
        ) else {
            continue;
        };

        let payload = CombatRealtimeEventV1 {
            v: 1,
            kind: CombatRealtimeKindV1::DeathEvent,
            tick: ev.at_tick,
            target_id,
            attacker_id: attacker_id_from_cause(ev.cause.as_str()),
            description: None,
            cause: Some(ev.cause.clone()),
        };

        let _ = redis
            .tx_outbound
            .send(RedisOutbound::CombatRealtime(payload));
    }
}

pub fn publish_combat_summary_on_interval(
    redis: Res<RedisBridgeResource>,
    mut summary: ResMut<CombatSummaryAccumulator>,
    world_state_timer: Res<super::WorldStateTimer>,
) {
    publish_combat_summary_from_parts(
        redis.as_ref(),
        summary.as_mut(),
        world_state_timer.as_ref(),
        WORLD_STATE_PUBLISH_INTERVAL_TICKS,
    );
}

fn publish_combat_summary_from_parts(
    redis: &RedisBridgeResource,
    summary: &mut CombatSummaryAccumulator,
    world_state_timer: &super::WorldStateTimer,
    interval_ticks: u64,
) {
    if interval_ticks == 0 {
        return;
    }

    if !world_state_timer.ticks.is_multiple_of(interval_ticks) {
        return;
    }

    let window_end_tick = world_state_timer.ticks;
    let default_start_tick = window_end_tick
        .saturating_sub(interval_ticks)
        .saturating_add(1);
    let window_start_tick = summary.window_start_tick.unwrap_or(default_start_tick);

    let payload = CombatSummaryV1 {
        v: 1,
        window_start_tick,
        window_end_tick,
        combat_event_count: summary.combat_event_count,
        death_event_count: summary.death_event_count,
    };
    let _ = redis
        .tx_outbound
        .send(RedisOutbound::CombatSummary(payload));

    summary.window_start_tick = Some(window_end_tick.saturating_add(1));
    summary.combat_event_count = 0;
    summary.death_event_count = 0;
}

fn resolve_canonical_id(
    entity: Entity,
    lifecycle_q: &Query<&Lifecycle>,
    client_q: &Query<&Username, With<valence::prelude::Client>>,
    npc_q: &Query<(), With<NpcMarker>>,
    cache: &mut HashMap<Entity, String>,
) -> Option<String> {
    if let Some(id) = cache.get(&entity) {
        return Some(id.clone());
    }

    let resolved = if let Ok(lifecycle) = lifecycle_q.get(entity) {
        Some(lifecycle.character_id.clone())
    } else if let Ok(username) = client_q.get(entity) {
        Some(canonical_player_id(username.0.as_str()))
    } else if npc_q.get(entity).is_ok() {
        Some(canonical_npc_id(entity))
    } else {
        None
    };

    if let Some(id) = resolved.as_ref() {
        cache.insert(entity, id.clone());
    }

    resolved
}

fn attacker_id_from_cause(cause: &str) -> Option<String> {
    cause.split_once(':').and_then(|(_, maybe_id)| {
        let trimmed = maybe_id.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam_channel::{unbounded, Receiver};
    use valence::prelude::{App, Update};

    const TEST_WORLD_STATE_INTERVAL_TICKS: u64 = 4;

    fn setup_app() -> (App, Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        app.insert_resource(crate::network::RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.insert_resource(CombatSummaryAccumulator::default());
        app.insert_resource(crate::network::WorldStateTimer::default());
        app.add_event::<CombatEvent>();
        app.add_event::<DeathEvent>();
        (app, rx_outbound)
    }

    #[test]
    fn publishes_realtime_for_combat_and_death_events() {
        let (mut app, rx_outbound) = setup_app();
        app.add_systems(Update, publish_combat_realtime_events);

        let attacker = app
            .world_mut()
            .spawn(Lifecycle {
                character_id: "offline:AttackerCanonical".to_string(),
                ..Default::default()
            })
            .id();
        let target = app
            .world_mut()
            .spawn(Lifecycle {
                character_id: "offline:TargetCanonical".to_string(),
                ..Default::default()
            })
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 33,
            description: "shared path hit".to_string(),
        });
        app.world_mut().send_event(DeathEvent {
            target,
            cause: "attack_intent:offline:Azure".to_string(),
            at_tick: 34,
        });

        app.update();

        let first = rx_outbound
            .try_recv()
            .expect("combat realtime publish for CombatEvent should exist");
        let second = rx_outbound
            .try_recv()
            .expect("combat realtime publish for DeathEvent should exist");

        match first {
            RedisOutbound::CombatRealtime(payload) => {
                assert_eq!(payload.v, 1);
                assert_eq!(payload.kind, CombatRealtimeKindV1::CombatEvent);
                assert_eq!(payload.tick, 33);
                assert_eq!(payload.target_id, "offline:TargetCanonical");
                assert_eq!(
                    payload.attacker_id.as_deref(),
                    Some("offline:AttackerCanonical")
                );
                assert_eq!(payload.description.as_deref(), Some("shared path hit"));
                assert!(payload.cause.is_none());
            }
            other => panic!("expected CombatRealtime outbound, got {other:?}"),
        }

        match second {
            RedisOutbound::CombatRealtime(payload) => {
                assert_eq!(payload.v, 1);
                assert_eq!(payload.kind, CombatRealtimeKindV1::DeathEvent);
                assert_eq!(payload.tick, 34);
                assert_eq!(payload.target_id, "offline:TargetCanonical");
                assert_eq!(payload.attacker_id.as_deref(), Some("offline:Azure"));
                assert!(payload.description.is_none());
                assert_eq!(
                    payload.cause.as_deref(),
                    Some("attack_intent:offline:Azure")
                );
            }
            other => panic!("expected CombatRealtime outbound, got {other:?}"),
        }

        let summary = app.world().resource::<CombatSummaryAccumulator>();
        assert_eq!(summary.window_start_tick, Some(33));
        assert_eq!(summary.combat_event_count, 1);
        assert_eq!(summary.death_event_count, 1);
    }

    #[test]
    fn summary_counts_events_and_resets_on_world_state_interval() {
        let (mut app, rx_outbound) = setup_app();
        app.add_systems(
            Update,
            (
                publish_combat_realtime_events,
                |redis: Res<crate::network::RedisBridgeResource>,
                 mut summary: ResMut<CombatSummaryAccumulator>,
                 timer: Res<crate::network::WorldStateTimer>| {
                    publish_combat_summary_from_parts(
                        redis.as_ref(),
                        summary.as_mut(),
                        timer.as_ref(),
                        TEST_WORLD_STATE_INTERVAL_TICKS,
                    );
                },
            ),
        );

        let attacker = app
            .world_mut()
            .spawn(Lifecycle {
                character_id: "offline:Attacker".to_string(),
                ..Default::default()
            })
            .id();
        let target = app
            .world_mut()
            .spawn(Lifecycle {
                character_id: "offline:Target".to_string(),
                ..Default::default()
            })
            .id();

        app.world_mut().send_event(CombatEvent {
            attacker,
            target,
            resolved_at_tick: 190,
            description: "hit".to_string(),
        });
        app.world_mut().send_event(DeathEvent {
            target,
            cause: "attack_intent:offline:Attacker".to_string(),
            at_tick: 195,
        });

        {
            let mut timer = app
                .world_mut()
                .resource_mut::<crate::network::WorldStateTimer>();
            timer.ticks = TEST_WORLD_STATE_INTERVAL_TICKS - 1;
        }

        app.update();

        {
            let mut timer = app
                .world_mut()
                .resource_mut::<crate::network::WorldStateTimer>();
            timer.ticks = TEST_WORLD_STATE_INTERVAL_TICKS;
        }

        app.update();

        let mut summary_payload: Option<CombatSummaryV1> = None;
        while let Ok(message) = rx_outbound.try_recv() {
            if let RedisOutbound::CombatSummary(payload) = message {
                summary_payload = Some(payload);
            }
        }

        let summary_payload =
            summary_payload.expect("summary publish should exist on 200-tick cadence");
        assert_eq!(summary_payload.v, 1);
        assert_eq!(summary_payload.window_start_tick, 190);
        assert_eq!(
            summary_payload.window_end_tick,
            TEST_WORLD_STATE_INTERVAL_TICKS
        );
        assert_eq!(summary_payload.combat_event_count, 1);
        assert_eq!(summary_payload.death_event_count, 1);

        let summary = app.world().resource::<CombatSummaryAccumulator>();
        assert_eq!(summary.combat_event_count, 0);
        assert_eq!(summary.death_event_count, 0);
        assert_eq!(
            summary.window_start_tick,
            Some(TEST_WORLD_STATE_INTERVAL_TICKS + 1)
        );
    }
}
