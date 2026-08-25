//! World/Formation C2S 请求分发。
//!
//! 顶层 ingress 负责 decode、版本、预算与 gate；本模块只把已经通过这些门禁的
//! typed 请求转换为 zhenfa 领域事件。这里不读取或修改 inventory/world，也不使用
//! 动态 registry、反射或字符串路由。

use bevy_ecs::system::SystemParam;
use valence::prelude::{bevy_ecs, Entity, Events, ResMut};

use crate::schema::client_request::ClientRequestV1;
use crate::zhenfa::trap_content::TrapTargetFace;
use crate::zhenfa::{
    ScatterBeadUseRequest, ZhenfaCarrierKind, ZhenfaDisarmMode, ZhenfaDisarmRequest, ZhenfaKind,
    ZhenfaPlaceRequest, ZhenfaTriggerRequest,
};

/// World/Formation ingress 所需的唯一事件写入面。
///
/// 每个 event resource 都是 optional，保持原 handler 在资源缺失时 fail-closed 的
/// 行为：记录 warning 并丢弃请求，不创建替代状态。
#[derive(SystemParam)]
pub(crate) struct WorldFormationRequestParams<'w> {
    pub zhenfa_place_tx: Option<ResMut<'w, Events<ZhenfaPlaceRequest>>>,
    pub zhenfa_trigger_tx: Option<ResMut<'w, Events<ZhenfaTriggerRequest>>>,
    pub zhenfa_disarm_tx: Option<ResMut<'w, Events<ZhenfaDisarmRequest>>>,
    pub qi_scatter_bead_use_tx: Option<ResMut<'w, Events<ScatterBeadUseRequest>>>,
}

/// 已通过 schema 解析的 World/Formation 请求。
///
/// 每个 schema variant 都对应唯一 Rust variant，保证路由在编译期闭合。
#[derive(Debug, PartialEq)]
pub(crate) enum WorldFormationRequest {
    ZhenfaPlace {
        x: i32,
        y: i32,
        z: i32,
        kind: ZhenfaKind,
        carrier: Option<ZhenfaCarrierKind>,
        qi_invest_ratio: f64,
        trigger: Option<String>,
        item_instance_id: Option<u64>,
        target_face: Option<TrapTargetFace>,
    },
    ZhenfaTrigger {
        instance_id: Option<u64>,
    },
    ZhenfaDisarm {
        x: i32,
        y: i32,
        z: i32,
        mode: ZhenfaDisarmMode,
    },
    QiScatterBeadUse {
        item_instance_id: u64,
        x: Option<i32>,
        y: Option<i32>,
        z: Option<i32>,
    },
}

/// 从总的 C2S schema enum 中取出 World/Formation 域；非本域请求原样交还顶层 handler。
pub(crate) fn try_into_world_formation_request(
    request: ClientRequestV1,
) -> Result<WorldFormationRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::ZhenfaPlace {
            x,
            y,
            z,
            kind,
            carrier,
            qi_invest_ratio,
            trigger,
            item_instance_id,
            target_face,
            ..
        } => Ok(WorldFormationRequest::ZhenfaPlace {
            x,
            y,
            z,
            kind,
            carrier,
            qi_invest_ratio,
            trigger,
            item_instance_id,
            target_face,
        }),
        ClientRequestV1::ZhenfaTrigger { instance_id, .. } => {
            Ok(WorldFormationRequest::ZhenfaTrigger { instance_id })
        }
        ClientRequestV1::ZhenfaDisarm { x, y, z, mode, .. } => {
            Ok(WorldFormationRequest::ZhenfaDisarm { x, y, z, mode })
        }
        ClientRequestV1::QiScatterBeadUse {
            item_instance_id,
            x,
            y,
            z,
            ..
        } => Ok(WorldFormationRequest::QiScatterBeadUse {
            item_instance_id,
            x,
            y,
            z,
        }),
        request => Err(request),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldFormationDispatchOutcome {
    Emitted,
    DroppedMissingEventResource,
    RejectedPartialCoordinates,
}

