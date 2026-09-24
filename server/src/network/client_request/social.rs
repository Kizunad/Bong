//! 玩家之间的 Social C2S 请求分发。
//!
//! 这里故意只把请求转换为 `social::events` 事件：目标解析是只读的，
//! 不触碰 inventory、world 或任何交易业务谓词。

use valence::prelude::{bevy_ecs, Entity, EntityManager, Events, ResMut};

use bevy_ecs::system::SystemParam;

use crate::schema::client_request::ClientRequestV1;
use crate::social::events::{
    SparringInviteResponseEvent, SparringInviteResponseKind, TradeOfferRequest,
    TradeOfferResponseEvent,
};

/// Social ingress 所需的唯一事件写入面。
///
/// `Option<ResMut<_>>` 保留了原 handler 在 event resource 缺失时的 fail-closed 行为：
/// 记录 warning 并丢弃请求，不创建任何替代状态。
#[derive(SystemParam)]
pub(crate) struct SocialRequestParams<'w> {
    pub sparring_invite_response_tx: Option<ResMut<'w, Events<SparringInviteResponseEvent>>>,
    pub trade_offer_request_tx: Option<ResMut<'w, Events<TradeOfferRequest>>>,
    pub trade_offer_response_tx: Option<ResMut<'w, Events<TradeOfferResponseEvent>>>,
}

/// 已经通过 schema 解析的 Social 请求。
///
/// 这个 enum 是编译期路由面；请求类型由 Rust variant 决定，不使用字符串 handler
/// registry、反射或动态路由。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SocialRequest {
    SparringInviteResponse {
        invite_id: String,
        accepted: bool,
        timed_out: bool,
    },
    TradeOfferRequest {
        target: String,
        offered_instance_id: u64,
    },
    TradeOfferResponse {
        offer_id: String,
        accepted: bool,
        requested_instance_id: Option<u64>,
    },
}

/// 从总的 C2S schema enum 中取出 Social 域；非 Social 请求原样交还顶层 handler。
pub(crate) fn try_into_social_request(
    request: ClientRequestV1,
) -> Result<SocialRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::SparringInviteResponse {
            invite_id,
            accepted,
            timed_out,
            ..
        } => Ok(SocialRequest::SparringInviteResponse {
            invite_id,
            accepted,
            timed_out,
        }),
        ClientRequestV1::TradeOfferRequest {
            target,
            offered_instance_id,
            ..
        } => Ok(SocialRequest::TradeOfferRequest {
            target,
            offered_instance_id,
        }),
        ClientRequestV1::TradeOfferResponse {
            offer_id,
            accepted,
            requested_instance_id,
            ..
        } => Ok(SocialRequest::TradeOfferResponse {
            offer_id,
            accepted,
            requested_instance_id,
        }),
        request => Err(request),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SocialDispatchOutcome {
    Emitted,
    DroppedMissingEventResource,
    RejectedInvalidTarget,
}

/// 分发一个 typed Social 请求并生成原有事件 payload。
///
/// `tick` 由顶层 handler 从同一个 `CombatClock` 快照传入，保证拆分前后的 tick
/// 语义一致；本函数不读取或写入 inventory/world。
pub(crate) fn dispatch_social_request(
    request: SocialRequest,
    player: Entity,
    tick: u64,
    params: &mut SocialRequestParams<'_>,
    entity_manager: Option<&EntityManager>,
) -> SocialDispatchOutcome {
    match request {
        SocialRequest::SparringInviteResponse {
            invite_id,
            accepted,
            timed_out,
        } => {
            let Some(response_tx) = params.sparring_invite_response_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped sparring_invite_response because SparringInviteResponseEvent resource is missing"
                );
                return SocialDispatchOutcome::DroppedMissingEventResource;
            };
            let kind = if timed_out {
                SparringInviteResponseKind::Timeout
            } else if accepted {
                SparringInviteResponseKind::Accept
            } else {
                SparringInviteResponseKind::Decline
            };
            response_tx.send(SparringInviteResponseEvent {
                player,
                invite_id,
                kind,
                tick,
            });
            SocialDispatchOutcome::Emitted
        }
        SocialRequest::TradeOfferRequest {
            target,
            offered_instance_id,
        } => {
            let Some(request_tx) = params.trade_offer_request_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped trade_offer_request because TradeOfferRequest event resource is missing"
                );
                return SocialDispatchOutcome::DroppedMissingEventResource;
            };
            let Some(target_entity) = resolve_trade_offer_target(target.as_str(), entity_manager)
            else {
                tracing::warn!(
                    "[bong][network] rejected trade_offer_request from {:?}: invalid target `{target}`",
                    player
                );
                return SocialDispatchOutcome::RejectedInvalidTarget;
            };
            request_tx.send(TradeOfferRequest {
                initiator: player,
                target: target_entity,
                offered_instance_id,
                tick,
            });
            SocialDispatchOutcome::Emitted
        }
        SocialRequest::TradeOfferResponse {
            offer_id,
            accepted,
            requested_instance_id,
        } => {
            let Some(response_tx) = params.trade_offer_response_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped trade_offer_response because TradeOfferResponseEvent resource is missing"
                );
                return SocialDispatchOutcome::DroppedMissingEventResource;
            };
            response_tx.send(TradeOfferResponseEvent {
                player,
                offer_id,
                accepted,
                requested_instance_id,
                tick,
            });
            SocialDispatchOutcome::Emitted
        }
    }
}

