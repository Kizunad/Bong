//! plan-tsy-zone-followup-v1 §2 — Bevy `TsyEnterEmit` / `TsyExitEmit` event →
//! Redis `bong:tsy_event` 频道 JSON publish。
//!
//! 流程：
//! 1. `tsy_portal` system 写 Bevy event（已 zone-v1 落地）
//! 2. 本桥 system 读 event + 解析 player 的 `Username` → wire schema struct
//! 3. 通过 `RedisBridgeResource.tx_outbound` 推 `RedisOutbound::TsyEnter / TsyExit`
//! 4. `redis_bridge::prepare_outbound_command` 序列化 + 发到 `CH_TSY_EVENT`

use valence::prelude::{EventReader, Query, Res, Username, With};

use super::redis_bridge::RedisOutbound;
use super::RedisBridgeResource;
use crate::npc::tsy_hostile::{TsyHostileArchetype, TsyNpcSpawned, TsySentinelPhaseChanged};
use crate::player::state::canonical_player_id;
use crate::schema::tsy::{
    TsyDimensionAnchorV1, TsyEnterEventV1, TsyExitEventV1, TsyFilteredItemV1,
};
use crate::schema::tsy_hostile::{
    TsyHostileArchetypeV1, TsyNpcSpawnedV1, TsySentinelPhaseChangedV1,
};
use crate::world::tsy_portal::{TsyEnterEmit, TsyExitEmit};

const TSY_EVENT_VERSION: u8 = 1;

/// 系统：把 `TsyEnterEmit` 转成 wire schema 推到 Redis outbound。
///
/// 玩家 `player_id` 解析：从 `Username` component 通过 `canonical_player_id`
/// 走（含 `offline:` 前缀）；无 username → 回退到 `entity:{:?}` 调试形态。
pub fn publish_tsy_enter_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TsyEnterEmit>,
    clients: Query<&Username, With<valence::prelude::Client>>,
) {
    for ev in events.read() {
        let player_id = clients
            .get(ev.player_entity)
            .map(|u| canonical_player_id(u.0.as_str()))
            .unwrap_or_else(|_| format!("entity:{:?}", ev.player_entity));

        let wire = TsyEnterEventV1 {
            v: TSY_EVENT_VERSION,
            kind: "tsy_enter".to_string(),
            // P0 不带 server-tick resource 直接可读；用 0 占位（agent 可对齐自己的时钟）
            // 真实 tick 跟随 future commit 把 CombatClock 透传到这里。
            tick: 0,
            player_id,
            family_id: ev.family_id.clone(),
            return_to: TsyDimensionAnchorV1 {
                dimension: ev.return_to.dimension.ident_str().to_string(),
                pos: [ev.return_to.pos.x, ev.return_to.pos.y, ev.return_to.pos.z],
            },
            filtered_items: ev
                .filtered
                .iter()
                .map(|f| TsyFilteredItemV1 {
                    instance_id: f.instance_id,
                    template_id: f.template_id.clone(),
                    reason: "spirit_quality_too_high".to_string(),
                })
                .collect(),
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::TsyEnter(wire)) {
            tracing::warn!("[bong][tsy_event_bridge] dropped TsyEnter: {error}");
        }
    }
}

/// 系统：把 `TsyExitEmit` 转成 wire schema 推到 Redis outbound。
pub fn publish_tsy_exit_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TsyExitEmit>,
    clients: Query<&Username, With<valence::prelude::Client>>,
) {
    for ev in events.read() {
        let player_id = clients
            .get(ev.player_entity)
            .map(|u| canonical_player_id(u.0.as_str()))
            .unwrap_or_else(|_| format!("entity:{:?}", ev.player_entity));

        let wire = TsyExitEventV1 {
            v: TSY_EVENT_VERSION,
            kind: "tsy_exit".to_string(),
            tick: 0, // see publish_tsy_enter_events: 占位至 future tick wiring
            player_id,
            family_id: ev.family_id.clone(),
            duration_ticks: ev.duration_ticks,
            // qi_drained_total — P0 占位 0；累计逻辑归 plan-tsy-loot-v1
            qi_drained_total: 0.0,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::TsyExit(wire)) {
            tracing::warn!("[bong][tsy_event_bridge] dropped TsyExit: {error}");
        }
    }
}

