//! plan-combat-skill-feedback-bridges-v1 P1 — 爆脉 v4 反馈整桥。
//!
//! 6 个纯 write-only 事件全部接入网络层。各 system 只读转发，严禁操作真元账。
//!
//! ## 路由决策（§8 #2 收口）
//!
//! | 事件 | Redis | S2C CustomPayload |
//! |------|-------|-------------------|
//! | ScarCircuitFormed  | bong:baomai_v4/scar_circuit_formed  | — |
//! | ScarCircuitBroken  | bong:baomai_v4/scar_circuit_broken  | — |
//! | IronCocoonStageUp  | bong:baomai_v4/iron_cocoon_stage_up | EventStreamPush(Combat) |
//! | CrackReadingResult | —                                    | bong:crack_reading |
//! | ResonanceLock      | bong:baomai_v4/resonance_lock       | bong:resonance_lock |
//! | ResonanceLockEnd   | bong:baomai_v4/resonance_lock_end   | bong:resonance_lock_end |

use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{ident, Client, Entity, EventReader, Query, Res, Username, With};

use crate::combat::baomai_v4::events::{
    CrackReadingResultEvent, IronCocoonStageUpEvent, ResonanceLockEndEvent, ResonanceLockEvent,
    ScarCircuitBrokenEvent, ScarCircuitFormedEvent,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, PayloadBuildError, SERVER_DATA_CHANNEL,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::baomai_v4::{
    lock_end_reason_wire, BaomaiV4IronCocoonStageUpV1, BaomaiV4ResonanceLockEndV1,
    BaomaiV4ResonanceLockV1, BaomaiV4ScarCircuitBrokenV1, BaomaiV4ScarCircuitFormedV1,
};
use crate::schema::combat_hud::{EventChannelV1, EventPriorityV1, EventStreamPushV1};
use crate::schema::common::MAX_PAYLOAD_BYTES;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

// ── helpers ──────────────────────────────────────────────────────────────────

fn entity_wire_id(entity: Entity, usernames: &Query<(Option<&Username>,)>) -> String {
    let username = usernames.get(entity).ok().and_then(|(u,)| u);
    username
        .map(|u| format!("offline:{}", u.0))
        .unwrap_or_else(|| format!("char:{}", entity.to_bits()))
}

fn current_unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn is_oversized_payload(payload_type: &'static str, bytes: &[u8]) -> bool {
    if bytes.len() <= MAX_PAYLOAD_BYTES {
        return false;
    }
    log_payload_build_error(
        payload_type,
        &PayloadBuildError::Oversize {
            size: bytes.len(),
            max: MAX_PAYLOAD_BYTES,
        },
    );
    true
}

// ── ScarCircuitFormed / Broken → Redis only ──────────────────────────────────

/// 疤纹回路形成/断裂 → Redis only（client 无独立 consumer，agent 叙事已足够）。
pub fn publish_scar_circuit_events(
    redis: Res<RedisBridgeResource>,
    mut formed_events: EventReader<ScarCircuitFormedEvent>,
    mut broken_events: EventReader<ScarCircuitBrokenEvent>,
    usernames: Query<(Option<&Username>,)>,
) {
    for event in formed_events.read() {
        let entity_id = entity_wire_id(event.entity, &usernames);
        let payload = BaomaiV4ScarCircuitFormedV1::new(entity_id, event.circuit, event.tick);
        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV4ScarCircuitFormed(payload))
        {
            tracing::warn!("[bong][baomai-v4] failed to queue ScarCircuitFormed: {err}");
        }
    }

    for event in broken_events.read() {
        let entity_id = entity_wire_id(event.entity, &usernames);
        let payload =
            BaomaiV4ScarCircuitBrokenV1::new(entity_id, event.circuit, event.reason, event.tick);
        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV4ScarCircuitBroken(payload))
        {
            tracing::warn!("[bong][baomai-v4] failed to queue ScarCircuitBroken: {err}");
        }
    }
}

// ── IronCocoonStageUp → Redis + EventStreamPush ──────────────────────────────

