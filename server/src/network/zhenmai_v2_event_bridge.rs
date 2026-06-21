use valence::prelude::{Client, Entity, EventReader, Query, Res, Username};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::zhenmai_v2::{
    BackfireAmplificationActiveEvent, LocalNeutralizeEvent, MeridianHardenEvent,
    MeridianSeveredVoluntaryEvent, MultiPointBackfireEvent, ParrySuccessEvent, ZhenmaiAttackKind,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::{log_payload_build_error, send_server_data_payload, RedisBridgeResource};
use crate::schema::cultivation::meridian_id_to_string;
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, ZhenmaiHudV1};
use crate::schema::zhenmai_v2::{ZhenmaiAttackKindV1, ZhenmaiSkillEventV1, ZhenmaiSkillIdV1};

/// 50ms per tick（TICKS_PER_SECOND = 20）。用于把 expires_at_tick → 相对 duration_ms。
const MS_PER_TICK: u64 = 1_000 / TICKS_PER_SECOND;

/// 把 (expires_at_tick - now_tick) 换算成相对 duration_ms。
/// expires 已过或相等 → 0（client `readDuration` 回退 DEFAULT_DURATION_MS）。
fn ticks_to_duration_ms(now_tick: u64, expires_at_tick: u64) -> u64 {
    expires_at_tick
        .saturating_sub(now_tick)
        .saturating_mul(MS_PER_TICK)
}

