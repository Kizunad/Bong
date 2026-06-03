//! plan-combat-skill-feedback-bridges-v1 P0 — 经脉永久 SEVERED 事件桥接器。
//!
//! `publish_meridian_severed_events` 单 system 覆盖全部 5+ emit 源：
//!   - `cultivation/meridian/severed.rs:230` meridian_severed_detection_tick
//!   - `cultivation/meridian/severed.rs:249` 直接 send
//!   - `combat/zhenmai_v2.rs:797`（VoluntarySever）
//!   - `combat/baomai_v3/skills.rs:870`（OverloadTear）
//!   - `combat/baomai_v4/dead_armor.rs:264`（CombatWound）
//!   - `cultivation/tribulation.rs:3211 & 3452`（TribulationFail）
//!
//! 每个 MeridianSeveredEvent：
//!   1. 发 RedisOutbound::MeridianSevered → agent 叙事。
//!   2. `source != VoluntarySever` 时向实体位置发 VfxEventRequest（SpawnParticle
//!      `bong:jiemai_sever_flash`），避免与 zhenmai SeverChain 主动技能重复发粒子。
//!
//! qi_concern = none：本桥只读转发，严禁操作真元账。

use valence::prelude::{DVec3, Entity, EventReader, EventWriter, Position, Query, Res, Username};

use crate::cultivation::meridian::severed::{MeridianSeveredEvent, SeveredSource};
use crate::network::redis_bridge::RedisOutbound;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::RedisBridgeResource;
use crate::schema::cultivation::meridian_id_to_string;
use crate::schema::meridian_severed::MeridianSeveredEventV1;
use crate::schema::vfx_event::VfxEventPayloadV1;

/// 经脉断脉 VFX 粒子 id（与 zhenmai_v2.rs:56 SEVER_FLASH_PARTICLE_ID 对齐）。
pub const SEVER_FLASH_PARTICLE_ID: &str = "bong:jiemai_sever_flash";

/// 断脉叙事+VFX 发射 system。
///
/// 参数：
///   - `redis`: 出站队列（send RedisOutbound::MeridianSevered）
///   - `events`: EventReader<MeridianSeveredEvent>（单入口覆盖所有来源）
///   - `vfx_events`: EventWriter<VfxEventRequest>（非 voluntary 时发粒子）
///   - `entities`: Query<(Option<&Username>, Option<&Position>)>（entity_wire_id + 位置）
pub fn publish_meridian_severed_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<MeridianSeveredEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    entities: Query<(Option<&Username>, Option<&Position>)>,
) {
    for event in events.read() {
        let entity_id = resolve_entity_id(event.entity, &entities);
        let meridian_id = meridian_id_to_string(event.meridian_id).to_string();

        // 1. agent 叙事：序列化 MeridianSeveredEventV1 → bong:meridian_severed
        let payload = MeridianSeveredEventV1::new(
            entity_id,
            meridian_id,
            event.source.clone(),
            event.at_tick,
        );
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::MeridianSevered(payload))
        {
            tracing::warn!(
                "[bong][meridian-severed] failed to queue MeridianSeveredEventV1: {error}"
            );
        }

        // 2. VFX：非 VoluntarySever 时发粒子（voluntary 已由 zhenmai SeverChain 自发，避免重复）
        if event.source != SeveredSource::VoluntarySever {
            let origin = entity_position(event.entity, &entities);
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: SEVER_FLASH_PARTICLE_ID.to_string(),
                    origin: [origin.x, origin.y + 1.0, origin.z],
                    direction: Some([0.0, 0.1, 0.0]),
                    color: None,
                    strength: Some(1.0),
                    count: Some(1),
                    duration_ticks: Some(20),
                },
            ));
        }
    }
}

/// entity_wire_id：有 Username 组件 → `"offline:{username}"`，否则 → `"char:{bits}"`。
fn resolve_entity_id(
    entity: Entity,
    entities: &Query<(Option<&Username>, Option<&Position>)>,
) -> String {
    let username = entities.get(entity).ok().and_then(|(u, _)| u);
    username
        .map(|u| format!("offline:{}", u.0))
        .unwrap_or_else(|| format!("char:{}", entity.to_bits()))
}

