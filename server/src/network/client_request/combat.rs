//! Combat 生命周期与盾牌输入的 C2S typed dispatch。
//!
//! 这个模块只负责把已经通过 schema/version 校验的请求转换为既有 Combat
//! intent event。它不读取或修改玩家 ECS 状态，也不实现死亡、复活或盾牌规则。

use bevy_ecs::system::SystemParam;
use valence::prelude::{bevy_ecs, Entity, Events, ResMut};

use crate::combat::events::{RevivalActionIntent, RevivalActionKind};
use crate::combat::shield_block::{LowerShieldIntent, RaiseShieldIntent};
use crate::schema::client_request::ClientRequestV1;

/// Combat ingress 唯一需要的事件写入面。
///
/// 三个资源都使用 `Option<ResMut<_>>`：请求入口在资源缺失时保持 fail-closed，
/// 不创建替代状态，也不触碰玩家 ECS。正常生产 wiring 仍由现有事件资源提供。
#[derive(SystemParam)]
pub(crate) struct CombatRequestParams<'w> {
    pub(crate) revival_tx: Option<ResMut<'w, Events<RevivalActionIntent>>>,
    pub(crate) raise_shield_tx: Option<ResMut<'w, Events<RaiseShieldIntent>>>,
    pub(crate) lower_shield_tx: Option<ResMut<'w, Events<LowerShieldIntent>>>,
}

/// 已经通过总 C2S schema 解析的 Combat 请求。
///
/// 这是编译期路由面；不使用字符串 handler registry、反射或动态注册。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CombatRequest {
    Reincarnate,
    Terminate,
    CreateNewCharacter,
    RaiseShield,
    LowerShield,
}

