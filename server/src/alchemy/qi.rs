//! 炼丹炉注灵的真元账本边界。
//!
//! 在线玩家的真元仍由 [`Cultivation::qi_current`] 持有；炉体侧用
//! `WorldQiAccount::container` 账户暂存已付款、尚未结算的注灵。两侧之间的每一步都先
//! 通过 `qi_physics::ledger` 完成可回滚的转移，再提交 ECS/session 字段，避免请求失败
//! 时凭空扣款或凭空造出炼丹火候。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, AppExit, Entity, Event, EventReader, Query, ResMut, Resource};

use crate::cultivation::components::Cultivation;
use crate::qi_physics::ledger::{
    qi_flow_overflow_account, transfer_external_qi_to_ledger, transfer_ledger_qi_to_external,
    QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount,
};
use crate::qi_physics::QiPhysicsError;

use super::furnace::AlchemyFurnace;
use super::session::AlchemySession;

/// 从 C2S 炉体路由转交给守恒结算系统的注灵请求。
#[derive(Debug, Clone, Event)]
pub struct InjectQiRequest {
    pub player: Entity,
    pub furnace: Entity,
    pub amount: f64,
}

/// 炉体上仍有未结算注灵时记录付款人。
///
/// `AlchemyFurnace` 被销毁后组件值不可再读，这张小表让清理系统仍能根据炉体实体
/// 找到账户和付款人，避免炉体销毁吞掉真元。它只记录身份，真实金额始终以 ledger
/// container 账户为准。
#[derive(Debug, Default, Resource)]
pub struct AlchemyQiReservationBook {
    owners: HashMap<Entity, String>,
}

impl AlchemyQiReservationBook {
    pub fn remember(&mut self, furnace: Entity, player_id: impl Into<String>) {
        self.owners.insert(furnace, player_id.into());
    }

    pub fn owner(&self, furnace: Entity) -> Option<&str> {
        self.owners.get(&furnace).map(String::as_str)
    }

    pub fn is_tracked(&self, furnace: Entity) -> bool {
        self.owners.contains_key(&furnace)
    }

    pub fn forget(&mut self, furnace: Entity) {
        self.owners.remove(&furnace);
    }
}

/// 每座炉体的稳定账本账户。账户余额就是炉内尚未结算的已付款真元。
pub fn furnace_qi_account(furnace: Entity) -> QiAccountId {
    QiAccountId::container(format!("alchemy:furnace:{}", furnace.to_bits()))
}

/// 把在线玩家真元原子地支付到炉体账户。
///
/// ledger 写入失败时，Cultivation/session 均保持原值；ledger 成功后余下的 ECS 写入均
/// 为已校验的减法，不再有失败分支。
pub fn debit_player_qi_to_furnace(
    player_id: &str,
    cultivation: &mut Cultivation,
    session: &mut AlchemySession,
    furnace: Entity,
    ledger: &mut WorldQiAccount,
    requested: f64,
) -> Result<f64, QiPhysicsError> {
    if !requested.is_finite() || requested <= 0.0 {
        return Err(QiPhysicsError::InvalidAmount {
            field: "alchemy.inject_qi.amount",
            value: requested,
        });
    }
    let available = cultivation.qi_current;
    if !available.is_finite() || available < 0.0 {
        return Err(QiPhysicsError::InvalidAmount {
            field: "cultivation.qi_current",
            value: available,
        });
    }
    if requested > available {
        return Err(QiPhysicsError::InsufficientQi {
            account: player_id.to_string(),
            available,
            requested,
        });
    }
    let after = available - requested;
    if !after.is_finite() || after == available {
        return Err(QiPhysicsError::UnrepresentableChange {
            field: "cultivation.qi_current",
            before: available,
            amount: requested,
        });
    }
    if !session.qi_injected.is_finite() || !session.qi_reserved.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "alchemy.session.qi_injected",
            value: session.qi_injected,
        });
    }
    let qi_after = session.qi_injected + requested;
    let reserved_after = session.qi_reserved + requested;
    if !qi_after.is_finite() || !reserved_after.is_finite() {
        return Err(QiPhysicsError::InvalidAmount {
            field: "alchemy.session.qi_injected",
            value: qi_after,
        });
    }

    let from = QiAccountId::player(player_id);
    let to = furnace_qi_account(furnace);
    transfer_external_qi_to_ledger(ledger, from, to, requested, QiTransferReason::Crafting)?;

    cultivation.qi_current = after;
    session.record_paid_qi(requested);
    Ok(requested)
}

