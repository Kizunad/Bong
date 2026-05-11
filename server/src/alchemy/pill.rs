//! 服药 → 污染 注入（plan-alchemy-v1 §2 + plan-shelflife-v1 §5.2 M5b）。
//!
//! 复用 `cultivation::Contamination / ContamSource` — 不新增字段。
//! 代谢速率天然由 MeridianSystem `sum_rate × integrity`（contamination_tick 做）决定。
//!
//! M5b：`consume_pill` 接收 shelflife `SpoilCheckOutcome` 驱动分支：
//! - `NotApplicable` / `Safe` → 正常消费
//! - `Warn` → 消费 + 额外 push Sharp contam（按腐败程度放大）
//! - `CriticalBlock` → 拒绝消费，返回 `PillConsumeOutcome.blocked = true`
//!
//! M5d：`consume_pill` 再接 `AgePeakCheck`（plan §5.3 陈丹峰值 bonus）：
//! - `Peaking { bonus_strength }` → qi_gain × (1 + bonus_strength)；outcome 携 bonus 供
//!   caller emit `AgeBonusRoll` event
//! - `NotApplicable` / `NotPeaking` → 无影响

use serde::{Deserialize, Serialize};

use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::combat::events::{ApplyStatusEffectIntent, StatusEffectKind};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation, Realm};
use crate::shelflife::{AgePeakCheck, SpoilCheckOutcome};

/// plan-shelflife-v1 §5.2 — Spoil `Warn` 档额外污染系数。
/// `extra_toxin = toxin_amount × (1 - current/threshold) × SPOIL_TOXIN_MULT`；
/// current 接近 threshold 时 extra ≈ 0，接近 CriticalBlock 边界 (0.1×threshold) 时 ≈ 0.9×toxin_amount。
/// 首版定 1.0（完全腐败场景 extra ≈ toxin_amount 即毒性翻倍）；M7 跨 plan 定稿时按
/// 实际玩家行为再调。
pub const SPOIL_TOXIN_MULT: f64 = 1.0;

/// 服药时的单体效果描述（plan §3.2 pill 效果的运行时形态）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PillEffect {
    /// 丹毒量（注入 Contamination）。
    pub toxin_amount: f64,
    pub toxin_color: ColorKind,
    /// 立即回 qi。
    #[serde(default)]
    pub qi_gain: Option<f64>,
    /// 未来扩展（plan §6 cultivation 钩子）：推进经脉打通进度。
    #[serde(default)]
    pub meridian_progress_bonus: Option<f64>,
}

/// plan-shelflife-v1 M5b — `consume_pill` 的结构化返回值。
///
/// `blocked=true` 时 `qi_gained` / `extra_toxin_added` 均为 0 — 调用侧据此触发
/// UI 二次确认（plan §5.2 "拒绝自动消费"）。
#[derive(Debug, Clone, PartialEq)]
pub struct PillConsumeOutcome {
    /// 实际生效的 qi_gain（blocked 时为 0.0；含 M5d Age bonus 放大）。
    pub qi_gained: f64,
    /// CriticalBlock 触发自动拒绝时为 true；Normal / Safe / Warn 均 false。
    pub blocked: bool,
    /// Spoil `Warn` 档额外 push 的污染量（color 同 `effect.toxin_color`）。
    /// Normal / Safe / Blocked 时为 0.0。
    pub extra_toxin_added: f64,
    /// plan §5.3 M5d — Age Peaking 触发时的 `peak_bonus`；caller emit `AgeBonusRoll` 用。
    /// NotApplicable / NotPeaking / blocked 时为 None。
    pub age_bonus_applied: Option<f32>,
}

/// plan §2.2 — 同色丹毒未排到阈值不允许再服。
/// 返回该色当前残留总量。
pub fn sum_drug_toxin(contam: &Contamination, color: ColorKind) -> f64 {
    contam
        .entries
        .iter()
        .filter(|e| e.color == color && e.attacker_id.is_none())
        .map(|e| e.amount)
        .sum()
}

pub const TOXIN_THRESHOLD: f64 = 1.0;

/// plan §2.2 `can_take`：同色丹毒聚合量 < THRESHOLD 才能吃。
pub fn can_take_pill(contam: &Contamination, color: ColorKind) -> bool {
    sum_drug_toxin(contam, color) < TOXIN_THRESHOLD
}

