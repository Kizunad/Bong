//! plan-alchemy-combat-v1 P2 — status_snapshot emit。
//!
//! 客户端已有 `StatusSnapshotHandler` / `StatusEffectStore`；这里把 server
//! `StatusEffects` 的变化转成同一 wire shape，避免为战场丹药另建 HUD 通道。

use valence::prelude::{Changed, Client, Entity, Query, Username, With};

use crate::combat::components::{BodyPart, StatusEffects};
use crate::combat::events::StatusEffectKind;
use crate::cultivation::tick::cultivation_acceleration_multiplier;
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::network::send_server_data_payload;

type StatusSnapshotEmitFilter = (With<Client>, Changed<StatusEffects>);

pub fn emit_status_snapshot_payloads(
    mut clients: Query<(Entity, &mut Client, &Username, &StatusEffects), StatusSnapshotEmitFilter>,
) {
    for (entity, mut client, username, status_effects) in &mut clients {
        let effects = status_effects
            .active
            .iter()
            .filter(|effect| effect.remaining_ticks > 0)
            .map(|effect| {
                serde_json::json!({
                    "id": status_effect_id(&effect.kind),
                    "name": status_effect_name(&effect.kind),
                    "kind": status_effect_category(&effect.kind),
                    "stacks": 1,
                    "remaining_ms": effect.remaining_ticks.saturating_mul(50),
                    "source_color": status_effect_color(&effect.kind),
                    "source_label": status_effect_source_label(&effect.kind),
                    "dispel": status_effect_dispel(&effect.kind)
                })
            })
            .collect::<Vec<_>>();
        // plan-cultivation-pacing-v1 P2.3：聚合修炼加速倍率，供 client HUD 显示。
        let accel_mult = cultivation_acceleration_multiplier(status_effects);
        let payload = serde_json::json!({
            "v": 1,
            "type": "status_snapshot",
            "effects": effects,
            "cultivation_acceleration": accel_mult
        });
        let Ok(bytes) = serde_json::to_vec(&payload) else {
            continue;
        };
        send_server_data_payload(&mut client, bytes.as_slice());
        tracing::debug!(
            "[bong][network] sent {} status_snapshot payload to entity {entity:?} for `{}` ({} effects)",
            SERVER_DATA_CHANNEL,
            username.0,
            status_effects.active.len()
        );
    }
}

fn status_effect_id(kind: &StatusEffectKind) -> String {
    match kind {
        StatusEffectKind::QiRegenPaused => "qi_regen_paused".to_string(),
        StatusEffectKind::AlchemyBuff(tag) => format!("alchemy_buff:{tag}"),
        StatusEffectKind::BodyPartResist(part) => {
            format!("body_part_resist:{}", body_part_wire(*part))
        }
        StatusEffectKind::BodyPartWeaken(part) => {
            format!("body_part_weaken:{}", body_part_wire(*part))
        }
        StatusEffectKind::HealthRegenBoost => "health_regen_boost".to_string(),
        StatusEffectKind::MirrorConcealment => "mirror_concealment".to_string(),
        StatusEffectKind::MirrorExposed => "mirror_exposed".to_string(),
        StatusEffectKind::SpiritTreasurePerception => "spirit_treasure_perception".to_string(),
        other => format!("{other:?}").to_ascii_lowercase(),
    }
}