/// 分发一个 typed World/Formation 请求并生成原有 zhenfa 领域事件。
///
/// `tick` 由顶层 handler 从同一个 `CombatClock` 快照传入，保证拆分前后的
/// `requested_at_tick` 完全一致。此函数只写事件资源，不触碰 inventory/world。
pub(crate) fn dispatch_world_formation_request(
    request: WorldFormationRequest,
    player: Entity,
    tick: u64,
    params: &mut WorldFormationRequestParams<'_>,
) -> WorldFormationDispatchOutcome {
    match request {
        WorldFormationRequest::ZhenfaPlace {
            x,
            y,
            z,
            kind,
            carrier,
            qi_invest_ratio,
            trigger,
            item_instance_id,
            target_face,
        } => {
            let Some(place_tx) = params.zhenfa_place_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped zhenfa_place because ZhenfaPlaceRequest event resource is missing"
                );
                return WorldFormationDispatchOutcome::DroppedMissingEventResource;
            };
            place_tx.send(ZhenfaPlaceRequest {
                player,
                pos: [x, y, z],
                kind,
                carrier: carrier.unwrap_or_default(),
                qi_invest_ratio,
                trigger,
                item_instance_id,
                target_face,
                requested_at_tick: tick,
            });
            WorldFormationDispatchOutcome::Emitted
        }
        WorldFormationRequest::ZhenfaTrigger { instance_id } => {
            let Some(trigger_tx) = params.zhenfa_trigger_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped zhenfa_trigger because ZhenfaTriggerRequest event resource is missing"
                );
                return WorldFormationDispatchOutcome::DroppedMissingEventResource;
            };
            trigger_tx.send(ZhenfaTriggerRequest {
                player,
                instance_id,
                requested_at_tick: tick,
            });
            WorldFormationDispatchOutcome::Emitted
        }
        WorldFormationRequest::ZhenfaDisarm { x, y, z, mode } => {
            let Some(disarm_tx) = params.zhenfa_disarm_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped zhenfa_disarm because ZhenfaDisarmRequest event resource is missing"
                );
                return WorldFormationDispatchOutcome::DroppedMissingEventResource;
            };
            disarm_tx.send(ZhenfaDisarmRequest {
                player,
                pos: [x, y, z],
                mode,
                requested_at_tick: tick,
            });
            WorldFormationDispatchOutcome::Emitted
        }
        WorldFormationRequest::QiScatterBeadUse {
            item_instance_id,
            x,
            y,
            z,
        } => {
            let Some(use_tx) = params.qi_scatter_bead_use_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped qi_scatter_bead_use because ScatterBeadUseRequest event resource is missing"
                );
                return WorldFormationDispatchOutcome::DroppedMissingEventResource;
            };
            let bury_pos = match (x, y, z) {
                (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                (None, None, None) => None,
                _ => {
                    tracing::warn!(
                        "[bong][network] dropped malformed qi_scatter_bead_use: x/y/z must be all present or all absent"
                    );
                    return WorldFormationDispatchOutcome::RejectedPartialCoordinates;
                }
            };
            use_tx.send(ScatterBeadUseRequest {
                player,
                item_instance_id,
                bury_pos,
                requested_at_tick: tick,
            });
            WorldFormationDispatchOutcome::Emitted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Component, Event, Events, Resource, Update};

    #[derive(Component, Debug, PartialEq, Eq)]
    struct WorldMarker(u8);

    #[derive(Resource, Default)]
    struct PendingWorldFormationRequest(Option<(Entity, u64, WorldFormationRequest)>);

    #[derive(Resource, Default)]
    struct LastDispatchOutcome(Option<WorldFormationDispatchOutcome>);

    fn dispatch_pending_world_formation_request(
        mut pending: ResMut<PendingWorldFormationRequest>,
        mut outcome: ResMut<LastDispatchOutcome>,
        mut params: WorldFormationRequestParams,
    ) {
        let Some((player, tick, request)) = pending.0.take() else {
            return;
        };
        outcome.0 = Some(dispatch_world_formation_request(
            request,
            player,
            tick,
            &mut params,
        ));
    }

    fn world_app() -> App {
        let mut app = App::new();
        app.insert_resource(PendingWorldFormationRequest::default());
        app.insert_resource(LastDispatchOutcome::default());
        app.add_systems(Update, dispatch_pending_world_formation_request);
        app
    }

    fn send_request(
        app: &mut App,
        player: Entity,
        tick: u64,
        request: WorldFormationRequest,
    ) -> WorldFormationDispatchOutcome {
        app.world_mut()
            .resource_mut::<PendingWorldFormationRequest>()
            .0 = Some((player, tick, request));
        app.update();
        app.world()
            .resource::<LastDispatchOutcome>()
            .0
            .expect("world dispatcher must report an outcome for every typed request")
    }

    fn drained_events<T: Event>(app: &mut App) -> Vec<T> {
        app.world_mut()
            .resource_mut::<Events<T>>()
            .drain()
            .collect()
    }

    #[test]
    fn typed_conversion_preserves_all_world_request_fields() {
        let place = try_into_world_formation_request(ClientRequestV1::ZhenfaPlace {
            v: 1,
            x: -3,
            y: 64,
            z: 8,
            kind: ZhenfaKind::NetworkArray,
            carrier: Some(ZhenfaCarrierKind::LingqiBlock),
            qi_invest_ratio: 0.875,
            trigger: Some("warning".to_owned()),
            item_instance_id: Some(7001),
            target_face: Some(TrapTargetFace::North),
        })
        .expect("zhenfa place must enter the typed world domain");
        assert_eq!(
            place,
            WorldFormationRequest::ZhenfaPlace {
                x: -3,
                y: 64,
                z: 8,
                kind: ZhenfaKind::NetworkArray,
                carrier: Some(ZhenfaCarrierKind::LingqiBlock),
                qi_invest_ratio: 0.875,
                trigger: Some("warning".to_owned()),
                item_instance_id: Some(7001),
                target_face: Some(TrapTargetFace::North),
            }
        );

        assert_eq!(
            try_into_world_formation_request(ClientRequestV1::ZhenfaTrigger {
                v: 1,
                instance_id: Some(42),
            })
            .expect("zhenfa trigger must enter the typed world domain"),
            WorldFormationRequest::ZhenfaTrigger {
                instance_id: Some(42)
            }
        );
        assert_eq!(
            try_into_world_formation_request(ClientRequestV1::ZhenfaDisarm {
                v: 1,
                x: 1,
                y: 65,
                z: -7,
                mode: ZhenfaDisarmMode::ForceBreak,
            })
            .expect("zhenfa disarm must enter the typed world domain"),
            WorldFormationRequest::ZhenfaDisarm {
                x: 1,
                y: 65,
                z: -7,
                mode: ZhenfaDisarmMode::ForceBreak,
            }
        );
        assert_eq!(
            try_into_world_formation_request(ClientRequestV1::QiScatterBeadUse {
                v: 1,
                item_instance_id: 9001,
                x: Some(4),
                y: Some(64),
                z: Some(5),
            })
            .expect("scatter bead use must enter the typed world domain"),
            WorldFormationRequest::QiScatterBeadUse {
                item_instance_id: 9001,
                x: Some(4),
                y: Some(64),
                z: Some(5),
            }
        );

        let non_world =
            try_into_world_formation_request(ClientRequestV1::BreakthroughRequest { v: 1 });
        assert!(
            matches!(
                non_world,
                Err(ClientRequestV1::BreakthroughRequest { v: 1 })
            ),
            "non-world requests must remain available to the parent handler"
        );
    }

    #[test]
    fn four_success_routes_preserve_event_fields_and_tick() {
        let mut app = world_app();
        app.add_event::<ZhenfaPlaceRequest>();
        app.add_event::<ZhenfaTriggerRequest>();
        app.add_event::<ZhenfaDisarmRequest>();
        app.add_event::<ScatterBeadUseRequest>();
        let player = app.world_mut().spawn(WorldMarker(7)).id();

        assert_eq!(
            send_request(
                &mut app,
                player,
                101,
                WorldFormationRequest::ZhenfaPlace {
                    x: -3,
                    y: 64,
                    z: 8,
                    kind: ZhenfaKind::NetworkArray,
                    carrier: None,
                    qi_invest_ratio: 0.625,
                    trigger: Some("warning".to_owned()),
                    item_instance_id: Some(7002),
                    target_face: Some(TrapTargetFace::East),
                },
            ),
            WorldFormationDispatchOutcome::Emitted
        );
        let place = drained_events::<ZhenfaPlaceRequest>(&mut app);
        assert_eq!(place.len(), 1, "one place request must emit one event");
        assert_eq!(place[0].player, player);
        assert_eq!(place[0].pos, [-3, 64, 8]);
        assert_eq!(place[0].kind, ZhenfaKind::NetworkArray);
        assert_eq!(
            place[0].carrier,
            ZhenfaCarrierKind::default(),
            "missing carrier must preserve carrier.unwrap_or_default()"
        );
        assert_eq!(place[0].qi_invest_ratio, 0.625);
        assert_eq!(place[0].trigger.as_deref(), Some("warning"));
        assert_eq!(place[0].item_instance_id, Some(7002));
        assert_eq!(place[0].target_face, Some(TrapTargetFace::East));
        assert_eq!(place[0].requested_at_tick, 101);

        assert_eq!(
            send_request(
                &mut app,
                player,
                102,
                WorldFormationRequest::ZhenfaTrigger { instance_id: None },
            ),
            WorldFormationDispatchOutcome::Emitted
        );
        let trigger = drained_events::<ZhenfaTriggerRequest>(&mut app);
        assert_eq!(trigger.len(), 1, "one trigger request must emit one event");
        assert_eq!(trigger[0].player, player);
        assert_eq!(trigger[0].instance_id, None);
        assert_eq!(trigger[0].requested_at_tick, 102);

        assert_eq!(
            send_request(
                &mut app,
                player,
                103,
                WorldFormationRequest::ZhenfaDisarm {
                    x: 1,
                    y: 65,
                    z: -7,
                    mode: ZhenfaDisarmMode::ForceBreak,
                },
            ),
            WorldFormationDispatchOutcome::Emitted
        );
        let disarm = drained_events::<ZhenfaDisarmRequest>(&mut app);
        assert_eq!(disarm.len(), 1, "one disarm request must emit one event");
        assert_eq!(disarm[0].player, player);
        assert_eq!(disarm[0].pos, [1, 65, -7]);
        assert_eq!(disarm[0].mode, ZhenfaDisarmMode::ForceBreak);
        assert_eq!(disarm[0].requested_at_tick, 103);

        for (tick, x, y, z, expected_pos) in [
            (104, Some(4), Some(64), Some(5), Some([4, 64, 5])),
            (105, None, None, None, None),
        ] {
            assert_eq!(
                send_request(
                    &mut app,
                    player,
                    tick,
                    WorldFormationRequest::QiScatterBeadUse {
                        item_instance_id: 9001,
                        x,
                        y,
                        z,
                    },
                ),
                WorldFormationDispatchOutcome::Emitted
            );
            let use_events = drained_events::<ScatterBeadUseRequest>(&mut app);
            assert_eq!(
                use_events.len(),
                1,
                "each valid scatter bead use emits one event"
            );
            assert_eq!(use_events[0].player, player);
            assert_eq!(use_events[0].item_instance_id, 9001);
            assert_eq!(use_events[0].bury_pos, expected_pos);
            assert_eq!(use_events[0].requested_at_tick, tick);
        }

        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(7)),
            "world formation dispatch must not mutate world state"
        );
    }

    #[test]
    fn missing_event_resource_is_fail_closed_for_all_four_routes() {
        let mut app = world_app();
        let player = app.world_mut().spawn(WorldMarker(5)).id();
        let requests = [
            WorldFormationRequest::ZhenfaPlace {
                x: 0,
                y: 64,
                z: 0,
                kind: ZhenfaKind::Trap,
                carrier: Some(ZhenfaCarrierKind::BeastCoreInlaid),
                qi_invest_ratio: 0.2,
                trigger: None,
                item_instance_id: None,
                target_face: None,
            },
            WorldFormationRequest::ZhenfaTrigger {
                instance_id: Some(11),
            },
            WorldFormationRequest::ZhenfaDisarm {
                x: 0,
                y: 64,
                z: 0,
                mode: ZhenfaDisarmMode::Disarm,
            },
            WorldFormationRequest::QiScatterBeadUse {
                item_instance_id: 12,
                x: Some(1),
                y: Some(64),
                z: Some(1),
            },
        ];

        for request in requests {
            assert_eq!(
                send_request(&mut app, player, 200, request),
                WorldFormationDispatchOutcome::DroppedMissingEventResource,
                "missing event resource must drop each world formation request"
            );
        }
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(5)),
            "missing event resources must not mutate world state"
        );
    }

    #[test]
    fn partial_scatter_coordinates_are_rejected_without_event() {
        let mut app = world_app();
        app.add_event::<ScatterBeadUseRequest>();
        let player = app.world_mut().spawn(WorldMarker(3)).id();
        let partial_coordinates = [
            (Some(1), Some(64), None),
            (Some(1), None, Some(2)),
            (None, Some(64), Some(2)),
            (Some(1), None, None),
            (None, Some(64), None),
            (None, None, Some(2)),
        ];

        for (x, y, z) in partial_coordinates {
            assert_eq!(
                send_request(
                    &mut app,
                    player,
                    300,
                    WorldFormationRequest::QiScatterBeadUse {
                        item_instance_id: 99,
                        x,
                        y,
                        z,
                    },
                ),
                WorldFormationDispatchOutcome::RejectedPartialCoordinates,
                "partial x/y/z coordinates must be rejected"
            );
            assert!(
                drained_events::<ScatterBeadUseRequest>(&mut app).is_empty(),
                "partial x/y/z coordinates must not emit ScatterBeadUseRequest"
            );
        }
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(3)),
            "partial-coordinate rejection must not mutate world state"
        );
    }
}