#[allow(clippy::too_many_arguments)]
pub fn publish_zhenmai_skill_events(
    redis: Res<RedisBridgeResource>,
    mut neutralize_events: EventReader<LocalNeutralizeEvent>,
    mut multipoint_events: EventReader<MultiPointBackfireEvent>,
    mut harden_events: EventReader<MeridianHardenEvent>,
    mut severed_events: EventReader<MeridianSeveredVoluntaryEvent>,
    mut amplification_events: EventReader<BackfireAmplificationActiveEvent>,
    mut parry_events: EventReader<ParrySuccessEvent>,
    usernames: Query<&Username>,
    mut clients: Query<&mut Client>,
) {
    for event in neutralize_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::Neutralize,
            event.caster,
            event.tick,
            &usernames,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        send_zhenmai_payload(&redis, payload);
        // S2C dual-emit（mirror dugu_v2）→ client ZhenmaiHudServerDataHandler "neutralize"
        send_zhenmai_hud(
            &mut clients,
            event.caster,
            ZhenmaiHudV1 {
                skill_id: "neutralize".to_string(),
                meridian_id: meridian_id_to_string(event.meridian_id).to_string(),
                contam_removed: event.contam_removed as f32,
                remaining_points: 0,
                damage_reduction: 0.0,
                k_drain: 0.0,
                duration_ms: 0,
                tick: event.tick,
            },
        );
    }

    for event in multipoint_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::Multipoint,
            event.defender,
            event.tick,
            &usernames,
        );
        payload.target_id = event
            .attacker
            .map(|attacker| entity_wire_id(usernames.get(attacker).ok(), attacker));
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.reflected_qi = Some(event.reflected_qi);
        send_zhenmai_payload(&redis, payload);
        // S2C dual-emit → "multipoint"（剩余反震点数 + 激活窗口 duration）
        send_zhenmai_hud(
            &mut clients,
            event.defender,
            ZhenmaiHudV1 {
                skill_id: "multipoint".to_string(),
                meridian_id: String::new(),
                contam_removed: 0.0,
                remaining_points: event.remaining_points,
                damage_reduction: 0.0,
                k_drain: 0.0,
                duration_ms: ticks_to_duration_ms(event.tick, event.expires_at_tick),
                tick: event.tick,
            },
        );
    }

    for event in harden_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::HardenMeridian,
            event.caster,
            event.tick,
            &usernames,
        );
        payload.meridian_ids = Some(
            event
                .meridian_ids
                .iter()
                .map(|id| meridian_id_to_string(*id).to_string())
                .collect(),
        );
        payload.damage_multiplier = Some(event.damage_multiplier);
        send_zhenmai_payload(&redis, payload);
        // S2C dual-emit → "harden"（减伤% + 首条经脉 + buff duration）
        // event.damage_multiplier 是「伤害通过率」（passthrough，e.g. Spirit 境 0.35 = 仅放过 35%
        // 伤害），但 client ZhenmaiHudPlanner.appendHarden 把 damage_reduction 当作「减伤比例」
        // （value1*100 → 「减伤X%」、条形填充 = value1，1.0=全免）。两者方向相反，必须在 wire
        // 转换：reduction = 1 - passthrough。Spirit 0.35→0.65，Void 0.20→0.80，Awaken 0.85→0.15。
        let damage_reduction = 1.0_f32 - event.damage_multiplier.clamp(0.0, 1.0);
        send_zhenmai_hud(
            &mut clients,
            event.caster,
            ZhenmaiHudV1 {
                skill_id: "harden".to_string(),
                meridian_id: event
                    .meridian_ids
                    .first()
                    .map(|id| meridian_id_to_string(*id).to_string())
                    .unwrap_or_default(),
                contam_removed: 0.0,
                remaining_points: 0,
                damage_reduction,
                k_drain: 0.0,
                duration_ms: ticks_to_duration_ms(event.tick, event.expires_at_tick),
                tick: event.tick,
            },
        );
    }

    for event in severed_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::SeverChain,
            event.caster,
            event.tick,
            &usernames,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.grants_amplification = Some(event.grants_amplification);
        send_zhenmai_payload(&redis, payload);
        // S2C dual-emit → "sever_chain"（断脉标记；增幅窗口在 amplification 事件携 k_drain+duration）
        send_zhenmai_hud(
            &mut clients,
            event.caster,
            ZhenmaiHudV1 {
                skill_id: "sever_chain".to_string(),
                meridian_id: meridian_id_to_string(event.meridian_id).to_string(),
                contam_removed: 0.0,
                remaining_points: 0,
                damage_reduction: 0.0,
                k_drain: 0.0,
                duration_ms: 0,
                tick: event.tick,
            },
        );
    }

    for event in amplification_events.read() {
        let started_tick = event
            .expires_at_tick
            .saturating_sub(crate::combat::zhenmai_v2::BACKFIRE_AMPLIFICATION_TICKS);
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::SeverChain,
            event.caster,
            started_tick,
            &usernames,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.k_drain = Some(event.k_drain);
        payload.self_damage_multiplier = Some(event.self_damage_multiplier);
        payload.expires_at_tick = Some(event.expires_at_tick);
        send_zhenmai_payload(&redis, payload);
        // S2C dual-emit → "sever_chain"（反噬增幅窗口：k_drain 强度 + 倒计时 duration）
        send_zhenmai_hud(
            &mut clients,
            event.caster,
            ZhenmaiHudV1 {
                skill_id: "sever_chain".to_string(),
                meridian_id: meridian_id_to_string(event.meridian_id).to_string(),
                contam_removed: 0.0,
                remaining_points: 0,
                damage_reduction: 0.0,
                k_drain: event.k_drain as f32,
                duration_ms: ticks_to_duration_ms(started_tick, event.expires_at_tick),
                tick: started_tick,
            },
        );
    }

    for event in parry_events.read() {
        // parry 无 agent 叙事 Redis 形态（ZhenmaiSkillEventV1 不含 parry skill_id 以外语义），
        // 但 S2C HUD 需要瞬态弹反闪现 → 仅 S2C dual-emit "parry"。
        send_zhenmai_hud(
            &mut clients,
            event.caster,
            ZhenmaiHudV1 {
                skill_id: "parry".to_string(),
                meridian_id: String::new(),
                contam_removed: 0.0,
                remaining_points: 0,
                damage_reduction: 0.0,
                k_drain: 0.0,
                duration_ms: 0,
                tick: event.tick,
            },
        );
    }
}

