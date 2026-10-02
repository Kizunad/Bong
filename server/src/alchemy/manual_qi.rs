//! 手动注元事务：服务端校验炉次，从玩家扣除真实真元并归还环境，最后确认注入量。

use valence::prelude::*;

use super::furnace::within_reach;
use super::world_effects::{AlchemyWorldAction, AlchemyWorldEffect};
use super::{AlchemyFurnace, Intervention, RecipeRegistry};
use crate::cultivation::components::{ActorQiIdentity, ActorQiKind, Cultivation};
use crate::cultivation::life_record::LifeRecord;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{alchemy_snapshot_emit, client_request_handler};
use crate::player::state::canonical_player_id;
use crate::qi_physics::{QiTransferReason, WorldQiAccount};
use crate::world::dimension::CurrentDimension;
use crate::world::events::EVENT_REALM_COLLAPSE;
use crate::world::zone::ZoneRegistry;

#[derive(Debug, Clone, Copy, Event)]
pub struct ManualQiInject {
    pub player: Entity,
    pub furnace: Entity,
    pub amount: f64,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn handle_manual_qi_injections(
    mut events: EventReader<ManualQiInject>,
    mut furnaces: Query<&mut AlchemyFurnace>,
    mut players: Query<(
        &Username,
        &mut Client,
        &mut Cultivation,
        &LifeRecord,
        &Position,
        &CurrentDimension,
    )>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    registry: Res<RecipeRegistry>,
    redis: Option<Res<crate::network::RedisBridgeResource>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut world_effects: Option<ResMut<Events<AlchemyWorldEffect>>>,
    unique_ids: Query<&UniqueId>,
) {
    for event in events.read() {
        let Ok((username, mut client, mut cultivation, life, position, dimension)) =
            players.get_mut(event.player)
        else {
            continue;
        };
        let Ok(mut furnace) = furnaces.get_mut(event.furnace) else {
            continue;
        };
        let player_id = canonical_player_id(username.0.as_str());
        let Some(pos) = furnace.pos else { continue };
        if !furnace.can_access(&player_id) || !within_reach(position.0, dimension.0, pos) {
            client.send_chat_message("§c[炼丹] 请靠近属于自己的丹炉后再注元");
            continue;
        }
        let center = DVec3::new(
            f64::from(pos.0) + 0.5,
            f64::from(pos.1),
            f64::from(pos.2) + 0.5,
        );
        let zone = zones
            .as_deref_mut()
            .and_then(|zones| zones.find_zone_mut_by_pos(dimension.0, center));
        if zone.as_deref().is_some_and(|zone| {
            zone.active_events
                .iter()
                .any(|event| event == EVENT_REALM_COLLAPSE)
        }) {
            client.send_chat_message("§c[炼丹] 此处灵气已坍缩，无法注元");
            continue;
        }
        let Some(session) = furnace.session.as_mut().filter(|session| !session.finished) else {
            continue;
        };
        let Some(ledger) = ledger.as_deref_mut() else {
            client.send_chat_message("§c[炼丹] 注元账本暂不可用，真元未扣除");
            continue;
        };
        let Ok(actor) = ActorQiIdentity::from_life_record(life, ActorQiKind::Player) else {
            continue;
        };
        if !event.amount.is_finite() || event.amount <= 0.0 {
            continue;
        }
        let amount = event.amount.min(cultivation.qi_current.max(0.0));
        if amount <= 0.0 {
            client.send_chat_message("§c[炼丹] 真元不足");
            continue;
        }
        match cultivation.release_to_zone(zone, ledger, &actor, amount, QiTransferReason::Crafting)
        {
            Ok(transfer) => {
                let intervention = Intervention::InjectQi(transfer.source_debited);
                session.apply_intervention(intervention.clone());
                AlchemyWorldEffect::emit(
                    world_effects.as_deref_mut(),
                    pos,
                    Some(session),
                    AlchemyWorldAction::InjectQi {
                        source: [position.0.x, position.0.y + 1.1, position.0.z],
                    },
                );
                if let Some(events) = vfx_events.as_deref_mut() {
                    let origin = DVec3::new(center.x, center.y + 0.8, center.z);
                    if let Ok(id) = unique_ids.get(event.player) {
                        events.send(VfxEventRequest::new(
                            origin,
                            crate::schema::vfx_event::VfxEventPayloadV1::PlayAnim {
                                target_player: id.0.to_string(),
                                anim_id: crate::network::vfx_animation_trigger::ANIM_ALCHEMY_STIR
                                    .into(),
                                priority: crate::network::vfx_animation_trigger::COMBAT_PRIORITY,
                                fade_in_ticks: Some(2),
                            },
                        ));
                    }
                }
                client_request_handler::publish_alchemy_intervention_result(
                    redis.as_deref(),
                    pos,
                    &session.recipe,
                    &player_id,
                    &intervention,
                    session.temp_current,
                    session.qi_injected,
                );
                alchemy_snapshot_emit::send_session_from_furnace(
                    &mut client,
                    &player_id,
                    &furnace,
                    &registry,
                );
            }
            Err(error) => {
                tracing::warn!("[alchemy] 注元事务拒绝：{error}");
                client.send_chat_message("§c[炼丹] 注元失败，真元未扣除");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alchemy::AlchemySession;
    use crate::qi_physics::qi_flow_overflow_account;
    use valence::testing::create_mock_client;

    fn injection_app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_event::<ManualQiInject>();
        app.add_event::<AlchemyWorldEffect>();
        app.insert_resource(RecipeRegistry::new());
        app.insert_resource(WorldQiAccount::default());
        app.add_systems(Update, handle_manual_qi_injections);
        let (bundle, _helper) = create_mock_client("Azure");
        let player = app
            .world_mut()
            .spawn(bundle)
            .insert((
                Cultivation {
                    qi_current: 2.0,
                    qi_max: 5.0,
                    ..Default::default()
                },
                LifeRecord::new("alchemy-test-player"),
                Position::new(DVec3::new(1.0, 64.0, 1.0)),
                CurrentDimension::default(),
            ))
            .id();
        let mut furnace = AlchemyFurnace::placed(BlockPos::new(2, 64, 1), 1);
        furnace.session = Some(AlchemySession::new("test".into(), "offline:Azure".into()));
        let furnace = app.world_mut().spawn(furnace).id();
        (app, player, furnace)
    }

    #[test]
    fn manual_injection_debits_once_and_conserves_without_a_zone() {
        let (mut app, player, furnace) = injection_app();
        for _ in 0..2 {
            app.world_mut().send_event(ManualQiInject {
                player,
                furnace,
                amount: 5.0,
            });
        }
        app.update();
        let effects = app.world().resource::<Events<AlchemyWorldEffect>>();
        let effects = effects.iter_current_update_events().collect::<Vec<_>>();
        assert_eq!(
            effects.len(),
            1,
            "第二次注元已无真元，不得广播成功光流和音效"
        );
        assert!(
            matches!(effects[0].action, AlchemyWorldAction::InjectQi { source } if source == [1.0, 65.1, 1.0])
        );
        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            0.0
        );
        assert_eq!(
            app.world()
                .get::<AlchemyFurnace>(furnace)
                .unwrap()
                .session
                .as_ref()
                .unwrap()
                .qi_injected,
            2.0
        );
        assert_eq!(
            app.world()
                .resource::<WorldQiAccount>()
                .balance(&qi_flow_overflow_account()),
            2.0,
            "无法定位环境时，注元必须进入受账本追踪的溢出池"
        );
    }

    #[test]
    fn missing_ledger_invalid_amount_and_foreign_session_preserve_both_sides() {
        for rejection in [
            "missing_ledger",
            "nan",
            "foreign_session",
            "too_far",
            "other_dimension",
            "finished",
        ] {
            let (mut app, player, furnace) = injection_app();
            match rejection {
                "missing_ledger" => {
                    app.world_mut().remove_resource::<WorldQiAccount>();
                }
                "foreign_session" => {
                    app.world_mut()
                        .get_mut::<AlchemyFurnace>(furnace)
                        .unwrap()
                        .session
                        .as_mut()
                        .unwrap()
                        .caster_id = "offline:Other".into();
                }
                "too_far" => {
                    app.world_mut().get_mut::<Position>(player).unwrap().0.x = 100.0;
                }
                "other_dimension" => {
                    app.world_mut()
                        .get_mut::<CurrentDimension>(player)
                        .unwrap()
                        .0 = crate::world::dimension::DimensionKind::Tsy;
                }
                "finished" => {
                    app.world_mut()
                        .get_mut::<AlchemyFurnace>(furnace)
                        .unwrap()
                        .session
                        .as_mut()
                        .unwrap()
                        .finished = true;
                }
                _ => {}
            }
            app.world_mut().send_event(ManualQiInject {
                player,
                furnace,
                amount: if rejection == "nan" { f64::NAN } else { 1.0 },
            });
            app.update();
            assert!(
                app.world()
                    .resource::<Events<AlchemyWorldEffect>>()
                    .is_empty(),
                "{rejection} 不得触发注元成功的动画或音效"
            );
            assert_eq!(
                app.world().get::<Cultivation>(player).unwrap().qi_current,
                2.0,
                "{rejection}"
            );
            assert_eq!(
                app.world()
                    .get::<AlchemyFurnace>(furnace)
                    .unwrap()
                    .session
                    .as_ref()
                    .unwrap()
                    .qi_injected,
                0.0,
                "{rejection}"
            );
        }
    }
}
