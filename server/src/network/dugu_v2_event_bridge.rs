use valence::prelude::{Client, Entity, EventReader, Query, Res, UniqueId};

use crate::combat::dugu_v2::{
    events::{DuguSkillVisual, TaintTier},
    EclipseNeedleEvent, PenetrateChainEvent, PermanentQiMaxDecayApplied, ReverseTriggeredEvent,
    SelfCureProgressEvent, ShroudActivatedEvent,
};
use crate::combat::woliu::entity_wire_id;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{log_payload_build_error, send_server_data_payload, RedisBridgeResource};
use crate::schema::dugu_v2::{
    DuguReverseTriggeredV1, DuguSelfCureProgressV1, DuguTaintTierV1, DuguV2SkillCastV1,
    DuguV2SkillIdV1,
};
use crate::schema::server_data::{
    DuguV2HudQiDecayV1, DuguV2HudSelfCureV1, DuguV2HudShroudActiveV1, DuguV2HudSkillCastV1,
    ServerDataPayloadV1, ServerDataV1,
};

pub fn publish_dugu_v2_eclipse_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<EclipseNeedleEvent>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let caster_wire = entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let target_wire = entity_wire_id(unique_ids.get(event.target).ok(), event.target);
        let payload = cast_payload(
            caster_wire.clone(),
            Some(target_wire.clone()),
            DuguV2SkillIdV1::Eclipse,
            event.tick,
            Some(taint_tier_payload(event.tier)),
            event.hp_loss,
            event.qi_loss,
            event.qi_max_loss,
            event.permanent_decay_rate_per_min,
            event.returned_zone_qi,
            event.reveal_probability,
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
        // S2C：通知施法者 HUD（caster 是玩家实体）
        send_s2c_to_entity(
            &mut clients,
            event.caster,
            ServerDataPayloadV1::DuguV2SkillCast(DuguV2HudSkillCastV1 {
                kind: "eclipse".to_string(),
                caster: caster_wire,
                target: Some(target_wire),
                taint_tier: Some(taint_tier_wire(event.tier)),
                reveal_probability: event.reveal_probability,
                tick: event.tick,
            }),
        );
    }
}

pub fn publish_dugu_v2_penetrate_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<PenetrateChainEvent>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let caster_wire = entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let target_wire = entity_wire_id(unique_ids.get(event.target).ok(), event.target);
        let payload = cast_payload(
            caster_wire.clone(),
            Some(target_wire.clone()),
            DuguV2SkillIdV1::Penetrate,
            event.tick,
            Some(taint_tier_payload(event.taint_tier)),
            0.0,
            0.0,
            0.0,
            event.permanent_decay_rate_per_min,
            0.0,
            event.reveal_probability,
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
        send_s2c_to_entity(
            &mut clients,
            event.caster,
            ServerDataPayloadV1::DuguV2SkillCast(DuguV2HudSkillCastV1 {
                kind: "penetrate".to_string(),
                caster: caster_wire,
                target: Some(target_wire),
                taint_tier: Some(taint_tier_wire(event.taint_tier)),
                reveal_probability: event.reveal_probability,
                tick: event.tick,
            }),
        );
    }
}

pub fn publish_dugu_v2_shroud_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ShroudActivatedEvent>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let caster_wire = entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let payload = cast_payload(
            caster_wire.clone(),
            None,
            DuguV2SkillIdV1::Shroud,
            event.tick,
            None,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            (1.0 - event.strength).clamp(0.0, 1.0),
            event.visual,
        );
        let _ = redis.tx_outbound.send(RedisOutbound::DuguV2Cast(payload));
        // S2C：shroud 状态单独推送 DuguV2ShroudActive（含持续时间）
        send_s2c_to_entity(
            &mut clients,
            event.caster,
            ServerDataPayloadV1::DuguV2ShroudActive(DuguV2HudShroudActiveV1 {
                caster: caster_wire,
                strength: event.strength,
                expires_at_tick: event.expires_at_tick,
                tick: event.tick,
            }),
        );
    }
}