pub fn publish_tsy_npc_spawned_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TsyNpcSpawned>,
) {
    for ev in events.read() {
        let wire = TsyNpcSpawnedV1 {
            v: TSY_EVENT_VERSION,
            kind: "tsy_npc_spawned".to_string(),
            family_id: ev.family_id.clone(),
            archetype: archetype_to_wire(ev.archetype),
            count: ev.count,
            at_tick: ev.at_tick,
        };
        if let Err(error) = redis.tx_outbound.send(RedisOutbound::TsyNpcSpawned(wire)) {
            tracing::warn!("[bong][tsy_event_bridge] dropped TsyNpcSpawned: {error}");
        }
    }
}

pub fn publish_tsy_sentinel_phase_changed_events(
    redis: Res<RedisBridgeResource>,
    mut events: EventReader<TsySentinelPhaseChanged>,
) {
    for ev in events.read() {
        let wire = TsySentinelPhaseChangedV1 {
            v: TSY_EVENT_VERSION,
            kind: "tsy_sentinel_phase_changed".to_string(),
            family_id: ev.family_id.clone(),
            container_entity_id: ev.container_entity_id,
            phase: ev.phase,
            max_phase: ev.max_phase,
            at_tick: ev.at_tick,
        };
        if let Err(error) = redis
            .tx_outbound
            .send(RedisOutbound::TsySentinelPhaseChanged(wire))
        {
            tracing::warn!("[bong][tsy_event_bridge] dropped TsySentinelPhaseChanged: {error}");
        }
    }
}

