use valence::prelude::{
    Changed, Client, Entity, EventReader, EventWriter, Position, Query, Res, UniqueId, With,
};

use crate::combat::components::StatusEffects;
use crate::combat::events::StatusEffectKind;
use crate::combat::status::has_active_status;
use crate::combat::CombatClock;
use crate::cultivation::full_power_strike::{
    ChargeInterruptedEvent, ChargingState, FullPowerReleasedEvent,
};
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    FullPowerChargingStateV1, FullPowerReleaseV1, ServerDataPayloadV1, ServerDataV1,
};
use crate::schema::vfx_event::VfxEventPayloadV1;

const CHARGING_ORB_EVENT_ID: &str = "bong:charging_orb";
const RELEASE_LIGHTNING_EVENT_ID: &str = "bong:release_lightning";
const EXHAUSTED_GREY_MIST_EVENT_ID: &str = "bong:exhausted_grey_mist";
const CHARGING_ORB_REFRESH_TICKS: u64 = 4;
const EXHAUSTED_MIST_REFRESH_TICKS: u64 = 20;

pub fn emit_full_power_charging_state_payloads(
    charging_q: Query<(Entity, &ChargingState, Option<&UniqueId>), Changed<ChargingState>>,
    position_q: Query<&Position>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for (entity, charging, unique_id) in &charging_q {
        send_server_data_to_entity(
            entity,
            ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
                caster_uuid: actor_id(entity, unique_id),
                active: true,
                qi_committed: charging.qi_committed,
                target_qi: charging.target_qi,
                started_tick: charging.started_at_tick,
            }),
            &mut clients,
        );

        if let Ok(position) = position_q.get(entity) {
            send_charging_orb_vfx(charging, position, &mut vfx_events);
        }
    }
}

pub fn emit_full_power_charged_orb_vfx(
    clock: Res<CombatClock>,
    charging_q: Query<(&ChargingState, &Position)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    if clock.tick % CHARGING_ORB_REFRESH_TICKS != 0 {
        return;
    }
    for (charging, position) in &charging_q {
        if is_fully_charged(charging) {
            send_charging_orb_vfx(charging, position, &mut vfx_events);
        }
    }
}

pub fn emit_full_power_charging_clear_payloads(
    mut released: EventReader<FullPowerReleasedEvent>,
    mut interrupted: EventReader<ChargeInterruptedEvent>,
    ids: Query<&UniqueId>,
    positions: Query<&Position>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    // 蓄力结束（释放或被打断）即停掉 windup_charge 循环动画。该动画 isLoop:true（设计为
    // "可持续到释放"的蓄力保持姿势），cast_full_power_charge 在蓄力开始时 PlayAnim 但无配套
    // StopAnim——不在此处停，玩家释放/被打断后手臂会一直卡在蓄力姿势（同采集循环动画 bug）。
    for event in released.read() {
        send_charging_clear(event.caster, event.at_tick, &ids, &mut clients);
        stop_windup_charge_anim(event.caster, &ids, &positions, &mut vfx_events);
    }
    for event in interrupted.read() {
        send_charging_clear(event.caster, event.at_tick, &ids, &mut clients);
        stop_windup_charge_anim(event.caster, &ids, &positions, &mut vfx_events);
    }
}

/// 给 caster 发 StopAnim(bong:windup_charge)，停掉蓄力保持的循环动画。
fn stop_windup_charge_anim(
    caster: Entity,
    ids: &Query<&UniqueId>,
    positions: &Query<&Position>,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let (Ok(unique_id), Ok(position)) = (ids.get(caster), positions.get(caster)) else {
        return;
    };
    vfx_events.send(VfxEventRequest::new(
        position.get(),
        VfxEventPayloadV1::StopAnim {
            target_player: unique_id.0.to_string(),
            anim_id: "bong:windup_charge".to_string(),
            fade_out_ticks: Some(2),
        },
    ));
}