pub fn publish_dugu_v2_self_cure_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<SelfCureProgressEvent>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let caster_wire = entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let payload = DuguSelfCureProgressV1 {
            caster: caster_wire.clone(),
            hours_used: event.hours_used,
            daily_hours_after: event.daily_hours_after,
            gain_percent: event.gain_percent,
            insidious_color_percent: event.insidious_color_percent,
            morphology_percent: event.morphology_percent,
            self_revealed: event.self_revealed,
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuguV2SelfCure(payload));
        // S2C：推送自蕴进度 + self_revealed flag（client DuguV2HudStateStore merge-update）
        send_s2c_to_entity(
            &mut clients,
            event.caster,
            ServerDataPayloadV1::DuguV2SelfCure(DuguV2HudSelfCureV1 {
                caster: caster_wire,
                gain_percent: event.gain_percent,
                self_revealed: event.self_revealed,
                tick: event.tick,
            }),
        );
    }
}

pub fn publish_dugu_v2_reverse_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<ReverseTriggeredEvent>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let caster_wire = entity_wire_id(unique_ids.get(event.caster).ok(), event.caster);
        let payload = DuguReverseTriggeredV1 {
            caster: caster_wire.clone(),
            affected_targets: event.affected_targets,
            burst_damage: event.burst_damage,
            returned_zone_qi: event.returned_zone_qi,
            juebi_delay_ticks: event.juebi_delay_ticks,
            center: [event.center.x, event.center.y, event.center.z],
            tick: event.tick,
        };
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::DuguV2Reverse(payload));
        // S2C：反噬招式通知（client 显示 HUD feedback）
        send_s2c_to_entity(
            &mut clients,
            event.caster,
            ServerDataPayloadV1::DuguV2SkillCast(DuguV2HudSkillCastV1 {
                kind: "reverse".to_string(),
                caster: caster_wire,
                target: None,
                taint_tier: None,
                reveal_probability: 0.0,
                tick: event.tick,
            }),
        );
    }
}