/// 把炉体账户中的真实余额退还给玩家，受玩家当前 qi_max 容量约束。
///
/// 返回实际退回玩家的数量。调用方应随后把账户余量送入 overflow；这样即使玩家在
/// 断线期间恢复了部分真元，也不会因容量不足而吞掉炉内余额。
pub fn refund_furnace_qi_to_player(
    player_id: &str,
    cultivation: &mut Cultivation,
    session: Option<&mut AlchemySession>,
    furnace: Entity,
    ledger: &mut WorldQiAccount,
) -> Result<f64, QiPhysicsError> {
    let account = furnace_qi_account(furnace);
    let available = ledger.balance(&account);
    if available <= 0.0 {
        return Ok(0.0);
    }
    let current = cultivation.qi_current;
    if !current.is_finite() || current < 0.0 {
        return Err(QiPhysicsError::InvalidAmount {
            field: "cultivation.qi_current",
            value: current,
        });
    }
    let effective_max = (cultivation.qi_max - cultivation.qi_max_frozen.unwrap_or(0.0)).max(0.0);
    let room = (effective_max - current).max(0.0);
    let requested = available.min(room);
    if requested <= 0.0 {
        return Ok(0.0);
    }

    ensure_session_can_remove_paid_qi(session.as_deref(), requested)?;

    let after = current + requested;
    if !after.is_finite() || after == current {
        return Err(QiPhysicsError::UnrepresentableChange {
            field: "cultivation.qi_current",
            before: current,
            amount: requested,
        });
    }

    transfer_ledger_qi_to_external(
        ledger,
        account,
        QiAccountId::player(player_id),
        requested,
        QiTransferReason::Crafting,
    )?;
    cultivation.qi_current = after;
    if let Some(session) = session {
        session.remove_paid_qi_after_transfer(requested);
    }
    Ok(requested)
}

/// 把炉体剩余账户余额送进稳定 overflow 账户。
///
/// 炉体结算/玩家容量不足时没有可直接写入的在线玩家或 zone owner，overflow 是
/// `WorldQiAccount` 的正式守恒落点；它仍保留完整 `QiTransfer` 审计，不会丢失真元。
pub fn release_furnace_qi_to_overflow(
    furnace: Entity,
    session: Option<&mut AlchemySession>,
    ledger: &mut WorldQiAccount,
) -> Result<f64, QiPhysicsError> {
    let from = furnace_qi_account(furnace);
    let amount = ledger.balance(&from);
    if amount <= 0.0 {
        return Ok(0.0);
    }
    ensure_session_can_remove_paid_qi(session.as_deref(), amount)?;
    let transfer = QiTransfer::new(
        from.clone(),
        qi_flow_overflow_account(),
        amount,
        QiTransferReason::ReleaseToZone,
    )?;
    ledger.transfer(transfer)?;
    if let Some(session) = session {
        session.remove_paid_qi_after_transfer(amount);
    }
    Ok(amount)
}

/// 确保 ledger 转账成功后，session 能够同步扣除同一笔玩家预留。
///
/// `qi_reserved` 是炉体账户中玩家付款的 ECS 镜像。转账前先验证它足够，才能保证
/// ledger 成功后 `remove_paid_qi_after_transfer` 不会静默截断，退款失败时两边仍保持原值。
fn ensure_session_can_remove_paid_qi(
    session: Option<&AlchemySession>,
    amount: f64,
) -> Result<(), QiPhysicsError> {
    let Some(session) = session else {
        return Ok(());
    };
    if !session.qi_reserved.is_finite() || session.qi_reserved < amount {
        return Err(QiPhysicsError::InvalidAmount {
            field: "alchemy.session.qi_reserved",
            value: session.qi_reserved,
        });
    }
    Ok(())
}

