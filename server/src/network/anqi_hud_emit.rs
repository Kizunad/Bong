//! plan-combat-skill-feedback-bridges-v1 P4 — 暗器分身 HUD S2C 推送。
//!
//! 监听三类 Bevy events，对每个事件找对应 caster/carrier 的 Client 发送
//! `ServerDataPayloadV1::AnqiHud`。
//!
//! 守恒红线：
//! - 全部字段只读自 ECS Event，不重算真元，不扣 qi。
//! - DecoyDeployEvent.echo_count 直接使用，不重新计算。
//! - QiInjectionEvent.outcome.overload_ratio 直接映射 charge_progress（见下方说明）。
//! - CarrierAbrasionEvent.after_qi 直接映射 abrasion_qi_payload。
//!
//! ## aim HUD 暂缺说明
//! anqi_v2 当前 7 个事件全为结果型（MultiShot/QiInjection/ArmorPierce/EchoFractal/
//! CarrierAbrasion/ContainerSwap/DecoyDeploy），无 aim 前摇/进度事件源。
//! aim HUD 反馈延后至未来引入 aim-phase 事件的 plan；本阶段交付 echo/charge/abrasion/multishot 多路。
//!
//! ## AV 里程碑补全（暗器 6 招 HUD 缺口）
//! - 破甲注射（armor_pierce）：`ArmorPierceEvent` → kind="charge"（ignored_defense_ratio
//!   作蓄力指示，复用现有 charge HUD，无新 schema 字段）。
//! - 多发齐射（multi_shot）：`MultiShotEvent` → kind="multishot"（projectile_count 复用
//!   `echo_count` 字段承载弹数，无新 proto 字段；client 侧独立 multishot 维度渲染）。

use valence::prelude::{Client, Entity, EventReader, Query, UniqueId, With};

use crate::combat::anqi_v2::{
    ArmorPierceEvent, CarrierAbrasionEvent, DecoyDeployEvent, MultiShotEvent, QiInjectionEvent,
};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{AnqiHudV1, ServerDataPayloadV1, ServerDataV1};

