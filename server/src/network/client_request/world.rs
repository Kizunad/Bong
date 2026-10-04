//! World C2S 请求分发。
//!
//! 顶层 ingress 负责 decode、版本、预算与 gate；本模块只把已经通过这些门禁的
//! typed 请求转换为世界、阵法和棺材领域事件。这里不读取或修改
//! inventory/world，也不使用动态 registry、反射或字符串路由。

use bevy_ecs::system::SystemParam;
use valence::prelude::{bevy_ecs, Entity, Events, ResMut};

use crate::coffin::{
    CoffinBreakRequest, CoffinEnterRequest, CoffinLeaveRequest, CoffinMenuReclaimRequest,
    CoffinPlaceRequest,
};
use crate::schema::client_request::ClientRequestV1;
use crate::world::block_place::BlockPlaceRequest;
use crate::world::spawn_tutorial::CoffinOpenRequest;
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
    pub coffin_open_tx: Option<ResMut<'w, Events<CoffinOpenRequest>>>,
    pub coffin_place_tx: Option<ResMut<'w, Events<CoffinPlaceRequest>>>,
    pub coffin_enter_tx: Option<ResMut<'w, Events<CoffinEnterRequest>>>,
    pub coffin_leave_tx: Option<ResMut<'w, Events<CoffinLeaveRequest>>>,
    pub coffin_break_tx: Option<ResMut<'w, Events<CoffinBreakRequest>>>,
    pub coffin_menu_reclaim_tx: Option<ResMut<'w, Events<CoffinMenuReclaimRequest>>>,
    pub block_place_tx: Option<ResMut<'w, Events<BlockPlaceRequest>>>,
    pub block_picker_give_tx:
        Option<ResMut<'w, Events<crate::cmd::dev::block_picker::BlockPickerGiveIntent>>>,
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

/// 已通过 schema 解析的世界交互请求。
///
/// 阵法请求和世界交互请求共用一个 ingress 参数面，但保留独立 typed enum，
/// 让每个域的 wire variant 都在编译期显式登记，避免顶层 handler 重新长回巨型 match。
#[derive(Debug, PartialEq)]
pub(crate) enum WorldInteractionRequest {
    CoffinOpen {
        pos: [i32; 3],
    },
    CoffinPlace {
        pos: [i32; 3],
        item_instance_id: u64,
    },
    BlockPlace {
        x: i32,
        y: i32,
        z: i32,
        item_instance_id: u64,
        target_face: TrapTargetFace,
    },
    BlockPickerGive {
        block_id: String,
        count: u32,
    },
    CoffinEnter {
        pos: [i32; 3],
    },
    CoffinLeave,
    CoffinBreak {
        pos: [i32; 3],
    },
    CoffinMenuReclaim {
        pos: [i32; 3],
    },
}

/// 从总的 C2S schema enum 提取世界交互域；非本域请求交还顶层 handler。
pub(crate) fn try_into_world_interaction_request(
    request: ClientRequestV1,
) -> Result<WorldInteractionRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::CoffinOpen { x, y, z, .. } => {
            Ok(WorldInteractionRequest::CoffinOpen { pos: [x, y, z] })
        }
        ClientRequestV1::CoffinPlace {
            x,
            y,
            z,
            item_instance_id,
            ..
        } => Ok(WorldInteractionRequest::CoffinPlace {
            pos: [x, y, z],
            item_instance_id,
        }),
        ClientRequestV1::BlockPlace {
            x,
            y,
            z,
            item_instance_id,
            target_face,
            ..
        } => Ok(WorldInteractionRequest::BlockPlace {
            x,
            y,
            z,
            item_instance_id,
            target_face,
        }),
        ClientRequestV1::BlockPickerGive {
            block_id, count, ..
        } => Ok(WorldInteractionRequest::BlockPickerGive { block_id, count }),
        ClientRequestV1::CoffinEnter { x, y, z, .. } => {
            Ok(WorldInteractionRequest::CoffinEnter { pos: [x, y, z] })
        }
        ClientRequestV1::CoffinLeave { .. } => Ok(WorldInteractionRequest::CoffinLeave),
        ClientRequestV1::CoffinBreak { x, y, z, .. } => {
            Ok(WorldInteractionRequest::CoffinBreak { pos: [x, y, z] })
        }
        ClientRequestV1::CoffinMenuReclaim { x, y, z, .. } => {
            Ok(WorldInteractionRequest::CoffinMenuReclaim { pos: [x, y, z] })
        }
        request => Err(request),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldInteractionDispatchOutcome {
    Emitted,
    DroppedMissingEventResource,
}

fn emit_world_event<T: bevy_ecs::event::Event>(
    tx: Option<&mut Events<T>>,
    event: T,
    request_name: &'static str,
) -> WorldInteractionDispatchOutcome {
    let Some(tx) = tx else {
        tracing::warn!(
            "[bong][network] dropped {request_name} because its event resource is missing"
        );
        return WorldInteractionDispatchOutcome::DroppedMissingEventResource;
    };
    tx.send(event);
    WorldInteractionDispatchOutcome::Emitted
}

