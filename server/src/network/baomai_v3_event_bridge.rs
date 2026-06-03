use valence::prelude::{Entity, EventReader, Query, Res, Username};

use crate::combat::baomai_v3::{
    BaomaiSkillEvent, BaomaiSkillId, BloodBurnEvent, BodyTranscendenceExpiredEvent,
    MountainShakeEvent, OverloadMeridianRippleEvent,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::baomai_v3::{
    BaomaiSkillEventV1, BaomaiSkillIdV1, BaomaiV3BloodBurnV1, BaomaiV3MountainShakeV1,
    BaomaiV3OverloadRippleV1, BaomaiV3TranscendenceExpiredV1,
};
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

// plan-combat-skill-feedback-bridges-v1 P2 — 爆脉 v3 残余事件 publish fns

/// 山震震波事件桥：`MountainShakeEvent` → `RedisOutbound::BaomaiV3MountainShake`
/// affected_count = event.affected.len()（只传数量，不做 entity 遍历）
pub fn publish_mountain_shake_event(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<MountainShakeEvent>,
    usernames: Query<&Username>,
) {
    for event in events.read() {
        let mut payload = BaomaiV3MountainShakeV1::new(
            entity_wire_id(usernames.get(event.caster).ok(), event.caster),
            event.affected.len(),
            event.tick,
        );
        payload.qi_spent = event.qi_spent;
        payload.radius_blocks = event.radius_blocks;
        payload.shock_damage = event.shock_damage;

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV3MountainShake(payload))
        {
            tracing::warn!("[bong][baomai-v3] failed to queue mountain_shake event: {error}");
        }
    }
}

/// 血燃事件桥：`BloodBurnEvent` → `RedisOutbound::BaomaiV3BloodBurn`
/// 携 ended_in_near_death 分支标志供 agent 差异化叙事。
pub fn publish_blood_burn_event(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<BloodBurnEvent>,
    usernames: Query<&Username>,
) {
    for event in events.read() {
        let mut payload = BaomaiV3BloodBurnV1::new(
            entity_wire_id(usernames.get(event.caster).ok(), event.caster),
            event.tick,
        );
        payload.hp_burned = event.hp_burned;
        payload.qi_multiplier = event.qi_multiplier;
        payload.active_until_tick = event.active_until_tick;
        payload.ended_in_near_death = event.ended_in_near_death;

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV3BloodBurn(payload))
        {
            tracing::warn!("[bong][baomai-v3] failed to queue blood_burn event: {error}");
        }
    }
}

/// 超越到期事件桥：`BodyTranscendenceExpiredEvent` → `RedisOutbound::BaomaiV3TranscendenceExpired`
pub fn publish_body_transcendence_expired(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<BodyTranscendenceExpiredEvent>,
    usernames: Query<&Username>,
) {
    for event in events.read() {
        let payload = BaomaiV3TranscendenceExpiredV1::new(
            entity_wire_id(usernames.get(event.caster).ok(), event.caster),
            event.tick,
        );

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV3TranscendenceExpired(payload))
        {
            tracing::warn!(
                "[bong][baomai-v3] failed to queue transcendence_expired event: {error}"
            );
        }
    }
}