/// plan-alchemy-v1 §2.1 + plan-shelflife-v1 §5.2/5.3 — 服药流程。
///
/// # 参数
/// - `effect` — pill 基础效果（toxin_amount / color / qi_gain）
/// - `contam` — 玩家污染状态（mut：push ContamSource）
/// - `cultivation` — 玩家修为（mut：增加 qi_current）
/// - `now_tick` — 当前 server tick（contam 记录时间戳）
/// - `spoil` — shelflife `spoil_check` 结果（caller 先查 registry + freshness 生成）
/// - `force_consume` — plan §5.2 二次确认路径：`CriticalBlock` 档玩家通过 UI 对话
///   框确认"像吃屎也要吃"后，caller 再次调 `consume_pill` 并置 `force_consume=true`；
///   此时按 Warn 公式用实际 (current, threshold) 算 extra_toxin（ratio ≈ 0.9-1.0）放大
///   至最大污染，消费得以进行。对 Safe / Warn / NotApplicable 不影响。
/// - `age` — shelflife `age_peak_check` 结果：`Peaking { bonus_strength }` 时把 qi_gain
///   乘以 `(1 + bonus_strength)` 作为 Age 路径的峰值加成（plan §5.3 "峰值消费"）。
///   NotApplicable / NotPeaking 时不影响。
///
/// # 分支（Spoil）
/// - `NotApplicable` / `Safe` → 正常消费：push 基础 contam + apply qi_gain
/// - `Warn` → 消费 + 额外 push Sharp contam（按 `1 - current/threshold` 放大）
/// - `CriticalBlock` + `force_consume=false` → 拒绝，无 contam / 无 qi / `blocked=true`
/// - `CriticalBlock` + `force_consume=true` → 按 Warn 公式消费（extra 接近 100%）
///
/// # 分支（Age M5d）
/// - `Peaking { bonus_strength }` → qi_gained × (1 + bonus_strength)，outcome 携 Some(bonus)
/// - `NotApplicable` / `NotPeaking` → qi_gain 不变，outcome 携 None
/// - **blocked 时不应用 Age bonus**（无消费 = 无加成）
///
/// 调用侧应在 `Warn` / `CriticalBlock` 时 emit `SpoilConsumeWarning`；
/// `age_bonus_applied = Some(_)` 时 emit `AgeBonusRoll`。
pub fn consume_pill(
    effect: &PillEffect,
    contam: &mut Contamination,
    cultivation: &mut Cultivation,
    now_tick: u64,
    spoil: SpoilCheckOutcome,
    force_consume: bool,
    age: AgePeakCheck,
) -> PillConsumeOutcome {
    // CriticalBlock + !force → 拒绝；+ force → 降级为 Warn 走标准逻辑。
    let effective_spoil = match spoil {
        SpoilCheckOutcome::CriticalBlock { .. } if !force_consume => {
            return PillConsumeOutcome {
                qi_gained: 0.0,
                blocked: true,
                extra_toxin_added: 0.0,
                age_bonus_applied: None,
            };
        }
        SpoilCheckOutcome::CriticalBlock {
            current_qi,
            spoil_threshold,
        } => SpoilCheckOutcome::Warn {
            current_qi,
            spoil_threshold,
        },
        other => other,
    };

    // 基础污染
    contam.entries.push(ContamSource {
        amount: effect.toxin_amount,
        color: effect.toxin_color,
        meridian_id: None,
        attacker_id: None,
        introduced_at: now_tick,
    });

    // Warn 档 — 额外污染
    let extra_toxin = match effective_spoil {
        SpoilCheckOutcome::Warn {
            current_qi,
            spoil_threshold,
        } => {
            let ratio = if spoil_threshold > 0.0 {
                (1.0 - (current_qi as f64 / spoil_threshold as f64)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let extra = effect.toxin_amount * ratio * SPOIL_TOXIN_MULT;
            if extra > 0.0 {
                contam.entries.push(ContamSource {
                    amount: extra,
                    color: effect.toxin_color,
                    meridian_id: None,
                    attacker_id: None,
                    introduced_at: now_tick,
                });
            }
            extra
        }
        _ => 0.0,
    };

    // M5d — Age Peaking 加成（乘在 qi_gain 上）
    let age_bonus = match age {
        AgePeakCheck::Peaking { bonus_strength } => Some(bonus_strength),
        _ => None,
    };

    // qi_gain（含 Age bonus）
    let qi_gained = match effect.qi_gain {
        Some(q) => {
            let before = cultivation.qi_current;
            let effective_q = match age_bonus {
                Some(b) => q * (1.0 + b as f64),
                None => q,
            };
            cultivation.qi_current = (before + effective_q).min(cultivation.qi_max);
            cultivation.qi_current - before
        }
        None => 0.0,
    };

    PillConsumeOutcome {
        qi_gained,
        blocked: false,
        extra_toxin_added: extra_toxin,
        age_bonus_applied: age_bonus,
    }
}

/// plan §2.3 过量强吃 —— 返回应追加的附带损伤（供调用侧施到经脉）。
/// 目前简化：每超出 THRESHOLD 0.5 → +severity 0.05
pub fn overdose_penalty(contam: &Contamination, color: ColorKind) -> f64 {
    let total = sum_drug_toxin(contam, color);
    if total < TOXIN_THRESHOLD {
        return 0.0;
    }
    let over = total - TOXIN_THRESHOLD;
    (over / 0.5) * 0.05
}

pub const COMBAT_PILL_IDS: [&str; 10] = [
    "huo_xue_dan",
    "xu_gu_gao",
    "duan_xu_san",
    "tie_bi_san",
    "jin_zhong_dan",
    "ning_jia_san",
    "ji_feng_dan",
    "suo_di_san",
    "hui_li_dan",
    "hu_gu_san",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatPillKind {
    HuoXueDan,
    XuGuGao,
    DuanXuSan,
    TieBiSan,
    JinZhongDan,
    NingJiaSan,
    JiFengDan,
    SuoDiSan,
    HuiLiDan,
    HuGuSan,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatPillSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: CombatPillKind,
    pub toxin_amount: f64,
    pub toxin_color: ColorKind,
    pub cast_duration_ticks: u64,
    pub positive_duration_ticks: u64,
    pub negative_duration_ticks: u64,
    pub vfx_event_id: &'static str,
    pub animation_id: &'static str,
    pub audio_recipe_id: &'static str,
}

pub fn combat_pill_spec(id: &str) -> Option<CombatPillSpec> {
    let seconds = crate::combat::components::TICKS_PER_SECOND;
    Some(match id {
        "huo_xue_dan" => CombatPillSpec {
            id: "huo_xue_dan",
            name: "活血丹",
            kind: CombatPillKind::HuoXueDan,
            toxin_amount: 0.15,
            toxin_color: ColorKind::Gentle,
            cast_duration_ticks: seconds + seconds / 2,
            positive_duration_ticks: 1,
            negative_duration_ticks: 60 * seconds,
            vfx_event_id: "bong:pill_huo_xue",
            animation_id: "bong:pill_huo_xue",
            audio_recipe_id: "pill_huo_xue_consume",
        },
        "xu_gu_gao" => CombatPillSpec {
            id: "xu_gu_gao",
            name: "续骨膏",
            kind: CombatPillKind::XuGuGao,
            toxin_amount: 0.25,
            toxin_color: ColorKind::Solid,
            cast_duration_ticks: 3 * seconds,
            positive_duration_ticks: 1,
            negative_duration_ticks: 120 * seconds,
            vfx_event_id: "bong:pill_xu_gu",
            animation_id: "bong:pill_xu_gu",
            audio_recipe_id: "pill_xu_gu_consume",
        },
        "duan_xu_san" => CombatPillSpec {
            id: "duan_xu_san",
            name: "断续散",
            kind: CombatPillKind::DuanXuSan,
            toxin_amount: 0.80,
            toxin_color: ColorKind::Turbid,
            cast_duration_ticks: 5 * seconds,
            positive_duration_ticks: 1,
            negative_duration_ticks: 300 * seconds,
            vfx_event_id: "bong:pill_duan_xu",
            animation_id: "bong:pill_duan_xu",
            audio_recipe_id: "pill_duan_xu_consume",
        },
        "tie_bi_san" => CombatPillSpec {
            id: "tie_bi_san",
            name: "铁壁散",
            kind: CombatPillKind::TieBiSan,
            toxin_amount: 0.30,
            toxin_color: ColorKind::Heavy,
            cast_duration_ticks: 2 * seconds,
            positive_duration_ticks: 90 * seconds,
            negative_duration_ticks: 90 * seconds,
            vfx_event_id: "bong:pill_tie_bi",
            animation_id: "bong:pill_tie_bi",
            audio_recipe_id: "pill_tie_bi_consume",
        },
        "jin_zhong_dan" => CombatPillSpec {
            id: "jin_zhong_dan",
            name: "金钟丹",
            kind: CombatPillKind::JinZhongDan,
            toxin_amount: 0.45,
            toxin_color: ColorKind::Heavy,
            cast_duration_ticks: seconds,
            positive_duration_ticks: 30 * seconds,
            negative_duration_ticks: 180 * seconds,
            vfx_event_id: "bong:pill_jin_zhong",
            animation_id: "bong:pill_jin_zhong",
            audio_recipe_id: "pill_jin_zhong_consume",
        },
        "ning_jia_san" => CombatPillSpec {
            id: "ning_jia_san",
            name: "凝甲散",
            kind: CombatPillKind::NingJiaSan,
            toxin_amount: 0.20,
            toxin_color: ColorKind::Solid,
            cast_duration_ticks: 2 * seconds,
            positive_duration_ticks: 60 * seconds,
            negative_duration_ticks: 60 * seconds,
            vfx_event_id: "bong:pill_ning_jia",
            animation_id: "bong:pill_ning_jia",
            audio_recipe_id: "pill_ning_jia_consume",
        },
        "ji_feng_dan" => CombatPillSpec {
            id: "ji_feng_dan",
            name: "疾风丹",
            kind: CombatPillKind::JiFengDan,
            toxin_amount: 0.20,
            toxin_color: ColorKind::Light,
            cast_duration_ticks: seconds,
            positive_duration_ticks: 60 * seconds,
            negative_duration_ticks: 80 * seconds,
            vfx_event_id: "bong:pill_ji_feng",
            animation_id: "bong:pill_ji_feng",
            audio_recipe_id: "pill_ji_feng_consume",
        },
        "suo_di_san" => CombatPillSpec {
            id: "suo_di_san",
            name: "缩地散",
            kind: CombatPillKind::SuoDiSan,
            toxin_amount: 0.35,
            toxin_color: ColorKind::Violent,
            cast_duration_ticks: seconds / 2,
            positive_duration_ticks: 10 * seconds,
            negative_duration_ticks: 10 * seconds,
            vfx_event_id: "bong:pill_suo_di",
            animation_id: "bong:pill_suo_di",
            audio_recipe_id: "pill_suo_di_consume",
        },
        "hui_li_dan" => CombatPillSpec {
            id: "hui_li_dan",
            name: "回力丹",
            kind: CombatPillKind::HuiLiDan,
            toxin_amount: 0.15,
            toxin_color: ColorKind::Mellow,
            cast_duration_ticks: seconds + seconds / 2,
            positive_duration_ticks: 90 * seconds,
            negative_duration_ticks: 90 * seconds,
            vfx_event_id: "bong:pill_hui_li",
            animation_id: "bong:pill_hui_li",
            audio_recipe_id: "pill_hui_li_consume",
        },
        "hu_gu_san" => CombatPillSpec {
            id: "hu_gu_san",
            name: "虎骨散",
            kind: CombatPillKind::HuGuSan,
            toxin_amount: 0.30,
            toxin_color: ColorKind::Heavy,
            cast_duration_ticks: 2 * seconds,
            positive_duration_ticks: 120 * seconds,
            negative_duration_ticks: 60 * seconds,
            vfx_event_id: "bong:pill_hu_gu",
            animation_id: "bong:pill_hu_gu",
            audio_recipe_id: "pill_hu_gu_consume",
        },
        _ => return None,
    })
}

pub fn mortal_pill_realm_scale(realm: Realm) -> (f32, f32) {
    match realm {
        Realm::Awaken | Realm::Induce | Realm::Condense => (1.0, 1.0),
        Realm::Solidify => (0.5, 0.8),
        Realm::Spirit => (0.15, 0.6),
        Realm::Void => (0.05, 0.4),
    }
}

pub fn scaled_grades(base: u8, scale: f32) -> u8 {
    (f32::from(base) * scale)
        .round()
        .clamp(0.0, f32::from(u8::MAX)) as u8
}

pub fn apply_wound_heal(wounds: &mut Wounds, target: Option<BodyPart>, grades: u8) -> usize {
    if grades == 0 {
        return 0;
    }
    let delta = wound_grade_delta(grades);
    let mut changed = 0usize;
    for wound in &mut wounds.entries {
        if target.is_some_and(|part| part != wound.location) {
            continue;
        }
        if is_severed_like(wound) {
            continue;
        }
        let before = wound.severity;
        wound.severity = (wound.severity - delta).max(0.0);
        wound.bleeding_per_sec = wound.bleeding_per_sec.max(0.0)
            * if before > f32::EPSILON {
                (wound.severity / before).clamp(0.0, 1.0)
            } else {
                0.0
            };
        if wound.severity < before {
            changed += 1;
        }
    }
    wounds.entries.retain(|wound| wound.severity >= 0.05);
    wounds.health_current =
        (wounds.health_current + delta * changed as f32).clamp(0.0, wounds.health_max);
    changed
}

pub fn apply_severed_mend(
    wounds: &mut Wounds,
    target: Option<BodyPart>,
    success_scale: f32,
) -> bool {
    if success_scale <= 0.0 {
        return false;
    }
    let Some(index) = wounds
        .entries
        .iter()
        .enumerate()
        .filter(|(_, wound)| target.is_none_or(|part| part == wound.location))
        .filter(|(_, wound)| is_severed_like(wound))
        .max_by(|(_, a), (_, b)| a.severity.total_cmp(&b.severity))
        .map(|(index, _)| index)
    else {
        return false;
    };
    let target_severity = 0.55 + (1.0 - success_scale.clamp(0.0, 1.0)) * 0.30;
    let wound = &mut wounds.entries[index];
    wound.severity = wound.severity.min(target_severity);
    wound.kind = WoundKind::Concussion;
    wound.bleeding_per_sec *= 0.35;
    true
}

pub fn apply_wound_worsen(
    wounds: &mut Wounds,
    parts: &[BodyPart],
    grades: u8,
    now_tick: u64,
    inflicted_by: Option<String>,
) -> usize {
    if grades == 0 {
        return 0;
    }
    let severity = wound_grade_delta(grades);
    for part in parts {
        wounds.entries.push(Wound {
            location: *part,
            kind: WoundKind::Concussion,
            severity,
            bleeding_per_sec: 0.0,
            created_at_tick: now_tick,
            inflicted_by: inflicted_by.clone(),
        });
    }
    parts.len()
}

pub fn worst_non_severed_part(wounds: &Wounds) -> Option<BodyPart> {
    wounds
        .entries
        .iter()
        .filter(|wound| !is_severed_like(wound))
        .max_by(|a, b| a.severity.total_cmp(&b.severity))
        .map(|wound| wound.location)
}

pub fn worst_severed_part(wounds: &Wounds) -> Option<BodyPart> {
    wounds
        .entries
        .iter()
        .filter(|wound| is_severed_like(wound))
        .max_by(|a, b| a.severity.total_cmp(&b.severity))
        .map(|wound| wound.location)
}

pub fn combat_pill_status_intents(
    target: valence::prelude::Entity,
    spec: CombatPillSpec,
    pos_scale: f32,
    neg_scale: f32,
    issued_at_tick: u64,
) -> Vec<ApplyStatusEffectIntent> {
    let mut out = Vec::new();
    let mut push = |kind, magnitude, duration_ticks| {
        if magnitude > 0.0 && duration_ticks > 0 {
            out.push(ApplyStatusEffectIntent {
                target,
                kind,
                magnitude,
                duration_ticks,
                issued_at_tick,
            });
        }
    };
    match spec.kind {
        CombatPillKind::HuoXueDan => {
            push(
                StatusEffectKind::WoundHeal,
                pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::Bleeding,
                0.075 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::XuGuGao => {
            push(
                StatusEffectKind::WoundHeal,
                2.0 * pos_scale,
                spec.positive_duration_ticks,
            );
            for part in [
                BodyPart::ArmL,
                BodyPart::ArmR,
                BodyPart::LegL,
                BodyPart::LegR,
            ] {
                push(
                    StatusEffectKind::BodyPartWeaken(part),
                    0.30 * neg_scale,
                    spec.negative_duration_ticks,
                );
            }
            push(
                StatusEffectKind::Slowed,
                0.15 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::DuanXuSan => {
            push(
                StatusEffectKind::WoundHeal,
                pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::Slowed,
                0.50 * neg_scale,
                spec.negative_duration_ticks,
            );
            push(StatusEffectKind::Stunned, 1.0, spec.negative_duration_ticks);
        }
        CombatPillKind::TieBiSan => {
            for part in [BodyPart::Chest, BodyPart::Abdomen] {
                push(
                    StatusEffectKind::BodyPartResist(part),
                    0.40 * pos_scale,
                    spec.positive_duration_ticks,
                );
            }
            for part in [
                BodyPart::ArmL,
                BodyPart::ArmR,
                BodyPart::LegL,
                BodyPart::LegR,
            ] {
                push(
                    StatusEffectKind::BodyPartWeaken(part),
                    0.25 * neg_scale,
                    spec.negative_duration_ticks,
                );
            }
        }
        CombatPillKind::JinZhongDan => {
            push(
                StatusEffectKind::DamageReduction,
                0.30 * pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::QiRegenBoost,
                0.001,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::NingJiaSan => {
            push(
                StatusEffectKind::BodyPartResist(BodyPart::ArmR),
                0.60 * pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::BodyPartWeaken(BodyPart::ArmR),
                0.35 * neg_scale,
                spec.negative_duration_ticks,
            );
            push(
                StatusEffectKind::DamageAmp,
                0.001,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::JiFengDan => {
            push(
                StatusEffectKind::SpeedBoost,
                0.35 * pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::StaminaCrash,
                0.10 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::SuoDiSan => {
            push(
                StatusEffectKind::SpeedBoost,
                0.80 * pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::StaminaCrash,
                0.05 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::HuiLiDan => {
            push(
                StatusEffectKind::StaminaRecovBoost,
                3.0_f32.mul_add(pos_scale, 0.0).max(1.0),
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::QiDrainForStamina,
                2.0 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
        CombatPillKind::HuGuSan => {
            push(
                StatusEffectKind::StaminaRecovBoost,
                0.50 * pos_scale,
                spec.positive_duration_ticks,
            );
            push(
                StatusEffectKind::StaminaCrash,
                0.30 * neg_scale,
                spec.negative_duration_ticks,
            );
        }
    }
    out
}

fn wound_grade_delta(grades: u8) -> f32 {
    f32::from(grades) * 0.25
}

fn is_severed_like(wound: &Wound) -> bool {
    wound.severity >= 0.85
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Contamination, Cultivation};

    fn fresh_contam() -> Contamination {
        Contamination::default()
    }

    fn basic_effect(qi_gain: Option<f64>) -> PillEffect {
        PillEffect {
            toxin_amount: 0.3,
            toxin_color: ColorKind::Mellow,
            qi_gain,
            meridian_progress_bonus: None,
        }
    }

    #[test]
    fn consume_pill_normal_appends_contam_and_restores_qi() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 24.0);
        assert!(!outcome.blocked);
        assert_eq!(outcome.extra_toxin_added, 0.0);
        assert_eq!(cult.qi_current, 24.0);
        assert_eq!(contam.entries.len(), 1);
        assert_eq!(contam.entries[0].color, ColorKind::Mellow);
        assert!(contam.entries[0].attacker_id.is_none());
        assert_eq!(contam.entries[0].introduced_at, 10);
    }

    #[test]
    fn qi_gain_clamped_to_qi_max() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 90.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(50.0)),
            &mut contam,
            &mut cult,
            0,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 10.0);
        assert_eq!(cult.qi_current, 100.0);
    }

    #[test]
    fn can_take_pill_blocks_when_same_color_exceeds_threshold() {
        let mut contam = fresh_contam();
        contam.entries.push(ContamSource {
            amount: 0.6,
            color: ColorKind::Mellow,
            meridian_id: None,
            attacker_id: None,
            introduced_at: 0,
        });
        contam.entries.push(ContamSource {
            amount: 0.5,
            color: ColorKind::Mellow,
            meridian_id: None,
            attacker_id: None,
            introduced_at: 1,
        });
        // 总量 1.1 ≥ 1.0 阈值
        assert!(!can_take_pill(&contam, ColorKind::Mellow));
        assert!(can_take_pill(&contam, ColorKind::Violent));
    }

    #[test]
    fn combat_contamination_not_counted_as_drug() {
        let mut contam = fresh_contam();
        contam.entries.push(ContamSource {
            amount: 2.0,
            color: ColorKind::Mellow,
            meridian_id: None,
            attacker_id: Some("offline:Attacker".into()), // 战斗来源
            introduced_at: 0,
        });
        assert!(can_take_pill(&contam, ColorKind::Mellow));
        assert_eq!(sum_drug_toxin(&contam, ColorKind::Mellow), 0.0);
    }

    #[test]
    fn overdose_penalty_scales_with_excess() {
        let mut contam = fresh_contam();
        contam.entries.push(ContamSource {
            amount: 1.5, // 超 0.5
            color: ColorKind::Violent,
            meridian_id: None,
            attacker_id: None,
            introduced_at: 0,
        });
        let severity = overdose_penalty(&contam, ColorKind::Violent);
        assert!((severity - 0.05).abs() < 1e-9);
    }

    #[test]
    fn overdose_penalty_zero_below_threshold() {
        let mut contam = fresh_contam();
        contam.entries.push(ContamSource {
            amount: 0.8,
            color: ColorKind::Violent,
            meridian_id: None,
            attacker_id: None,
            introduced_at: 0,
        });
        assert_eq!(overdose_penalty(&contam, ColorKind::Violent), 0.0);
    }

    // ============== M5b Spoil 分支 ==============

    #[test]
    fn consume_pill_spoil_safe_same_as_normal() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Safe { current_qi: 80.0 },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 24.0);
        assert!(!outcome.blocked);
        assert_eq!(outcome.extra_toxin_added, 0.0);
        assert_eq!(contam.entries.len(), 1);
    }

    #[test]
    fn consume_pill_spoil_warn_adds_extra_contam() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        // current=25, threshold=50 → ratio=0.5 → extra = 0.3 × 0.5 × 1.0 = 0.15
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Warn {
                current_qi: 25.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 24.0);
        assert!(!outcome.blocked);
        assert!((outcome.extra_toxin_added - 0.15).abs() < 1e-9);
        assert_eq!(contam.entries.len(), 2);
        // 第二条 entry 应为 extra toxin，color 同基础
        assert_eq!(contam.entries[1].color, ColorKind::Mellow);
        assert!((contam.entries[1].amount - 0.15).abs() < 1e-9);
    }

    #[test]
    fn consume_pill_spoil_warn_edge_current_equals_threshold_zero_extra() {
        // current ≈ threshold → ratio=0 → extra=0（即便是 Warn 档亦然，边界场景）
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Warn {
                current_qi: 50.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.extra_toxin_added, 0.0);
        assert_eq!(contam.entries.len(), 1); // 仅基础，无 extra
    }

    #[test]
    fn consume_pill_spoil_warn_near_critical_near_full_extra() {
        // current=5, threshold=50 → ratio=0.9 → extra = 0.3 × 0.9 × 1.0 = 0.27
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Warn {
                current_qi: 5.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert!((outcome.extra_toxin_added - 0.27).abs() < 1e-9);
        assert_eq!(contam.entries.len(), 2);
    }

    #[test]
    fn consume_pill_spoil_critical_block_refuses_all_effects() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::CriticalBlock {
                current_qi: 2.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(outcome.qi_gained, 0.0);
        assert!(outcome.blocked);
        assert_eq!(outcome.extra_toxin_added, 0.0);
        // 无 contam 新增，qi 不变
        assert_eq!(contam.entries.len(), 0);
        assert_eq!(cult.qi_current, 50.0);
    }

    #[test]
    fn consume_pill_spoil_critical_block_force_consume_goes_through() {
        // Codex P2 (PR #38) 回归：CriticalBlock + force_consume=true 应按 Warn 公式消费，
        // 不再永久 blocked；plan §5.2 "拒绝自动消费，需玩家二次确认"的二次确认路径。
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        };
        // current=2, threshold=50 → ratio=0.96 → extra = 0.3 × 0.96 × 1.0 = 0.288
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::CriticalBlock {
                current_qi: 2.0,
                spoil_threshold: 50.0,
            },
            true,
            AgePeakCheck::NotApplicable,
        );
        assert!(!outcome.blocked, "force_consume should bypass block");
        assert_eq!(outcome.qi_gained, 24.0);
        assert!((outcome.extra_toxin_added - 0.288).abs() < 1e-9);
        // 基础 + extra = 2 条 contam
        assert_eq!(contam.entries.len(), 2);
        assert_eq!(cult.qi_current, 74.0);
    }

    #[test]
    fn consume_pill_force_consume_noop_when_not_critical() {
        // Safe / Warn / NotApplicable 下 force_consume 应无副作用（行为一致）
        let mut contam_a = fresh_contam();
        let mut cult_a = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let mut contam_b = fresh_contam();
        let mut cult_b = cult_a.clone();

        let a = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam_a,
            &mut cult_a,
            10,
            SpoilCheckOutcome::Safe { current_qi: 80.0 },
            false,
            AgePeakCheck::NotApplicable,
        );
        let b = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam_b,
            &mut cult_b,
            10,
            SpoilCheckOutcome::Safe { current_qi: 80.0 },
            true,
            AgePeakCheck::NotApplicable,
        );
        assert_eq!(a, b);
        assert_eq!(cult_a.qi_current, cult_b.qi_current);
        assert_eq!(contam_a.entries.len(), contam_b.entries.len());
    }

    #[test]
    fn consume_pill_spoil_warn_zero_threshold_defensive() {
        // 防御性：malformed spoil_threshold=0 时 ratio=1.0（完全腐败），不除零 panic
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Warn {
                current_qi: 0.0,
                spoil_threshold: 0.0,
            },
            false,
            AgePeakCheck::NotApplicable,
        );
        assert!((outcome.extra_toxin_added - 0.3).abs() < 1e-9);
    }

    // ============== M5d Age Peaking 分支 ==============

    #[test]
    fn age_peaking_applies_qi_bonus() {
        // Peaking bonus_strength=0.5 → qi_gain 24 × (1 + 0.5) = 36
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::Peaking {
                bonus_strength: 0.5,
            },
        );
        assert_eq!(outcome.qi_gained, 36.0);
        assert_eq!(outcome.age_bonus_applied, Some(0.5));
        assert!(!outcome.blocked);
        assert_eq!(cult.qi_current, 36.0);
    }

    #[test]
    fn age_not_peaking_no_bonus() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::NotPeaking,
        );
        assert_eq!(outcome.qi_gained, 24.0);
        assert_eq!(outcome.age_bonus_applied, None);
    }

    #[test]
    fn age_peaking_respects_qi_max_clamp() {
        // qi_max=100, qi_current=90, qi_gain=50 × 1.5 = 75 → 实际补 10
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 90.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(50.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::NotApplicable,
            false,
            AgePeakCheck::Peaking {
                bonus_strength: 0.5,
            },
        );
        assert_eq!(outcome.qi_gained, 10.0);
        assert_eq!(outcome.age_bonus_applied, Some(0.5));
        assert_eq!(cult.qi_current, 100.0);
    }

    #[test]
    fn blocked_suppresses_age_bonus() {
        // CriticalBlock + !force：blocked=true 且 age_bonus_applied=None（无消费 = 无加成）。
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 50.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::CriticalBlock {
                current_qi: 2.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::Peaking {
                bonus_strength: 0.5,
            },
        );
        assert!(outcome.blocked);
        assert_eq!(outcome.qi_gained, 0.0);
        assert_eq!(outcome.age_bonus_applied, None);
        assert_eq!(cult.qi_current, 50.0);
    }

    #[test]
    fn age_peaking_stacks_with_spoil_warn() {
        // 同时 Warn（额外 contam）和 Peaking（qi bonus）：两种效果叠加。
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        // Warn: current=25, threshold=50 → extra = 0.3 × 0.5 × 1.0 = 0.15
        // Peaking: bonus=0.5 → qi_gain = 24 × 1.5 = 36
        let outcome = consume_pill(
            &basic_effect(Some(24.0)),
            &mut contam,
            &mut cult,
            10,
            SpoilCheckOutcome::Warn {
                current_qi: 25.0,
                spoil_threshold: 50.0,
            },
            false,
            AgePeakCheck::Peaking {
                bonus_strength: 0.5,
            },
        );
        assert_eq!(outcome.qi_gained, 36.0);
        assert!((outcome.extra_toxin_added - 0.15).abs() < 1e-9);
        assert_eq!(outcome.age_bonus_applied, Some(0.5));
        assert_eq!(contam.entries.len(), 2);
    }

    #[test]
    fn mortal_pill_realm_scale_matches_combat_plan_breakpoints() {
        assert_eq!(mortal_pill_realm_scale(Realm::Awaken), (1.0, 1.0));
        assert_eq!(mortal_pill_realm_scale(Realm::Solidify), (0.5, 0.8));
        assert_eq!(mortal_pill_realm_scale(Realm::Spirit), (0.15, 0.6));
        assert_eq!(mortal_pill_realm_scale(Realm::Void), (0.05, 0.4));
        assert_eq!(
            scaled_grades(1, 0.15),
            0,
            "通灵服活血丹的凡药恢复等级应衰减到 0"
        );
        assert_eq!(
            scaled_grades(1, 0.4),
            0,
            "化虚服缩地散的腿伤副作用应衰减到 0"
        );
    }

    #[test]
    fn wound_heal_ignores_severed_like_wounds() {
        let mut wounds = Wounds {
            health_current: 50.0,
            ..Default::default()
        };
        wounds.entries.push(Wound {
            location: BodyPart::ArmL,
            kind: WoundKind::Cut,
            severity: 0.90,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });
        wounds.entries.push(Wound {
            location: BodyPart::Chest,
            kind: WoundKind::Cut,
            severity: 0.50,
            bleeding_per_sec: 1.0,
            created_at_tick: 0,
            inflicted_by: None,
        });

        let changed = apply_wound_heal(&mut wounds, None, 1);

        assert_eq!(changed, 1);
        assert!(wounds.entries.iter().any(|wound| {
            wound.location == BodyPart::ArmL && (wound.severity - 0.90).abs() < 1e-6
        }));
        assert!(wounds.entries.iter().any(|wound| {
            wound.location == BodyPart::Chest && (wound.severity - 0.25).abs() < 1e-6
        }));
    }

    #[test]
    fn severed_mend_downgrades_only_severed_target() {
        let mut wounds = Wounds::default();
        wounds.entries.push(Wound {
            location: BodyPart::ArmR,
            kind: WoundKind::Cut,
            severity: 0.92,
            bleeding_per_sec: 2.0,
            created_at_tick: 0,
            inflicted_by: None,
        });

        assert!(apply_severed_mend(&mut wounds, Some(BodyPart::ArmR), 1.0));

        let wound = &wounds.entries[0];
        assert_eq!(wound.location, BodyPart::ArmR);
        assert_eq!(wound.kind, WoundKind::Concussion);
        assert!((wound.severity - 0.55).abs() < 1e-6);
        assert!((wound.bleeding_per_sec - 0.7).abs() < 1e-6);
    }

    #[test]
    fn combat_pill_status_intents_scale_resist_and_qi_drain() {
        let entity = valence::prelude::Entity::from_raw(7);
        let tie_bi = combat_pill_spec("tie_bi_san").unwrap();
        let tie_bi_intents = combat_pill_status_intents(entity, tie_bi, 0.5, 0.8, 10);
        assert!(tie_bi_intents.iter().any(|intent| {
            intent.kind == StatusEffectKind::BodyPartResist(BodyPart::Chest)
                && (intent.magnitude - 0.20).abs() < 1e-6
        }));

        let hui_li = combat_pill_spec("hui_li_dan").unwrap();
        let hui_li_intents = combat_pill_status_intents(entity, hui_li, 1.0, 0.6, 10);
        assert!(hui_li_intents.iter().any(|intent| {
            intent.kind == StatusEffectKind::QiDrainForStamina
                && (intent.magnitude - 1.2).abs() < 1e-6
        }));
    }
}