fn status_effect_name(kind: &StatusEffectKind) -> String {
    match kind {
        StatusEffectKind::Bleeding => "流血".to_string(),
        StatusEffectKind::Slowed => "迟缓".to_string(),
        StatusEffectKind::Stunned => "僵直".to_string(),
        StatusEffectKind::DamageAmp => "伤害放大".to_string(),
        StatusEffectKind::DamageReduction => "减伤".to_string(),
        StatusEffectKind::BreakthroughBoost => "破境助力".to_string(),
        StatusEffectKind::Humility => "谦抑".to_string(),
        StatusEffectKind::InsightHallucination => "幻觉".to_string(),
        StatusEffectKind::VortexCasting => "绝灵持涡".to_string(),
        StatusEffectKind::AntiSpiritPressurePill => "抗灵压".to_string(),
        StatusEffectKind::Frailty => "风烛".to_string(),
        StatusEffectKind::QiRegenBoost => "回气变化".to_string(),
        StatusEffectKind::InsightFlash => "顿悟闪念".to_string(),
        StatusEffectKind::QiCapPermMinus => "真元上限折损".to_string(),
        StatusEffectKind::ContaminationBoost => "丹毒加重".to_string(),
        StatusEffectKind::AlchemyBuff(tag) => format!("丹药副效：{tag}"),
        StatusEffectKind::ParryRecovery => "截脉收势".to_string(),
        StatusEffectKind::SwordParrying => "剑格架势".to_string(),
        StatusEffectKind::ShieldBlocking => "盾牌格挡".to_string(),
        StatusEffectKind::Staggered => "反震硬直".to_string(),
        StatusEffectKind::Disoriented => "迷乱".to_string(),
        StatusEffectKind::WoundHeal => "伤口回稳".to_string(),
        StatusEffectKind::BodyPartResist(part) => format!("{}硬化", body_part_name(*part)),
        StatusEffectKind::BodyPartWeaken(part) => format!("{}脆弱", body_part_name(*part)),
        StatusEffectKind::SpeedBoost => "疾行".to_string(),
        StatusEffectKind::StaminaRecovBoost => "回力".to_string(),
        StatusEffectKind::HealthRegenBoost => "养伤".to_string(),
        StatusEffectKind::StaminaCrash => "体力虚脱".to_string(),
        StatusEffectKind::QiDrainForStamina => "真元换体".to_string(),
        StatusEffectKind::LegStrain => "腿部应力伤".to_string(),
        StatusEffectKind::QiRegenPaused => "真元停滞".to_string(),
        StatusEffectKind::MirrorConcealment => "镜隐".to_string(),
        StatusEffectKind::MirrorExposed => "镜照暴露".to_string(),
        StatusEffectKind::SpiritTreasurePerception => "灵宝感知".to_string(),
        StatusEffectKind::ResonanceLocked => "共振锁定".to_string(),
        StatusEffectKind::VoidCoreActive => "虚心坍缩".to_string(),
        StatusEffectKind::CultivationAcceleration => "修炼加速".to_string(),
        StatusEffectKind::QiRegenSlowed => "回气减速".to_string(),
        StatusEffectKind::DamageVulnerability => "脆弱易伤".to_string(),
        StatusEffectKind::ExtraordinaryMeridianAcceleration => "奇经加速".to_string(),
    }
}

fn status_effect_category(kind: &StatusEffectKind) -> &'static str {
    match kind {
        StatusEffectKind::Bleeding => "dot",
        StatusEffectKind::Stunned
        | StatusEffectKind::VortexCasting
        | StatusEffectKind::ParryRecovery
        | StatusEffectKind::Staggered
        | StatusEffectKind::Disoriented
        | StatusEffectKind::VoidCoreActive => "control",
        StatusEffectKind::DamageReduction
        | StatusEffectKind::BreakthroughBoost
        | StatusEffectKind::AntiSpiritPressurePill
        | StatusEffectKind::QiRegenBoost
        | StatusEffectKind::InsightFlash
        | StatusEffectKind::WoundHeal
        | StatusEffectKind::BodyPartResist(_)
        | StatusEffectKind::SpeedBoost
        | StatusEffectKind::StaminaRecovBoost
        | StatusEffectKind::HealthRegenBoost
        | StatusEffectKind::MirrorConcealment
        | StatusEffectKind::SwordParrying
        | StatusEffectKind::ShieldBlocking
        | StatusEffectKind::SpiritTreasurePerception
        | StatusEffectKind::CultivationAcceleration
        | StatusEffectKind::ExtraordinaryMeridianAcceleration => "buff",
        StatusEffectKind::Slowed
        | StatusEffectKind::DamageAmp
        | StatusEffectKind::Humility
        | StatusEffectKind::InsightHallucination
        | StatusEffectKind::Frailty
        | StatusEffectKind::QiCapPermMinus
        | StatusEffectKind::ContaminationBoost
        | StatusEffectKind::BodyPartWeaken(_)
        | StatusEffectKind::StaminaCrash
        | StatusEffectKind::QiDrainForStamina
        | StatusEffectKind::LegStrain
        | StatusEffectKind::QiRegenPaused
        | StatusEffectKind::MirrorExposed
        | StatusEffectKind::ResonanceLocked
        | StatusEffectKind::QiRegenSlowed
        | StatusEffectKind::DamageVulnerability => "debuff",
        StatusEffectKind::AlchemyBuff(_) => "unknown",
    }
}