/// 向施法者实体发送 zhenmai_hud S2C payload（mirror dugu_v2 send_s2c_to_entity）。
fn send_zhenmai_hud(clients: &mut Query<&mut Client>, entity: Entity, hud: ZhenmaiHudV1) {
    let Ok(mut client) = clients.get_mut(entity) else {
        return;
    };
    let envelope = ServerDataV1::new(ServerDataPayloadV1::ZhenmaiHud(hud));
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
        "[bong][network] sent {} {} payload (zhenmai_v2)",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

fn send_zhenmai_payload(redis: &RedisBridgeResource, payload: ZhenmaiSkillEventV1) {
    if let Err(error) = redis
        .tx_outbound
        .send(RedisOutbound::ZhenmaiSkillEvent(payload))
    {
        tracing::warn!("[bong][zhenmai] failed to queue zhenmai skill event: {error}");
    }
}

fn base_payload(
    skill_id: ZhenmaiSkillIdV1,
    caster: Entity,
    tick: u64,
    usernames: &Query<&Username>,
) -> ZhenmaiSkillEventV1 {
    ZhenmaiSkillEventV1::new(
        skill_id,
        entity_wire_id(usernames.get(caster).ok(), caster),
        tick,
    )
}

fn entity_wire_id(username: Option<&Username>, entity: Entity) -> String {
    username
        .map(|username| format!("offline:{}", username.0))
        .unwrap_or_else(|| format!("char:{}", entity.to_bits()))
}

fn attack_kind_payload(kind: ZhenmaiAttackKind) -> ZhenmaiAttackKindV1 {
    match kind {
        ZhenmaiAttackKind::RealYuan => ZhenmaiAttackKindV1::RealYuan,
        ZhenmaiAttackKind::PhysicalCarrier => ZhenmaiAttackKindV1::PhysicalCarrier,
        ZhenmaiAttackKind::TaintedYuan => ZhenmaiAttackKindV1::TaintedYuan,
        ZhenmaiAttackKind::Array => ZhenmaiAttackKindV1::Array,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Update};

    use crate::combat::zhenmai_v2::BACKFIRE_AMPLIFICATION_TICKS;
    use crate::cultivation::components::MeridianId;

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<LocalNeutralizeEvent>();
        app.add_event::<MultiPointBackfireEvent>();
        app.add_event::<MeridianHardenEvent>();
        app.add_event::<MeridianSeveredVoluntaryEvent>();
        app.add_event::<BackfireAmplificationActiveEvent>();
        app.add_event::<ParrySuccessEvent>();
        app.add_systems(Update, publish_zhenmai_skill_events);
        (app, rx_outbound)
    }

    #[test]
    fn publishes_sever_chain_and_amplification_payloads() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(MeridianSeveredVoluntaryEvent {
            caster,
            meridian_id: MeridianId::Heart,
            attack_kind: ZhenmaiAttackKind::TaintedYuan,
            grants_amplification: true,
            tick: 42,
        });
        app.world_mut()
            .send_event(BackfireAmplificationActiveEvent {
                caster,
                meridian_id: MeridianId::Heart,
                attack_kind: ZhenmaiAttackKind::TaintedYuan,
                k_drain: 1.5,
                self_damage_multiplier: 0.5,
                expires_at_tick: 42 + BACKFIRE_AMPLIFICATION_TICKS,
            });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("sever-chain event should publish")
        {
            RedisOutbound::ZhenmaiSkillEvent(payload) => {
                assert_eq!(payload.skill_id, ZhenmaiSkillIdV1::SeverChain);
                assert_eq!(payload.meridian_id.as_deref(), Some("Heart"));
                assert_eq!(payload.attack_kind, Some(ZhenmaiAttackKindV1::TaintedYuan));
                assert_eq!(payload.grants_amplification, Some(true));
                assert_eq!(payload.tick, 42);
                assert!(payload.caster_id.starts_with("char:"));
            }
            other => panic!("expected zhenmai sever-chain outbound, got {other:?}"),
        }

        match rx_outbound
            .try_recv()
            .expect("amplification event should publish")
        {
            RedisOutbound::ZhenmaiSkillEvent(payload) => {
                assert_eq!(payload.skill_id, ZhenmaiSkillIdV1::SeverChain);
                assert_eq!(payload.meridian_id.as_deref(), Some("Heart"));
                assert_eq!(payload.attack_kind, Some(ZhenmaiAttackKindV1::TaintedYuan));
                assert_eq!(payload.k_drain, Some(1.5));
                assert_eq!(payload.self_damage_multiplier, Some(0.5));
                assert_eq!(
                    payload.expires_at_tick,
                    Some(42 + BACKFIRE_AMPLIFICATION_TICKS)
                );
                assert_eq!(payload.tick, 42);
            }
            other => panic!("expected zhenmai amplification outbound, got {other:?}"),
        }
    }

    #[test]
    fn publishes_multipoint_backfire_target_and_reflection() {
        let (mut app, rx_outbound) = app_with_bridge();
        let defender = app.world_mut().spawn_empty().id();
        let attacker = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(MultiPointBackfireEvent {
            defender,
            attacker: Some(attacker),
            attack_kind: ZhenmaiAttackKind::PhysicalCarrier,
            contact_index: 3,
            reflected_qi: 12.5,
            remaining_points: 2,
            expires_at_tick: 199,
            tick: 99,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("multipoint event should publish")
        {
            RedisOutbound::ZhenmaiSkillEvent(payload) => {
                assert_eq!(payload.skill_id, ZhenmaiSkillIdV1::Multipoint);
                assert_eq!(
                    payload.attack_kind,
                    Some(ZhenmaiAttackKindV1::PhysicalCarrier)
                );
                assert_eq!(payload.reflected_qi, Some(12.5));
                assert_eq!(payload.tick, 99);
                assert!(payload.caster_id.starts_with("char:"));
                assert!(payload
                    .target_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("char:")));
            }
            other => panic!("expected zhenmai multipoint outbound, got {other:?}"),
        }
    }

    #[test]
    fn publishes_routable_offline_username_for_player_caster() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn(Username("Azure".to_string())).id();

        app.world_mut().send_event(LocalNeutralizeEvent {
            caster,
            meridian_id: MeridianId::Lung,
            contam_removed: 2.0,
            qi_spent: 8.0,
            tick: 64,
        });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("neutralize event should publish")
        {
            RedisOutbound::ZhenmaiSkillEvent(payload) => {
                assert_eq!(payload.skill_id, ZhenmaiSkillIdV1::Neutralize);
                assert_eq!(payload.caster_id, "offline:Azure");
                assert_eq!(payload.meridian_id.as_deref(), Some("Lung"));
            }
            other => panic!("expected zhenmai neutralize outbound, got {other:?}"),
        }
    }

    #[test]
    fn publishes_harden_damage_multiplier_without_self_damage_semantics() {
        let (mut app, rx_outbound) = app_with_bridge();
        let caster = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(MeridianHardenEvent {
            caster,
            meridian_ids: vec![MeridianId::Lung],
            damage_multiplier: 0.35,
            expires_at_tick: 677,
            tick: 77,
        });

        app.update();

        match rx_outbound.try_recv().expect("harden event should publish") {
            RedisOutbound::ZhenmaiSkillEvent(payload) => {
                assert_eq!(payload.skill_id, ZhenmaiSkillIdV1::HardenMeridian);
                assert_eq!(payload.damage_multiplier, Some(0.35));
                assert_eq!(payload.self_damage_multiplier, None);
                assert_eq!(payload.meridian_ids, Some(vec!["Lung".to_string()]));
            }
            other => panic!("expected zhenmai harden outbound, got {other:?}"),
        }
    }

    // ─── S2C dual-emit（mirror dugu_v2）：每招追发 zhenmai_hud 给 caster ──────

    mod s2c {
        use super::*;

        use crossbeam_channel::TryRecvError;
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::{create_mock_client, MockClientHelper};

        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;

        fn app_with_client_bridge() -> (
            App,
            crossbeam_channel::Receiver<RedisOutbound>,
            valence::prelude::Entity,
            MockClientHelper,
        ) {
            let mut app = App::new();
            let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
            let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
            app.insert_resource(RedisBridgeResource {
                tx_outbound,
                rx_inbound,
            });
            app.add_event::<LocalNeutralizeEvent>();
            app.add_event::<MultiPointBackfireEvent>();
            app.add_event::<MeridianHardenEvent>();
            app.add_event::<MeridianSeveredVoluntaryEvent>();
            app.add_event::<BackfireAmplificationActiveEvent>();
            app.add_event::<ParrySuccessEvent>();
            app.add_systems(Update, publish_zhenmai_skill_events);

            let (client_bundle, helper) = create_mock_client("Caster");
            let caster = app.world_mut().spawn(client_bundle).id();
            (app, rx_outbound, caster, helper)
        }

        fn flush_all_client_packets(app: &mut App) {
            let world = app.world_mut();
            let mut q = world.query::<&mut Client>();
            for mut c in q.iter_mut(world) {
                c.flush_packets().expect("mock client flush should succeed");
            }
        }

        fn drained_zhenmai_hud(helper: &mut MockClientHelper) -> Vec<serde_json::Value> {
            let mut payloads = Vec::new();
            for frame in helper.collect_received().0 {
                let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                    continue;
                };
                if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                    continue;
                }
                let v: serde_json::Value =
                    serde_json::from_slice(packet.data.0 .0).expect("server_data payload is JSON");
                if v["type"] == serde_json::json!("zhenmai_hud") {
                    payloads.push(v);
                }
            }
            payloads
        }

        #[test]
        fn neutralize_dual_emits_redis_and_s2c() {
            let (mut app, rx_outbound, caster, mut helper) = app_with_client_bridge();
            app.world_mut().send_event(LocalNeutralizeEvent {
                caster,
                meridian_id: MeridianId::Lung,
                contam_removed: 2.5,
                qi_spent: 8.0,
                tick: 64,
            });
            app.update();
            flush_all_client_packets(&mut app);

            // Redis 链回归（S2C 追发不破坏 agent 叙事）
            assert!(
                matches!(
                    rx_outbound.try_recv(),
                    Ok(RedisOutbound::ZhenmaiSkillEvent(_))
                ),
                "neutralize 应仍发 1 条 Redis ZhenmaiSkillEvent"
            );

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(
                huds.len(),
                1,
                "neutralize 应发 1 条 zhenmai_hud S2C；实际={}",
                huds.len()
            );
            assert_eq!(huds[0]["skill_id"], serde_json::json!("neutralize"));
            assert_eq!(
                huds[0]["meridian_id"],
                serde_json::json!("Lung"),
                "meridian_id 须传给 client（去毒维度按经脉显示）"
            );
            assert!(
                (huds[0]["contam_removed"].as_f64().unwrap() - 2.5).abs() < 1e-4,
                "contam_removed 须只读自事件 2.5，实际={}",
                huds[0]["contam_removed"]
            );
        }

        #[test]
        fn multipoint_dual_emit_carries_remaining_points_and_duration() {
            let (mut app, _rx, caster, mut helper) = app_with_client_bridge();
            // expires 比 tick 大 100 tick → 100*50 = 5000ms
            app.world_mut().send_event(MultiPointBackfireEvent {
                defender: caster,
                attacker: None,
                attack_kind: ZhenmaiAttackKind::RealYuan,
                contact_index: 1,
                reflected_qi: 10.0,
                remaining_points: 4,
                expires_at_tick: 200,
                tick: 100,
            });
            app.update();
            flush_all_client_packets(&mut app);

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(huds.len(), 1, "multipoint 应发 1 条 zhenmai_hud S2C");
            assert_eq!(huds[0]["skill_id"], serde_json::json!("multipoint"));
            assert_eq!(
                huds[0]["remaining_points"],
                serde_json::json!(4u32),
                "remaining_points 须只读自事件 4，实际={}",
                huds[0]["remaining_points"]
            );
            assert_eq!(
                huds[0]["duration_ms"],
                serde_json::json!(5_000u64),
                "duration_ms 须 = (200-100)tick * 50ms = 5000，实际={}",
                huds[0]["duration_ms"]
            );
        }

        #[test]
        fn harden_dual_emit_carries_reduction_meridian_and_duration() {
            let (mut app, _rx, caster, mut helper) = app_with_client_bridge();
            app.world_mut().send_event(MeridianHardenEvent {
                caster,
                meridian_ids: vec![MeridianId::Heart, MeridianId::Lung],
                // damage_multiplier=0.35 是「伤害通过率」（Spirit 境放过 35% 伤害）→
                // wire damage_reduction 须转成「减伤比例」0.65（实际减伤 65%），与 client
                // ZhenmaiHudPlanner「减伤%」语义对齐。
                damage_multiplier: 0.35,
                expires_at_tick: 90,
                tick: 70,
            });
            app.update();
            flush_all_client_packets(&mut app);

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(huds.len(), 1, "harden 应发 1 条 zhenmai_hud S2C");
            assert_eq!(huds[0]["skill_id"], serde_json::json!("harden"));
            assert_eq!(
                huds[0]["meridian_id"],
                serde_json::json!("Heart"),
                "harden meridian_id 须取首条经脉 Heart，实际={}",
                huds[0]["meridian_id"]
            );
            assert!(
                (huds[0]["damage_reduction"].as_f64().unwrap() - 0.65).abs() < 1e-4,
                "damage_reduction 须 = 1 - damage_multiplier(0.35) = 0.65（client 把它当减伤比例渲染「减伤65%」），实际={}",
                huds[0]["damage_reduction"]
            );
            assert_eq!(
                huds[0]["duration_ms"],
                serde_json::json!(1_000u64),
                "duration_ms 须 = (90-70)tick * 50ms = 1000，实际={}",
                huds[0]["duration_ms"]
            );
        }

        #[test]
        fn amplification_dual_emit_carries_k_drain_and_window_duration() {
            let (mut app, _rx, caster, mut helper) = app_with_client_bridge();
            let expires = BACKFIRE_AMPLIFICATION_TICKS; // started_tick = 0
            app.world_mut()
                .send_event(BackfireAmplificationActiveEvent {
                    caster,
                    meridian_id: MeridianId::Heart,
                    attack_kind: ZhenmaiAttackKind::TaintedYuan,
                    k_drain: 1.5,
                    self_damage_multiplier: 0.5,
                    expires_at_tick: expires,
                });
            app.update();
            flush_all_client_packets(&mut app);

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(
                huds.len(),
                1,
                "amplification 应发 1 条 sever_chain zhenmai_hud S2C"
            );
            assert_eq!(huds[0]["skill_id"], serde_json::json!("sever_chain"));
            assert!(
                (huds[0]["k_drain"].as_f64().unwrap() - 1.5).abs() < 1e-4,
                "k_drain 须只读自事件 1.5，实际={}",
                huds[0]["k_drain"]
            );
            // BACKFIRE_AMPLIFICATION_TICKS = 60*20 = 1200 tick → 1200*50 = 60000ms
            assert_eq!(
                huds[0]["duration_ms"],
                serde_json::json!(60_000u64),
                "增幅窗口 duration_ms 须 = 60s = 60000，实际={}",
                huds[0]["duration_ms"]
            );
        }

        #[test]
        fn parry_dual_emit_s2c_only_no_redis() {
            let (mut app, rx_outbound, caster, mut helper) = app_with_client_bridge();
            app.world_mut()
                .send_event(ParrySuccessEvent { caster, tick: 42 });
            app.update();
            flush_all_client_packets(&mut app);

            // parry 仅 S2C，不走 agent Redis 链
            assert!(
                matches!(rx_outbound.try_recv(), Err(TryRecvError::Empty)),
                "parry 不应发 Redis（无 agent 叙事形态），仅 S2C HUD"
            );

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(huds.len(), 1, "parry 应发 1 条 zhenmai_hud S2C");
            assert_eq!(
                huds[0]["skill_id"],
                serde_json::json!("parry"),
                "parry S2C skill_id 须为 'parry'（client flashParry 维度）"
            );
        }

        #[test]
        fn sever_chain_voluntary_dual_emits_redis_and_s2c() {
            let (mut app, rx_outbound, caster, mut helper) = app_with_client_bridge();
            app.world_mut().send_event(MeridianSeveredVoluntaryEvent {
                caster,
                meridian_id: MeridianId::Heart,
                attack_kind: ZhenmaiAttackKind::TaintedYuan,
                grants_amplification: true,
                tick: 42,
            });
            app.update();
            flush_all_client_packets(&mut app);

            assert!(
                matches!(
                    rx_outbound.try_recv(),
                    Ok(RedisOutbound::ZhenmaiSkillEvent(_))
                ),
                "voluntary sever 应仍发 Redis"
            );

            let huds = drained_zhenmai_hud(&mut helper);
            assert_eq!(huds.len(), 1, "voluntary sever 应发 1 条 zhenmai_hud S2C");
            assert_eq!(huds[0]["skill_id"], serde_json::json!("sever_chain"));
            assert_eq!(huds[0]["meridian_id"], serde_json::json!("Heart"));
        }
    }
}