fn entity_position(
    entity: Entity,
    entities: &Query<(Option<&Username>, Option<&Position>)>,
) -> DVec3 {
    entities
        .get(entity)
        .ok()
        .and_then(|(_, p)| p)
        .map(|p| p.get())
        .unwrap_or(DVec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Events, Update};

    use crate::cultivation::components::MeridianId;
    use crate::network::RedisBridgeResource;

    fn app_with_bridge() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let mut app = App::new();
        let (tx_outbound, rx_outbound) = crossbeam_channel::unbounded();
        let (_tx_inbound, rx_inbound) = crossbeam_channel::unbounded();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<MeridianSeveredEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, publish_meridian_severed_events);
        (app, rx_outbound)
    }

    /// 单条 BackfireOverload 事件 → Redis outbound 1 条 + VFX 1 条（非 voluntary）
    #[test]
    fn backfire_overload_emits_redis_and_vfx() {
        let (mut app, rx_outbound) = app_with_bridge();

        // spawn a dummy entity with no Username or Position
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Lung,
            source: SeveredSource::BackfireOverload,
            at_tick: 100,
        });

        app.update();

        // Redis 出站：1 条 MeridianSevered
        let outbound = rx_outbound
            .try_recv()
            .expect("should receive one Redis msg");
        match outbound {
            RedisOutbound::MeridianSevered(evt) => {
                assert_eq!(
                    evt.event_type, "meridian_severed",
                    "type field should be 'meridian_severed'"
                );
                assert_eq!(evt.source, SeveredSource::BackfireOverload);
                assert_eq!(evt.at_tick, 100);
                // entity has no Username → char:{bits}
                assert!(
                    evt.entity_id.starts_with("char:"),
                    "NPC entity_id should be char:... prefix"
                );
            }
            other => panic!("expected MeridianSevered, got {other:?}"),
        }
        // No more Redis messages
        assert!(
            rx_outbound.try_recv().is_err(),
            "should have exactly 1 Redis message, no more"
        );

        // VFX 出站：非 voluntary → 1 条 VfxEventRequest
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let count = vfx_events.len();
        assert_eq!(
            count, 1,
            "BackfireOverload should produce exactly 1 VFX event, got {count}"
        );
    }

    /// VoluntarySever → Redis 1 条，但 VFX 0 条（避免与 zhenmai SeverChain 重复）
    #[test]
    fn voluntary_sever_skips_vfx() {
        let (mut app, rx_outbound) = app_with_bridge();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Du,
            source: SeveredSource::VoluntarySever,
            at_tick: 200,
        });

        app.update();

        // Redis 出站：1 条（叙事仍要发）
        let outbound = rx_outbound.try_recv().expect("Redis msg expected");
        match outbound {
            RedisOutbound::MeridianSevered(evt) => {
                assert_eq!(evt.source, SeveredSource::VoluntarySever);
                assert_eq!(evt.at_tick, 200);
            }
            other => panic!("expected MeridianSevered, got {other:?}"),
        }

        // VFX 出站：0 条（避免重复 jiemai_sever_flash）
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert_eq!(
            vfx_events.len(),
            0,
            "VoluntarySever should NOT produce VFX (duplicate of zhenmai SeverChain)"
        );
    }

    /// Other(String) 来源 → Redis + VFX 均发
    #[test]
    fn other_source_emits_redis_and_vfx() {
        let (mut app, rx_outbound) = app_with_bridge();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Heart,
            source: SeveredSource::Other("上古遗物反噬".to_string()),
            at_tick: 300,
        });

        app.update();

        let outbound = rx_outbound.try_recv().expect("Redis msg expected");
        match outbound {
            RedisOutbound::MeridianSevered(evt) => {
                assert_eq!(evt.source, SeveredSource::Other("上古遗物反噬".to_string()));
            }
            other => panic!("expected MeridianSevered, got {other:?}"),
        }

        // VFX 出站：1 条（Other 视同非 voluntary）
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert_eq!(vfx_events.len(), 1);
    }

    /// 3 条不同来源事件 → Redis 3 条
    #[test]
    fn multiple_events_in_same_tick() {
        let (mut app, rx_outbound) = app_with_bridge();

        let e1 = app.world_mut().spawn_empty().id();
        let e2 = app.world_mut().spawn_empty().id();
        let e3 = app.world_mut().spawn_empty().id();

        {
            let world = app.world_mut();
            world.send_event(MeridianSeveredEvent {
                entity: e1,
                meridian_id: MeridianId::Lung,
                source: SeveredSource::CombatWound,
                at_tick: 1,
            });
            world.send_event(MeridianSeveredEvent {
                entity: e2,
                meridian_id: MeridianId::Ren,
                source: SeveredSource::TribulationFail,
                at_tick: 2,
            });
            world.send_event(MeridianSeveredEvent {
                entity: e3,
                meridian_id: MeridianId::Liver,
                source: SeveredSource::DuguDistortion,
                at_tick: 3,
            });
        }

        app.update();

        let mut received = Vec::new();
        while let Ok(msg) = rx_outbound.try_recv() {
            received.push(msg);
        }
        assert_eq!(
            received.len(),
            3,
            "3 events should produce 3 Redis messages, got {}",
            received.len()
        );

        // VFX：3 条非 voluntary → 3 VFX
        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        assert_eq!(vfx_events.len(), 3);
    }

    /// VFX payload 验证：event_id = SEVER_FLASH_PARTICLE_ID
    #[test]
    fn vfx_event_uses_correct_particle_id() {
        let (mut app, _rx) = app_with_bridge();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Heart,
            source: SeveredSource::CombatWound,
            at_tick: 50,
        });

        app.update();

        let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
        let mut reader = vfx_events.get_reader();
        let vfx = reader
            .read(vfx_events)
            .next()
            .expect("VFX event should exist");
        match &vfx.payload {
            VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
                assert_eq!(
                    event_id, SEVER_FLASH_PARTICLE_ID,
                    "VFX event_id should be {SEVER_FLASH_PARTICLE_ID}"
                );
            }
            other => panic!("expected SpawnParticle, got {other:?}"),
        }
    }

    /// OverloadTear → Redis payload 字段验证
    #[test]
    fn overload_tear_payload_fields() {
        let (mut app, rx_outbound) = app_with_bridge();

        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().send_event(MeridianSeveredEvent {
            entity,
            meridian_id: MeridianId::Kidney,
            source: SeveredSource::OverloadTear,
            at_tick: 9999,
        });

        app.update();

        let outbound = rx_outbound.try_recv().expect("Redis msg expected");
        match outbound {
            RedisOutbound::MeridianSevered(evt) => {
                assert_eq!(evt.v, 1, "v must be 1");
                assert_eq!(evt.event_type, "meridian_severed");
                assert_eq!(evt.meridian_id, "Kidney");
                assert_eq!(evt.source, SeveredSource::OverloadTear);
                assert_eq!(evt.at_tick, 9999);
            }
            other => panic!("expected MeridianSevered, got {other:?}"),
        }
    }
}