/// 活茧阶段提升 → Redis 叙事 + EventStreamPush(Combat) 客户端事件流。
pub fn push_iron_cocoon_stage_up(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<IronCocoonStageUpEvent>,
    usernames: Query<(Option<&Username>,)>,
    mut clients: Query<(&Username, &mut Client)>,
) {
    let now_ms = current_unix_millis();
    for event in events.read() {
        let entity_id = entity_wire_id(event.entity, &usernames);

        // 1. Redis → agent 叙事
        let redis_payload = BaomaiV4IronCocoonStageUpV1::new(
            entity_id.clone(),
            event.from,
            event.to,
            event.total_overloads,
            event.tick,
        );
        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV4IronCocoonStageUp(redis_payload))
        {
            tracing::warn!("[bong][baomai-v4] failed to queue IronCocoonStageUp Redis: {err}");
        }

        // 2. EventStreamPush(Combat) → client event_flow
        let from_name = iron_cocoon_stage_cn(event.from);
        let to_name = iron_cocoon_stage_cn(event.to);
        let text = format!(
            "活茧升阶：{from_name} → {to_name}（累计过载 {} 次）",
            event.total_overloads
        );

        let payload = ServerDataV1::new(ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
            channel: EventChannelV1::Combat,
            priority: EventPriorityV1::P1Important,
            source_tag: "iron_cocoon_stage_up".to_string(),
            text,
            color: 0,
            created_at_ms: now_ms,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };

        if let Ok((_, mut client)) = clients.get_mut(event.entity) {
            send_server_data_payload(&mut client, payload_bytes.as_slice());
            tracing::debug!(
                "[bong][baomai-v4] sent {} {} iron_cocoon_stage_up to {entity_id}",
                SERVER_DATA_CHANNEL,
                payload_type,
            );
        }
    }
}

fn iron_cocoon_stage_cn(
    stage: crate::combat::baomai_v4::iron_cocoon::IronCocoonStage,
) -> &'static str {
    use crate::combat::baomai_v4::iron_cocoon::IronCocoonStage;
    match stage {
        IronCocoonStage::None => "凡身",
        IronCocoonStage::ToughSkin => "茧皮",
        IronCocoonStage::IronBone => "茧骨",
        IronCocoonStage::DullFlesh => "茧肉",
        IronCocoonStage::ScarForged => "茧灵",
    }
}

// ── CrackReading → S2C CustomPayload only ────────────────────────────────────

/// 裂读结果 → `bong:crack_reading` S2C CustomPayload（client HUD overlay only，无 Redis）。
pub fn emit_crack_reading_payload(
    mut events: EventReader<CrackReadingResultEvent>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in events.read() {
        let payload =
            crate::combat::baomai_v4::crack_reading::to_client_payload(&event.result, event.tick);
        let bytes = match serde_json::to_vec(&payload) {
            Ok(b) => b,
            Err(err) => {
                tracing::warn!(
                    "[bong][baomai-v4] crack_reading serialize failed for entity {:?}: {err}",
                    event.reader
                );
                continue;
            }
        };
        if is_oversized_payload("crack_reading", &bytes) {
            continue;
        }

        let Ok(mut client) = clients.get_mut(event.reader) else {
            // reader 不是在线玩家（NPC 不需要 HUD）
            continue;
        };

        client.send_custom_payload(ident!("bong:crack_reading"), &bytes);
        tracing::debug!(
            "[bong][baomai-v4] sent bong:crack_reading to {:?} (target={:?}, deep={})",
            event.reader,
            event.target,
            event.result.is_deep_read,
        );
    }
}

// ── ResonanceLock/End → Redis + S2C CustomPayload ────────────────────────────

