//! plan-baomai-v4 §4 — 死脉甲（Dead Meridian Armor）。
//!
//! 主动绝脉技（VoluntarySever）施放条件检查、施放效果处理、
//! `DeadMeridianArmor` 组件（污染免疫）、经脉→肢体映射、
//! contamination 拦截系统。
//!
//! **§8.1 #3 决议**：仅允许 12 正经 + Ren + Du（共 14 条），排除其余 6 条奇经。
//! **§8.1 #6 决议**：v1 不允许战斗中被断的经脉"认领"为死脉甲。
//!
//! P3-P5 常数前置声明，后续 PR 消费——允许暂时 dead_code。
#![allow(dead_code)]
// Inner attribute valid: this file is a module root (dead_armor.rs as `mod dead_armor`).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, EventReader, EventWriter, Query, Res};

use crate::combat::baomai_v3::events::BaomaiSkillId;
use crate::combat::baomai_v3::state::BaomaiMastery;
use crate::combat::baomai_v4::constants::{
    VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS, VOLUNTARY_SEVER_MAX_INTEGRITY,
    VOLUNTARY_SEVER_MIN_MASTERY, VOLUNTARY_SEVER_MIN_OVERLOADS,
};
use crate::combat::baomai_v4::events::VoluntarySeverEvent;
use crate::combat::baomai_v4::scar_circuit::{ActiveScarCircuits, ScarCircuitKind};
use crate::combat::baomai_v4::scar_history::ScarHistory;
use crate::combat::components::{BodyPart, CombatState};
use crate::combat::CombatClock;
use crate::cultivation::components::{MeridianId, MeridianSystem};
use crate::cultivation::meridian::severed::{
    enforce_severed_state, MeridianSeveredEvent, MeridianSeveredPermanent, SeveredSource,
};

use super::events::{CircuitBreakReason, ScarCircuitBrokenEvent};

/// 绝脉技 skill ID（plan §4.2）。
pub const VOLUNTARY_SEVER_SKILL_ID: &str = "baomai.voluntary_sever";

/// 允许绝脉的经脉集合：12 正经 + Ren + Du（§8.1 #3 决议）。
pub const VOLUNTARY_SEVER_ALLOWED: [MeridianId; 14] = [
    MeridianId::Lung,
    MeridianId::LargeIntestine,
    MeridianId::Stomach,
    MeridianId::Spleen,
    MeridianId::Heart,
    MeridianId::SmallIntestine,
    MeridianId::Bladder,
    MeridianId::Kidney,
    MeridianId::Pericardium,
    MeridianId::TripleEnergizer,
    MeridianId::Gallbladder,
    MeridianId::Liver,
    MeridianId::Ren,
    MeridianId::Du,
];

/// 检查经脉是否在允许绝脉的集合中。
pub fn is_voluntary_sever_allowed(id: MeridianId) -> bool {
    VOLUNTARY_SEVER_ALLOWED.contains(&id)
}

/// 施放条件检查结果（plan §4.2）。
#[derive(Debug, Clone, PartialEq)]
pub enum VoluntarySeverReject {
    /// total_overloads < 80。
    InsufficientOverloads { current: u32, required: u32 },
    /// 所有 baomai 技能 mastery < 60。
    InsufficientMastery { max_mastery: u8, required: f32 },
    /// 目标经脉 integrity >= 0.70（不够损伤）。
    MeridianTooHealthy { integrity: f64, max_allowed: f64 },
    /// 目标经脉已 SEVERED。
    AlreadySevered,
    /// 3s 内有战斗事件。
    InCombat,
    /// 目标经脉不在允许列表（§8.1 #3：排除 6 条奇经）。
    MeridianNotAllowed,
}

