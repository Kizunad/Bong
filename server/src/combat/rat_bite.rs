use valence::prelude::{
    bevy_ecs, Client, Commands, DVec3, Entity, Event, EventReader, EventWriter, Position, Query,
    ResMut, With,
};
use valence::protocol::encode::WritePacket;
use valence::protocol::packets::play::DamageTiltS2c;
use valence::protocol::VarInt;

use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use crate::fauna::rat_phase::MeditatingState;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::audio_event_emit::{
    AudioRecipient, PlaySoundRecipeRequest, AUDIO_MELEE_RADIUS,
};
use crate::network::gameplay_vfx;
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::npc::spawn_rat::RatBlackboard;
use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason, WorldQiAccount};
use crate::schema::server_data::{
    CombatEventFloaterEntryV1, CombatEventFloaterV1, ServerDataPayloadV1, ServerDataV1,
};

/// plan-ambient-threat-v1 P2 —— 咬中瞬间灰白尘粒反馈：4 粒 burst、8 tick 生命周期、
/// 灰白 `#9B9B8C`，自咬点向上飘。无既有"咬击"VFX 可复用（SeekQiSourceAction/兽潮咬击均
/// 未挂粒子），新增最小反馈，经 `VfxBootstrap.registerDefaults()` 注册。
const RAT_BITE_NIP_COLOR: &str = "#9B9B8C";
const RAT_BITE_NIP_STRENGTH: f32 = 0.85;
const RAT_BITE_NIP_COUNT: u32 = 4;
const RAT_BITE_NIP_DURATION_TICKS: u32 = 8;
/// `entity.silverfish.ambient` pitch 0.7 vol 0.5（`assets/audio/recipes/rat_bite_nip.json`）。
const RAT_BITE_NIP_AUDIO_RECIPE: &str = "rat_bite_nip";