/// 共振锁定开始 → Redis 叙事 + `bong:resonance_lock` S2C CustomPayload。
pub fn emit_resonance_lock_payloads(
    redis: Res<RedisBridgeResource>,
    mut lock_events: EventReader<ResonanceLockEvent>,
    mut lock_end_events: EventReader<ResonanceLockEndEvent>,
    usernames: Query<(Option<&Username>,)>,
    mut clients: Query<&mut Client, With<Client>>,
) {
    for event in lock_events.read() {
        let fighter_a_id = entity_wire_id(event.fighter_a, &usernames);
        let fighter_b_id = entity_wire_id(event.fighter_b, &usernames);

        // 1. Redis → agent 叙事
        let redis_payload = BaomaiV4ResonanceLockV1::new(
            fighter_a_id.clone(),
            fighter_b_id.clone(),
            event.started_at,
            event.ends_at,
        );
        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV4ResonanceLock(redis_payload))
        {
            tracing::warn!("[bong][baomai-v4] failed to queue ResonanceLock Redis: {err}");
        }

        // 2. S2C → client HUD/VFX（双方各收一条）
        let s2c_payload = serde_json::json!({
            "v": 1,
            "fighter_a_id": fighter_a_id,
            "fighter_b_id": fighter_b_id,
            "started_at": event.started_at,
            "ends_at": event.ends_at,
        });
        let bytes = match serde_json::to_vec(&s2c_payload) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!("[bong][baomai-v4] resonance_lock serialize failed: {err}");
                continue;
            }
        };
        if is_oversized_payload("resonance_lock", &bytes) {
            continue;
        }
        for fighter in [event.fighter_a, event.fighter_b] {
            if let Ok(mut client) = clients.get_mut(fighter) {
                client.send_custom_payload(ident!("bong:resonance_lock"), &bytes);
            }
        }
    }

    for event in lock_end_events.read() {
        let fighter_a_id = entity_wire_id(event.fighter_a, &usernames);
        let fighter_b_id = entity_wire_id(event.fighter_b, &usernames);

        let reason_wire = lock_end_reason_wire(event.reason, |e| entity_wire_id(e, &usernames));

        // 1. Redis → agent 叙事
        let redis_payload = BaomaiV4ResonanceLockEndV1::new(
            fighter_a_id.clone(),
            fighter_b_id.clone(),
            reason_wire.clone(),
            event.tick,
        );
        if let Err(err) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV4ResonanceLockEnd(redis_payload))
        {
            tracing::warn!("[bong][baomai-v4] failed to queue ResonanceLockEnd Redis: {err}");
        }

        // 2. S2C → client HUD（双方各收一条）
        let reason_json = serde_json::to_value(&reason_wire).unwrap_or(serde_json::Value::Null);
        let s2c_payload = serde_json::json!({
            "v": 1,
            "fighter_a_id": fighter_a_id,
            "fighter_b_id": fighter_b_id,
            "reason": reason_json,
            "tick": event.tick,
        });
        let bytes = match serde_json::to_vec(&s2c_payload) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!("[bong][baomai-v4] resonance_lock_end serialize failed: {err}");
                continue;
            }
        };
        if is_oversized_payload("resonance_lock_end", &bytes) {
            continue;
        }
        for fighter in [event.fighter_a, event.fighter_b] {
            if let Ok(mut client) = clients.get_mut(fighter) {
                client.send_custom_payload(ident!("bong:resonance_lock_end"), &bytes);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::combat::baomai_v4::events::{CircuitBreakReason, LockEndReason};
    use crate::combat::baomai_v4::iron_cocoon::IronCocoonStage;
    use crate::combat::baomai_v4::scar_circuit::ScarCircuitKind;
    use crate::network::RedisBridgeResource;
    use crate::schema::baomai_v4::LockEndReasonWire;
    use crate::schema::common::MAX_PAYLOAD_BYTES;

    fn flush_clients(app: &mut App) {
        let world = app.world_mut();
        let mut clients = world.query::<&mut Client>();
        for mut client in clients.iter_mut(world) {
            client.flush_packets().expect("mock client should flush");
        }
    }

    fn collect_payloads(helper: &mut MockClientHelper, channel: &str) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != channel {
                continue;
            }
            let value = serde_json::from_slice(packet.data.0 .0).expect("payload must be JSON");
            out.push(value);
        }
        out
    }

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        (app, rx_outbound)
    }

    // ── ScarCircuitFormed ─────────────────────────────────────────────────────

    fn app_scar_circuit() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (mut app, rx) = app_with_bridge();
        app.add_event::<ScarCircuitFormedEvent>();
        app.add_event::<ScarCircuitBrokenEvent>();
        app.add_systems(Update, publish_scar_circuit_events);
        (app, rx)
    }

    #[test]
    fn scar_circuit_formed_emits_redis() {
        let (mut app, rx) = app_scar_circuit();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ScarCircuitFormedEvent {
            entity,
            circuit: ScarCircuitKind::TigerMouth,
            tick: 100,
        });
        app.update();
        let outbound = rx
            .try_recv()
            .expect("ScarCircuitFormed should produce 1 Redis msg");
        match outbound {
            RedisOutbound::BaomaiV4ScarCircuitFormed(payload) => {
                assert_eq!(payload.v, 1, "BaomaiV4ScarCircuitFormedV1.v must be 1");
                assert_eq!(payload.tick, 100, "tick should match event");
            }
            other => panic!("expected BaomaiV4ScarCircuitFormed, got {other:?}"),
        }
        assert!(
            rx.try_recv().is_err(),
            "exactly 1 Redis msg for 1 ScarCircuitFormed event"
        );
    }

    #[test]
    fn scar_circuit_broken_all_reasons() {
        let reasons = [
            CircuitBreakReason::Healed,
            CircuitBreakReason::Deepened,
            CircuitBreakReason::Severed,
            CircuitBreakReason::Closed,
        ];
        for reason in reasons {
            let (mut app, rx) = app_scar_circuit();
            let entity = app.world_mut().spawn_empty().id();
            app.world_mut().send_event(ScarCircuitBrokenEvent {
                entity,
                circuit: ScarCircuitKind::HeartLung,
                reason,
                tick: 50,
            });
            app.update();
            let outbound = rx
                .try_recv()
                .expect("ScarCircuitBroken should produce 1 Redis msg");
            match outbound {
                RedisOutbound::BaomaiV4ScarCircuitBroken(p) => {
                    assert_eq!(p.v, 1);
                    assert_eq!(p.tick, 50);
                }
                other => panic!("expected BaomaiV4ScarCircuitBroken, got {other:?}"),
            }
        }
    }

    #[test]
    fn no_event_no_redis() {
        let (mut app, rx) = app_scar_circuit();
        app.update();
        assert!(
            rx.try_recv().is_err(),
            "no event should produce no Redis messages"
        );
    }

    // ── IronCocoonStageUp ─────────────────────────────────────────────────────

    fn app_iron_cocoon() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (mut app, rx) = app_with_bridge();
        app.add_event::<IronCocoonStageUpEvent>();
        app.add_systems(Update, push_iron_cocoon_stage_up);
        (app, rx)
    }

    #[test]
    fn iron_cocoon_stage_up_emits_redis() {
        let (mut app, rx) = app_iron_cocoon();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(IronCocoonStageUpEvent {
            entity,
            from: IronCocoonStage::None,
            to: IronCocoonStage::ToughSkin,
            total_overloads: 50,
            tick: 300,
        });
        app.update();
        let outbound = rx
            .try_recv()
            .expect("IronCocoonStageUp should produce 1 Redis msg");
        match outbound {
            RedisOutbound::BaomaiV4IronCocoonStageUp(p) => {
                assert_eq!(p.v, 1, "v must be 1");
                assert_eq!(p.total_overloads, 50, "total_overloads should match");
                assert_eq!(p.tick, 300, "tick should match");
            }
            other => panic!("expected BaomaiV4IronCocoonStageUp, got {other:?}"),
        }
    }

    #[test]
    fn iron_cocoon_no_event_no_redis() {
        let (mut app, rx) = app_iron_cocoon();
        app.update();
        assert!(rx.try_recv().is_err(), "no event → no Redis");
    }

    // ── CrackReading ──────────────────────────────────────────────────────────

    #[test]
    fn crack_reading_no_event_no_payload() {
        let mut app = App::new();
        app.add_event::<CrackReadingResultEvent>();
        app.add_systems(Update, emit_crack_reading_payload);
        app.update();
        // No panic = pass (entities are empty, nothing to send)
    }

    // ── ResonanceLock ─────────────────────────────────────────────────────────

    fn app_resonance() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (mut app, rx) = app_with_bridge();
        app.add_event::<ResonanceLockEvent>();
        app.add_event::<ResonanceLockEndEvent>();
        app.add_systems(Update, emit_resonance_lock_payloads);
        (app, rx)
    }

    #[test]
    fn resonance_lock_emits_redis() {
        let (mut app, rx) = app_resonance();
        let a = app.world_mut().spawn_empty().id();
        let b = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ResonanceLockEvent {
            fighter_a: a,
            fighter_b: b,
            started_at: 1000,
            ends_at: 1060,
        });
        app.update();
        let outbound = rx
            .try_recv()
            .expect("ResonanceLock should produce 1 Redis msg");
        match outbound {
            RedisOutbound::BaomaiV4ResonanceLock(p) => {
                assert_eq!(p.v, 1, "v must be 1");
                assert_eq!(p.started_at, 1000);
                assert_eq!(p.ends_at, 1060);
                assert!(
                    p.fighter_a_id.starts_with("char:"),
                    "NPC entity should use char: prefix"
                );
            }
            other => panic!("expected BaomaiV4ResonanceLock, got {other:?}"),
        }
        assert!(rx.try_recv().is_err(), "exactly 1 Redis msg");
    }

    #[test]
    fn oversized_resonance_lock_payload_is_not_sent() {
        let (mut app, rx) = app_resonance();
        let (huge_bundle, mut huge_helper) = create_mock_client("x".repeat(MAX_PAYLOAD_BYTES));
        let (peer_bundle, _peer_helper) = create_mock_client("Peer");
        let a = app.world_mut().spawn(huge_bundle).id();
        let b = app.world_mut().spawn(peer_bundle).id();

        app.world_mut().send_event(ResonanceLockEvent {
            fighter_a: a,
            fighter_b: b,
            started_at: 1000,
            ends_at: 1060,
        });

        app.update();
        flush_clients(&mut app);

        assert!(
            matches!(rx.try_recv(), Ok(RedisOutbound::BaomaiV4ResonanceLock(_))),
            "Redis 叙事不应受 S2C 超限保护影响"
        );
        let payloads = collect_payloads(&mut huge_helper, "bong:resonance_lock");
        assert!(
            payloads.is_empty(),
            "超出 MAX_PAYLOAD_BYTES 的 resonance_lock 不应下发；实际收到 {} 帧",
            payloads.len()
        );
    }

    #[test]
    fn resonance_lock_end_expired_emits_redis() {
        let (mut app, rx) = app_resonance();
        let a = app.world_mut().spawn_empty().id();
        let b = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ResonanceLockEndEvent {
            fighter_a: a,
            fighter_b: b,
            reason: LockEndReason::Expired,
            tick: 1060,
        });
        app.update();
        let outbound = rx
            .try_recv()
            .expect("ResonanceLockEnd should produce 1 Redis msg");
        match outbound {
            RedisOutbound::BaomaiV4ResonanceLockEnd(p) => {
                assert_eq!(p.v, 1);
                assert_eq!(p.tick, 1060);
                assert_eq!(
                    p.reason,
                    LockEndReasonWire::Expired,
                    "Expired reason must map to LockEndReasonWire::Expired"
                );
            }
            other => panic!("expected BaomaiV4ResonanceLockEnd, got {other:?}"),
        }
    }

    #[test]
    fn resonance_lock_end_retreated_has_entity_id() {
        let (mut app, rx) = app_resonance();
        let a = app.world_mut().spawn_empty().id();
        let b = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ResonanceLockEndEvent {
            fighter_a: a,
            fighter_b: b,
            reason: LockEndReason::Retreated(a),
            tick: 1030,
        });
        app.update();
        let outbound = rx.try_recv().expect("ResonanceLockEnd should emit");
        match outbound {
            RedisOutbound::BaomaiV4ResonanceLockEnd(p) => {
                assert!(
                    matches!(p.reason, LockEndReasonWire::Retreated { .. }),
                    "Retreated reason must map to LockEndReasonWire::Retreated"
                );
                if let LockEndReasonWire::Retreated { entity_id } = p.reason {
                    assert!(
                        entity_id.starts_with("char:"),
                        "retreated entity_id should use char: prefix for NPC entity"
                    );
                }
            }
            other => panic!("expected BaomaiV4ResonanceLockEnd, got {other:?}"),
        }
    }

    #[test]
    fn resonance_lock_end_death_has_entity_id() {
        let (mut app, rx) = app_resonance();
        let a = app.world_mut().spawn_empty().id();
        let b = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(ResonanceLockEndEvent {
            fighter_a: a,
            fighter_b: b,
            reason: LockEndReason::Death(b),
            tick: 1010,
        });
        app.update();
        let outbound = rx.try_recv().expect("ResonanceLockEnd should emit");
        match outbound {
            RedisOutbound::BaomaiV4ResonanceLockEnd(p) => {
                assert!(
                    matches!(p.reason, LockEndReasonWire::Death { .. }),
                    "Death reason must map to LockEndReasonWire::Death"
                );
            }
            other => panic!("expected BaomaiV4ResonanceLockEnd, got {other:?}"),
        }
    }

    #[test]
    fn resonance_no_event_no_redis() {
        let (mut app, rx) = app_resonance();
        app.update();
        assert!(rx.try_recv().is_err(), "no event → no Redis message");
    }
}