fn status_effect_source_label(kind: &StatusEffectKind) -> &'static str {
    match kind {
        StatusEffectKind::MirrorConcealment
        | StatusEffectKind::MirrorExposed
        | StatusEffectKind::SpiritTreasurePerception => "灵宝",
        StatusEffectKind::HealthRegenBoost => "养伤设施",
        // plan-cultivation-pacing-v1 P2.3：修炼类 buff/debuff 来源标注"修炼丹药"。
        StatusEffectKind::CultivationAcceleration
        | StatusEffectKind::ExtraordinaryMeridianAcceleration
        | StatusEffectKind::QiRegenSlowed
        | StatusEffectKind::DamageVulnerability => "修炼丹药",
        _ => "战场丹药",
    }
}

fn status_effect_color(kind: &StatusEffectKind) -> i32 {
    match status_effect_category(kind) {
        "buff" => 0xFF55CC66_u32 as i32,
        "debuff" => 0xFFFF8030_u32 as i32,
        "dot" => 0xFFE05050_u32 as i32,
        "control" => 0xFFB060FF_u32 as i32,
        _ => 0xFFA0A0A0_u32 as i32,
    }
}

fn status_effect_dispel(kind: &StatusEffectKind) -> i32 {
    match kind {
        StatusEffectKind::QiCapPermMinus => 5,
        StatusEffectKind::Stunned | StatusEffectKind::StaminaCrash => 3,
        StatusEffectKind::BodyPartWeaken(_) | StatusEffectKind::QiDrainForStamina => 2,
        StatusEffectKind::HealthRegenBoost => 1,
        _ => 1,
    }
}

fn body_part_wire(part: BodyPart) -> &'static str {
    match part {
        BodyPart::Head => "head",
        BodyPart::Chest => "chest",
        BodyPart::Back => "back",
        BodyPart::Abdomen => "abdomen",
        BodyPart::ArmL => "arm_l",
        BodyPart::ArmR => "arm_r",
        BodyPart::LegL => "leg_l",
        BodyPart::LegR => "leg_r",
    }
}