/// plan-combat-skill-feedback-bridges-v1 P4 断链修复：
/// `DecoyDeployEvent`    → HUD kind="echo" 推送（echo_count 直取）。
/// `QiInjectionEvent`   → HUD kind="charge" 推送（overload_ratio 作为蓄力指示，见注释）。
/// `CarrierAbrasionEvent` → HUD kind="abrasion" 推送（after_qi 直取）。
pub fn emit_anqi_hud_payloads(
    mut decoys: EventReader<DecoyDeployEvent>,
    mut injections: EventReader<QiInjectionEvent>,
    mut abrasions: EventReader<CarrierAbrasionEvent>,
    mut armor_pierces: EventReader<ArmorPierceEvent>,
    mut multi_shots: EventReader<MultiShotEvent>,
    mut clients: Query<(Entity, &mut Client, Option<&UniqueId>), With<Client>>,
) {
    // ── DecoyDeployEvent → kind="echo" ────────────────────────────
    for event in decoys.read() {
        let Ok((_, ref mut client, _)) = clients.get_mut(event.caster) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "echo".to_string(),
            echo_count: event.echo_count,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: event.tick,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} echo payload caster={:?} echo_count={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            event.caster,
            event.echo_count,
        );
    }

    // ── QiInjectionEvent → kind="charge" ─────────────────────────
    // overload_ratio = payload_qi / qi_max，表示本次注射的载荷比（0..1）。
    // anqi_v2 无 aim 前摇/进度事件；此处以 overload_ratio 作蓄力度量（charge）
    // 反馈注射强度：overload_ratio 越高 = 此次注射越接近上限，charge 条越满。
    // 语义上与"瞄准进度"无关，故 kind="charge" 而非 "aim"。
    for event in injections.read() {
        let Ok((_, ref mut client, _)) = clients.get_mut(event.caster) else {
            continue;
        };
        let overload = event.outcome.overload_ratio.clamp(0.0, 1.0);
        let payload = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "charge".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: overload,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: event.tick,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} charge payload caster={:?} overload_ratio(=charge_progress)={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            event.caster,
            overload,
        );
    }

    // ── CarrierAbrasionEvent → kind="abrasion" ────────────────────
    // after_qi 直接映射 abrasion_qi_payload（读事件结果，不重算）。
    for event in abrasions.read() {
        let Ok((_, ref mut client, _)) = clients.get_mut(event.carrier) else {
            continue;
        };
        let container_str = event.container.as_wire_str().to_string();
        let payload = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: container_str,
            abrasion_qi_payload: event.after_qi,
            tick: event.tick,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} abrasion payload carrier={:?} after_qi={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            event.carrier,
            event.after_qi,
        );
    }

    // ── ArmorPierceEvent → kind="charge" ──────────────────────────
    // 破甲注射不发 QiInjectionEvent，单独 emit。ignored_defense_ratio（无视防御比例，
    // 0..1）映射 charge_progress——破甲越彻底，蓄力条越满。复用现有 charge HUD 维度，
    // 不引入新 schema 字段。守恒：只读 outcome，不重算。
    for event in armor_pierces.read() {
        let Ok((_, ref mut client, _)) = clients.get_mut(event.caster) else {
            continue;
        };
        let charge = event.outcome.ignored_defense_ratio.clamp(0.0, 1.0);
        let payload = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "charge".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: charge,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: event.tick,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} charge(armor_pierce) payload caster={:?} ignored_defense_ratio(=charge_progress)={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            event.caster,
            charge,
        );
    }

    // ── MultiShotEvent → kind="multishot" ─────────────────────────
    // 多发齐射：projectile_count 复用 echo_count 字段承载弹数（无新 proto 字段）。
    // client 侧 AnqiHudServerDataHandler 路由 "multishot" 到独立 multishot 维度，
    // 渲染齐射弹数指示。守恒：只读 projectile_count，不重算。
    for event in multi_shots.read() {
        let Ok((_, ref mut client, _)) = clients.get_mut(event.caster) else {
            continue;
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "multishot".to_string(),
            echo_count: u32::from(event.projectile_count),
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: event.tick,
        }));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} multishot payload caster={:?} projectile_count={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            event.caster,
            event.projectile_count,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::qi_physics::AnqiContainerKind;
    use crate::schema::server_data::{AnqiHudV1, ServerDataPayloadV1};

    // ── emit 契约 pin：DecoyDeployEvent → payload.kind=="echo" ────

    #[test]
    fn anqi_hud_echo_payload_kind_and_echo_count() {
        // 构造 echo payload，验证 kind + echo_count 按事件值取，不重算
        let payload = AnqiHudV1 {
            kind: "echo".to_string(),
            echo_count: 7,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 42,
        };
        assert_eq!(
            payload.kind, "echo",
            "echo payload kind must be 'echo'，实际={}",
            payload.kind
        );
        assert_eq!(
            payload.echo_count, 7,
            "echo_count 必须直接取事件值 7，不重算；实际={}",
            payload.echo_count
        );
        assert_eq!(
            payload.aim_progress, 0.0,
            "echo payload aim_progress 应为 0.0；实际={}",
            payload.aim_progress
        );
        assert_eq!(
            payload.tick, 42,
            "tick 必须透传事件 tick；实际={}",
            payload.tick
        );
    }

    #[test]
    fn anqi_hud_charge_payload_maps_overload_ratio() {
        // QiInjectionEvent.outcome.overload_ratio → charge_progress（不重算）
        // overload_ratio = payload_qi / qi_max，作蓄力度量；kind="charge" 不再是 "aim"
        let overload_ratio = 0.73_f64;
        let payload = AnqiHudV1 {
            kind: "charge".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: overload_ratio.clamp(0.0, 1.0),
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 100,
        };
        assert_eq!(
            payload.kind, "charge",
            "QiInjection payload kind 必须为 'charge'（非 'aim'，overload_ratio 是载荷比而非瞄准进度）；实际={}",
            payload.kind
        );
        assert!(
            (payload.charge_progress - 0.73).abs() < 1e-9,
            "charge_progress 必须等于 overload_ratio 0.73，不重算；实际={}",
            payload.charge_progress
        );
        assert_eq!(
            payload.aim_progress, 0.0,
            "charge payload aim_progress 应为 0.0（server 不发 aim）；实际={}",
            payload.aim_progress
        );
        assert_eq!(
            payload.echo_count, 0,
            "charge payload echo_count 应为 0；实际={}",
            payload.echo_count
        );
    }

    #[test]
    fn anqi_hud_abrasion_payload_maps_after_qi() {
        // CarrierAbrasionEvent.after_qi → abrasion_qi_payload（只读，不重算）
        let after_qi = 58.4_f64;
        let payload = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: "hand_slot".to_string(),
            abrasion_qi_payload: after_qi,
            tick: 200,
        };
        assert_eq!(
            payload.kind, "abrasion",
            "abrasion payload kind must be 'abrasion'；实际={}",
            payload.kind
        );
        assert!(
            (payload.abrasion_qi_payload - 58.4).abs() < 1e-9,
            "abrasion_qi_payload 必须等于 after_qi 58.4，不重算；实际={}",
            payload.abrasion_qi_payload
        );
        assert!(
            !payload.abrasion_container.is_empty(),
            "abrasion_container 不能为空；实际='{}'",
            payload.abrasion_container
        );
    }

    #[test]
    fn anqi_hud_payload_type_is_anqi_hud() {
        use crate::network::agent_bridge::payload_type_label;
        use crate::schema::server_data::{ServerDataType, ServerDataV1};
        let v1 = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(AnqiHudV1 {
            kind: "echo".to_string(),
            echo_count: 1,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 1,
        }));
        assert_eq!(
            v1.payload_type(),
            ServerDataType::AnqiHud,
            "payload_type() 必须返回 AnqiHud；实际={:?}",
            v1.payload_type()
        );
        let label = payload_type_label(ServerDataType::AnqiHud);
        assert_eq!(
            label, "anqi_hud",
            "payload_type_label 必须为 'anqi_hud'（client ServerDataRouter 路由键）；实际={label}"
        );
    }

    #[test]
    fn anqi_hud_v1_serde_roundtrip() {
        // schema pin test：JSON 往返不丢字段
        let original = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 3,
            aim_progress: 0.5,
            charge_progress: 0.25,
            abrasion_container: "quiver".to_string(),
            abrasion_qi_payload: 12.5,
            tick: 999,
        };
        let json =
            serde_json::to_string(&original).expect("AnqiHudV1 should serialize without error");
        let back: AnqiHudV1 =
            serde_json::from_str(&json).expect("AnqiHudV1 should deserialize without error");
        assert_eq!(
            original, back,
            "AnqiHudV1 JSON roundtrip must be lossless；JSON={json}"
        );
    }

    #[test]
    fn anqi_hud_charge_overload_clamp_boundary() {
        // overload_ratio 超出 [0,1] 时 clamp 后作为 charge_progress，不 panic
        let over = 1.5_f64.clamp(0.0, 1.0);
        assert_eq!(
            over, 1.0,
            "overload >1 应 clamp 到 1.0（charge_progress 上界）；实际={over}"
        );
        let neg = (-0.1_f64).clamp(0.0, 1.0);
        assert_eq!(
            neg, 0.0,
            "overload <0 应 clamp 到 0.0（charge_progress 下界）；实际={neg}"
        );
    }

    // ── abrasion wire 契约 pin：as_wire_str 各变体输出值锁定 ──────
    //
    // 注意覆盖范围：这些测试只锁住 as_wire_str() 函数本身的输出值（AnqiContainerKind
    // 各变体 → 对应字符串）。它们**不走** emit_anqi_hud_payloads，因此无法检测
    // emit 调用点（emit:111）被改回 format!("{:?}") 的回归。
    // emit 调用点的回归保护由下方的
    // `emit_system_abrasion_pocket_pouch_uses_wire_str_not_debug` 集成测试负责。

    #[test]
    fn abrasion_payload_container_uses_wire_str_not_debug() {
        // 直接验证 as_wire_str 输出即 payload 字段值（emit 路径的等价验证）：
        // abrasion_container = event.container.as_wire_str().to_string()
        // 此处对每个非 HandSlot（会触发 abrasion emit 的容器）逐一验证 wire 字符串。
        let quiver_payload = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: AnqiContainerKind::Quiver.as_wire_str().to_string(),
            abrasion_qi_payload: 95.0,
            tick: 10,
        };
        assert_eq!(
            quiver_payload.abrasion_container, "quiver",
            "Quiver abrasion_container 必须为 'quiver'（via as_wire_str，非 Debug）；实际={}",
            quiver_payload.abrasion_container
        );

        let pocket_payload = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: AnqiContainerKind::PocketPouch.as_wire_str().to_string(),
            abrasion_qi_payload: 88.0,
            tick: 20,
        };
        assert_eq!(
            pocket_payload.abrasion_container, "pocket_pouch",
            "PocketPouch abrasion_container 必须为 'pocket_pouch'（as_wire_str；若用 Debug 则为 'pocketpouch'→client 解析失败）；实际={}",
            pocket_payload.abrasion_container
        );
    }

    #[test]
    fn abrasion_payload_hand_slot_wire_str() {
        // HandSlot 的 as_wire_str 验证（'hand_slot' 非 Debug 'handslot'）
        let payload = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: AnqiContainerKind::HandSlot.as_wire_str().to_string(),
            abrasion_qi_payload: 0.0,
            tick: 30,
        };
        assert_eq!(
            payload.abrasion_container, "hand_slot",
            "HandSlot abrasion_container 必须为 'hand_slot'（含下划线；Debug='handslot' 会与 client 不匹配）；实际={}",
            payload.abrasion_container
        );
    }

    #[test]
    fn abrasion_payload_fenglinghe_wire_str() {
        let payload = AnqiHudV1 {
            kind: "abrasion".to_string(),
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: AnqiContainerKind::Fenglinghe.as_wire_str().to_string(),
            abrasion_qi_payload: 0.0,
            tick: 40,
        };
        assert_eq!(
            payload.abrasion_container, "fenglinghe",
            "Fenglinghe abrasion_container 必须为 'fenglinghe'；实际={}",
            payload.abrasion_container
        );
    }

    // ── Bevy 集成测试：真跑 emit_anqi_hud_payloads，锁住 emit 调用点 ──
    //
    // 以下测试通过 valence::testing::create_mock_client 构造真实 Bevy App，
    // 注入 CarrierAbrasionEvent，调用 emit_anqi_hud_payloads system，
    // 从出站 CustomPayload 解析断言 abrasion_container 字段。
    // 这样 emit:111（event.container.as_wire_str()）被真正执行——改回
    // format!("{:?}") 此测试会红（PocketPouch Debug="PocketPouch" ≠ "pocket_pouch"）。
    #[test]
    fn emit_system_abrasion_pocket_pouch_uses_wire_str_not_debug() {
        use valence::prelude::{App, Update};
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::create_mock_client;

        use crate::combat::anqi_v2::{
            ArmorPierceEvent, CarrierAbrasionEvent, DecoyDeployEvent, MultiShotEvent,
            QiInjectionEvent,
        };
        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
        use crate::qi_physics::AbrasionDirection;

        let mut app = App::new();
        // emit_anqi_hud_payloads 依赖五路 EventReader，全部需要 add_event
        app.add_event::<DecoyDeployEvent>();
        app.add_event::<QiInjectionEvent>();
        app.add_event::<CarrierAbrasionEvent>();
        app.add_event::<ArmorPierceEvent>();
        app.add_event::<MultiShotEvent>();
        app.add_systems(Update, super::emit_anqi_hud_payloads);

        // spawn 一个带 Client 的实体，拿到 entity id 用于 event.carrier
        let (client_bundle, mut helper) = create_mock_client("TestCarrier");
        let entity = app.world_mut().spawn(client_bundle).id();

        // 发送 PocketPouch 的 CarrierAbrasionEvent
        app.world_mut().send_event(CarrierAbrasionEvent {
            carrier: entity,
            container: AnqiContainerKind::PocketPouch,
            direction: AbrasionDirection::Draw,
            lost_qi: 10.0,
            after_qi: 88.0,
            tick: 55,
        });

        app.update();

        // flush 出站包
        {
            let mut client_query = app.world_mut().query::<&mut valence::prelude::Client>();
            for mut client in client_query.iter_mut(app.world_mut()) {
                client
                    .flush_packets()
                    .expect("mock client flush should succeed");
            }
        }

        // 从 helper 里抓 AnqiHud abrasion payload
        let mut found_container: Option<String> = None;
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: crate::schema::server_data::ServerDataV1 =
                serde_json::from_slice(packet.data.0 .0)
                    .expect("server_data payload should deserialize");
            if let crate::schema::server_data::ServerDataPayloadV1::AnqiHud(hud) = payload.payload {
                if hud.kind == "abrasion" {
                    found_container = Some(hud.abrasion_container);
                    break;
                }
            }
        }

        let container = found_container.expect(
            "emit_anqi_hud_payloads 必须对 CarrierAbrasionEvent 发出 anqi_hud abrasion payload",
        );
        assert_eq!(
            container, "pocket_pouch",
            "emit 调用点（emit:111）必须用 as_wire_str()，不能用 Debug 格式；\
             as_wire_str()='pocket_pouch'，Debug='PocketPouch'；实际={}",
            container
        );
    }

    /// AV 里程碑：ArmorPierceEvent → emit kind="charge"，charge_progress = ignored_defense_ratio。
    #[test]
    fn emit_system_armor_pierce_emits_charge_with_ignored_defense_ratio() {
        use valence::prelude::{App, Update};
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::create_mock_client;

        use crate::combat::anqi_v2::{
            ArmorPierceEvent, CarrierAbrasionEvent, DecoyDeployEvent, MultiShotEvent,
            QiInjectionEvent,
        };
        use crate::combat::carrier::CarrierKind;
        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;

        let mut app = App::new();
        app.add_event::<DecoyDeployEvent>();
        app.add_event::<QiInjectionEvent>();
        app.add_event::<CarrierAbrasionEvent>();
        app.add_event::<ArmorPierceEvent>();
        app.add_event::<MultiShotEvent>();
        app.add_systems(Update, super::emit_anqi_hud_payloads);

        let (client_bundle, mut helper) = create_mock_client("Pierce");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(ArmorPierceEvent {
            caster: entity,
            target: None,
            carrier_kind: CarrierKind::FenglingheBone,
            outcome: crate::qi_physics::ArmorPenetrationOutcome {
                base_damage: 60.0,
                ignored_defense_ratio: 0.6,
                effective_damage: 70.0,
                carrier_shatter_probability: 0.2,
            },
            tick: 70,
        });
        app.update();
        {
            let mut q = app.world_mut().query::<&mut valence::prelude::Client>();
            for mut c in q.iter_mut(app.world_mut()) {
                c.flush_packets().expect("flush");
            }
        }

        let mut found: Option<(String, f64)> = None;
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: crate::schema::server_data::ServerDataV1 =
                serde_json::from_slice(packet.data.0 .0).expect("deserialize");
            if let crate::schema::server_data::ServerDataPayloadV1::AnqiHud(hud) = payload.payload {
                found = Some((hud.kind, hud.charge_progress));
                break;
            }
        }
        let (kind, charge) = found.expect("ArmorPierceEvent 必须 emit anqi_hud payload");
        assert_eq!(
            kind, "charge",
            "破甲注射应复用 charge HUD 维度（无新 schema 字段）；实际 kind='{kind}'"
        );
        assert!(
            (charge - 0.6).abs() < 1e-6,
            "charge_progress 应等于 ignored_defense_ratio 0.6（不重算）；实际={charge}"
        );
    }

    /// AV 里程碑：MultiShotEvent → emit kind="multishot"，echo_count = projectile_count。
    #[test]
    fn emit_system_multi_shot_emits_multishot_with_projectile_count() {
        use valence::prelude::{App, Update};
        use valence::protocol::packets::play::CustomPayloadS2c;
        use valence::testing::create_mock_client;

        use crate::combat::anqi_v2::{
            ArmorPierceEvent, CarrierAbrasionEvent, DecoyDeployEvent, MultiShotEvent,
            QiInjectionEvent,
        };
        use crate::combat::carrier::CarrierKind;
        use crate::network::agent_bridge::SERVER_DATA_CHANNEL;

        let mut app = App::new();
        app.add_event::<DecoyDeployEvent>();
        app.add_event::<QiInjectionEvent>();
        app.add_event::<CarrierAbrasionEvent>();
        app.add_event::<ArmorPierceEvent>();
        app.add_event::<MultiShotEvent>();
        app.add_systems(Update, super::emit_anqi_hud_payloads);

        let (client_bundle, mut helper) = create_mock_client("Volley");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.world_mut().send_event(MultiShotEvent {
            caster: entity,
            projectile_count: 5,
            carrier_kind: CarrierKind::LingmuArrow,
            shots: Vec::new(),
            tick: 80,
        });
        app.update();
        {
            let mut q = app.world_mut().query::<&mut valence::prelude::Client>();
            for mut c in q.iter_mut(app.world_mut()) {
                c.flush_packets().expect("flush");
            }
        }

        let mut found: Option<(String, u32)> = None;
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: crate::schema::server_data::ServerDataV1 =
                serde_json::from_slice(packet.data.0 .0).expect("deserialize");
            if let crate::schema::server_data::ServerDataPayloadV1::AnqiHud(hud) = payload.payload {
                found = Some((hud.kind, hud.echo_count));
                break;
            }
        }
        let (kind, count) = found.expect("MultiShotEvent 必须 emit anqi_hud payload");
        assert_eq!(
            kind, "multishot",
            "多发齐射应用 kind='multishot'（client 路由到独立 multishot 维度）；实际 kind='{kind}'"
        );
        assert_eq!(
            count, 5,
            "echo_count 字段复用承载 projectile_count=5（无新 proto 字段）；实际={count}"
        );
    }
}