#[derive(Debug, Clone, Copy, Event, PartialEq)]
pub struct RatBiteEvent {
    pub rat: Entity,
    pub target: Entity,
    pub qi_steal: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn apply_rat_bite_qi_drain(
    mut bites: EventReader<RatBiteEvent>,
    mut cultivators: Query<&mut Cultivation>,
    mut rats: Query<&mut RatBlackboard>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut qi_transfers: EventWriter<QiTransfer>,
    mut ledger: Option<ResMut<WorldQiAccount>>,
    mut clients: Query<&mut Client>,
    positions: Query<&Position>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    mut audio_events: EventWriter<PlaySoundRecipeRequest>,
) {
    for bite in bites.read() {
        if bite.qi_steal == 0 {
            continue;
        }
        let Ok(mut cultivation) = cultivators.get_mut(bite.target) else {
            continue;
        };
        if cultivation.qi_current <= 0.0 {
            continue;
        }
        let Ok(mut rat) = rats.get_mut(bite.rat) else {
            continue;
        };

        let before = cultivation.qi_current;
        cultivation.qi_current =
            (cultivation.qi_current - f64::from(bite.qi_steal)).clamp(0.0, cultivation.qi_max);
        let drained = (before - cultivation.qi_current).max(0.0);
        if drained > 0.0 {
            let from = QiAccountId::player(bite.target.index().to_string());
            let rat_account = QiAccountId::npc(format!("rat:{}", bite.rat.index()));
            if let Ok(transfer) = QiTransfer::new(
                from,
                rat_account.clone(),
                drained,
                QiTransferReason::RatBiteDrain,
            ) {
                match ledger.as_deref_mut() {
                    Some(account) => {
                        credit_rat_bite_drain(account, &rat_account, drained, transfer.clone());
                    }
                    None => {
                        tracing::debug!(
                            "[bong][combat] rat bite qi ledger unavailable, skipping credit for {:?}",
                            rat_account
                        );
                    }
                }
                qi_transfers.send(transfer);
            }
            // §8.1 决议 #1 point 4 —— `drained_qi` 从"独立累加"改为镜像 `npc:rat:<id>`
            // 账户余额（照抄 `fauna::mimic_spider` 的 `blackboard.drained_qi =
            // ledger.balance(&spider_account)` 镜像模式）；`ledger` 缺席时保留原地累加
            // fallback，无 ledger 资源的 headless 测试行为不退化。
            rat.drained_qi = ledger
                .as_deref()
                .map(|account| account.balance(&rat_account))
                .unwrap_or(rat.drained_qi + drained);
            send_bite_feedback(&mut clients, bite.target, drained as f32);
            if let Ok(position) = positions.get(bite.target) {
                emit_rat_bite_nip_av(position.get(), &mut vfx_events, &mut audio_events);
            }
        }
        if before > 0.0 && cultivation.qi_current <= f64::EPSILON {
            deaths.send(CultivationDeathTrigger {
                entity: bite.target,
                cause: CultivationDeathCause::SwarmQiDrain,
                context: serde_json::json!({
                    "rat": format!("{:?}", bite.rat),
                    "qi_steal": bite.qi_steal,
                }),
            });
        }
    }
}

/// §8.1 决议 #1 point 1 —— 鼠咬吸取的真元落入 `npc:rat:<id>` 账户，逻辑照抄
/// `npc::skull_fiend::credit_skull_fiend_drain`：`set_balance` 累加余额后
/// `push_transfer_audit` 留下审计轨迹（`transfer` 复用 caller 已构造好的
/// `QiTransfer`，不重复构造）。`set_balance` 失败（理论上不会发生，`amount` 恒为
/// 非负有限值）时只记 debug 日志、跳过审计追加，不 panic。
fn credit_rat_bite_drain(
    account: &mut WorldQiAccount,
    rat_account: &QiAccountId,
    amount: f64,
    transfer: QiTransfer,
) {
    let balance = account.balance(rat_account);
    if account
        .set_balance(rat_account.clone(), balance + amount)
        .is_err()
    {
        tracing::debug!(
            "[bong][combat] rat bite qi ledger credit failed for {:?}, amount={}",
            rat_account,
            amount
        );
        return;
    }
    account.push_transfer_audit(transfer);
}

/// plan-ambient-threat-v1 P2 —— 咬中瞬间的粒子 + SFX（全复用既有原语：`SpawnParticle`
/// payload 走既有 `bong:vfx_event` channel，SFX 走既有 `PlaySoundRecipeRequest` /
/// `bong:audio/play` channel，无新协议）。
fn emit_rat_bite_nip_av(
    target_position: DVec3,
    vfx_events: &mut EventWriter<VfxEventRequest>,
    audio_events: &mut EventWriter<PlaySoundRecipeRequest>,
) {
    let particle_origin = target_position + DVec3::new(0.0, 0.2, 0.0);
    vfx_events.send(gameplay_vfx::spawn_request(
        gameplay_vfx::RAT_BITE_NIP,
        particle_origin,
        Some([0.0, 1.0, 0.0]),
        RAT_BITE_NIP_COLOR,
        RAT_BITE_NIP_STRENGTH,
        RAT_BITE_NIP_COUNT,
        RAT_BITE_NIP_DURATION_TICKS,
    ));
    audio_events.send(PlaySoundRecipeRequest {
        recipe_id: RAT_BITE_NIP_AUDIO_RECIPE.to_string(),
        instance_id: 0,
        pos: Some([
            target_position.x.floor() as i32,
            target_position.y.floor() as i32,
            target_position.z.floor() as i32,
        ]),
        flag: None,
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Radius {
            origin: target_position,
            radius: AUDIO_MELEE_RADIUS,
        },
    });
}

/// plan-ambient-threat-v1 P2 —— 打坐打断：被鼠咬中的目标若持有 `MeditatingState`
/// （对齐兽潮咬击"咬中打断凝神"既有语义），立即移除，让该目标退出静坐态。
pub fn interrupt_meditation_on_rat_bite(
    mut commands: Commands,
    mut bites: EventReader<RatBiteEvent>,
    meditating: Query<(), With<MeditatingState>>,
) {
    for bite in bites.read() {
        if meditating.contains(bite.target) {
            commands.entity(bite.target).remove::<MeditatingState>();
        }
    }
}

fn send_bite_feedback(clients: &mut Query<&mut Client>, target: Entity, drained: f32) {
    let Ok(mut client) = clients.get_mut(target) else {
        return;
    };

    let entry = CombatEventFloaterEntryV1 {
        kind: "qi_damage".to_string(),
        amount: drained,
        text: format!("-{}", drained.round() as i64),
        x: 0.0,
        y: 0.0,
        z: 0.0,
        outgoing: false, // 发给被咬玩家：承伤视角
    };
    let payload = ServerDataV1::new(ServerDataPayloadV1::CombatEventFloater(
        CombatEventFloaterV1 {
            events: vec![entry],
        },
    ));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());

    client.write_packet(&DamageTiltS2c {
        entity_id: VarInt(0),
        yaw: 0.0,
    });