/// 进程收到 `AppExit` 时，炉体实体不会进入持久化白名单；先把所有仍托管在炉体账户的
/// 余额转入固定 overflow，随后 persistence 的 Last flush 才会落盘该账户。没有退出
/// 请求时函数严格 no-op，避免每帧改账本。
pub(crate) fn flush_furnace_qi_on_shutdown(
    mut app_exit: EventReader<AppExit>,
    mut furnaces: Query<(Entity, &mut AlchemyFurnace)>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut reservations: Option<ResMut<AlchemyQiReservationBook>>,
) {
    if app_exit.read().next().is_none() {
        return;
    }
    let Some(ledger) = ledger.as_deref_mut() else {
        return;
    };
    for (furnace_entity, mut furnace) in furnaces.iter_mut() {
        if ledger.balance(&furnace_qi_account(furnace_entity)) <= 0.0 {
            continue;
        }
        match release_furnace_qi_to_overflow(furnace_entity, furnace.session.as_mut(), ledger) {
            Ok(_) => {
                if let Some(reservations) = reservations.as_deref_mut() {
                    reservations.forget(furnace_entity);
                }
            }
            Err(error) => tracing::error!(
                "[bong][alchemy] shutdown furnace={furnace_entity:?} qi flush failed: {error}"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::alchemy::furnace::AlchemyFurnace;
    use crate::cultivation::components::Cultivation;
    use crate::qi_physics::ledger::{assert_conservation, summarize_world_qi};
    use crate::qi_physics::WorldQiBudget;
    use crate::schema::common::SPIRIT_QI_TOTAL;
    use valence::prelude::{App, AppExit, Client, Events, Last, Mut, Update};
    use valence::testing::create_mock_client;

    fn app_with_player(qi_current: f64) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        let player = app
            .world_mut()
            .spawn(Cultivation {
                qi_current,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        (app, player)
    }

    #[test]
    fn paid_injection_moves_qi_without_changing_world_total() {
        let (mut app, player) = app_with_player(SPIRIT_QI_TOTAL);
        let furnace = app.world_mut().spawn_empty().id();
        let mut session = AlchemySession::new("hui_yuan_pill_v0".into(), "offline:alice".into());
        let before = summarize_world_qi(app.world_mut());
        assert_eq!(before.budget_initial_total, SPIRIT_QI_TOTAL);

        let amount = 12.5;
        app.world_mut()
            .resource_scope(|world, mut ledger: Mut<'_, WorldQiAccount>| {
                let mut cultivation = world.get_mut::<Cultivation>(player).unwrap();
                debit_player_qi_to_furnace(
                    "offline:alice",
                    &mut cultivation,
                    &mut session,
                    furnace,
                    &mut ledger,
                    amount,
                )
                .expect("a player with enough qi must be able to pay the furnace");
            });

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0).expect("paid injection must conserve qi");
        assert_eq!(session.qi_reserved, amount);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace)),
            amount
        );
    }

    #[test]
    fn rejected_injection_leaves_player_session_and_ledger_unchanged() {
        let (mut app, player) = app_with_player(3.0);
        let furnace = app.world_mut().spawn_empty().id();
        let mut session = AlchemySession::new("hui_yuan_pill_v0".into(), "offline:alice".into());
        let before = summarize_world_qi(app.world_mut());
        app.world_mut()
            .resource_scope(|world, mut ledger: Mut<'_, WorldQiAccount>| {
                let mut cultivation = world.get_mut::<Cultivation>(player).unwrap();
                let error = debit_player_qi_to_furnace(
                    "offline:alice",
                    &mut cultivation,
                    &mut session,
                    furnace,
                    &mut ledger,
                    4.0,
                )
                .expect_err("insufficient qi must reject before any ledger mutation");
                assert!(matches!(error, QiPhysicsError::InsufficientQi { .. }));
            });
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0).expect("rejected payment must be a no-op");
        assert_eq!(session.qi_injected, 0.0);
        assert_eq!(session.qi_reserved, 0.0);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace)),
            0.0
        );
    }

    #[test]
    fn refund_returns_capacity_and_routes_excess_to_overflow() {
        let (mut app, player) = app_with_player(90.0);
        let furnace = app.world_mut().spawn_empty().id();
        let mut session = AlchemySession::new("hui_yuan_pill_v0".into(), "offline:alice".into());
        let before = summarize_world_qi(app.world_mut());
        app.world_mut()
            .resource_scope(|world, mut ledger: Mut<'_, WorldQiAccount>| {
                let mut cultivation = world.get_mut::<Cultivation>(player).unwrap();
                debit_player_qi_to_furnace(
                    "offline:alice",
                    &mut cultivation,
                    &mut session,
                    furnace,
                    &mut ledger,
                    5.0,
                )
                .unwrap();
                cultivation.qi_max = 87.0;
                let returned = refund_furnace_qi_to_player(
                    "offline:alice",
                    &mut cultivation,
                    Some(&mut session),
                    furnace,
                    &mut ledger,
                )
                .unwrap();
                assert_eq!(returned, 2.0);
                let overflow =
                    release_furnace_qi_to_overflow(furnace, Some(&mut session), &mut ledger)
                        .unwrap();
                assert_eq!(overflow, 3.0);
            });
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("refund and overflow settlement must conserve qi");
        assert_eq!(session.qi_reserved, 0.0);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace)),
            0.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&qi_flow_overflow_account()),
            3.0
        );
    }

    #[test]
    fn inject_request_system_commits_player_and_furnace_together() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_event::<InjectQiRequest>();
        app.add_systems(
            Update,
            crate::network::client_request_handler::settle_alchemy_inject_qi_requests,
        );

        let (client_bundle, _helper) = create_mock_client("Alice");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(Cultivation {
                qi_current: SPIRIT_QI_TOTAL,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let mut furnace = AlchemyFurnace::placed(valence::prelude::BlockPos::new(2, 64, 3), 1);
        furnace.owner = Some("offline:Alice".to_string());
        furnace.session = Some(AlchemySession::new(
            "hui_yuan_pill_v0".to_string(),
            "offline:Alice".to_string(),
        ));
        let furnace_entity = app.world_mut().spawn(furnace).id();
        let before = summarize_world_qi(app.world_mut());
        app.world_mut()
            .resource_mut::<Events<InjectQiRequest>>()
            .send(InjectQiRequest {
                player,
                furnace: furnace_entity,
                amount: 7.5,
            });

        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("accepted request must debit player and credit furnace atomically");
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            SPIRIT_QI_TOTAL - 7.5
        );
        let furnace = app.world().get::<AlchemyFurnace>(furnace_entity).unwrap();
        let session = furnace.session.as_ref().unwrap();
        assert_eq!(session.qi_reserved, 7.5);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            7.5
        );
    }

    #[test]
    fn second_payer_is_rejected_while_first_furnace_reservation_is_pending() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.insert_resource(crate::alchemy::recipe::load_recipe_registry().unwrap());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_event::<InjectQiRequest>();
        app.add_systems(
            Update,
            crate::network::client_request_handler::settle_alchemy_inject_qi_requests,
        );

        let (alice_bundle, _alice_helper) = create_mock_client("Alice");
        let alice = app
            .world_mut()
            .spawn(alice_bundle)
            .insert(Cultivation {
                qi_current: SPIRIT_QI_TOTAL / 2.0,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let (bob_bundle, _bob_helper) = create_mock_client("Bob");
        let bob = app
            .world_mut()
            .spawn(bob_bundle)
            .insert(Cultivation {
                qi_current: SPIRIT_QI_TOTAL / 2.0,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let mut furnace = AlchemyFurnace::new(1);
        furnace.session = Some(AlchemySession::new(
            "hui_yuan_pill_v0".to_string(),
            "offline:Alice".to_string(),
        ));
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<Events<InjectQiRequest>>()
            .send(InjectQiRequest {
                player: alice,
                furnace: furnace_entity,
                amount: 7.5,
            });
        app.world_mut()
            .resource_mut::<Events<InjectQiRequest>>()
            .send(InjectQiRequest {
                player: bob,
                furnace: furnace_entity,
                amount: 5.0,
            });

        let before = summarize_world_qi(app.world_mut());
        app.update();
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("rejecting a second payer must preserve the qi total");
        assert_eq!(
            app.world().get::<Cultivation>(alice).unwrap().qi_current,
            SPIRIT_QI_TOTAL / 2.0 - 7.5
        );
        assert_eq!(
            app.world().get::<Cultivation>(bob).unwrap().qi_current,
            SPIRIT_QI_TOTAL / 2.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            7.5
        );
        assert_eq!(
            app.world()
                .resource::<AlchemyQiReservationBook>()
                .owner(furnace_entity),
            Some("offline:Alice")
        );
    }

    #[test]
    fn finished_session_waits_for_take_back_before_refund() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_systems(
            Update,
            crate::network::client_request_handler::settle_finished_alchemy_furnace_qi,
        );

        let (client_bundle, _helper) = create_mock_client("Alice");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(Cultivation {
                qi_current: 80.0,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let mut furnace = AlchemyFurnace::new(1);
        let mut session =
            AlchemySession::new("hui_yuan_pill_v0".to_string(), "offline:Alice".to_string());
        session.record_paid_qi(20.0);
        session.finished = true;
        furnace.session = Some(session);
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(furnace_qi_account(furnace_entity), 20.0)
            .unwrap();
        app.world_mut()
            .resource_mut::<AlchemyQiReservationBook>()
            .remember(furnace_entity, "offline:Alice");

        let before = summarize_world_qi(app.world_mut());
        app.update();
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("finished session waiting for take_back must conserve qi");
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            80.0,
            "完成但尚未收取的 session 不得提前退回付款真元"
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            20.0
        );
        assert_eq!(
            app.world()
                .get::<AlchemyFurnace>(furnace_entity)
                .and_then(|furnace| furnace.session.as_ref())
                .map(|session| session.qi_injected),
            Some(20.0),
            "等待收取的 session 必须保留已付款注灵供结算使用"
        );
        assert!(app
            .world()
            .resource::<AlchemyQiReservationBook>()
            .is_tracked(furnace_entity));

        app.world_mut()
            .get_mut::<AlchemyFurnace>(furnace_entity)
            .unwrap()
            .session = None;
        let before_collect = summarize_world_qi(app.world_mut());
        app.update();
        let after_collect = summarize_world_qi(app.world_mut());
        assert_conservation(&before_collect, &after_collect, 0.0)
            .expect("take_back 后的炉体结算必须守恒");
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            SPIRIT_QI_TOTAL
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            0.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&qi_flow_overflow_account()),
            0.0
        );
        assert!(!app
            .world()
            .resource::<AlchemyQiReservationBook>()
            .is_tracked(furnace_entity));
        assert!(app
            .world()
            .get::<AlchemyFurnace>(furnace_entity)
            .unwrap()
            .session
            .is_none());
    }

    #[test]
    fn failed_refund_keeps_ledger_player_and_session_unchanged() {
        let (mut app, player) = app_with_player(80.0);
        let furnace = app.world_mut().spawn_empty().id();
        let mut session = AlchemySession::new("hui_yuan_pill_v0".into(), "offline:alice".into());
        session.record_paid_qi(5.0);
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(QiAccountId::player("offline:alice"), 1.0e20)
            .unwrap();
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(furnace_qi_account(furnace), 5.0)
            .unwrap();
        let before = summarize_world_qi(app.world_mut());

        let error = app
            .world_mut()
            .resource_scope(|world, mut ledger: Mut<'_, WorldQiAccount>| {
                let mut cultivation = world.get_mut::<Cultivation>(player).unwrap();
                refund_furnace_qi_to_player(
                    "offline:alice",
                    &mut cultivation,
                    Some(&mut session),
                    furnace,
                    &mut ledger,
                )
                .expect_err("a session without a matching paid reservation must reject refund")
            });
        assert!(matches!(
            error,
            QiPhysicsError::InvalidAmount {
                field: "destination_balance",
                ..
            }
        ));
        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("failed refund must leave the qi total unchanged");
        assert_eq!(session.qi_reserved, 5.0);
        assert_eq!(session.qi_injected, 5.0);
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace)),
            5.0
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            80.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&QiAccountId::player("offline:alice")),
            1.0e20
        );
    }

    #[test]
    fn disconnect_refunds_tracked_furnace_qi_before_player_despawn() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_systems(
            Update,
            crate::network::client_request_handler::refund_alchemy_qi_on_disconnect,
        );

        let (client_bundle, _helper) = create_mock_client("Alice");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(Cultivation {
                qi_current: 80.0,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let mut furnace = AlchemyFurnace::new(1);
        furnace.owner = Some("offline:Alice".to_string());
        furnace.session = Some(AlchemySession::new(
            "hui_yuan_pill_v0".to_string(),
            "offline:Alice".to_string(),
        ));
        furnace.session.as_mut().unwrap().record_paid_qi(20.0);
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(furnace_qi_account(furnace_entity), 20.0)
            .unwrap();
        app.world_mut()
            .resource_mut::<AlchemyQiReservationBook>()
            .remember(furnace_entity, "offline:Alice");
        let before = summarize_world_qi(app.world_mut());

        app.world_mut().entity_mut(player).remove::<Client>();
        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("disconnect refund must preserve total qi");
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            SPIRIT_QI_TOTAL
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            0.0
        );
        assert!(!app
            .world()
            .resource::<AlchemyQiReservationBook>()
            .is_tracked(furnace_entity));
    }

    #[test]
    fn removed_furnace_refunds_owner_or_overflow_instead_of_dropping_qi() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_systems(
            Update,
            crate::network::client_request_handler::refund_removed_alchemy_furnace_qi,
        );

        let (client_bundle, _helper) = create_mock_client("Alice");
        let player = app
            .world_mut()
            .spawn(client_bundle)
            .insert(Cultivation {
                qi_current: 80.0,
                qi_max: SPIRIT_QI_TOTAL,
                ..Cultivation::default()
            })
            .id();
        let mut furnace = AlchemyFurnace::new(1);
        furnace.owner = Some("offline:Alice".to_string());
        furnace.session = Some(AlchemySession::new(
            "hui_yuan_pill_v0".to_string(),
            "offline:Alice".to_string(),
        ));
        furnace.session.as_mut().unwrap().record_paid_qi(20.0);
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(furnace_qi_account(furnace_entity), 20.0)
            .unwrap();
        app.world_mut()
            .resource_mut::<AlchemyQiReservationBook>()
            .remember(furnace_entity, "offline:Alice");
        let before = summarize_world_qi(app.world_mut());

        app.world_mut()
            .entity_mut(furnace_entity)
            .remove::<AlchemyFurnace>();
        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("removed furnace settlement must preserve total qi");
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            SPIRIT_QI_TOTAL
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            0.0
        );
    }

    #[test]
    fn shutdown_moves_active_furnace_balance_to_persistent_overflow() {
        let mut app = App::new();
        app.insert_resource(WorldQiBudget::from_total(SPIRIT_QI_TOTAL));
        app.insert_resource(WorldQiAccount::default());
        app.init_resource::<AlchemyQiReservationBook>();
        app.add_event::<AppExit>();
        app.add_systems(Last, flush_furnace_qi_on_shutdown);

        let mut furnace = AlchemyFurnace::new(1);
        furnace.session = Some(AlchemySession::new(
            "hui_yuan_pill_v0".to_string(),
            "offline:Alice".to_string(),
        ));
        furnace.session.as_mut().unwrap().record_paid_qi(9.0);
        let furnace_entity = app.world_mut().spawn(furnace).id();
        app.world_mut()
            .resource_mut::<WorldQiAccount>()
            .set_balance(furnace_qi_account(furnace_entity), 9.0)
            .unwrap();
        let before = summarize_world_qi(app.world_mut());
        app.world_mut().send_event(AppExit::Success);
        app.update();

        let after = summarize_world_qi(app.world_mut());
        assert_conservation(&before, &after, 0.0)
            .expect("shutdown furnace flush must preserve total qi");
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&furnace_qi_account(furnace_entity)),
            0.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&qi_flow_overflow_account()),
            9.0
        );
    }
}