/// 检查是否可以施放绝脉技（plan §4.2 施放条件）。
///
/// 返回 `Ok(())` 时可以开始 100-tick 引导；返回 `Err` 时携带拒绝原因。
pub fn check_voluntary_sever(
    target_meridian: MeridianId,
    history: &ScarHistory,
    mastery: Option<&BaomaiMastery>,
    meridians: &MeridianSystem,
    severed: Option<&MeridianSeveredPermanent>,
    combat_state: Option<&CombatState>,
    now_tick: u64,
) -> Result<(), VoluntarySeverReject> {
    // §8.1 #3: 仅 12 正经 + Ren + Du
    if !is_voluntary_sever_allowed(target_meridian) {
        return Err(VoluntarySeverReject::MeridianNotAllowed);
    }

    // total_overloads >= 80
    if history.total_overloads < VOLUNTARY_SEVER_MIN_OVERLOADS {
        return Err(VoluntarySeverReject::InsufficientOverloads {
            current: history.total_overloads,
            required: VOLUNTARY_SEVER_MIN_OVERLOADS,
        });
    }

    // baomai mastery (任一技能) >= 60
    let max_mastery = max_baomai_mastery(mastery);
    if (max_mastery as f32) < VOLUNTARY_SEVER_MIN_MASTERY {
        return Err(VoluntarySeverReject::InsufficientMastery {
            max_mastery,
            required: VOLUNTARY_SEVER_MIN_MASTERY,
        });
    }

    // 目标经脉 integrity < 0.70
    let m = meridians.get(target_meridian);
    if m.integrity >= VOLUNTARY_SEVER_MAX_INTEGRITY {
        return Err(VoluntarySeverReject::MeridianTooHealthy {
            integrity: m.integrity,
            max_allowed: VOLUNTARY_SEVER_MAX_INTEGRITY,
        });
    }

    // NOT SEVERED
    if let Some(s) = severed {
        if s.is_severed(target_meridian) {
            return Err(VoluntarySeverReject::AlreadySevered);
        }
    }

    // 非战斗状态（3s 内无 CombatEvent）
    if let Some(cs) = combat_state {
        if let Some(until) = cs.in_combat_until_tick {
            // in_combat_until_tick 使用 IN_COMBAT_WINDOW_TICKS (15s)，但绝脉技只要求 3s。
            // 用 last_attack_at_tick + VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS 作为判定。
            if let Some(last_attack) = cs.last_attack_at_tick {
                if now_tick < last_attack.saturating_add(VOLUNTARY_SEVER_COMBAT_COOLDOWN_TICKS) {
                    return Err(VoluntarySeverReject::InCombat);
                }
            }
            // 如果 in_combat_until_tick 仍然活跃但 last_attack 够久远，
            // 则允许施放（in_combat_window 是 15s，绝脉技只要求 3s 脱战）。
            // 但若只有 in_combat_until_tick 无 last_attack_at_tick，检查窗口：
            if cs.last_attack_at_tick.is_none() && now_tick < until {
                return Err(VoluntarySeverReject::InCombat);
            }
        }
    }

    Ok(())
}

/// 获取所有 baomai 技能中的最大 mastery 等级。
pub fn max_baomai_mastery(mastery: Option<&BaomaiMastery>) -> u8 {
    let Some(m) = mastery else {
        return 0;
    };
    // BaomaiSkillId::ALL is #[cfg(test)]; enumerate all variants explicitly.
    let skills = [
        BaomaiSkillId::BengQuan,
        BaomaiSkillId::FullPowerCharge,
        BaomaiSkillId::FullPowerRelease,
        BaomaiSkillId::MountainShake,
        BaomaiSkillId::BloodBurn,
        BaomaiSkillId::Disperse,
    ];
    skills
        .iter()
        .map(|skill| m.level(*skill))
        .max()
        .unwrap_or(0)
}

// ── DeadMeridianArmor 组件 ──

/// 死脉甲组件——记录因主动绝脉获得污染免疫的肢体区域。
///
/// 仅 `SeveredSource::VoluntarySever` 产生的 SEVERED 经脉获得免疫。
/// 战斗断裂不行（§8.1 #6 决议）。
#[derive(Component, Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeadMeridianArmor {
    /// 因主动绝脉获得污染免疫的肢体区域。
    pub immune_regions: HashSet<BodyPart>,
}

impl DeadMeridianArmor {
    /// 检查某肢体区域是否有污染免疫。
    pub fn is_immune(&self, body_part: BodyPart) -> bool {
        self.immune_regions.contains(&body_part)
    }
}

/// 经脉→肢体映射（plan §4.3，worldview §四:293 正经按肢分布）。
///
/// 返回 `None` 表示该经脉无可命中体表映射（6 条排除的奇经）。
///
/// ## Head / Abdomen 设计决议（刻意弱点区）
///
/// `classify_body_part`（raycast.rs）穷举只产出
/// `Head / Chest / ArmL / ArmR / Abdomen / LegL / LegR`，**永不产出 Back**。
/// 因此：
/// - `BodyPart::Back` 不是可命中区域，映射到 Back 的免疫永不触发。
/// - Du 脉（督脉）走背脊，映射到实际可命中的 **Chest**（与 Ren 共享躯干防护），
///   让玩家断 Du 真能换到保护。
/// - `Head` 与 `Abdomen` 无任何经脉映射，是刻意设计的永久弱点区，
///   死脉甲对头/腹命中无保护（攻方可绕过全部免疫）。
pub fn meridian_to_body_part(id: MeridianId) -> Option<BodyPart> {
    match id {
        // 左臂·手三阴
        MeridianId::Lung | MeridianId::Heart | MeridianId::Pericardium => Some(BodyPart::ArmL),
        // 右臂·手三阳
        MeridianId::LargeIntestine | MeridianId::SmallIntestine | MeridianId::TripleEnergizer => {
            Some(BodyPart::ArmR)
        }
        // 左腿·足三阴
        MeridianId::Spleen | MeridianId::Kidney | MeridianId::Liver => Some(BodyPart::LegL),
        // 右腿·足三阳
        MeridianId::Stomach | MeridianId::Bladder | MeridianId::Gallbladder => Some(BodyPart::LegR),
        // 躯干（任督两脉共享 Chest——Back 不可命中，见上方决议注释）
        MeridianId::Ren => Some(BodyPart::Chest),
        MeridianId::Du => Some(BodyPart::Chest),
        // 其余 6 条奇经无体部映射
        MeridianId::Chong
        | MeridianId::Dai
        | MeridianId::YinQiao
        | MeridianId::YangQiao
        | MeridianId::YinWei
        | MeridianId::YangWei => None,
    }
}