fn body_part_name(part: BodyPart) -> &'static str {
    match part {
        BodyPart::Head => "头部",
        BodyPart::Chest => "胸部",
        BodyPart::Back => "背部",
        BodyPart::Abdomen => "腹部",
        BodyPart::ArmL => "左臂",
        BodyPart::ArmR => "右臂",
        BodyPart::LegL => "左腿",
        BodyPart::LegR => "右腿",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── plan-cultivation-pacing-v1 P2.3 status_effect wire shape 测试 ──

    #[test]
    fn cultivation_acceleration_id_is_lowercase() {
        let id = status_effect_id(&StatusEffectKind::CultivationAcceleration);
        assert!(
            id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "CultivationAcceleration id should be lowercase snake_case, got: {id}"
        );
    }

    #[test]
    fn cultivation_acceleration_name_is_chinese() {
        assert_eq!(
            status_effect_name(&StatusEffectKind::CultivationAcceleration),
            "修炼加速"
        );
    }

    #[test]
    fn qi_regen_slowed_name() {
        assert_eq!(
            status_effect_name(&StatusEffectKind::QiRegenSlowed),
            "回气减速"
        );
    }

    #[test]
    fn damage_vulnerability_name() {
        assert_eq!(
            status_effect_name(&StatusEffectKind::DamageVulnerability),
            "脆弱易伤"
        );
    }

    #[test]
    fn extraordinary_meridian_acceleration_name() {
        assert_eq!(
            status_effect_name(&StatusEffectKind::ExtraordinaryMeridianAcceleration),
            "奇经加速"
        );
    }

    #[test]
    fn cultivation_acceleration_is_buff_category() {
        assert_eq!(
            status_effect_category(&StatusEffectKind::CultivationAcceleration),
            "buff"
        );
        assert_eq!(
            status_effect_category(&StatusEffectKind::ExtraordinaryMeridianAcceleration),
            "buff"
        );
    }

    #[test]
    fn qi_regen_slowed_is_debuff_category() {
        assert_eq!(
            status_effect_category(&StatusEffectKind::QiRegenSlowed),
            "debuff"
        );
    }

    #[test]
    fn damage_vulnerability_is_debuff_category() {
        assert_eq!(
            status_effect_category(&StatusEffectKind::DamageVulnerability),
            "debuff"
        );
    }

    #[test]
    fn cultivation_effects_source_label_is_cultivation_pill() {
        assert_eq!(
            status_effect_source_label(&StatusEffectKind::CultivationAcceleration),
            "修炼丹药"
        );
        assert_eq!(
            status_effect_source_label(&StatusEffectKind::QiRegenSlowed),
            "修炼丹药"
        );
        assert_eq!(
            status_effect_source_label(&StatusEffectKind::DamageVulnerability),
            "修炼丹药"
        );
        assert_eq!(
            status_effect_source_label(&StatusEffectKind::ExtraordinaryMeridianAcceleration),
            "修炼丹药"
        );
    }

    #[test]
    fn health_regen_boost_wire_shape_is_furniture_buff() {
        let kind = StatusEffectKind::HealthRegenBoost;

        assert_eq!(status_effect_id(&kind), "health_regen_boost");
        assert_eq!(status_effect_name(&kind), "养伤");
        assert_eq!(status_effect_category(&kind), "buff");
        assert_eq!(status_effect_source_label(&kind), "养伤设施");
        assert_eq!(status_effect_color(&kind), 0xFF55CC66_u32 as i32);
        assert_eq!(status_effect_dispel(&kind), 1);
    }

    #[test]
    fn buff_color_is_green_family() {
        let color = status_effect_color(&StatusEffectKind::CultivationAcceleration);
        // buff category → 0xFF55CC66
        assert_eq!(color, 0xFF55CC66_u32 as i32);
    }

    #[test]
    fn debuff_color_is_orange_family() {
        let color = status_effect_color(&StatusEffectKind::QiRegenSlowed);
        // debuff category → 0xFFFF8030
        assert_eq!(color, 0xFFFF8030_u32 as i32);
    }

    // ── plan-cultivation-pacing-v1 P2.3 cultivation_acceleration 协议契约测试 ──

    /// 辅助函数：模拟 emit system 中的 payload 构造逻辑，
    /// 以纯函数形式测试 wire shape 包含 cultivation_acceleration 字段。
    fn build_test_status_snapshot_payload(
        status_effects: &crate::combat::components::StatusEffects,
    ) -> serde_json::Value {
        let effects: Vec<serde_json::Value> = status_effects
            .active
            .iter()
            .filter(|e| e.remaining_ticks > 0)
            .map(|e| {
                serde_json::json!({
                    "id": status_effect_id(&e.kind),
                    "name": status_effect_name(&e.kind),
                    "kind": status_effect_category(&e.kind),
                    "stacks": 1,
                    "remaining_ms": e.remaining_ticks.saturating_mul(50),
                    "source_color": status_effect_color(&e.kind),
                    "source_label": status_effect_source_label(&e.kind),
                    "dispel": status_effect_dispel(&e.kind),
                })
            })
            .collect();
        let accel_mult =
            crate::cultivation::tick::cultivation_acceleration_multiplier(status_effects);
        serde_json::json!({
            "v": 1,
            "type": "status_snapshot",
            "effects": effects,
            "cultivation_acceleration": accel_mult,
        })
    }

    #[test]
    fn status_snapshot_payload_includes_cultivation_acceleration_field() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 2.0,
                remaining_ticks: 100,
                source_pill: None,
            }],
        };
        let payload = build_test_status_snapshot_payload(&se);

        assert_eq!(
            payload["type"], "status_snapshot",
            "payload type 应为 status_snapshot"
        );
        assert_eq!(payload["v"], 1, "payload version 应为 1");
        assert_eq!(
            payload["cultivation_acceleration"], 3.0,
            "CultivationAcceleration(mag=2.0) 应产生 cultivation_acceleration=3.0 (1+2)；\
             实际 {}",
            payload["cultivation_acceleration"]
        );
        // effects 数组应包含一条 CultivationAcceleration 条目
        let effects = payload["effects"].as_array().expect("effects 应为数组");
        assert_eq!(effects.len(), 1, "应有 1 条 active effect");
        assert_eq!(effects[0]["name"], "修炼加速");
        assert_eq!(effects[0]["kind"], "buff");
    }

    #[test]
    fn status_snapshot_payload_cultivation_acceleration_defaults_to_one() {
        use crate::combat::components::StatusEffects;

        // 无任何 CultivationAcceleration buff 时，字段应为 1.0
        let se = StatusEffects { active: vec![] };
        let payload = build_test_status_snapshot_payload(&se);

        assert_eq!(
            payload["cultivation_acceleration"], 1.0,
            "无 CultivationAcceleration buff 时 cultivation_acceleration 应为 1.0；\
             实际 {}",
            payload["cultivation_acceleration"]
        );
        let effects = payload["effects"].as_array().expect("effects 应为数组");
        assert!(effects.is_empty(), "空 StatusEffects 不应产生 effects 条目");
    }

    #[test]
    fn status_snapshot_payload_cultivation_acceleration_caps_at_five() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        // 多条 stacking 超过 cap：mag=3.0 + mag=2.0 = sum 5.0 → 1+5 = 6 → cap 5.0
        let se = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 3.0,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 2.0,
                    remaining_ticks: 100,
                    source_pill: None,
                },
            ],
        };
        let payload = build_test_status_snapshot_payload(&se);

        assert_eq!(
            payload["cultivation_acceleration"], 5.0,
            "多条 CultivationAcceleration(mag=3+2) 叠加超 cap 应为 5.0；\
             实际 {}",
            payload["cultivation_acceleration"]
        );
    }

    #[test]
    fn status_snapshot_payload_cultivation_acceleration_ignores_expired_buffs() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let se = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 1.0,
                    remaining_ticks: 50,
                    source_pill: None,
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 2.0,
                    remaining_ticks: 0, // expired
                    source_pill: None,
                },
            ],
        };
        let payload = build_test_status_snapshot_payload(&se);

        assert_eq!(
            payload["cultivation_acceleration"], 2.0,
            "expired buff(remaining_ticks=0) 不应计入 cultivation_acceleration；\
             期望 2.0 (1+1.0)，实际 {}",
            payload["cultivation_acceleration"]
        );
        // expired effect 也不应出现在 effects 数组
        let effects = payload["effects"].as_array().expect("effects 应为数组");
        assert_eq!(
            effects.len(),
            1,
            "expired buff 不应出现在 effects 数组；实际 {} 条",
            effects.len()
        );
    }

    #[test]
    fn status_snapshot_payload_cultivation_acceleration_coexists_with_other_effects() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};

        let se = StatusEffects {
            active: vec![
                ActiveStatusEffect {
                    kind: StatusEffectKind::CultivationAcceleration,
                    magnitude: 0.5,
                    remaining_ticks: 100,
                    source_pill: None,
                },
                ActiveStatusEffect {
                    kind: StatusEffectKind::Bleeding,
                    magnitude: 1.0,
                    remaining_ticks: 60,
                    source_pill: None,
                },
            ],
        };
        let payload = build_test_status_snapshot_payload(&se);

        assert_eq!(
            payload["cultivation_acceleration"], 1.5,
            "只有 CultivationAcceleration(mag=0.5) 计入 → 1.5；\
             Bleeding 不影响 cultivation_acceleration；实际 {}",
            payload["cultivation_acceleration"]
        );
        let effects = payload["effects"].as_array().expect("effects 应为数组");
        assert_eq!(effects.len(), 2, "应有 2 条 active effects");
    }
}