fn dispatch_coffin_interaction(
    request: WorldInteractionRequest,
    player: Entity,
    tick: u64,
    params: &mut WorldFormationRequestParams<'_>,
) -> WorldInteractionDispatchOutcome {
    match request {
        WorldInteractionRequest::CoffinOpen { pos } => emit_world_event(
            params.coffin_open_tx.as_deref_mut(),
            CoffinOpenRequest { player, pos, tick },
            "coffin_open",
        ),
        WorldInteractionRequest::CoffinPlace {
            pos,
            item_instance_id,
        } => emit_world_event(
            params.coffin_place_tx.as_deref_mut(),
            CoffinPlaceRequest {
                player,
                pos: valence::prelude::BlockPos::new(pos[0], pos[1], pos[2]),
                item_instance_id,
                tick,
            },
            "coffin_place",
        ),
        WorldInteractionRequest::CoffinEnter { pos } => emit_world_event(
            params.coffin_enter_tx.as_deref_mut(),
            CoffinEnterRequest {
                player,
                pos: valence::prelude::BlockPos::new(pos[0], pos[1], pos[2]),
                tick,
            },
            "coffin_enter",
        ),
        WorldInteractionRequest::CoffinLeave => emit_world_event(
            params.coffin_leave_tx.as_deref_mut(),
            CoffinLeaveRequest { player },
            "coffin_leave",
        ),
        WorldInteractionRequest::CoffinBreak { pos } => emit_world_event(
            params.coffin_break_tx.as_deref_mut(),
            CoffinBreakRequest {
                player,
                pos: valence::prelude::BlockPos::new(pos[0], pos[1], pos[2]),
                tick,
            },
            "coffin_break",
        ),
        WorldInteractionRequest::CoffinMenuReclaim { pos } => emit_world_event(
            params.coffin_menu_reclaim_tx.as_deref_mut(),
            CoffinMenuReclaimRequest {
                player,
                pos: valence::prelude::BlockPos::new(pos[0], pos[1], pos[2]),
                tick,
            },
            "coffin_menu_reclaim",
        ),
        _ => unreachable!("coffin dispatcher received a non-coffin request"),
    }
}