/// 永久真元上限衰减事件 → 仅 S2C（不走 Redis，避免淹没 agent channel）。
/// 守恒红线：loss/qi_max_after 均只读自 ECS 已扣量，不重算。
pub fn publish_permanent_qi_max_decay_to_client(
    mut events: EventReader<PermanentQiMaxDecayApplied>,
    unique_ids: Query<&UniqueId>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let target_wire = entity_wire_id(unique_ids.get(event.target).ok(), event.target);
        send_s2c_to_entity(
            &mut clients,
            event.target,
            ServerDataPayloadV1::PermanentQiMaxDecayApplied(DuguV2HudQiDecayV1 {
                target: target_wire,
                loss: event.loss,
                qi_max_after: event.qi_max_after,
                tick: event.tick,
            }),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_payload(
    caster: String,
    target: Option<String>,
    skill: DuguV2SkillIdV1,
    tick: u64,
    taint_tier: Option<DuguTaintTierV1>,
    hp_loss: f32,
    qi_loss: f32,
    qi_max_loss: f32,
    permanent_decay_rate_per_min: f32,
    returned_zone_qi: f32,
    reveal_probability: f32,
    visual: DuguSkillVisual,
) -> DuguV2SkillCastV1 {
    DuguV2SkillCastV1 {
        caster,
        target,
        skill,
        tick,
        taint_tier,
        hp_loss,
        qi_loss,
        qi_max_loss,
        permanent_decay_rate_per_min,
        returned_zone_qi,
        reveal_probability,
        animation_id: visual.animation_id.to_string(),
        particle_id: visual.particle_id.to_string(),
        sound_recipe_id: visual.sound_recipe_id.to_string(),
        icon_texture: visual.icon_texture.to_string(),
    }
}

fn taint_tier_payload(tier: TaintTier) -> DuguTaintTierV1 {
    match tier {
        TaintTier::Immediate => DuguTaintTierV1::Immediate,
        TaintTier::Temporary => DuguTaintTierV1::Temporary,
        TaintTier::Permanent => DuguTaintTierV1::Permanent,
    }
}

/// 返回毒蛊层级的 wire 字符串（用于 DuguV2HudSkillCastV1.taint_tier 字段）。
/// 使用 serde 序列化而非 Debug 格式，避免 P4 教训：format!("{:?}") = "Temporary" 而非 "temporary"。
fn taint_tier_wire(tier: TaintTier) -> String {
    match tier {
        TaintTier::Immediate => "immediate".to_string(),
        TaintTier::Temporary => "temporary".to_string(),
        TaintTier::Permanent => "permanent".to_string(),
    }
}

/// 向指定实体发送 S2C ServerData payload（毒蛊 v2 HUD 专用）。
fn send_s2c_to_entity(
    clients: &mut Query<&mut Client>,
    entity: Entity,
    payload: ServerDataPayloadV1,
) {
    let Ok(mut client) = clients.get_mut(entity) else {
        return;
    };
    let envelope = ServerDataV1::new(payload);
    let payload_type = payload_type_label(envelope.payload_type());
    let payload_bytes = match serialize_server_data_payload(&envelope) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload (dugu_v2)",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossbeam_channel::TryRecvError;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::combat::dugu_v2::events::{DuguSkillVisual, TaintTier};
    use crate::cultivation::components::Realm;
    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use crate::network::redis_bridge::RedisOutbound;

    fn visual() -> DuguSkillVisual {
        DuguSkillVisual {
            animation_id: "bong:dugu_needle_throw",
            particle_id: "bong:dugu_taint_pulse",
            sound_recipe_id: "dugu_needle_hiss",
            hud_hint: "蚀针",
            icon_texture: "bong:textures/gui/skill/dugu_eclipse.png",
        }
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut q = world.query::<&mut Client>();
        for mut c in q.iter_mut(world) {
            c.flush_packets().expect("mock client flush should succeed");
        }
    }

    fn drained_payloads(helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            payloads.push(serde_json::from_slice::<serde_json::Value>(packet.data.0 .0).unwrap());
        }
        payloads
    }

    // ─── eclipse ─────────────────────────────────────────────────

    #[test]
    fn eclipse_emit_sends_redis_and_s2c() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<EclipseNeedleEvent>();
        app.add_systems(Update, publish_dugu_v2_eclipse_events);

        let (client_bundle, mut helper) = create_mock_client("Caster");
        let caster = app.world_mut().spawn(client_bundle).id();
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(EclipseNeedleEvent {
            caster,
            target,
            target_realm: Realm::Solidify,
            tier: TaintTier::Temporary,
            injected_qi: 25.0,
            hp_loss: 15.0,
            qi_loss: 25.0,
            qi_max_loss: 3.0,
            permanent_decay_rate_per_min: 0.0,
            returned_zone_qi: 24.75,
            reveal_probability: 0.0042,
            tick: 42,
            visual: visual(),
        });
        app.update();
        flush_all_client_packets(&mut app);

        // Redis 仍发（回归：S2C 追加不破坏 agent 链）
        match rx_outbound.try_recv().expect("eclipse Redis send expected") {
            RedisOutbound::DuguV2Cast(p) => {
                assert_eq!(
                    p.skill,
                    DuguV2SkillIdV1::Eclipse,
                    "Redis payload skill 应为 Eclipse，不被 S2C 追加改变"
                );
                assert_eq!(p.taint_tier, Some(DuguTaintTierV1::Temporary));
                assert!(
                    (p.reveal_probability - 0.0042).abs() < f32::EPSILON,
                    "reveal_probability 应保持 0.0042，实际={}",
                    p.reveal_probability
                );
            }
            other => panic!("期望 DuguV2Cast Redis outbound，实际={other:?}"),
        }
        assert!(
            matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
            "eclipse emit 应只发一条 Redis message，不因 S2C 追加而多发"
        );

        // S2C payload 发给 caster
        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "eclipse emit 应向 caster 发 1 条 S2C payload；实际={}",
            payloads.len()
        );
        assert_eq!(
            payloads[0]["type"], "dugu_v2_skill_cast",
            "S2C payload type 应为 dugu_v2_skill_cast；实际={}",
            payloads[0]["type"]
        );
        assert_eq!(
            payloads[0]["kind"], "eclipse",
            "S2C kind 应为 eclipse；实际={}",
            payloads[0]["kind"]
        );
        assert_eq!(
            payloads[0]["taint_tier"].as_str().unwrap(),
            "temporary",
            "taint_tier 应为 snake_case 'temporary'（不是 Debug 格式 'Temporary'）；实际={}",
            payloads[0]["taint_tier"]
        );
    }

    // ─── penetrate ───────────────────────────────────────────────

    #[test]
    fn penetrate_emit_sends_redis_and_s2c() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PenetrateChainEvent>();
        app.add_systems(Update, publish_dugu_v2_penetrate_events);

        let (client_bundle, mut helper) = create_mock_client("Caster");
        let caster = app.world_mut().spawn(client_bundle).id();
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(PenetrateChainEvent {
            caster,
            target,
            taint_tier: TaintTier::Permanent,
            multiplier: 2.5,
            affected_targets: 1,
            permanent_decay_rate_per_min: 0.001,
            reveal_probability: 0.007,
            returned_zone_qi: 0.0,
            tick: 99,
            visual: visual(),
        });
        app.update();
        flush_all_client_packets(&mut app);

        match rx_outbound
            .try_recv()
            .expect("penetrate Redis send expected")
        {
            RedisOutbound::DuguV2Cast(p) => {
                assert_eq!(
                    p.skill,
                    DuguV2SkillIdV1::Penetrate,
                    "Redis skill 应为 Penetrate"
                );
                assert_eq!(p.taint_tier, Some(DuguTaintTierV1::Permanent));
            }
            other => panic!("期望 DuguV2Cast Redis outbound，实际={other:?}"),
        }
        assert!(matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)));

        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "penetrate 应发 1 条 S2C；实际={}",
            payloads.len()
        );
        assert_eq!(payloads[0]["type"], "dugu_v2_skill_cast");
        assert_eq!(payloads[0]["kind"], "penetrate");
        assert_eq!(
            payloads[0]["taint_tier"].as_str().unwrap(),
            "permanent",
            "taint_tier wire 应为 'permanent'（snake_case）"
        );
    }

    // ─── shroud ──────────────────────────────────────────────────

    #[test]
    fn shroud_emit_sends_redis_and_s2c_shroud_active() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<ShroudActivatedEvent>();
        app.add_systems(Update, publish_dugu_v2_shroud_events);

        let (client_bundle, mut helper) = create_mock_client("Shroud");
        let caster = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(ShroudActivatedEvent {
            caster,
            strength: 0.8,
            expires_at_tick: 200,
            tick: 100,
            visual: visual(),
        });
        app.update();
        flush_all_client_packets(&mut app);

        // Redis 发 DuguV2Cast（shroud kind）
        match rx_outbound.try_recv().expect("shroud Redis send expected") {
            RedisOutbound::DuguV2Cast(p) => {
                assert_eq!(p.skill, DuguV2SkillIdV1::Shroud, "Redis skill 应为 Shroud");
            }
            other => panic!("期望 DuguV2Cast Redis outbound，实际={other:?}"),
        }
        assert!(matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)));

        // S2C 发 dugu_v2_shroud_active（含 expires_at_tick）
        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "shroud 应发 1 条 S2C shroud_active；实际={}",
            payloads.len()
        );
        assert_eq!(
            payloads[0]["type"], "dugu_v2_shroud_active",
            "S2C type 应为 dugu_v2_shroud_active；实际={}",
            payloads[0]["type"]
        );
        assert_eq!(
            payloads[0]["expires_at_tick"], 200,
            "expires_at_tick 应为 200（持续时间不丢）；实际={}",
            payloads[0]["expires_at_tick"]
        );
        assert!(
            (payloads[0]["strength"].as_f64().unwrap() - 0.8).abs() < 1e-4,
            "strength 应为 0.8；实际={}",
            payloads[0]["strength"]
        );
    }

    // ─── self_cure ───────────────────────────────────────────────

    #[test]
    fn self_cure_emit_sends_redis_and_s2c_with_self_revealed() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<SelfCureProgressEvent>();
        app.add_systems(Update, publish_dugu_v2_self_cure_events);

        let (client_bundle, mut helper) = create_mock_client("SelfCure");
        let caster = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(SelfCureProgressEvent {
            caster,
            hours_used: 2.5,
            daily_hours_after: 10.5,
            gain_percent: 37.2,
            insidious_color_percent: 15.0,
            morphology_percent: 22.0,
            self_revealed: true,
            tick: 300,
        });
        app.update();
        flush_all_client_packets(&mut app);

        // Redis 仍发 DuguV2SelfCure（agent 叙事链回归）
        match rx_outbound
            .try_recv()
            .expect("self_cure Redis send expected")
        {
            RedisOutbound::DuguV2SelfCure(p) => {
                assert!(
                    p.self_revealed,
                    "Redis payload self_revealed 应为 true（agent 链需要此字段叙事）"
                );
                assert!(
                    (p.gain_percent - 37.2).abs() < 0.01,
                    "Redis payload gain_percent 应为 37.2；实际={}",
                    p.gain_percent
                );
            }
            other => panic!("期望 DuguV2SelfCure Redis outbound，实际={other:?}"),
        }
        assert!(matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)));

        // S2C 发 dugu_v2_self_cure，self_revealed=true
        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "self_cure 应发 1 条 S2C；实际={}",
            payloads.len()
        );
        assert_eq!(payloads[0]["type"], "dugu_v2_self_cure");
        assert_eq!(
            payloads[0]["self_revealed"], true,
            "S2C self_revealed 应为 true → client DuguV2HudStateStore.selfRevealed sticky flag"
        );
        assert!(
            (payloads[0]["gain_percent"].as_f64().unwrap() - 37.2).abs() < 0.1,
            "gain_percent 应为 37.2；实际={}",
            payloads[0]["gain_percent"]
        );
    }

    // ─── reverse ─────────────────────────────────────────────────

    #[test]
    fn reverse_emit_sends_redis_and_s2c() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<ReverseTriggeredEvent>();
        app.add_systems(Update, publish_dugu_v2_reverse_events);

        let (client_bundle, mut helper) = create_mock_client("Reverse");
        let caster = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(ReverseTriggeredEvent {
            caster,
            affected_targets: 3,
            burst_damage: 120.0,
            returned_zone_qi: 50.0,
            juebi_delay_ticks: Some(5),
            tick: 500,
            center: valence::math::DVec3::new(0.0, 64.0, 0.0),
            visual: visual(),
        });
        app.update();
        flush_all_client_packets(&mut app);

        match rx_outbound.try_recv().expect("reverse Redis send expected") {
            RedisOutbound::DuguV2Reverse(p) => {
                assert_eq!(p.affected_targets, 3, "Redis affected_targets 应为 3");
                assert_eq!(p.juebi_delay_ticks, Some(5));
            }
            other => panic!("期望 DuguV2Reverse Redis outbound，实际={other:?}"),
        }
        assert!(matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)));

        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "reverse 应发 1 条 S2C；实际={}",
            payloads.len()
        );
        assert_eq!(payloads[0]["type"], "dugu_v2_skill_cast");
        assert_eq!(payloads[0]["kind"], "reverse");
    }

    // ─── permanent_qi_max_decay ──────────────────────────────────

    #[test]
    fn permanent_qi_decay_emit_sends_s2c_only_no_redis() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PermanentQiMaxDecayApplied>();
        app.add_systems(Update, publish_permanent_qi_max_decay_to_client);

        let (client_bundle, mut helper) = create_mock_client("Target");
        let target = app.world_mut().spawn(client_bundle).id();
        let caster = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(PermanentQiMaxDecayApplied {
            target,
            caster,
            loss: 0.45,
            qi_max_after: 99.55,
            tick: 1000,
        });
        app.update();
        flush_all_client_packets(&mut app);

        // 不走 Redis（避免淹没 agent channel）
        assert!(
            matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
            "permanent_qi_max_decay 不应发 Redis（仅 S2C）"
        );

        // S2C 发 permanent_qi_max_decay_applied，只读 loss/qi_max_after
        let payloads = drained_payloads(&mut helper);
        assert_eq!(
            payloads.len(),
            1,
            "permanent_qi_decay 应发 1 条 S2C；实际={}",
            payloads.len()
        );
        assert_eq!(payloads[0]["type"], "permanent_qi_max_decay_applied");
        assert!(
            (payloads[0]["loss"].as_f64().unwrap() - 0.45).abs() < 0.01,
            "loss 应为 0.45（只读不重算）；实际={}",
            payloads[0]["loss"]
        );
        assert!(
            (payloads[0]["qi_max_after"].as_f64().unwrap() - 99.55).abs() < 0.1,
            "qi_max_after 应为 99.55（只读不重算）；实际={}",
            payloads[0]["qi_max_after"]
        );
    }

    // ─── 旧有 snapshot 回归（保证原有 Redis 功能不被破坏） ────────

    #[test]
    fn eclipse_payload_uses_resolved_reveal_probability() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<EclipseNeedleEvent>();
        app.add_systems(Update, publish_dugu_v2_eclipse_events);

        let caster = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(EclipseNeedleEvent {
            caster,
            target,
            target_realm: Realm::Solidify,
            tier: TaintTier::Temporary,
            injected_qi: 25.0,
            hp_loss: 15.0,
            qi_loss: 25.0,
            qi_max_loss: 3.0,
            permanent_decay_rate_per_min: 0.0,
            returned_zone_qi: 24.75,
            reveal_probability: 0.0042,
            tick: 42,
            visual: visual(),
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("dugu eclipse payload should publish")
        {
            RedisOutbound::DuguV2Cast(payload) => {
                assert_eq!(payload.skill, DuguV2SkillIdV1::Eclipse);
                assert_eq!(payload.taint_tier, Some(DuguTaintTierV1::Temporary));
                assert!((payload.reveal_probability - 0.0042).abs() < f32::EPSILON);
            }
            other => panic!("expected dugu v2 cast outbound, got {other:?}"),
        }
    }

    #[test]
    fn penetrate_payload_preserves_event_taint_tier() {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<PenetrateChainEvent>();
        app.add_systems(Update, publish_dugu_v2_penetrate_events);

        let caster = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(PenetrateChainEvent {
            caster,
            target,
            taint_tier: TaintTier::Temporary,
            multiplier: 2.5,
            affected_targets: 1,
            permanent_decay_rate_per_min: 0.0,
            reveal_probability: 0.007,
            returned_zone_qi: 0.0,
            tick: 99,
            visual: visual(),
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("dugu penetrate payload should publish")
        {
            RedisOutbound::DuguV2Cast(payload) => {
                assert_eq!(payload.skill, DuguV2SkillIdV1::Penetrate);
                assert_eq!(payload.taint_tier, Some(DuguTaintTierV1::Temporary));
                assert!((payload.reveal_probability - 0.007).abs() < f32::EPSILON);
            }
            other => panic!("expected dugu v2 cast outbound, got {other:?}"),
        }
    }
}