pub fn emit_full_power_release_payloads(
    mut released: EventReader<FullPowerReleasedEvent>,
    ids: Query<&UniqueId>,
    positions: Query<&Position>,
    mut clients: Query<&mut Client, With<Client>>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in released.read() {
        let target_uuid = event
            .target
            .map(|target| actor_id(target, ids.get(target).ok()));
        send_server_data_to_entity(
            event.caster,
            ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
                caster_uuid: actor_id(event.caster, ids.get(event.caster).ok()),
                target_uuid,
                qi_released: event.qi_released,
                tick: event.at_tick,
                hit_position: event.hit_position,
            }),
            &mut clients,
        );

        if let Ok(position) = positions.get(event.caster) {
            let origin = position.get();
            let direction = event.hit_position.map(|hit| {
                [
                    hit[0] - origin.x,
                    hit[1] - (origin.y + 1.0),
                    hit[2] - origin.z,
                ]
            });
            vfx_events.send(VfxEventRequest::new(
                origin,
                VfxEventPayloadV1::SpawnParticle {
                    event_id: RELEASE_LIGHTNING_EVENT_ID.to_string(),
                    origin: [origin.x, origin.y + 1.0, origin.z],
                    direction,
                    color: Some("#B445FF".to_string()),
                    strength: Some(1.0),
                    count: Some(18),
                    duration_ticks: Some(10),
                },
            ));
        }
    }
}

/// 全力一击释放瞬间的虚脱灰雾 VFX（一次性）。
///
/// 虚脱状态本身现走标准 status（StatusEffectKind::Exhausted），其 HUD「虚脱」由
/// status_snapshot 驱动；此处仅保留全局可见的灰雾粒子表现，由释放事件触发。
/// 不再发 `FullPowerExhaustedState` S2C payload（旧独立 HUD 路径已废）。
pub fn emit_full_power_exhausted_mist_vfx(
    mut released: EventReader<FullPowerReleasedEvent>,
    positions: Query<&Position>,
    statuses: Query<&StatusEffects>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    for event in released.read() {
        // 仅当释放确实施加了虚脱 debuff 时才放灰雾（flow>1.0 乘风免虚脱时不放）。
        let exhausted = statuses
            .get(event.caster)
            .is_ok_and(|status| has_active_status(status, StatusEffectKind::Exhausted));
        if !exhausted {
            continue;
        }
        if let Ok(position) = positions.get(event.caster) {
            send_exhausted_mist_vfx(position, &mut vfx_events);
        }
    }
}

/// 虚脱期间的灰雾刷新 VFX。改读标准 StatusEffects（StatusEffectKind::Exhausted），
/// 不再依赖游离 `Exhausted` 组件。
pub fn emit_full_power_exhausted_mist_refresh_vfx(
    clock: Res<CombatClock>,
    exhausted_q: Query<(&StatusEffects, &Position)>,
    mut vfx_events: EventWriter<VfxEventRequest>,
) {
    if clock.tick % EXHAUSTED_MIST_REFRESH_TICKS != 0 {
        return;
    }
    for (status, position) in &exhausted_q {
        if !has_active_status(status, StatusEffectKind::Exhausted) {
            continue;
        }
        send_exhausted_mist_vfx(position, &mut vfx_events);
    }
}

fn send_charging_clear(
    entity: Entity,
    tick: u64,
    ids: &Query<&UniqueId>,
    clients: &mut Query<&mut Client, With<Client>>,
) {
    send_server_data_to_entity(
        entity,
        ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
            caster_uuid: actor_id(entity, ids.get(entity).ok()),
            active: false,
            qi_committed: 0.0,
            target_qi: 0.0,
            started_tick: tick,
        }),
        clients,
    );
}

fn send_server_data_to_entity(
    entity: Entity,
    payload: ServerDataPayloadV1,
    clients: &mut Query<&mut Client, With<Client>>,
) {
    let payload = ServerDataV1::new(payload);
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };

    let Ok(mut client) = clients.get_mut(entity) else {
        return;
    };
    send_server_data_payload(&mut client, payload_bytes.as_slice());
}

fn actor_id(entity: Entity, unique_id: Option<&UniqueId>) -> String {
    unique_id
        .map(|id| id.0.to_string())
        .unwrap_or_else(|| format!("entity:{entity:?}"))
}

fn charge_strength(charging: &ChargingState) -> f32 {
    if charging.target_qi <= f64::EPSILON {
        return 0.1;
    }
    (charging.qi_committed / charging.target_qi).clamp(0.1, 1.0) as f32
}

fn is_fully_charged(charging: &ChargingState) -> bool {
    charging.target_qi > f64::EPSILON && charging.qi_committed + f64::EPSILON >= charging.target_qi
}

fn send_charging_orb_vfx(
    charging: &ChargingState,
    position: &Position,
    vfx_events: &mut EventWriter<VfxEventRequest>,
) {
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: CHARGING_ORB_EVENT_ID.to_string(),
            origin: [origin.x, origin.y + 1.0, origin.z],
            direction: None,
            color: Some("#C43CFF".to_string()),
            strength: Some(charge_strength(charging)),
            count: Some(10),
            duration_ticks: Some(8),
        },
    ));
}