/// 分发一个 typed 世界交互请求并复用既有领域事件。
///
/// 这里仅写 event resource，保持拆分前的 fail-closed 行为；棺材和方块
/// 的所有权、距离与业务校验仍由各自领域系统负责。
pub(crate) fn dispatch_world_interaction_request(
    request: WorldInteractionRequest,
    player: Entity,
    tick: u64,
    params: &mut WorldFormationRequestParams<'_>,
) -> WorldInteractionDispatchOutcome {
    match request {
        request @ (WorldInteractionRequest::CoffinOpen { .. }
        | WorldInteractionRequest::CoffinPlace { .. }
        | WorldInteractionRequest::CoffinEnter { .. }
        | WorldInteractionRequest::CoffinLeave
        | WorldInteractionRequest::CoffinBreak { .. }
        | WorldInteractionRequest::CoffinMenuReclaim { .. }) => {
            dispatch_coffin_interaction(request, player, tick, params)
        }
        WorldInteractionRequest::BlockPlace {
            x,
            y,
            z,
            item_instance_id,
            target_face,
        } => emit_world_event(
            params.block_place_tx.as_deref_mut(),
            BlockPlaceRequest {
                client: player,
                x,
                y,
                z,
                item_instance_id,
                target_face,
            },
            "block_place",
        ),
        WorldInteractionRequest::BlockPickerGive { block_id, count } => emit_world_event(
            params.block_picker_give_tx.as_deref_mut(),
            crate::cmd::dev::block_picker::BlockPickerGiveIntent {
                player,
                block_id,
                count,
            },
            "block_picker_give",
        ),
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

    #[derive(Resource, Default)]
    struct PendingWorldInteractionRequest(Option<(Entity, u64, WorldInteractionRequest)>);

    #[derive(Resource, Default)]
    struct LastInteractionDispatchOutcome(Option<WorldInteractionDispatchOutcome>);

    fn dispatch_pending_world_interaction_request(
        mut pending: ResMut<PendingWorldInteractionRequest>,
        mut outcome: ResMut<LastInteractionDispatchOutcome>,
        mut params: WorldFormationRequestParams,
    ) {
        let Some((player, tick, request)) = pending.0.take() else {
            return;
        };
        outcome.0 = Some(dispatch_world_interaction_request(
            request,
            player,
            tick,
            &mut params,
        ));
    }

    fn world_interaction_app() -> App {
        let mut app = App::new();
        app.insert_resource(PendingWorldInteractionRequest::default());
        app.insert_resource(LastInteractionDispatchOutcome::default());
        app.add_systems(Update, dispatch_pending_world_interaction_request);
        app
    }

    fn send_interaction_request(
        app: &mut App,
        player: Entity,
        tick: u64,
        request: WorldInteractionRequest,
    ) -> WorldInteractionDispatchOutcome {
        app.world_mut()
            .resource_mut::<PendingWorldInteractionRequest>()
            .0 = Some((player, tick, request));
        app.update();
        app.world_mut()
            .resource_mut::<LastInteractionDispatchOutcome>()
            .0
            .take()
            .expect("world interaction dispatcher must report every typed request")
    }

    #[test]
    fn typed_conversion_covers_all_world_interaction_variants() {
        let cases = [
            (
                ClientRequestV1::CoffinOpen {
                    v: 1,
                    x: -1,
                    y: 64,
                    z: 2,
                },
                WorldInteractionRequest::CoffinOpen { pos: [-1, 64, 2] },
            ),
            (
                ClientRequestV1::CoffinPlace {
                    v: 1,
                    x: 3,
                    y: 65,
                    z: -4,
                    item_instance_id: 11,
                },
                WorldInteractionRequest::CoffinPlace {
                    pos: [3, 65, -4],
                    item_instance_id: 11,
                },
            ),
            (
                ClientRequestV1::BlockPlace {
                    v: 1,
                    x: 5,
                    y: 66,
                    z: 7,
                    item_instance_id: 12,
                    target_face: TrapTargetFace::North,
                },
                WorldInteractionRequest::BlockPlace {
                    x: 5,
                    y: 66,
                    z: 7,
                    item_instance_id: 12,
                    target_face: TrapTargetFace::North,
                },
            ),
            (
                ClientRequestV1::BlockPickerGive {
                    v: 1,
                    block_id: "stone".to_owned(),
                    count: 4,
                },
                WorldInteractionRequest::BlockPickerGive {
                    block_id: "stone".to_owned(),
                    count: 4,
                },
            ),
            (
                ClientRequestV1::CoffinEnter {
                    v: 1,
                    x: 8,
                    y: 67,
                    z: 9,
                },
                WorldInteractionRequest::CoffinEnter { pos: [8, 67, 9] },
            ),
            (
                ClientRequestV1::CoffinLeave { v: 1 },
                WorldInteractionRequest::CoffinLeave,
            ),
            (
                ClientRequestV1::CoffinBreak {
                    v: 1,
                    x: 10,
                    y: 68,
                    z: 11,
                },
                WorldInteractionRequest::CoffinBreak { pos: [10, 68, 11] },
            ),
            (
                ClientRequestV1::CoffinMenuReclaim {
                    v: 1,
                    x: 12,
                    y: 69,
                    z: 13,
                },
                WorldInteractionRequest::CoffinMenuReclaim { pos: [12, 69, 13] },
            ),
        ];
        for (wire_request, expected) in cases {
            assert_eq!(
                try_into_world_interaction_request(wire_request).ok(),
                Some(expected),
                "each world interaction wire variant must enter its typed route"
            );
        }

        assert!(matches!(
            try_into_world_interaction_request(ClientRequestV1::BreakthroughRequest { v: 1 }),
            Err(ClientRequestV1::BreakthroughRequest { v: 1 })
        ));
    }

    #[test]
    fn interaction_dispatch_preserves_domain_event_payloads() {
        let mut app = world_interaction_app();
        app.add_event::<CoffinPlaceRequest>();
        app.add_event::<BlockPlaceRequest>();
        let player = app.world_mut().spawn(WorldMarker(9)).id();

        assert_eq!(
            send_interaction_request(
                &mut app,
                player,
                401,
                WorldInteractionRequest::CoffinPlace {
                    pos: [-3, 64, 5],
                    item_instance_id: 77,
                },
            ),
            WorldInteractionDispatchOutcome::Emitted
        );
        let coffin = app
            .world_mut()
            .resource_mut::<Events<CoffinPlaceRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(coffin.len(), 1);
        assert_eq!(coffin[0].player, player);
        assert_eq!(coffin[0].pos, valence::prelude::BlockPos::new(-3, 64, 5));
        assert_eq!(coffin[0].item_instance_id, 77);
        assert_eq!(coffin[0].tick, 401);

        assert_eq!(
            send_interaction_request(
                &mut app,
                player,
                402,
                WorldInteractionRequest::BlockPlace {
                    x: 6,
                    y: 65,
                    z: 7,
                    item_instance_id: 78,
                    target_face: TrapTargetFace::East,
                },
            ),
            WorldInteractionDispatchOutcome::Emitted
        );
        let block = app
            .world_mut()
            .resource_mut::<Events<BlockPlaceRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(block.len(), 1);
        assert_eq!(block[0].client, player);
        assert_eq!((block[0].x, block[0].y, block[0].z), (6, 65, 7));
        assert_eq!(block[0].target_face, TrapTargetFace::East);

        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(9)),
            "typed world dispatch must not mutate the player entity"
        );
    }

    #[test]
    fn missing_interaction_event_resource_is_fail_closed() {
        let mut app = world_interaction_app();
        let player = app.world_mut().spawn(WorldMarker(4)).id();
        assert_eq!(
            send_interaction_request(&mut app, player, 405, WorldInteractionRequest::CoffinLeave,),
            WorldInteractionDispatchOutcome::DroppedMissingEventResource
        );
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(4)),
            "missing world event resources must not mutate the player"
        );
    }
}