/// 将客户端回传的 MC protocol entity id 解析为 ECS entity。
///
/// `entity_bits:` 一直是被明确拒绝的输入，避免客户端伪造本地 ECS bits；只有
/// `entity:<protocol-id>` 走 Valence 的权威 `EntityManager` 映射。
pub(crate) fn resolve_trade_offer_target(
    raw: &str,
    entity_manager: Option<&EntityManager>,
) -> Option<Entity> {
    let raw = raw.trim();
    if raw.is_empty() || raw.starts_with("entity_bits:") {
        return None;
    }
    let protocol_id = raw.strip_prefix("entity:")?.parse::<i32>().ok()?;
    entity_manager?.get_by_id(protocol_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::entity::{EntityId, EntityPlugin};
    use valence::prelude::{
        App, Component, DVec3, EntityKind, Event, OldPosition, Position, Res, Resource, Update,
    };

    #[derive(Resource, Default)]
    struct PendingSocialRequest(Option<(Entity, u64, SocialRequest)>);

    #[derive(Resource, Default)]
    struct LastDispatchOutcome(Option<SocialDispatchOutcome>);

    #[derive(Component, Debug, PartialEq, Eq)]
    struct WorldMarker(u8);

    fn dispatch_pending_social_request(
        mut pending: ResMut<PendingSocialRequest>,
        mut outcome: ResMut<LastDispatchOutcome>,
        mut params: SocialRequestParams,
        entity_manager: Option<Res<EntityManager>>,
    ) {
        let Some((player, tick, request)) = pending.0.take() else {
            return;
        };
        outcome.0 = Some(dispatch_social_request(
            request,
            player,
            tick,
            &mut params,
            entity_manager.as_deref(),
        ));
    }

    fn social_app(with_entity_plugin: bool) -> App {
        let mut app = App::new();
        if with_entity_plugin {
            app.add_plugins(EntityPlugin);
        }
        app.insert_resource(PendingSocialRequest::default());
        app.insert_resource(LastDispatchOutcome::default());
        app.add_systems(Update, dispatch_pending_social_request);
        app
    }

    fn send_request(
        app: &mut App,
        player: Entity,
        tick: u64,
        request: SocialRequest,
    ) -> SocialDispatchOutcome {
        app.world_mut().resource_mut::<PendingSocialRequest>().0 = Some((player, tick, request));
        app.update();
        app.world()
            .resource::<LastDispatchOutcome>()
            .0
            .expect("social dispatcher must report an outcome for every typed request")
    }

    fn drained_events<T: Event>(app: &mut App) -> Vec<T> {
        app.world_mut()
            .resource_mut::<Events<T>>()
            .drain()
            .collect()
    }

    #[test]
    fn typed_conversion_preserves_social_request_fields() {
        let sparring = try_into_social_request(ClientRequestV1::SparringInviteResponse {
            v: 1,
            invite_id: "sparring:1".to_owned(),
            accepted: false,
            timed_out: true,
        })
        .expect("sparring request must enter the Social typed domain");
        assert_eq!(
            sparring,
            SocialRequest::SparringInviteResponse {
                invite_id: "sparring:1".to_owned(),
                accepted: false,
                timed_out: true,
            }
        );

        let trade_request = try_into_social_request(ClientRequestV1::TradeOfferRequest {
            v: 1,
            target: "entity:42".to_owned(),
            offered_instance_id: 1001,
        })
        .expect("trade request must enter the Social typed domain");
        assert_eq!(
            trade_request,
            SocialRequest::TradeOfferRequest {
                target: "entity:42".to_owned(),
                offered_instance_id: 1001,
            }
        );

        let trade_response = try_into_social_request(ClientRequestV1::TradeOfferResponse {
            v: 1,
            offer_id: "trade:expired".to_owned(),
            accepted: false,
            requested_instance_id: None,
        })
        .expect("trade response must enter the Social typed domain");
        assert_eq!(
            trade_response,
            SocialRequest::TradeOfferResponse {
                offer_id: "trade:expired".to_owned(),
                accepted: false,
                requested_instance_id: None,
            }
        );
    }

    #[test]
    fn sparring_accept_decline_timeout_preserve_event_payload_and_tick() {
        let mut app = social_app(false);
        app.add_event::<SparringInviteResponseEvent>();
        let player = app.world_mut().spawn(WorldMarker(7)).id();

        for (accepted, timed_out, expected_kind) in [
            (true, false, SparringInviteResponseKind::Accept),
            (false, false, SparringInviteResponseKind::Decline),
            (false, true, SparringInviteResponseKind::Timeout),
        ] {
            let invite_id = format!("sparring:{accepted}:{timed_out}");
            let tick = 41 + u64::from(timed_out);
            let outcome = send_request(
                &mut app,
                player,
                tick,
                SocialRequest::SparringInviteResponse {
                    invite_id: invite_id.clone(),
                    accepted,
                    timed_out,
                },
            );
            assert_eq!(
                outcome,
                SocialDispatchOutcome::Emitted,
                "sparring {accepted}/{timed_out} should emit its typed event"
            );
            let events = drained_events::<SparringInviteResponseEvent>(&mut app);
            assert_eq!(
                events.len(),
                1,
                "each sparring response emits exactly one event"
            );
            assert_eq!(
                events[0].player, player,
                "event player must preserve the requester"
            );
            assert_eq!(
                events[0].invite_id, invite_id,
                "invite_id must be forwarded without normalization"
            );
            assert_eq!(
                events[0].kind, expected_kind,
                "accepted/timed_out mapping must preserve the existing event semantics"
            );
            assert_eq!(
                events[0].tick, tick,
                "CombatClock tick must be forwarded unchanged"
            );
        }
    }

    #[test]
    fn trade_request_resolves_protocol_target_and_preserves_payload() {
        let mut app = social_app(true);
        app.add_event::<TradeOfferRequest>();
        let player = app.world_mut().spawn(WorldMarker(9)).id();
        let target = app
            .world_mut()
            .spawn((
                EntityId::default(),
                EntityKind::new(1),
                Position::new(DVec3::ZERO),
                OldPosition::new(DVec3::ZERO),
            ))
            .id();
        app.update();
        let target_protocol_id = app
            .world()
            .get::<EntityId>(target)
            .expect("EntityPlugin must assign the trade target protocol id")
            .get();

        let outcome = send_request(
            &mut app,
            player,
            73,
            SocialRequest::TradeOfferRequest {
                target: format!(" entity:{target_protocol_id} "),
                offered_instance_id: 9001,
            },
        );
        assert_eq!(outcome, SocialDispatchOutcome::Emitted);
        let events = drained_events::<TradeOfferRequest>(&mut app);
        assert_eq!(
            events.len(),
            1,
            "a valid trade target emits exactly one event"
        );
        assert_eq!(
            events[0].initiator, player,
            "initiator must preserve the requester"
        );
        assert_eq!(
            events[0].target, target,
            "protocol target must resolve via EntityManager"
        );
        assert_eq!(
            events[0].offered_instance_id, 9001,
            "offered instance id must be forwarded unchanged"
        );
        assert_eq!(
            events[0].tick, 73,
            "trade request tick must be forwarded unchanged"
        );
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(9)),
            "social dispatch must not mutate the requester world component"
        );
    }

    #[test]
    fn invalid_trade_targets_are_rejected_without_event() {
        let mut app = social_app(true);
        app.add_event::<TradeOfferRequest>();
        let player = app.world_mut().spawn(WorldMarker(3)).id();
        app.update();

        for target in ["", "entity_bits:1", "entity:not-a-number", "player:alice"] {
            let outcome = send_request(
                &mut app,
                player,
                80,
                SocialRequest::TradeOfferRequest {
                    target: target.to_owned(),
                    offered_instance_id: 1,
                },
            );
            assert_eq!(
                outcome,
                SocialDispatchOutcome::RejectedInvalidTarget,
                "invalid target `{target}` must be rejected before event emission"
            );
            assert!(
                drained_events::<TradeOfferRequest>(&mut app).is_empty(),
                "invalid target `{target}` must not create TradeOfferRequest"
            );
        }
    }

    #[test]
    fn missing_event_resource_is_fail_closed() {
        let mut app = social_app(false);
        let player = app.world_mut().spawn(WorldMarker(5)).id();
        let outcome = send_request(
            &mut app,
            player,
            88,
            SocialRequest::SparringInviteResponse {
                invite_id: "sparring:missing-resource".to_owned(),
                accepted: true,
                timed_out: false,
            },
        );
        assert_eq!(
            outcome,
            SocialDispatchOutcome::DroppedMissingEventResource,
            "missing event resource must drop the request instead of fabricating state"
        );
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(5)),
            "missing-resource handling must not mutate world state"
        );
    }

    #[test]
    fn trade_request_missing_event_resource_is_fail_closed() {
        let mut app = social_app(true);
        let player = app.world_mut().spawn(WorldMarker(6)).id();
        let target = app
            .world_mut()
            .spawn((
                EntityId::default(),
                EntityKind::new(1),
                Position::new(DVec3::ZERO),
                OldPosition::new(DVec3::ZERO),
            ))
            .id();
        app.update();
        let target_protocol_id = app
            .world()
            .get::<EntityId>(target)
            .expect("EntityPlugin must assign the trade target protocol id")
            .get();

        let outcome = send_request(
            &mut app,
            player,
            89,
            SocialRequest::TradeOfferRequest {
                target: format!("entity:{target_protocol_id}"),
                offered_instance_id: 9002,
            },
        );
        assert_eq!(
            outcome,
            SocialDispatchOutcome::DroppedMissingEventResource,
            "trade request must be dropped when its event resource is absent"
        );
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(6)),
            "missing trade event resource must not mutate the requester world state"
        );
    }

    #[test]
    fn trade_response_accept_decline_and_expired_id_preserve_all_fields() {
        let mut app = social_app(false);
        app.add_event::<TradeOfferResponseEvent>();
        let player = app.world_mut().spawn(WorldMarker(1)).id();

        for (offer_id, accepted, requested_instance_id) in [
            ("trade:accepted", true, Some(2002)),
            ("trade:declined", false, None),
            ("trade:expired", false, None),
        ] {
            let outcome = send_request(
                &mut app,
                player,
                91,
                SocialRequest::TradeOfferResponse {
                    offer_id: offer_id.to_owned(),
                    accepted,
                    requested_instance_id,
                },
            );
            assert_eq!(outcome, SocialDispatchOutcome::Emitted);
            let events = drained_events::<TradeOfferResponseEvent>(&mut app);
            assert_eq!(
                events.len(),
                1,
                "each trade response emits exactly one event"
            );
            assert_eq!(
                events[0].player, player,
                "response player must preserve the requester"
            );
            assert_eq!(
                events[0].offer_id, offer_id,
                "accepted, declined, and expired offer ids must remain opaque and complete"
            );
            assert_eq!(
                events[0].accepted, accepted,
                "accepted flag must be forwarded unchanged"
            );
            assert_eq!(
                events[0].requested_instance_id, requested_instance_id,
                "requested instance option must be forwarded unchanged"
            );
            assert_eq!(
                events[0].tick, 91,
                "trade response tick must be forwarded unchanged"
            );
        }
    }
}