fn send_exhausted_mist_vfx(position: &Position, vfx_events: &mut EventWriter<VfxEventRequest>) {
    let origin = position.get();
    vfx_events.send(VfxEventRequest::new(
        origin,
        VfxEventPayloadV1::SpawnParticle {
            event_id: EXHAUSTED_GREY_MIST_EVENT_ID.to_string(),
            origin: [origin.x, origin.y + 0.8, origin.z],
            direction: None,
            color: Some("#7D7782".to_string()),
            strength: Some(0.65),
            count: Some(12),
            duration_ticks: Some(40),
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
    use valence::prelude::{App, Events, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn spawn_mock_client(app: &mut App, name: &str) -> (Entity, MockClientHelper) {
        let (bundle, helper) = create_mock_client(name);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush");
        }
    }

    fn collect_full_power_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataPayloadV1> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server data payload should decode");
            match payload.payload {
                ServerDataPayloadV1::FullPowerChargingState(state) => {
                    payloads.push(ServerDataPayloadV1::FullPowerChargingState(state));
                }
                ServerDataPayloadV1::FullPowerRelease(event) => {
                    payloads.push(ServerDataPayloadV1::FullPowerRelease(event));
                }
                ServerDataPayloadV1::FullPowerExhaustedState(state) => {
                    payloads.push(ServerDataPayloadV1::FullPowerExhaustedState(state));
                }
                _ => {}
            }
        }
        payloads
    }

    #[test]
    fn charging_state_is_sent_only_to_caster_but_vfx_is_global() {
        let mut app = App::new();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_charging_state_payloads);

        let (caster, mut caster_helper) = spawn_mock_client(&mut app, "Caster");
        let (_observer, mut observer_helper) = spawn_mock_client(&mut app, "Observer");
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 75.0,
            target_qi: 150.0,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let caster_payloads = collect_full_power_payloads(&mut caster_helper);
        let observer_payloads = collect_full_power_payloads(&mut observer_helper);
        assert_eq!(caster_payloads.len(), 1);
        assert!(matches!(
            &caster_payloads[0],
            ServerDataPayloadV1::FullPowerChargingState(state)
                if state.active && state.qi_committed == 75.0 && state.target_qi == 150.0
        ));
        assert!(observer_payloads.is_empty());
        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            1
        );
    }

    #[test]
    fn release_payload_is_sent_only_to_caster_but_vfx_is_global() {
        let mut app = App::new();
        app.add_event::<FullPowerReleasedEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_release_payloads);

        let (caster, mut caster_helper) = spawn_mock_client(&mut app, "Caster");
        let (target, mut observer_helper) = spawn_mock_client(&mut app, "Observer");
        app.world_mut().send_event(FullPowerReleasedEvent {
            caster,
            target: Some(target),
            qi_released: 120.0,
            at_tick: 42,
            hit_position: Some([1.0, 65.0, 1.0]),
            realm_gap_tier: None,
        });

        app.update();
        flush_all_client_packets(&mut app);

        let caster_payloads = collect_full_power_payloads(&mut caster_helper);
        let observer_payloads = collect_full_power_payloads(&mut observer_helper);
        assert_eq!(caster_payloads.len(), 1);
        assert!(matches!(
            &caster_payloads[0],
            ServerDataPayloadV1::FullPowerRelease(event)
                if event.qi_released == 120.0 && event.tick == 42
        ));
        assert!(observer_payloads.is_empty());
        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            1
        );
    }

    #[test]
    fn charging_clear_stops_windup_charge_anim_on_release_and_interrupt() {
        // 回归锁：蓄力结束（释放或被打断）必须 emit StopAnim(windup_charge) 停掉 isLoop:true 的
        // 蓄力保持动画；否则玩家释放/被打断后手臂卡在蓄力姿势（cast_full_power_charge 只 PlayAnim）。
        use crate::cultivation::full_power_strike::InterruptTrigger;

        // 释放路径
        {
            let mut app = App::new();
            app.add_event::<FullPowerReleasedEvent>();
            app.add_event::<ChargeInterruptedEvent>();
            app.add_event::<VfxEventRequest>();
            app.add_systems(Update, emit_full_power_charging_clear_payloads);
            let (caster, _h) = spawn_mock_client(&mut app, "Caster");
            app.world_mut().send_event(FullPowerReleasedEvent {
                caster,
                target: None,
                qi_released: 100.0,
                at_tick: 10,
                hit_position: None,
                realm_gap_tier: None,
            });
            app.update();
            assert!(
                app.world()
                    .resource::<Events<VfxEventRequest>>()
                    .iter_current_update_events()
                    .any(|r| matches!(&r.payload,
                        VfxEventPayloadV1::StopAnim { anim_id, .. } if anim_id == "bong:windup_charge")),
                "蓄力释放应 emit StopAnim(bong:windup_charge) 停掉循环蓄力动画"
            );
        }
        // 打断路径
        {
            let mut app = App::new();
            app.add_event::<FullPowerReleasedEvent>();
            app.add_event::<ChargeInterruptedEvent>();
            app.add_event::<VfxEventRequest>();
            app.add_systems(Update, emit_full_power_charging_clear_payloads);
            let (caster, _h) = spawn_mock_client(&mut app, "Caster");
            app.world_mut().send_event(ChargeInterruptedEvent {
                caster,
                qi_lost: 50.0,
                qi_refunded: 25.0,
                trigger: InterruptTrigger::Damage,
                at_tick: 10,
            });
            app.update();
            assert!(
                app.world()
                    .resource::<Events<VfxEventRequest>>()
                    .iter_current_update_events()
                    .any(|r| matches!(&r.payload,
                        VfxEventPayloadV1::StopAnim { anim_id, .. } if anim_id == "bong:windup_charge")),
                "蓄力被打断应 emit StopAnim(bong:windup_charge)"
            );
        }
    }

    #[test]
    fn fully_charged_orb_vfx_refreshes_after_state_stops_changing() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 24 });
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_charged_orb_vfx);

        let (caster, _helper) = spawn_mock_client(&mut app, "Caster");
        app.world_mut().entity_mut(caster).insert(ChargingState {
            slot: 0,
            started_at_tick: 10,
            qi_committed: 150.0,
            target_qi: 150.0,
        });

        app.update();

        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            1
        );
    }

    /// 虚脱灰雾刷新现读标准 StatusEffects（StatusEffectKind::Exhausted）。
    #[test]
    fn exhausted_mist_vfx_refreshes_while_exhausted() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 40 });
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_exhausted_mist_refresh_vfx);

        let (caster, _helper) = spawn_mock_client(&mut app, "Caster");
        app.world_mut().entity_mut(caster).insert(StatusEffects {
            active: vec![crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::Exhausted,
                magnitude: 0.5,
                remaining_ticks: 200,
                source_pill: None,
            }],
        });

        app.update();

        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            1
        );
    }

    /// 无虚脱 status 时不刷新灰雾。
    #[test]
    fn exhausted_mist_vfx_skips_when_not_exhausted() {
        let mut app = App::new();
        app.insert_resource(CombatClock { tick: 40 });
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_exhausted_mist_refresh_vfx);

        let (caster, _helper) = spawn_mock_client(&mut app, "Caster");
        app.world_mut()
            .entity_mut(caster)
            .insert(StatusEffects::default());

        app.update();

        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            0
        );
    }

    /// 释放瞬间施加了虚脱 debuff → 放一次灰雾；同时不发任何 FullPowerExhaustedState payload
    /// （旧独立 HUD 路径已废，虚脱 HUD 改由 status_snapshot 驱动）。
    #[test]
    fn release_emits_mist_once_and_no_exhausted_state_payload() {
        let mut app = App::new();
        app.add_event::<FullPowerReleasedEvent>();
        app.add_event::<VfxEventRequest>();
        app.add_systems(Update, emit_full_power_exhausted_mist_vfx);

        let (caster, mut caster_helper) = spawn_mock_client(&mut app, "Caster");
        app.world_mut().entity_mut(caster).insert(StatusEffects {
            active: vec![crate::combat::components::ActiveStatusEffect {
                kind: StatusEffectKind::Exhausted,
                magnitude: 0.5,
                remaining_ticks: 200,
                source_pill: None,
            }],
        });
        app.world_mut().send_event(FullPowerReleasedEvent {
            caster,
            target: None,
            qi_released: 100.0,
            at_tick: 10,
            hit_position: None,
            realm_gap_tier: None,
        });

        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            app.world()
                .resource::<Events<VfxEventRequest>>()
                .iter_current_update_events()
                .count(),
            1,
            "释放施加虚脱时应放一次灰雾 VFX"
        );
        let payloads = collect_full_power_payloads(&mut caster_helper);
        assert!(
            !payloads
                .iter()
                .any(|p| matches!(p, ServerDataPayloadV1::FullPowerExhaustedState(_))),
            "不应再发 FullPowerExhaustedState payload（虚脱改由 status_snapshot 驱动）"
        );
    }
}