/// 从总的 C2S schema enum 中取出 Combat 域；非 Combat 请求原样交还顶层 handler。
pub(crate) fn try_into_combat_request(
    request: ClientRequestV1,
) -> Result<CombatRequest, ClientRequestV1> {
    match request {
        ClientRequestV1::CombatReincarnate { .. } => Ok(CombatRequest::Reincarnate),
        ClientRequestV1::CombatTerminate { .. } => Ok(CombatRequest::Terminate),
        ClientRequestV1::CombatCreateNewCharacter { .. } => Ok(CombatRequest::CreateNewCharacter),
        ClientRequestV1::RaiseShield { .. } => Ok(CombatRequest::RaiseShield),
        ClientRequestV1::LowerShield { .. } => Ok(CombatRequest::LowerShield),
        request => Err(request),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatDispatchOutcome {
    Emitted,
    DroppedMissingEventResource,
}

/// 分发一个 typed Combat 请求并生成原有事件 payload。
///
/// `tick` 从顶层 handler 的同一个 `CombatClock` 快照传入，保证拆分前后的
/// `RevivalActionIntent.issued_at_tick` 完全一致。函数只写事件资源，不改 ECS。
pub(crate) fn dispatch_combat_request(
    request: CombatRequest,
    player: Entity,
    tick: u64,
    params: &mut CombatRequestParams<'_>,
) -> CombatDispatchOutcome {
    match request {
        CombatRequest::Reincarnate
        | CombatRequest::Terminate
        | CombatRequest::CreateNewCharacter => {
            let Some(revival_tx) = params.revival_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped combat revival request because RevivalActionIntent event resource is missing"
                );
                return CombatDispatchOutcome::DroppedMissingEventResource;
            };
            let action = match request {
                CombatRequest::Reincarnate => RevivalActionKind::Reincarnate,
                CombatRequest::Terminate => RevivalActionKind::Terminate,
                CombatRequest::CreateNewCharacter => RevivalActionKind::CreateNewCharacter,
                CombatRequest::RaiseShield | CombatRequest::LowerShield => unreachable!(),
            };
            revival_tx.send(RevivalActionIntent {
                entity: player,
                action,
                issued_at_tick: tick,
            });
            CombatDispatchOutcome::Emitted
        }
        CombatRequest::RaiseShield => {
            let Some(raise_shield_tx) = params.raise_shield_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped raise_shield because RaiseShieldIntent event resource is missing"
                );
                return CombatDispatchOutcome::DroppedMissingEventResource;
            };
            raise_shield_tx.send(RaiseShieldIntent { player });
            CombatDispatchOutcome::Emitted
        }
        CombatRequest::LowerShield => {
            let Some(lower_shield_tx) = params.lower_shield_tx.as_deref_mut() else {
                tracing::warn!(
                    "[bong][network] dropped lower_shield because LowerShieldIntent event resource is missing"
                );
                return CombatDispatchOutcome::DroppedMissingEventResource;
            };
            lower_shield_tx.send(LowerShieldIntent { player });
            CombatDispatchOutcome::Emitted
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use valence::prelude::{App, Component, Event, Resource, Update};

    #[derive(Resource, Default)]
    struct PendingCombatRequest(Option<(Entity, u64, CombatRequest)>);

    #[derive(Resource, Default)]
    struct LastDispatchOutcome(Option<CombatDispatchOutcome>);

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct WorldMarker(u8);

    fn dispatch_pending_combat_request(
        mut pending: ResMut<PendingCombatRequest>,
        mut outcome: ResMut<LastDispatchOutcome>,
        mut params: CombatRequestParams,
    ) {
        let Some((player, tick, request)) = pending.0.take() else {
            return;
        };
        outcome.0 = Some(dispatch_combat_request(request, player, tick, &mut params));
    }

    fn combat_app(with_revival: bool, with_raise_shield: bool, with_lower_shield: bool) -> App {
        let mut app = App::new();
        if with_revival {
            app.add_event::<RevivalActionIntent>();
        }
        if with_raise_shield {
            app.add_event::<RaiseShieldIntent>();
        }
        if with_lower_shield {
            app.add_event::<LowerShieldIntent>();
        }
        app.insert_resource(PendingCombatRequest::default());
        app.insert_resource(LastDispatchOutcome::default());
        app.add_systems(Update, dispatch_pending_combat_request);
        app
    }

    fn send_request(
        app: &mut App,
        player: Entity,
        tick: u64,
        request: CombatRequest,
    ) -> CombatDispatchOutcome {
        app.world_mut().resource_mut::<PendingCombatRequest>().0 = Some((player, tick, request));
        app.update();
        app.world_mut()
            .resource_mut::<LastDispatchOutcome>()
            .0
            .take()
            .expect("combat dispatcher must report an outcome for every typed request")
    }

    fn drained_events<T: Event>(app: &mut App) -> Vec<T> {
        app.world_mut()
            .resource_mut::<Events<T>>()
            .drain()
            .collect()
    }

    #[test]
    fn typed_conversion_covers_all_combat_variants_and_preserves_non_combat_request() {
        let cases = [
            (
                ClientRequestV1::CombatReincarnate { v: 1 },
                CombatRequest::Reincarnate,
            ),
            (
                ClientRequestV1::CombatTerminate { v: 1 },
                CombatRequest::Terminate,
            ),
            (
                ClientRequestV1::CombatCreateNewCharacter { v: 1 },
                CombatRequest::CreateNewCharacter,
            ),
            (
                ClientRequestV1::RaiseShield { v: 1 },
                CombatRequest::RaiseShield,
            ),
            (
                ClientRequestV1::LowerShield { v: 1 },
                CombatRequest::LowerShield,
            ),
        ];

        for (wire_request, expected) in cases {
            let typed = try_into_combat_request(wire_request).ok();
            assert_eq!(
                typed,
                Some(expected),
                "each Combat wire variant must enter its matching typed route"
            );
        }

        let non_combat = ClientRequestV1::BreakthroughRequest { v: 1 };
        assert!(
            matches!(
                try_into_combat_request(non_combat),
                Err(ClientRequestV1::BreakthroughRequest { v: 1 })
            ),
            "non-Combat requests must be returned to the top-level handler unchanged"
        );
    }

    #[test]
    fn revival_requests_preserve_action_player_tick_and_event_count() {
        let mut app = combat_app(true, false, false);
        let player = app.world_mut().spawn(WorldMarker(7)).id();
        let cases = [
            (
                CombatRequest::Reincarnate,
                RevivalActionKind::Reincarnate,
                101,
            ),
            (CombatRequest::Terminate, RevivalActionKind::Terminate, 202),
            (
                CombatRequest::CreateNewCharacter,
                RevivalActionKind::CreateNewCharacter,
                303,
            ),
        ];

        for (request, expected_action, tick) in cases {
            assert_eq!(
                send_request(&mut app, player, tick, request),
                CombatDispatchOutcome::Emitted,
                "a configured revival event resource must accept the typed request"
            );
            let events = drained_events::<RevivalActionIntent>(&mut app);
            assert_eq!(
                events.len(),
                1,
                "each revival request must emit exactly one event, preserving pre-split count"
            );
            assert_eq!(events[0].entity, player, "revival player must be preserved");
            assert_eq!(
                events[0].action, expected_action,
                "the three requests must map to their exact RevivalActionKind"
            );
            assert_eq!(
                events[0].issued_at_tick, tick,
                "CombatClock.tick must pass through without normalization"
            );
        }

        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(7)),
            "typed combat dispatch must not mutate player ECS state"
        );
    }

    #[test]
    fn shield_requests_preserve_player_tick_and_event_count() {
        let mut app = combat_app(false, true, true);
        let player = app.world_mut().spawn(WorldMarker(9)).id();
        let tick = 707;

        assert_eq!(
            send_request(&mut app, player, tick, CombatRequest::RaiseShield),
            CombatDispatchOutcome::Emitted,
            "configured RaiseShieldIntent resource must accept raise_shield"
        );
        assert_eq!(
            send_request(&mut app, player, tick, CombatRequest::LowerShield),
            CombatDispatchOutcome::Emitted,
            "configured LowerShieldIntent resource must accept lower_shield"
        );

        let raised = drained_events::<RaiseShieldIntent>(&mut app);
        let lowered = drained_events::<LowerShieldIntent>(&mut app);
        assert_eq!(raised.len(), 1, "raise_shield must emit exactly one intent");
        assert_eq!(
            lowered.len(),
            1,
            "lower_shield must emit exactly one intent"
        );
        assert_eq!(raised[0].player, player, "raise player must be preserved");
        assert_eq!(lowered[0].player, player, "lower player must be preserved");
        assert_eq!(
            app.world().get::<WorldMarker>(player),
            Some(&WorldMarker(9)),
            "shield typed dispatch must not mutate player ECS state"
        );
    }

    #[test]
    fn missing_event_resource_drops_matching_request_without_ecs_mutation() {
        let cases = [
            (CombatRequest::Reincarnate, false, false, false, "revival"),
            (
                CombatRequest::RaiseShield,
                false,
                false,
                false,
                "raise shield",
            ),
            (
                CombatRequest::LowerShield,
                false,
                false,
                false,
                "lower shield",
            ),
        ];

        for (request, with_revival, with_raise, with_lower, label) in cases {
            let mut app = combat_app(with_revival, with_raise, with_lower);
            let player = app.world_mut().spawn(WorldMarker(11)).id();
            assert_eq!(
                send_request(&mut app, player, 808, request),
                CombatDispatchOutcome::DroppedMissingEventResource,
                "missing {label} event resource must fail closed and drop the request"
            );
            assert_eq!(
                app.world().get::<WorldMarker>(player),
                Some(&WorldMarker(11)),
                "missing {label} resource must not mutate player ECS state"
            );
        }
    }
}
