use super::*;

#[test]
fn anqi_hud_invalid_outbound_values_fail_serialization() {
    let invalid_payloads = [
        AnqiHudV1 {
            kind: AnqiHudKindV1::Aim,
            echo_count: 0,
            aim_progress: f64::NAN,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Charge,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: f64::INFINITY,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Abrasion,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: -1.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Multishot,
            echo_count: ANQI_HUD_ECHO_COUNT_MAX + 1,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Abrasion,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: "unknown".to_string(),
            abrasion_qi_payload: 0.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Abrasion,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: "quiver".to_string(),
            abrasion_qi_payload: ANQI_HUD_QI_PAYLOAD_MAX * 2.0,
            tick: 0,
        },
        AnqiHudV1 {
            kind: AnqiHudKindV1::Echo,
            echo_count: 0,
            aim_progress: 0.0,
            charge_progress: 0.0,
            abrasion_container: String::new(),
            abrasion_qi_payload: 0.0,
            tick: ANQI_HUD_TICK_MAX + 1,
        },
    ];

    for payload in invalid_payloads {
        let wrapper = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(payload));
        assert!(
            serde_json::to_value(wrapper).is_err(),
            "invalid outbound anqi_hud payload must fail serialization"
        );
    }
}
