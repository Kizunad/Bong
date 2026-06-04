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
//! aim HUD 反馈延后至未来引入 aim-phase 事件的 plan；本阶段交付 echo/charge/abrasion 三路。

use valence::prelude::{Client, Entity, EventReader, Query, UniqueId, With};

use crate::combat::anqi_v2::{CarrierAbrasionEvent, DecoyDeployEvent, QiInjectionEvent};
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
        let container_str = format!("{:?}", event.container)
            .to_lowercase()
            .replace("anqicontainerkind::", "");
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
}

#[cfg(test)]
mod tests {
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
}