fn archetype_to_wire(archetype: TsyHostileArchetype) -> TsyHostileArchetypeV1 {
    match archetype {
        TsyHostileArchetype::Daoxiang => TsyHostileArchetypeV1::Daoxiang,
        TsyHostileArchetype::Zhinian => TsyHostileArchetypeV1::Zhinian,
        TsyHostileArchetype::GuardianRelicSentinel => TsyHostileArchetypeV1::GuardianRelicSentinel,
        TsyHostileArchetype::Fuya => TsyHostileArchetypeV1::Fuya,
        TsyHostileArchetype::SkullFiend => TsyHostileArchetypeV1::SkullFiend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::redis_bridge::RedisOutbound;
    use crate::network::RedisBridgeResource;
    use crate::world::dimension::DimensionKind;
    use crate::world::tsy::DimensionAnchor;
    use crate::world::tsy_filter::FilteredItem;
    use crate::world::tsy_portal::{TsyEnterEmit, TsyExitEmit};
    use crossbeam_channel::unbounded;
    use valence::prelude::{App, DVec3, Entity, Update};
    use valence::testing::create_mock_client;

    fn setup_app() -> (App, crossbeam_channel::Receiver<RedisOutbound>) {
        let (tx_outbound, rx_outbound) = unbounded();
        let (_tx_inbound, rx_inbound) = unbounded();
        let mut app = App::new();
        app.insert_resource(RedisBridgeResource {
            tx_outbound,
            rx_inbound,
        });
        app.add_event::<TsyEnterEmit>();
        app.add_event::<TsyExitEmit>();
        app.add_event::<TsyNpcSpawned>();
        app.add_event::<TsySentinelPhaseChanged>();
        app.add_systems(
            Update,
            (
                publish_tsy_enter_events,
                publish_tsy_exit_events,
                publish_tsy_npc_spawned_events,
                publish_tsy_sentinel_phase_changed_events,
            ),
        );
        (app, rx_outbound)
    }

    #[test]
    fn publish_tsy_enter_event_emits_redis_outbound_with_correct_fields() {
        let (mut app, rx) = setup_app();
        let dummy = app.world_mut().spawn(()).id();
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: dummy,
            family_id: "tsy_lingxu_01".to_string(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(2.5, 65.0, 0.0),
            },
            filtered: vec![FilteredItem {
                instance_id: 7,
                template_id: "bone_coin".to_string(),
                before_name: "满灵骨币".to_string(),
                before_spirit_quality: 0.8,
            }],
        });
        app.update();

        let outbound = rx.try_recv().expect("expected one RedisOutbound");
        let RedisOutbound::TsyEnter(wire) = outbound else {
            panic!("expected RedisOutbound::TsyEnter, got {outbound:?}");
        };
        assert_eq!(wire.v, 1);
        assert_eq!(wire.kind, "tsy_enter");
        assert_eq!(wire.family_id, "tsy_lingxu_01");
        assert_eq!(wire.return_to.dimension, "minecraft:overworld");
        assert_eq!(wire.return_to.pos, [2.5, 65.0, 0.0]);
        assert_eq!(wire.filtered_items.len(), 1);
        assert_eq!(wire.filtered_items[0].instance_id, 7);
        assert_eq!(wire.filtered_items[0].template_id, "bone_coin");
        assert_eq!(wire.filtered_items[0].reason, "spirit_quality_too_high");
        // 没装 Username component → fallback "entity:..."
        assert!(wire.player_id.starts_with("entity:"));
    }

    #[test]
    fn publish_tsy_enter_event_resolves_username_to_canonical_player_id() {
        let (mut app, rx) = setup_app();
        // 装真 Client bundle 让 Query<&Username, With<Client>> 命中
        // canonical_player_id 走 "offline:Foo" 格式
        let (client_bundle, _helper) = create_mock_client("Kiz");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: entity,
            family_id: "tsy_lingxu_01".to_string(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 65.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();
        let outbound = rx.try_recv().expect("expected one outbound");
        let RedisOutbound::TsyEnter(wire) = outbound else {
            panic!("wrong outbound");
        };
        assert_eq!(wire.player_id, "offline:Kiz");
    }

    #[test]
    fn publish_tsy_exit_event_emits_correct_payload() {
        let (mut app, rx) = setup_app();
        let dummy = app.world_mut().spawn(()).id();
        app.world_mut().send_event(TsyExitEmit {
            player_entity: dummy,
            family_id: "tsy_lingxu_01".to_string(),
            duration_ticks: 12000,
        });
        app.update();
        let outbound = rx.try_recv().expect("expected one outbound");
        let RedisOutbound::TsyExit(wire) = outbound else {
            panic!("wrong outbound");
        };
        assert_eq!(wire.v, 1);
        assert_eq!(wire.kind, "tsy_exit");
        assert_eq!(wire.family_id, "tsy_lingxu_01");
        assert_eq!(wire.duration_ticks, 12000);
        assert_eq!(wire.qi_drained_total, 0.0);
    }

    #[test]
    fn publish_tsy_npc_spawned_event_emits_correct_payload() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(TsyNpcSpawned {
            family_id: "tsy_zongmen_yiji_01".to_string(),
            archetype: TsyHostileArchetype::GuardianRelicSentinel,
            count: 3,
            at_tick: 12000,
        });
        app.update();
        let outbound = rx.try_recv().expect("expected one outbound");
        let RedisOutbound::TsyNpcSpawned(wire) = outbound else {
            panic!("wrong outbound");
        };
        assert_eq!(wire.v, 1);
        assert_eq!(wire.kind, "tsy_npc_spawned");
        assert_eq!(wire.family_id, "tsy_zongmen_yiji_01");
        assert_eq!(wire.archetype, TsyHostileArchetypeV1::GuardianRelicSentinel);
        assert_eq!(wire.count, 3);
        assert_eq!(wire.at_tick, 12000);
    }

    #[test]
    fn publish_tsy_sentinel_phase_changed_event_emits_correct_payload() {
        let (mut app, rx) = setup_app();
        app.world_mut().send_event(TsySentinelPhaseChanged {
            family_id: "tsy_zongmen_yiji_01".to_string(),
            container_entity_id: 42,
            phase: 1,
            max_phase: 3,
            at_tick: 12345,
        });
        app.update();
        let outbound = rx.try_recv().expect("expected one outbound");
        let RedisOutbound::TsySentinelPhaseChanged(wire) = outbound else {
            panic!("wrong outbound");
        };
        assert_eq!(wire.v, 1);
        assert_eq!(wire.kind, "tsy_sentinel_phase_changed");
        assert_eq!(wire.family_id, "tsy_zongmen_yiji_01");
        assert_eq!(wire.container_entity_id, 42);
        assert_eq!(wire.phase, 1);
        assert_eq!(wire.max_phase, 3);
        assert_eq!(wire.at_tick, 12345);
    }

    #[test]
    fn payload_round_trips_through_redis_command_serialization() {
        // 端到端：发 emit → bridge 推 RedisOutbound → prepare_outbound_command 拿 publish channel + payload
        // → JSON 反序列化回 wire struct → 字段一致
        let (mut app, rx) = setup_app();
        let dummy = Entity::from_raw(42);
        app.world_mut().send_event(TsyEnterEmit {
            player_entity: dummy,
            family_id: "tsy_lingxu_01".to_string(),
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(2.5, 65.0, 0.0),
            },
            filtered: Vec::new(),
        });
        app.update();
        let outbound = rx.try_recv().expect("outbound present");

        // serialize via the same path the production redis loop uses
        let json = match outbound {
            RedisOutbound::TsyEnter(ref w) => serde_json::to_string(w).expect("serialize"),
            _ => panic!("expected TsyEnter"),
        };
        let parsed: TsyEnterEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.kind, "tsy_enter");
        assert_eq!(parsed.return_to.dimension, "minecraft:overworld");
    }
}