/// 过载涟漪事件桥：`OverloadMeridianRippleEvent` → `RedisOutbound::BaomaiV3OverloadRipple`
/// Bevy 多 reader 安全：baomai_v4/scar_history.rs:56 的既有 reader 不受影响。
pub fn publish_overload_ripple_event(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<OverloadMeridianRippleEvent>,
    usernames: Query<&Username>,
) {
    for event in events.read() {
        let skill_id = match event.skill {
            BaomaiSkillId::BengQuan => BaomaiSkillIdV1::BengQuan,
            BaomaiSkillId::FullPowerCharge => BaomaiSkillIdV1::FullPowerCharge,
            BaomaiSkillId::FullPowerRelease => BaomaiSkillIdV1::FullPowerRelease,
            BaomaiSkillId::MountainShake => BaomaiSkillIdV1::MountainShake,
            BaomaiSkillId::BloodBurn => BaomaiSkillIdV1::BloodBurn,
            BaomaiSkillId::Disperse => BaomaiSkillIdV1::Disperse,
        };
        let mut payload = BaomaiV3OverloadRippleV1::new(
            entity_wire_id(usernames.get(event.caster).ok(), event.caster),
            event.tick,
            skill_id,
        );
        payload.severity_delta = event.severity_delta;
        payload.total_severity = event.total_severity;
        payload.meridian_ids = event
            .meridian_ids
            .iter()
            .map(|id| meridian_id_to_string(*id).to_string())
            .collect();

        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::BaomaiV3OverloadRipple(payload))
        {
            tracing::warn!("[bong][baomai-v3] failed to queue overload_ripple event: {error}");
        }
    }
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

    fn app_with_bridge_p2() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<MountainShakeEvent>();
        app.add_event::<BloodBurnEvent>();
        app.add_event::<BodyTranscendenceExpiredEvent>();
        app.add_event::<OverloadMeridianRippleEvent>();
        app.add_systems(Update, publish_mountain_shake_event);
        app.add_systems(Update, publish_blood_burn_event);
        app.add_systems(Update, publish_body_transcendence_expired);
        app.add_systems(Update, publish_overload_ripple_event);
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

    // plan-combat-skill-feedback-bridges-v1 P2 — publish fn tests

    #[test]
    fn mountain_shake_event_publishes_with_affected_count_and_damage() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        let caster = app.world_mut().spawn_empty().id();
        let target1 = app.world_mut().spawn_empty().id();
        let target2 = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<MountainShakeEvent>>()
            .send(MountainShakeEvent {
                caster,
                affected: vec![target1, target2],
                tick: 200,
                qi_spent: 1200.0,
                radius_blocks: 5.0,
                shock_damage: 420.0,
            });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("mountain_shake event should publish")
        {
            RedisOutbound::BaomaiV3MountainShake(payload) => {
                assert_eq!(
                    payload.affected_count, 2,
                    "affected_count must equal affected.len()=2"
                );
                assert_eq!(payload.qi_spent, 1200.0);
                assert_eq!(payload.radius_blocks, 5.0);
                assert_eq!(payload.shock_damage, 420.0);
                assert!(payload.caster_id.starts_with("char:"));
            }
            other => panic!("expected BaomaiV3MountainShake outbound, got {other:?}"),
        }
    }

    #[test]
    fn mountain_shake_no_event_produces_no_outbound() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        app.update();
        assert!(
            rx_outbound.try_recv().is_err(),
            "no MountainShakeEvent → no outbound message"
        );
    }

    #[test]
    fn blood_burn_event_publishes_near_death_true() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<BloodBurnEvent>>()
            .send(BloodBurnEvent {
                caster,
                tick: 300,
                hp_burned: 300.0,
                qi_multiplier: 5.0,
                active_until_tick: 300,
                ended_in_near_death: true,
            });

        app.update();

        // skip mountain_shake (no event), check blood_burn
        match rx_outbound
            .try_recv()
            .expect("blood_burn event should publish")
        {
            RedisOutbound::BaomaiV3BloodBurn(payload) => {
                assert!(
                    payload.ended_in_near_death,
                    "ended_in_near_death must be forwarded as true"
                );
                assert_eq!(payload.hp_burned, 300.0);
                assert_eq!(payload.qi_multiplier, 5.0);
            }
            other => panic!("expected BaomaiV3BloodBurn outbound, got {other:?}"),
        }
    }

    #[test]
    fn blood_burn_event_publishes_near_death_false() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<BloodBurnEvent>>()
            .send(BloodBurnEvent {
                caster,
                tick: 300,
                hp_burned: 150.0,
                qi_multiplier: 3.5,
                active_until_tick: 360,
                ended_in_near_death: false,
            });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("blood_burn event (normal) should publish")
        {
            RedisOutbound::BaomaiV3BloodBurn(payload) => {
                assert!(
                    !payload.ended_in_near_death,
                    "normal blood_burn ended_in_near_death must be false"
                );
                assert_eq!(payload.hp_burned, 150.0);
            }
            other => panic!("expected BaomaiV3BloodBurn outbound, got {other:?}"),
        }
    }

    #[test]
    fn blood_burn_no_event_produces_no_outbound() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        app.update();
        assert!(
            rx_outbound.try_recv().is_err(),
            "no BloodBurnEvent → no outbound message"
        );
    }

    #[test]
    fn transcendence_expired_publishes_tick() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<BodyTranscendenceExpiredEvent>>()
            .send(BodyTranscendenceExpiredEvent { caster, tick: 700 });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("transcendence_expired should publish")
        {
            RedisOutbound::BaomaiV3TranscendenceExpired(payload) => {
                assert_eq!(payload.tick, 700);
                assert!(payload.caster_id.starts_with("char:"));
            }
            other => panic!("expected BaomaiV3TranscendenceExpired outbound, got {other:?}"),
        }
    }

    #[test]
    fn transcendence_expired_no_event_produces_no_outbound() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        app.update();
        assert!(
            rx_outbound.try_recv().is_err(),
            "no BodyTranscendenceExpiredEvent → no outbound message"
        );
    }

    #[test]
    fn overload_ripple_publishes_total_severity_and_meridian_ids() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        let caster = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Events<OverloadMeridianRippleEvent>>()
            .send(OverloadMeridianRippleEvent {
                caster,
                tick: 150,
                skill: BaomaiSkillId::BengQuan,
                severity_delta: 0.05,
                total_severity: 0.35,
                meridian_ids: vec![MeridianId::LargeIntestine, MeridianId::Lung],
            });

        app.update();

        match rx_outbound
            .try_recv()
            .expect("overload_ripple should publish")
        {
            RedisOutbound::BaomaiV3OverloadRipple(payload) => {
                assert_eq!(payload.skill_id, BaomaiSkillIdV1::BengQuan);
                assert_eq!(payload.severity_delta, 0.05);
                assert_eq!(payload.total_severity, 0.35);
                assert_eq!(payload.meridian_ids.len(), 2);
            }
            other => panic!("expected BaomaiV3OverloadRipple outbound, got {other:?}"),
        }
    }

    #[test]
    fn overload_ripple_no_event_produces_no_outbound() {
        let (mut app, rx_outbound) = app_with_bridge_p2();
        app.update();
        assert!(
            rx_outbound.try_recv().is_err(),
            "no OverloadMeridianRippleEvent → no outbound message"
        );
    }
}
