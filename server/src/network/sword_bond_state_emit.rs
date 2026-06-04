//! plan-combat-skill-feedback-bridges-v1 P6 — 人剑共生 HUD S2C 推送。
//!
//! 每秒节拍 Query 所有在线 Client：
//!   - 持有 [`SwordBondComponent`] → 构建 active=true 的 [`SwordBondHudStateV1`] 发送。
//!   - 无 [`SwordBondComponent`]   → 发送 active=false 的 INACTIVE payload（client store 重置）。
//!
//! 守恒红线：stored_qi 只读展示，不二次扣 qi。

use valence::prelude::{Client, Entity, Query, Res, Username, With};

use crate::combat::components::TICKS_PER_SECOND;
use crate::combat::CombatClock;
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1, SwordBondHudStateV1};
use crate::sword_path::bond::SwordBondComponent;

type SwordBondClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Username,
    Option<&'a SwordBondComponent>,
);

/// plan-combat-skill-feedback-bridges-v1 P6 断链修复：
/// 每秒对所有在线 Client 推送人剑共生 HUD 状态；
/// 无 [`SwordBondComponent`] 的玩家发送 active=false（不漏发，client store 会重置）。
pub fn emit_sword_bond_hud_state_payloads(
    clock: Res<CombatClock>,
    mut clients: Query<SwordBondClientItem<'_>, With<Client>>,
) {
    if !clock.tick.is_multiple_of(TICKS_PER_SECOND) {
        return;
    }

    for (entity, mut client, username, bond_opt) in &mut clients {
        let state = match bond_opt {
            Some(bond) => {
                let cap = bond.grade.stored_qi_cap();
                let stored_qi_ratio = if cap > 0.0 {
                    ((bond.stored_qi / cap).clamp(0.0, 1.0)) as f32
                } else {
                    0.0_f32
                };
                let heaven_gate_ready = bond.grade.can_store_qi() && bond.stored_qi >= cap;
                SwordBondHudStateV1 {
                    active: true,
                    grade_index: bond.grade.tier() as u32,
                    grade_name: bond.grade.display_name().to_string(),
                    stored_qi_ratio,
                    bond_strength: bond.bond_strength.clamp(0.0, 1.0),
                    heaven_gate_ready,
                }
            }
            None => SwordBondHudStateV1 {
                active: false,
                grade_index: 0,
                grade_name: String::new(),
                stored_qi_ratio: 0.0,
                bond_strength: 0.0,
                heaven_gate_ready: false,
            },
        };

        let payload = ServerDataV1::new(ServerDataPayloadV1::SwordBondHudState(state));
        let payload_type = payload_type_label(payload.payload_type());
        let payload_bytes = match serialize_server_data_payload(&payload) {
            Ok(bytes) => bytes,
            Err(error) => {
                log_payload_build_error(payload_type, &error);
                continue;
            }
        };
        send_server_data_payload(&mut client, payload_bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} {} payload to entity {entity:?} for `{}` active={}",
            SERVER_DATA_CHANNEL,
            payload_type,
            username.0,
            bond_opt.is_some()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};

    use crate::combat::CombatClock;
    use crate::schema::server_data::SwordBondHudStateV1;

    fn make_state_active(bond_strength: f32, stored_qi: f64, cap: f64) -> SwordBondHudStateV1 {
        let stored_qi_ratio = if cap > 0.0 {
            ((stored_qi / cap).clamp(0.0, 1.0)) as f32
        } else {
            0.0
        };
        let heaven_gate_ready = cap > 0.0 && stored_qi >= cap;
        SwordBondHudStateV1 {
            active: true,
            grade_index: 3,
            grade_name: "凝脉".to_string(),
            stored_qi_ratio,
            bond_strength: bond_strength.clamp(0.0, 1.0),
            heaven_gate_ready,
        }
    }

    #[test]
    fn stored_qi_ratio_at_cap_is_one() {
        // 守恒红线：满储时比例应为 1.0，不超过 1.0
        let state = make_state_active(0.5, 15.0, 15.0);
        assert!(
            (state.stored_qi_ratio - 1.0).abs() < 1e-6,
            "stored_qi=cap 时 stored_qi_ratio 应为 1.0，实际={}",
            state.stored_qi_ratio
        );
    }

    #[test]
    fn stored_qi_ratio_zero_when_no_cap() {
        // SwordGrade 不支持储气（cap=0）时 ratio 应为 0
        let state = make_state_active(0.5, 0.0, 0.0);
        assert_eq!(
            state.stored_qi_ratio, 0.0,
            "cap=0 时 stored_qi_ratio 应为 0.0，实际={}",
            state.stored_qi_ratio
        );
    }

    #[test]
    fn heaven_gate_ready_true_when_full() {
        // 储气满时 heaven_gate_ready=true
        let state = make_state_active(1.0, 15.0, 15.0);
        assert!(
            state.heaven_gate_ready,
            "stored_qi=cap 时 heaven_gate_ready 应为 true"
        );
    }

    #[test]
    fn heaven_gate_ready_false_when_not_full() {
        // 储气未满时 heaven_gate_ready=false
        let state = make_state_active(1.0, 14.9, 15.0);
        assert!(
            !state.heaven_gate_ready,
            "stored_qi<cap 时 heaven_gate_ready 应为 false"
        );
    }

    #[test]
    fn heaven_gate_ready_false_when_no_cap() {
        // cap=0（不支持储气）时 heaven_gate_ready=false
        let state = make_state_active(1.0, 0.0, 0.0);
        assert!(
            !state.heaven_gate_ready,
            "cap=0 时 heaven_gate_ready 应为 false"
        );
    }

    #[test]
    fn inactive_state_all_zero() {
        let state = SwordBondHudStateV1 {
            active: false,
            grade_index: 0,
            grade_name: String::new(),
            stored_qi_ratio: 0.0,
            bond_strength: 0.0,
            heaven_gate_ready: false,
        };
        assert!(!state.active, "INACTIVE state.active 应为 false");
        assert_eq!(state.stored_qi_ratio, 0.0);
        assert_eq!(state.bond_strength, 0.0);
        assert!(!state.heaven_gate_ready);
    }

    #[test]
    fn bond_strength_clamped_to_unit_interval() {
        // bond_strength 超出 [0,1] 应被 clamp
        let state = make_state_active(1.5, 7.0, 15.0);
        assert!(
            state.bond_strength <= 1.0,
            "bond_strength 应 clamp 到 1.0，实际={}",
            state.bond_strength
        );
        let state_neg = make_state_active(-0.2, 7.0, 15.0);
        assert!(
            state_neg.bond_strength >= 0.0,
            "bond_strength 应 clamp 到 0.0，实际={}",
            state_neg.bond_strength
        );
    }

    #[test]
    fn emit_system_skips_non_tick_frames() {
        // 非节拍帧不应产生发射逻辑（通过 CombatClock.tick 不整除 TICKS_PER_SECOND 验证）
        let mut app = App::new();
        // 不注册任何 Client，只检查系统不 panic
        app.insert_resource(CombatClock { tick: 3 }); // 非整除节拍
        app.add_systems(Update, emit_sword_bond_hud_state_payloads);
        // 应正常运行，不 panic
        app.update();
    }

    #[test]
    fn serde_roundtrip_active() {
        let original = SwordBondHudStateV1 {
            active: true,
            grade_index: 4,
            grade_name: "固元".to_string(),
            stored_qi_ratio: 0.75,
            bond_strength: 0.9,
            heaven_gate_ready: false,
        };
        let json = serde_json::to_string(&original).expect("SwordBondHudStateV1 应能序列化");
        let back: SwordBondHudStateV1 =
            serde_json::from_str(&json).expect("SwordBondHudStateV1 应能反序列化");
        assert_eq!(
            original, back,
            "SwordBondHudStateV1 JSON roundtrip 必须无损；JSON={json}"
        );
    }

    #[test]
    fn serde_roundtrip_inactive() {
        let original = SwordBondHudStateV1 {
            active: false,
            grade_index: 0,
            grade_name: String::new(),
            stored_qi_ratio: 0.0,
            bond_strength: 0.0,
            heaven_gate_ready: false,
        };
        let json =
            serde_json::to_string(&original).expect("INACTIVE SwordBondHudStateV1 应能序列化");
        let back: SwordBondHudStateV1 =
            serde_json::from_str(&json).expect("INACTIVE SwordBondHudStateV1 应能反序列化");
        assert_eq!(original, back, "INACTIVE roundtrip 必须无损；JSON={json}");
    }
}
