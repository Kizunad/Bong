use std::collections::HashMap;

use valence::prelude::{Entity, EventReader, Query, Res, ResMut, Resource, Username, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use super::WORLD_STATE_PUBLISH_INTERVAL_TICKS;
use crate::combat::components::Lifecycle;
use crate::combat::events::{CombatEvent, DeathEvent, DeathInsightRequested};
use crate::npc::brain::canonical_npc_id;
use crate::npc::spawn::NpcMarker;
use crate::player::state::canonical_player_id;
use crate::schema::combat_event::{
    CombatBodyPartV1, CombatRealtimeEventV1, CombatRealtimeKindV1, CombatSummaryV1,
    CombatWoundKindV1,
};

#[derive(Debug, Default)]
pub struct CombatSummaryAccumulator {
    pub window_start_tick: Option<u64>,
    pub combat_event_count: u64,
    pub death_event_count: u64,
    pub damage_total: f32,
    pub contam_delta_total: f64,
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
        summary.damage_total += ev.damage;
        summary.contam_delta_total += ev.contam_delta;

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
            body_part: Some(map_body_part(ev.body_part)),
            wound_kind: Some(map_wound_kind(ev.wound_kind)),
            damage: Some(ev.damage),
            contam_delta: Some(ev.contam_delta),
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
            body_part: None,
            wound_kind: None,
            damage: None,
            contam_delta: None,
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

pub fn publish_death_insight_requests(
    redis: Res<RedisBridgeResource>,
    mut reader: EventReader<DeathInsightRequested>,
) {
    for ev in reader.read() {
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DeathInsight(ev.payload.clone()));
    }
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
    let damage_total = round_f32(summary.damage_total);
    let contam_delta_total = round_f64(summary.contam_delta_total);

    let payload = CombatSummaryV1 {
        v: 1,
        window_start_tick,
        window_end_tick,
        combat_event_count: summary.combat_event_count,
        death_event_count: summary.death_event_count,
        damage_total,
        contam_delta_total,
    };
    let _ = redis
        .tx_outbound
        .send(RedisOutbound::CombatSummary(payload));

    summary.window_start_tick = Some(window_end_tick.saturating_add(1));
    summary.combat_event_count = 0;
    summary.death_event_count = 0;
    summary.damage_total = 0.0;
    summary.contam_delta_total = 0.0;
}

fn round_f32(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}

fn round_f64(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
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

fn map_body_part(body_part: crate::combat::components::BodyPart) -> CombatBodyPartV1 {
    match body_part {
        crate::combat::components::BodyPart::Head => CombatBodyPartV1::Head,
        crate::combat::components::BodyPart::Chest => CombatBodyPartV1::Chest,
        crate::combat::components::BodyPart::Abdomen => CombatBodyPartV1::Abdomen,
        crate::combat::components::BodyPart::ArmL => CombatBodyPartV1::ArmL,
        crate::combat::components::BodyPart::ArmR => CombatBodyPartV1::ArmR,
        crate::combat::components::BodyPart::LegL => CombatBodyPartV1::LegL,
        crate::combat::components::BodyPart::LegR => CombatBodyPartV1::LegR,
    }
}

fn map_wound_kind(wound_kind: crate::combat::components::WoundKind) -> CombatWoundKindV1 {
    match wound_kind {
        crate::combat::components::WoundKind::Cut => CombatWoundKindV1::Cut,
        crate::combat::components::WoundKind::Blunt => CombatWoundKindV1::Blunt,
        crate::combat::components::WoundKind::Pierce => CombatWoundKindV1::Pierce,
        crate::combat::components::WoundKind::Burn => CombatWoundKindV1::Burn,
        crate::combat::components::WoundKind::Concussion => CombatWoundKindV1::Concussion,
    }
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
        app.add_event::<DeathInsightRequested>();
        (app, rx_outbound)
    }

    #[test]
    fn publishes_death_insight_requests_to_redis_outbound() {
        let (mut app, rx_outbound) = setup_app();
        app.add_systems(Update, publish_death_insight_requests);

        app.world_mut().send_event(DeathInsightRequested {
            payload: crate::schema::death_insight::DeathInsightRequestV1 {
                v: 1,
                request_id: "death_insight:offline:Azure:84000:3".to_string(),
                character_id: "offline:Azure".to_string(),
                at_tick: 84_000,
                cause: "bleed_out".to_string(),
                category: crate::schema::death_insight::DeathInsightCategoryV1::Combat,
                realm: Some("Awaken".to_string()),
                player_realm: Some("mortal".to_string()),
                zone_kind: crate::schema::death_insight::DeathInsightZoneKindV1::Ordinary,
                death_count: 3,
                rebirth_chance: Some(0.8),
                lifespan_remaining_years: Some(70.0),
                recent_biography: vec!["t84000:near_death:bleed_out".to_string()],
                position: None,
                known_spirit_eyes: Vec::new(),
                context: serde_json::json!({"will_terminate": false}),
            },
        });

        app.update();

        let outbound = rx_outbound
            .try_recv()
            .expect("death insight outbound should be published");
        match outbound {
            RedisOutbound::DeathInsight(payload) => {
                assert_eq!(payload.v, 1);
                assert_eq!(payload.character_id, "offline:Azure");
                assert_eq!(payload.cause, "bleed_out");
            }
            other => panic!("expected DeathInsight outbound, got {other:?}"),
        }
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
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: crate::combat::components::WoundKind::Blunt,
            damage: 20.0,
            contam_delta: 5.0,
            description: "attack_intent offline:AttackerCanonical -> offline:TargetCanonical hit Chest with Blunt for 20.0 damage at 0.90 reach decay".to_string(),
        });
        app.world_mut().send_event(DeathEvent {
            target,
            cause: "attack_intent:offline:Azure".to_string(),
            attacker: Some(attacker),
            attacker_player_id: Some("offline:Azure".to_string()),
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
                assert_eq!(payload.body_part, Some(CombatBodyPartV1::Chest));
                assert_eq!(payload.wound_kind, Some(CombatWoundKindV1::Blunt));
                assert_eq!(payload.damage, Some(20.0));
                assert_eq!(payload.contam_delta, Some(5.0));
                assert_eq!(
                    payload.description.as_deref(),
                    Some("attack_intent offline:AttackerCanonical -> offline:TargetCanonical hit Chest with Blunt for 20.0 damage at 0.90 reach decay")
                );
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
                assert!(payload.body_part.is_none());
                assert!(payload.wound_kind.is_none());
                assert!(payload.damage.is_none());
                assert!(payload.contam_delta.is_none());
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
        assert_eq!(summary.damage_total, 20.0);
        assert_eq!(summary.contam_delta_total, 5.0);
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
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: crate::combat::components::WoundKind::Pierce,
            damage: 12.0,
            contam_delta: 3.0,
            description: "hit".to_string(),
        });
        app.world_mut().send_event(DeathEvent {
            target,
            cause: "attack_intent:offline:Attacker".to_string(),
            attacker: Some(attacker),
            attacker_player_id: Some("offline:Attacker".to_string()),
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
        assert_eq!(summary_payload.damage_total, 12.0);
        assert_eq!(summary_payload.contam_delta_total, 3.0);

        let summary = app.world().resource::<CombatSummaryAccumulator>();
        assert_eq!(summary.combat_event_count, 0);
        assert_eq!(summary.death_event_count, 0);
        assert_eq!(summary.damage_total, 0.0);
        assert_eq!(summary.contam_delta_total, 0.0);
        assert_eq!(
            summary.window_start_tick,
            Some(TEST_WORLD_STATE_INTERVAL_TICKS + 1)
        );
    }

    #[test]
    fn summary_rounds_float_totals_to_stable_millis() {
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
            resolved_at_tick: 1,
            body_part: crate::combat::components::BodyPart::Chest,
            wound_kind: crate::combat::components::WoundKind::Cut,
            damage: 0.1 + 0.2,
            contam_delta: 0.1 + 0.2,
            description: "rounded hit".to_string(),
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

        let summary_payload = summary_payload.expect("rounded summary publish should exist");
        assert_eq!(summary_payload.damage_total, 0.3);
        assert_eq!(summary_payload.contam_delta_total, 0.3);
    }
}
