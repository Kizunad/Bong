use valence::prelude::{Entity, EventReader, Query, Res, UniqueId};

use crate::combat::zhenmai_v2::{
    BackfireAmplificationActiveEvent, LocalNeutralizeEvent, MeridianHardenEvent,
    MeridianSeveredVoluntaryEvent, MultiPointBackfireEvent, ZhenmaiAttackKind,
};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::RedisBridgeResource;
use crate::schema::cultivation::meridian_id_to_string;
use crate::schema::zhenmai_v2::{ZhenmaiAttackKindV1, ZhenmaiSkillEventV1, ZhenmaiSkillIdV1};

pub fn publish_zhenmai_skill_events(
    redis: Res<RedisBridgeResource>,
    mut neutralize_events: EventReader<LocalNeutralizeEvent>,
    mut multipoint_events: EventReader<MultiPointBackfireEvent>,
    mut harden_events: EventReader<MeridianHardenEvent>,
    mut severed_events: EventReader<MeridianSeveredVoluntaryEvent>,
    mut amplification_events: EventReader<BackfireAmplificationActiveEvent>,
    unique_ids: Query<&UniqueId>,
) {
    for event in neutralize_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::Neutralize,
            event.caster,
            event.tick,
            &unique_ids,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ZhenmaiSkillEvent(payload));
    }

    for event in multipoint_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::Multipoint,
            event.defender,
            event.tick,
            &unique_ids,
        );
        payload.target_id = event
            .attacker
            .map(|attacker| entity_wire_id(unique_ids.get(attacker).ok(), attacker));
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.reflected_qi = Some(event.reflected_qi);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ZhenmaiSkillEvent(payload));
    }

    for event in harden_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::HardenMeridian,
            event.caster,
            event.tick,
            &unique_ids,
        );
        payload.meridian_ids = Some(
            event
                .meridian_ids
                .iter()
                .map(|id| meridian_id_to_string(*id).to_string())
                .collect(),
        );
        payload.damage_multiplier = Some(event.damage_multiplier);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ZhenmaiSkillEvent(payload));
    }

    for event in severed_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::SeverChain,
            event.caster,
            event.tick,
            &unique_ids,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.grants_amplification = Some(event.grants_amplification);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ZhenmaiSkillEvent(payload));
    }

    for event in amplification_events.read() {
        let mut payload = base_payload(
            ZhenmaiSkillIdV1::SeverChain,
            event.caster,
            event
                .expires_at_tick
                .saturating_sub(crate::combat::zhenmai_v2::BACKFIRE_AMPLIFICATION_TICKS),
            &unique_ids,
        );
        payload.meridian_id = Some(meridian_id_to_string(event.meridian_id).to_string());
        payload.attack_kind = Some(attack_kind_payload(event.attack_kind));
        payload.k_drain = Some(event.k_drain);
        payload.self_damage_multiplier = Some(event.self_damage_multiplier);
        payload.expires_at_tick = Some(event.expires_at_tick);
        let _ = redis
            .tx_outbound
            .send(RedisOutbound::ZhenmaiSkillEvent(payload));
    }
}

fn base_payload(
    skill_id: ZhenmaiSkillIdV1,
    caster: Entity,
    tick: u64,
    unique_ids: &Query<&UniqueId>,
) -> ZhenmaiSkillEventV1 {
    ZhenmaiSkillEventV1::new(
        skill_id,
        entity_wire_id(unique_ids.get(caster).ok(), caster),
        tick,
    )
}

fn entity_wire_id(unique_id: Option<&UniqueId>, entity: Entity) -> String {
    unique_id
        .map(|unique_id| format!("player:{}", unique_id.0))
        .unwrap_or_else(|| format!("entity:{}", entity.to_bits()))
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
                assert!(payload.caster_id.starts_with("entity:"));
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
                assert!(payload.caster_id.starts_with("entity:"));
                assert!(payload
                    .target_id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("entity:")));
            }
            other => panic!("expected zhenmai multipoint outbound, got {other:?}"),
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
}