// ── 系统 ──

/// 处理 `VoluntarySeverEvent`：施放效果（plan §4.2）。
///
/// 1. integrity → 0.0，写入 MeridianSeveredPermanent(VoluntarySever)
/// 2. 所有依赖该经脉的 SkillMeridianDependencies 被拦截（已有机制）
/// 3. 经过该经脉的 ScarCircuit 立即断裂
/// 4. 该经脉对应的 BodyPart 获得 ContaminationImmunity 标记
pub fn voluntary_sever_apply_system(
    mut events: EventReader<VoluntarySeverEvent>,
    mut targets: Query<(
        &mut MeridianSystem,
        &mut MeridianSeveredPermanent,
        &mut DeadMeridianArmor,
        Option<&mut ActiveScarCircuits>,
    )>,
    mut severed_events: EventWriter<MeridianSeveredEvent>,
    mut broken_events: EventWriter<ScarCircuitBrokenEvent>,
    _clock: Res<CombatClock>,
) {
    for ev in events.read() {
        let Ok((mut meridians, mut severed, mut armor, circuits)) = targets.get_mut(ev.entity)
        else {
            tracing::warn!(
                "[bong][baomai_v4][dead_armor] dropped VoluntarySeverEvent for {:?}: missing components",
                ev.entity,
            );
            continue;
        };

        // 1. Sever the meridian.
        let inserted = severed.insert(ev.meridian, SeveredSource::VoluntarySever, ev.tick);
        if !inserted {
            // Already severed (shouldn't happen if check_voluntary_sever passed).
            tracing::warn!(
                "[bong][baomai_v4][dead_armor] meridian {:?} already severed for {:?}",
                ev.meridian,
                ev.entity,
            );
            continue;
        }
        enforce_severed_state(&mut meridians, ev.meridian);

        // Also emit MeridianSeveredEvent for downstream consumers.
        severed_events.send(MeridianSeveredEvent {
            entity: ev.entity,
            meridian_id: ev.meridian,
            source: SeveredSource::VoluntarySever,
            at_tick: ev.tick,
        });

        // 3. Break any active circuits involving this meridian.
        if let Some(mut circuits) = circuits {
            let active: Vec<ScarCircuitKind> = circuits.circuits.iter().copied().collect();
            for kind in active {
                if kind.involves(ev.meridian) {
                    circuits.circuits.remove(&kind);
                    circuits.formed_at.remove(&kind);
                    broken_events.send(ScarCircuitBrokenEvent {
                        entity: ev.entity,
                        circuit: kind,
                        reason: CircuitBreakReason::Severed,
                        tick: ev.tick,
                    });
                }
            }
        }

        // 4. Grant contamination immunity for the mapped body part.
        if let Some(body_part) = meridian_to_body_part(ev.meridian) {
            armor.immune_regions.insert(body_part);
            tracing::info!(
                "[bong][baomai_v4][dead_armor] entity={:?} meridian={:?} -> immune {:?}",
                ev.entity,
                ev.meridian,
                body_part,
            );
        }
    }
}

/// 检查 contamination 是否应被死脉甲拦截。
///
/// 返回 `true` 表示该 body_part 有 VoluntarySever 来源的免疫，应拦截 contamination。
/// 仅检查免疫映射表，不过滤物理伤害。
///
/// **守恒决议（drop_no_release）**：被拦截的 contamination 直接丢弃，不调用
/// `qi_release_to_zone`。理由：pollution 量由伤害公式凭空派生（非攻方真元账户的直接转移），
/// 拦截后 release_to_zone 反而向 zone 注入从未扣走的真元，造成通胀。
pub fn should_block_contamination(armor: &DeadMeridianArmor, body_part: BodyPart) -> bool {
    armor.is_immune(body_part)
}
