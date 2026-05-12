//! plan-alchemy-combat-v1 P2 — status_snapshot emit。
//!
//! 客户端已有 `StatusSnapshotHandler` / `StatusEffectStore`；这里把 server
//! `StatusEffects` 的变化转成同一 wire shape，避免为战场丹药另建 HUD 通道。

use valence::prelude::{Changed, Client, Entity, Query, Username, With};

use crate::combat::components::{BodyPart, StatusEffects};
use crate::combat::events::StatusEffectKind;
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
        let payload = serde_json::json!({
            "v": 1,
            "type": "status_snapshot",
            "effects": effects
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
        StatusEffectKind::Staggered => "反震硬直".to_string(),
        StatusEffectKind::Disoriented => "迷乱".to_string(),
        StatusEffectKind::WoundHeal => "伤口回稳".to_string(),
        StatusEffectKind::BodyPartResist(part) => format!("{}硬化", body_part_name(*part)),
        StatusEffectKind::BodyPartWeaken(part) => format!("{}脆弱", body_part_name(*part)),
        StatusEffectKind::SpeedBoost => "疾行".to_string(),
        StatusEffectKind::StaminaRecovBoost => "回力".to_string(),
        StatusEffectKind::StaminaCrash => "体力虚脱".to_string(),
        StatusEffectKind::QiDrainForStamina => "真元换体".to_string(),
        StatusEffectKind::LegStrain => "腿部应力伤".to_string(),
        StatusEffectKind::QiRegenPaused => "真元停滞".to_string(),
        StatusEffectKind::MirrorConcealment => "镜隐".to_string(),
        StatusEffectKind::MirrorExposed => "镜照暴露".to_string(),
        StatusEffectKind::SpiritTreasurePerception => "灵宝感知".to_string(),
    }
}

fn status_effect_category(kind: &StatusEffectKind) -> &'static str {
    match kind {
        StatusEffectKind::Bleeding => "dot",
        StatusEffectKind::Stunned
        | StatusEffectKind::VortexCasting
        | StatusEffectKind::ParryRecovery
        | StatusEffectKind::Staggered
        | StatusEffectKind::Disoriented => "control",
        StatusEffectKind::DamageReduction
        | StatusEffectKind::BreakthroughBoost
        | StatusEffectKind::AntiSpiritPressurePill
        | StatusEffectKind::QiRegenBoost
        | StatusEffectKind::InsightFlash
        | StatusEffectKind::WoundHeal
        | StatusEffectKind::BodyPartResist(_)
        | StatusEffectKind::SpeedBoost
        | StatusEffectKind::StaminaRecovBoost
        | StatusEffectKind::MirrorConcealment
        | StatusEffectKind::SwordParrying
        | StatusEffectKind::SpiritTreasurePerception => "buff",
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
        | StatusEffectKind::MirrorExposed => "debuff",
        StatusEffectKind::AlchemyBuff(_) => "unknown",
    }
}

fn status_effect_source_label(kind: &StatusEffectKind) -> &'static str {
    match kind {
        StatusEffectKind::MirrorConcealment
        | StatusEffectKind::MirrorExposed
        | StatusEffectKind::SpiritTreasurePerception => "灵宝",
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
        _ => 1,
    }
}

fn body_part_wire(part: BodyPart) -> &'static str {
    match part {
        BodyPart::Head => "head",
        BodyPart::Chest => "chest",
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
        BodyPart::Abdomen => "腹部",
        BodyPart::ArmL => "左臂",
        BodyPart::ArmR => "右臂",
        BodyPart::LegL => "左腿",
        BodyPart::LegR => "右腿",
    }
}