    tracing::debug!(
        "[bong][network] sent {} {} (qi_damage) + DamageTilt for rat bite",
        SERVER_DATA_CHANNEL,
        payload_type,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, ChunkPos, Events, Update};

    use crate::cultivation::components::{Cultivation, Realm};

    fn cultivation(qi_current: f64) -> Cultivation {
        Cultivation {
            realm: Realm::Induce,
            qi_current,
            qi_max: 10.0,
            ..Default::default()
        }
    }

    fn rat_blackboard() -> RatBlackboard {
        RatBlackboard::new("spawn", ChunkPos::new(0, 0))
    }

    fn rat_bite_app() -> App {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_event::<CultivationDeathTrigger>();
        app.add_event::<QiTransfer>();
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_systems(Update, apply_rat_bite_qi_drain);
        app
    }

    #[test]
    fn rat_bite_drains_only_qi_no_hp_damage() {
        let mut app = rat_bite_app();
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(target).unwrap().qi_current,
            3.0,
            "Rat bite must drain Cultivation.qi_current directly"
        );
        assert!(
            app.world()
                .resource::<Events<CultivationDeathTrigger>>()
                .is_empty(),
            "nonlethal rat bites must not emit cultivation death"
        );
    }

    #[test]
    fn rat_bite_records_qi_transfer_to_rat_account() {
        let mut app = rat_bite_app();
        app.insert_resource(WorldQiAccount::default());
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        let rat_bb = app.world().get::<RatBlackboard>(rat).unwrap();
        assert_eq!(rat_bb.drained_qi, 2.0, "鼠储量应同步累计实际吸取量");

        let rat_account = QiAccountId::npc(format!("rat:{}", rat.index()));
        let ledger = app.world().resource::<WorldQiAccount>();
        assert_eq!(
            ledger.balance(&rat_account),
            2.0,
            "§8.1 决议 #1 —— 鼠咬吸取的真元必须真实落入 npc:rat ledger 账户，而不是只 \
             emit QiTransfer 审计事件却从无 system 消费落账"
        );

        let transfers = app.world().resource::<Events<QiTransfer>>();
        let mut reader = transfers.get_reader();
        let transfer = reader
            .read(transfers)
            .next()
            .expect("鼠咬吸取真元必须 emit QiTransfer 审计轨迹");
        assert_eq!(transfer.reason, QiTransferReason::RatBiteDrain);
        assert_eq!(
            transfer.from,
            QiAccountId::player(target.index().to_string())
        );
        assert_eq!(transfer.to, rat_account);
        assert_eq!(transfer.amount, 2.0);
    }

    #[test]
    fn rat_bite_conserves_total_qi_end_to_end() {
        // §P0 验收抓手 #1 —— 咬前后 player_qi + ledger_qi 严格相等：被偷真元必须转入
        // npc:rat 账户，而不是从 Cultivation.qi_current 扣掉后凭空消失。测试 app 无
        // ZoneRegistry / PlayerInventory，故 zone_qi / container_qi 桶恒为 0，可省略。
        let mut app = rat_bite_app();
        app.insert_resource(WorldQiAccount::default());
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        let player_qi_before: f64 = app
            .world_mut()
            .query::<&Cultivation>()
            .iter(app.world())
            .map(|cultivation| cultivation.qi_current)
            .sum();
        let ledger_qi_before = app.world().resource::<WorldQiAccount>().total();
        assert_eq!(
            ledger_qi_before, 0.0,
            "sanity: 咬击前 ledger 应为空账本（无预置余额）"
        );
        let total_before = player_qi_before + ledger_qi_before;

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        let player_qi_after: f64 = app
            .world_mut()
            .query::<&Cultivation>()
            .iter(app.world())
            .map(|cultivation| cultivation.qi_current)
            .sum();
        let ledger_qi_after = app.world().resource::<WorldQiAccount>().total();
        let total_after = player_qi_after + ledger_qi_after;

        assert!(
            player_qi_after < player_qi_before,
            "sanity: 鼠咬必须实际扣减玩家真元，否则本守恒对拍无意义"
        );
        assert!(
            (total_before - total_after).abs() < 1e-9,
            "drained qi 应转入 npc:rat 账户而非消失，实际 before={total_before} after={total_after}"
        );
    }

    #[test]
    fn rat_bite_without_rat_blackboard_does_not_drain_qi() {
        let mut app = rat_bite_app();
        let rat = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(target).unwrap().qi_current,
            5.0,
            "缺 RatBlackboard 时不能扣玩家真元，否则吸取量无处入账"
        );
        assert!(
            app.world().resource::<Events<QiTransfer>>().is_empty(),
            "缺 RatBlackboard 时不应 emit QiTransfer"
        );
    }

    #[test]
    fn qi_drain_to_zero_emits_swarm_death_trigger() {
        let mut app = rat_bite_app();
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        let target = app.world_mut().spawn(cultivation(1.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        let deaths = app.world().resource::<Events<CultivationDeathTrigger>>();
        let event = deaths
            .iter_current_update_events()
            .next()
            .expect("rat bite to zero qi should emit death trigger");
        assert_eq!(event.entity, target);
        assert_eq!(event.cause, CultivationDeathCause::SwarmQiDrain);
    }

    #[test]
    fn rat_bite_emits_particle_and_sfx_feedback_at_target_position() {
        let mut app = rat_bite_app();
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        let target = app
            .world_mut()
            .spawn((cultivation(5.0), Position::new([1.0, 64.0, 2.0])))
            .id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        let vfx = app.world().resource::<Events<VfxEventRequest>>();
        let vfx_event = vfx
            .iter_current_update_events()
            .next()
            .expect("successful rat bite must emit a bite-nip particle VfxEventRequest");
        match &vfx_event.payload {
            crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle {
                event_id,
                color,
                count,
                duration_ticks,
                ..
            } => {
                assert_eq!(event_id, gameplay_vfx::RAT_BITE_NIP);
                assert_eq!(color.as_deref(), Some(RAT_BITE_NIP_COLOR));
                assert_eq!(*count, Some(RAT_BITE_NIP_COUNT as u16));
                assert_eq!(*duration_ticks, Some(RAT_BITE_NIP_DURATION_TICKS as u16));
            }
            other => panic!("expected SpawnParticle payload for rat bite feedback, got {other:?}"),
        }

        let audio = app.world().resource::<Events<PlaySoundRecipeRequest>>();
        let audio_event = audio
            .iter_current_update_events()
            .next()
            .expect("successful rat bite must emit a PlaySoundRecipeRequest");
        assert_eq!(audio_event.recipe_id, RAT_BITE_NIP_AUDIO_RECIPE);
        assert_eq!(
            audio_event.recipient,
            AudioRecipient::Radius {
                origin: DVec3::new(1.0, 64.0, 2.0),
                radius: AUDIO_MELEE_RADIUS,
            }
        );
    }

    #[test]
    fn rat_bite_without_target_position_still_drains_qi_but_skips_av() {
        let mut app = rat_bite_app();
        let rat = app.world_mut().spawn(rat_blackboard()).id();
        // 目标没有 Position（例如极端测试场景）——守恒路径必须不受影响，只是跳过 AV。
        let target = app.world_mut().spawn(cultivation(5.0)).id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 2,
        });
        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(target).unwrap().qi_current,
            3.0,
            "无 Position 不应影响真元守恒扣减"
        );
        assert!(
            app.world().resource::<Events<VfxEventRequest>>().is_empty(),
            "无 Position 时找不到粒子 origin，必须跳过 AV 而不是 panic 或用假坐标"
        );
        assert!(
            app.world()
                .resource::<Events<PlaySoundRecipeRequest>>()
                .is_empty(),
            "无 Position 时同理跳过 SFX"
        );
    }

    fn meditation_interrupt_app() -> App {
        let mut app = App::new();
        app.add_event::<RatBiteEvent>();
        app.add_systems(Update, interrupt_meditation_on_rat_bite);
        app
    }

    #[test]
    fn rat_bite_interrupts_meditating_target() {
        let mut app = meditation_interrupt_app();
        let rat = app.world_mut().spawn_empty().id();
        let target = app
            .world_mut()
            .spawn(crate::fauna::rat_phase::MeditatingState { since_tick: 5 })
            .id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 1,
        });
        app.update();

        assert!(
            app.world().get::<MeditatingState>(target).is_none(),
            "打坐中的目标被鼠咬中必须打断凝神（对齐兽潮咬击既有语义），MeditatingState 应被移除"
        );
    }

    #[test]
    fn rat_bite_on_non_meditating_target_is_noop_for_meditation_interrupt() {
        let mut app = meditation_interrupt_app();
        let rat = app.world_mut().spawn_empty().id();
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(RatBiteEvent {
            rat,
            target,
            qi_steal: 1,
        });
        app.update();

        assert!(
            app.world().get::<MeditatingState>(target).is_none(),
            "本就没有 MeditatingState 的目标被咬不应报错或凭空插入该组件"
        );
    }
}
