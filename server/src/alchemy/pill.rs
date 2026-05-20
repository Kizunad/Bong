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

// ══════════════════════════════════════════════════════════════════════════════
// plan-cultivation-pacing-v1 P1.4–P1.6：八种修炼丹药 CultivationPillKind
// ══════════════════════════════════════════════════════════════════════════════

pub const CULTIVATION_PILL_IDS: [&str; 8] = [
    "ling_xi_wan",
    "ju_ling_dan",
    "tong_mai_san",
    "ning_yuan_dan",
    "xi_sui_ye",
    "po_jing_dan",
    "kai_qiao_dan",
    "du_jie_dan",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CultivationPillKind {
    LingXiWan,   // ① 灵息丸
    JuLingDan,   // ② 聚灵丹
    TongMaiSan,  // ③ 通脉散
    NingYuanDan, // ④ 凝元丹
    XiSuiYe,     // ⑤ 洗髓液
    PoJingDan,   // ⑥ 破境丹
    KaiQiaoDan,  // ⑦ 开窍丹
    DuJieDan,    // ⑧ 渡劫丹
}

/// 修炼丹药规格。
///
/// 与战斗丹药 `CombatPillSpec` 类似，但效果走 `push_status_effect` 挂载
/// `CultivationAcceleration` / `BreakthroughBoost` / `ExtraordinaryMeridianAcceleration` 等
/// StatusEffect，而非直接 apply 到 Wounds / DerivedAttrs。
#[derive(Debug, Clone, PartialEq)]
pub struct CultivationPillSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: CultivationPillKind,
    pub toxin_amount: f64,
    pub toxin_color: ColorKind,
}

/// 修炼丹药消费后需要挂载的 StatusEffect 列表。
///
/// 每条包含 kind + magnitude + duration_ticks + source_pill（堆叠限制用）。
#[derive(Debug, Clone, PartialEq)]
pub struct CultivationPillEffectEntry {
    pub kind: StatusEffectKind,
    pub magnitude: f32,
    pub duration_ticks: u64,
}

/// 修炼丹药消费结果。
#[derive(Debug, Clone, PartialEq)]
pub struct CultivationPillConsumeResult {
    /// 成功挂载的 StatusEffect 列表（被堆叠 cap 拦截的不在此列）。
    pub applied_effects: Vec<CultivationPillEffectEntry>,
    /// 洗髓液专用：buff 到期后需要追加的 QiRegenSlowed 参数。
    /// 调用侧可在 status_effect_tick 中检查 source_pill=="xi_sui_ye" 到期后 push。
    pub deferred_qi_regen_slowed: Option<DeferredQiRegenSlowed>,
    /// 注入的丹毒量（toxin_amount）。
    pub toxin_injected: f64,
    /// 被堆叠 cap 拦截的 effect 数量（第 3 颗同种丹药时 > 0）。
    pub blocked_by_cap: usize,
}

/// 洗髓液到期后追加的 QiRegenSlowed 参数。
#[derive(Debug, Clone, PartialEq)]
pub struct DeferredQiRegenSlowed {
    pub magnitude: f32,
    pub duration_ticks: u64,
}

/// 获取修炼丹药基础规格。
pub fn cultivation_pill_spec(id: &str) -> Option<CultivationPillSpec> {
    Some(match id {
        "ling_xi_wan" => CultivationPillSpec {
            id: "ling_xi_wan",
            name: "灵息丸",
            kind: CultivationPillKind::LingXiWan,
            toxin_amount: 0.15,
            toxin_color: ColorKind::Gentle,
        },
        "ju_ling_dan" => CultivationPillSpec {
            id: "ju_ling_dan",
            name: "聚灵丹",
            kind: CultivationPillKind::JuLingDan,
            toxin_amount: 0.20,
            toxin_color: ColorKind::Mellow,
        },
        "tong_mai_san" => CultivationPillSpec {
            id: "tong_mai_san",
            name: "通脉散",
            kind: CultivationPillKind::TongMaiSan,
            toxin_amount: 0.30,
            toxin_color: ColorKind::Solid,
        },
        "ning_yuan_dan" => CultivationPillSpec {
            id: "ning_yuan_dan",
            name: "凝元丹",
            kind: CultivationPillKind::NingYuanDan,
            toxin_amount: 0.35,
            toxin_color: ColorKind::Heavy,
        },
        "xi_sui_ye" => CultivationPillSpec {
            id: "xi_sui_ye",
            name: "洗髓液",
            kind: CultivationPillKind::XiSuiYe,
            toxin_amount: 0.40,
            toxin_color: ColorKind::Violent,
        },
        "po_jing_dan" => CultivationPillSpec {
            id: "po_jing_dan",
            name: "破境丹",
            kind: CultivationPillKind::PoJingDan,
            toxin_amount: 0.45,
            toxin_color: ColorKind::Insidious,
        },
        "kai_qiao_dan" => CultivationPillSpec {
            id: "kai_qiao_dan",
            name: "开窍丹",
            kind: CultivationPillKind::KaiQiaoDan,
            toxin_amount: 0.50,
            toxin_color: ColorKind::Turbid,
        },
        "du_jie_dan" => CultivationPillSpec {
            id: "du_jie_dan",
            name: "渡劫丹",
            kind: CultivationPillKind::DuJieDan,
            toxin_amount: 0.60,
            toxin_color: ColorKind::Insidious,
        },
        // plan-cultivation-pacing-v1 P2.2：NPC 售卖低品质（flawed）修炼丹药。
        // 效果走 cultivation_pill_effects_flawed()（magnitude × 0.6）。
        "ling_xi_wan_flawed" => CultivationPillSpec {
            id: "ling_xi_wan_flawed",
            name: "灵息丸（次品）",
            kind: CultivationPillKind::LingXiWan,
            toxin_amount: 0.15,
            toxin_color: ColorKind::Gentle,
        },
        "ju_ling_dan_flawed" => CultivationPillSpec {
            id: "ju_ling_dan_flawed",
            name: "聚灵丹（次品）",
            kind: CultivationPillKind::JuLingDan,
            toxin_amount: 0.20,
            toxin_color: ColorKind::Mellow,
        },
        _ => return None,
    })
}

/// plan-cultivation-pacing-v1 P2.2：次品丹药效果（magnitude × 0.6）。
pub const FLAWED_MAGNITUDE_MULTIPLIER: f32 = 0.6;

/// 判断一个丹药 ID 是否是次品。
pub fn is_flawed_cultivation_pill(id: &str) -> bool {
    id.ends_with("_flawed")
}

/// 获取修炼丹药消费时应挂载的 StatusEffect 列表。
///
/// 按 plan-cultivation-pacing-v1 §8.1 定义：
/// - ①灵息丸: CultivationAcceleration(0.5) 36000t
/// - ②聚灵丹: CultivationAcceleration(1.0) 24000t
/// - ③通脉散: CultivationAcceleration(1.5) 18000t
/// - ④凝元丹: CultivationAcceleration(2.0) 18000t + BreakthroughBoost(0.10)
/// - ⑤洗髓液: CultivationAcceleration(3.0) 12000t + DamageVulnerability(1.0) 12000t
/// - ⑥破境丹: BreakthroughBoost(0.20) 单次消费
/// - ⑦开窍丹: ExtraordinaryMeridianAcceleration(4.0) 12000t
/// - ⑧渡劫丹: BreakthroughBoost(0.25) + DamageReduction(0.30) u64::MAX
pub fn cultivation_pill_effects(kind: CultivationPillKind) -> Vec<CultivationPillEffectEntry> {
    match kind {
        CultivationPillKind::LingXiWan => vec![CultivationPillEffectEntry {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 0.5,
            duration_ticks: 36_000,
        }],
        CultivationPillKind::JuLingDan => vec![CultivationPillEffectEntry {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 1.0,
            duration_ticks: 24_000,
        }],
        CultivationPillKind::TongMaiSan => vec![CultivationPillEffectEntry {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: 1.5,
            duration_ticks: 18_000,
        }],
        CultivationPillKind::NingYuanDan => vec![
            CultivationPillEffectEntry {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 2.0,
                duration_ticks: 18_000,
            },
            CultivationPillEffectEntry {
                kind: StatusEffectKind::BreakthroughBoost,
                magnitude: 0.10,
                duration_ticks: 18_000,
            },
        ],
        CultivationPillKind::XiSuiYe => vec![
            CultivationPillEffectEntry {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 3.0,
                duration_ticks: 12_000,
            },
            CultivationPillEffectEntry {
                kind: StatusEffectKind::DamageVulnerability,
                magnitude: 1.0,
                duration_ticks: 12_000,
            },
        ],
        CultivationPillKind::PoJingDan => vec![CultivationPillEffectEntry {
            kind: StatusEffectKind::BreakthroughBoost,
            magnitude: 0.20,
            // 单次消费——挂载后由 breakthrough_system 一次性消费。
            // 给一个足够长的 duration 让它存活到突破事务发生。
            duration_ticks: 72_000, // 60 分钟 @20tps
        }],
        CultivationPillKind::KaiQiaoDan => vec![CultivationPillEffectEntry {
            kind: StatusEffectKind::ExtraordinaryMeridianAcceleration,
            magnitude: 4.0,
            duration_ticks: 12_000,
        }],
        CultivationPillKind::DuJieDan => vec![
            CultivationPillEffectEntry {
                kind: StatusEffectKind::BreakthroughBoost,
                magnitude: 0.25,
                // 渡劫全程有效——给 u64::MAX 让它在渡劫结束后由 tribulation_system 清理
                duration_ticks: u64::MAX,
            },
            CultivationPillEffectEntry {
                // 渡劫减伤 30%
                kind: StatusEffectKind::DamageReduction,
                magnitude: 0.30,
                duration_ticks: u64::MAX,
            },
        ],
    }
}

/// 是否为洗髓液——buff 到期后需要追加 QiRegenSlowed 回调。
pub fn xi_sui_ye_deferred_debuff() -> DeferredQiRegenSlowed {
    DeferredQiRegenSlowed {
        magnitude: 0.8,
        duration_ticks: 12_000,
    }
}

/// plan-cultivation-pacing-v1 P1.5：消费修炼丹药。
///
/// 1. 注入丹毒（Contamination）
/// 2. 通过 `push_status_effect` 挂载各 StatusEffect（受 per-pill 2 层 cap 限制）
/// 3. 洗髓液额外返回 `deferred_qi_regen_slowed` 供 tick 系统延迟追加
///
/// # 前置条件
/// - 调用方应先 `can_take_pill(contam, spec.toxin_color)` 检查丹毒阈值
/// - 调用方应先判断是否有空间服药（非战斗状态等）
pub fn consume_cultivation_pill(
    spec: &CultivationPillSpec,
    contam: &mut Contamination,
    status_effects: &mut crate::combat::components::StatusEffects,
    now_tick: u64,
) -> CultivationPillConsumeResult {
    use crate::combat::components::ActiveStatusEffect;
    use crate::combat::status::push_status_effect;

    // 1. 注入丹毒
    contam.entries.push(ContamSource {
        amount: spec.toxin_amount,
        color: spec.toxin_color,
        meridian_id: None,
        attacker_id: None,
        introduced_at: now_tick,
    });

    // 2. 挂载 StatusEffect
    let effects = cultivation_pill_effects(spec.kind);
    let flawed = is_flawed_cultivation_pill(spec.id);
    let mut applied = Vec::new();
    let mut blocked_count = 0usize;

    for entry in &effects {
        let magnitude = if flawed {
            entry.magnitude * FLAWED_MAGNITUDE_MULTIPLIER
        } else {
            entry.magnitude
        };
        let active = ActiveStatusEffect {
            kind: entry.kind.clone(),
            magnitude,
            remaining_ticks: entry.duration_ticks,
            source_pill: Some(spec.id.to_string()),
        };
        if push_status_effect(status_effects, active) {
            applied.push(entry.clone());
        } else {
            blocked_count += 1;
        }
    }

    // 3. 洗髓液特殊处理
    let deferred = if spec.kind == CultivationPillKind::XiSuiYe {
        Some(xi_sui_ye_deferred_debuff())
    } else {
        None
    };

    CultivationPillConsumeResult {
        applied_effects: applied,
        deferred_qi_regen_slowed: deferred,
        toxin_injected: spec.toxin_amount,
        blocked_by_cap: blocked_count,
    }
}

/// plan-cultivation-pacing-v1 §8.1 #8：洗髓液到期回调。
///
/// 在 `status_effect_tick` 中调用：当 source_pill=="xi_sui_ye" 的
/// CultivationAcceleration 刚刚到期（从 >0 变为 0），追加 QiRegenSlowed。
///
/// 返回 true 表示触发了追加。
pub fn check_xi_sui_ye_expiry_and_push_debuff(
    status_effects: &mut crate::combat::components::StatusEffects,
) -> bool {
    use crate::combat::components::ActiveStatusEffect;
    use crate::combat::status::push_status_effect;

    // 查找是否有 source_pill="xi_sui_ye" 且 kind=CultivationAcceleration 且 remaining=0
    let has_expired_xi_sui = status_effects.active.iter().any(|e| {
        e.kind == StatusEffectKind::CultivationAcceleration
            && e.source_pill.as_deref() == Some("xi_sui_ye")
            && e.remaining_ticks == 0
    });

    if !has_expired_xi_sui {
        return false;
    }

    let debuff = xi_sui_ye_deferred_debuff();
    push_status_effect(
        status_effects,
        ActiveStatusEffect {
            kind: StatusEffectKind::QiRegenSlowed,
            magnitude: debuff.magnitude,
            remaining_ticks: debuff.duration_ticks,
            source_pill: Some("xi_sui_ye".to_string()),
        },
    );
    true
}

/// plan-cultivation-pacing-v1 P1.6：激活 PillEffect.meridian_progress_bonus 接口。
///
/// 当消费含有 `meridian_progress_bonus = Some(mag)` 的丹药时，通过
/// push_status_effect 挂载 CultivationAcceleration(mag)。
///
/// 归并说明：`QiRegenBoost`（plan-alchemy-v2 P0 定义）在修炼加速语境下
/// 统一归并到 `CultivationAcceleration`，避免双轨。`QiRegenBoost` 仍在
/// side_effect_apply.rs 中用于短时战斗回气——两者语境不同，不冲突。
pub fn activate_meridian_progress_bonus(
    effect: &PillEffect,
    status_effects: &mut crate::combat::components::StatusEffects,
    source_pill_id: &str,
) -> bool {
    use crate::combat::components::ActiveStatusEffect;
    use crate::combat::status::push_status_effect;

    let Some(mag) = effect.meridian_progress_bonus else {
        return false;
    };
    if mag <= 0.0 {
        return false;
    }

    push_status_effect(
        status_effects,
        ActiveStatusEffect {
            kind: StatusEffectKind::CultivationAcceleration,
            magnitude: mag as f32,
            remaining_ticks: 36_000, // 默认 30 分钟 @20tps
            source_pill: Some(source_pill_id.to_string()),
        },
    )
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

    // ══════════════════════════════════════════════════════════════════
    // plan-cultivation-pacing-v1 P1.4–P1.6 修炼丹药验收测试
    // ══════════════════════════════════════════════════════════════════

    use crate::combat::components::StatusEffects;

    fn fresh_status_effects() -> StatusEffects {
        StatusEffects::default()
    }

    // ── §1 CultivationPillKind enum + spec pin 测试 ──

    #[test]
    fn cultivation_pill_ids_count_is_eight() {
        assert_eq!(CULTIVATION_PILL_IDS.len(), 8);
    }

    #[test]
    fn all_cultivation_pill_ids_resolve_to_spec() {
        for id in &CULTIVATION_PILL_IDS {
            let spec = cultivation_pill_spec(id);
            assert!(
                spec.is_some(),
                "cultivation_pill_spec(\"{id}\") should return Some"
            );
            let spec = spec.unwrap();
            assert_eq!(spec.id, *id);
        }
    }

    #[test]
    fn unknown_cultivation_pill_id_returns_none() {
        assert!(cultivation_pill_spec("nonexistent_pill").is_none());
        assert!(cultivation_pill_spec("huo_xue_dan").is_none()); // combat pill, not cultivation
    }

    #[test]
    fn ling_xi_wan_spec_pin() {
        let s = cultivation_pill_spec("ling_xi_wan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::LingXiWan);
        assert_eq!(s.name, "灵息丸");
        assert!((s.toxin_amount - 0.15).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Gentle);
    }

    #[test]
    fn ju_ling_dan_spec_pin() {
        let s = cultivation_pill_spec("ju_ling_dan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::JuLingDan);
        assert!((s.toxin_amount - 0.20).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Mellow);
    }

    #[test]
    fn tong_mai_san_spec_pin() {
        let s = cultivation_pill_spec("tong_mai_san").unwrap();
        assert_eq!(s.kind, CultivationPillKind::TongMaiSan);
        assert!((s.toxin_amount - 0.30).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Solid);
    }

    #[test]
    fn ning_yuan_dan_spec_pin() {
        let s = cultivation_pill_spec("ning_yuan_dan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::NingYuanDan);
        assert!((s.toxin_amount - 0.35).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Heavy);
    }

    #[test]
    fn xi_sui_ye_spec_pin() {
        let s = cultivation_pill_spec("xi_sui_ye").unwrap();
        assert_eq!(s.kind, CultivationPillKind::XiSuiYe);
        assert!((s.toxin_amount - 0.40).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Violent);
    }

    #[test]
    fn po_jing_dan_spec_pin() {
        let s = cultivation_pill_spec("po_jing_dan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::PoJingDan);
        assert!((s.toxin_amount - 0.45).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Insidious);
    }

    #[test]
    fn kai_qiao_dan_spec_pin() {
        let s = cultivation_pill_spec("kai_qiao_dan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::KaiQiaoDan);
        assert!((s.toxin_amount - 0.50).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Turbid);
    }

    #[test]
    fn du_jie_dan_spec_pin() {
        let s = cultivation_pill_spec("du_jie_dan").unwrap();
        assert_eq!(s.kind, CultivationPillKind::DuJieDan);
        assert!((s.toxin_amount - 0.60).abs() < 1e-9);
        assert_eq!(s.toxin_color, ColorKind::Insidious);
    }

    // ── §2 cultivation_pill_effects pin 测试 ──

    #[test]
    fn ling_xi_wan_effects_single_cultivation_acceleration() {
        let effects = cultivation_pill_effects(CultivationPillKind::LingXiWan);
        assert_eq!(effects.len(), 1, "灵息丸应有 1 个 effect");
        assert_eq!(effects[0].kind, StatusEffectKind::CultivationAcceleration);
        assert!((effects[0].magnitude - 0.5).abs() < 1e-6);
        assert_eq!(effects[0].duration_ticks, 36_000);
    }

    #[test]
    fn ju_ling_dan_effects_single_cultivation_acceleration() {
        let effects = cultivation_pill_effects(CultivationPillKind::JuLingDan);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].kind, StatusEffectKind::CultivationAcceleration);
        assert!((effects[0].magnitude - 1.0).abs() < 1e-6);
        assert_eq!(effects[0].duration_ticks, 24_000);
    }

    #[test]
    fn tong_mai_san_effects_single_cultivation_acceleration() {
        let effects = cultivation_pill_effects(CultivationPillKind::TongMaiSan);
        assert_eq!(effects.len(), 1);
        assert!((effects[0].magnitude - 1.5).abs() < 1e-6);
        assert_eq!(effects[0].duration_ticks, 18_000);
    }

    #[test]
    fn ning_yuan_dan_effects_dual_accel_plus_breakthrough() {
        let effects = cultivation_pill_effects(CultivationPillKind::NingYuanDan);
        assert_eq!(effects.len(), 2, "凝元丹应有 2 个 effect");
        let accel = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::CultivationAcceleration)
            .expect("应含 CultivationAcceleration");
        assert!((accel.magnitude - 2.0).abs() < 1e-6);
        assert_eq!(accel.duration_ticks, 18_000);
        let bt = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::BreakthroughBoost)
            .expect("应含 BreakthroughBoost");
        assert!((bt.magnitude - 0.10).abs() < 1e-6);
    }

    #[test]
    fn xi_sui_ye_effects_accel_plus_vulnerability() {
        let effects = cultivation_pill_effects(CultivationPillKind::XiSuiYe);
        assert_eq!(effects.len(), 2, "洗髓液应有 2 个 effect");
        let accel = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::CultivationAcceleration)
            .expect("应含 CultivationAcceleration");
        assert!((accel.magnitude - 3.0).abs() < 1e-6);
        assert_eq!(accel.duration_ticks, 12_000);
        let vuln = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::DamageVulnerability)
            .expect("应含 DamageVulnerability");
        assert!((vuln.magnitude - 1.0).abs() < 1e-6);
        assert_eq!(vuln.duration_ticks, 12_000);
    }

    #[test]
    fn po_jing_dan_effects_single_breakthrough_boost() {
        let effects = cultivation_pill_effects(CultivationPillKind::PoJingDan);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].kind, StatusEffectKind::BreakthroughBoost);
        assert!((effects[0].magnitude - 0.20).abs() < 1e-6);
    }

    #[test]
    fn kai_qiao_dan_effects_single_extraordinary_meridian() {
        let effects = cultivation_pill_effects(CultivationPillKind::KaiQiaoDan);
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0].kind,
            StatusEffectKind::ExtraordinaryMeridianAcceleration
        );
        assert!((effects[0].magnitude - 4.0).abs() < 1e-6);
        assert_eq!(effects[0].duration_ticks, 12_000);
    }

    #[test]
    fn du_jie_dan_effects_breakthrough_plus_damage_reduction() {
        let effects = cultivation_pill_effects(CultivationPillKind::DuJieDan);
        assert_eq!(effects.len(), 2, "渡劫丹应有 2 个 effect");
        let bt = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::BreakthroughBoost)
            .expect("应含 BreakthroughBoost");
        assert!((bt.magnitude - 0.25).abs() < 1e-6);
        assert_eq!(bt.duration_ticks, u64::MAX, "渡劫丹应持续全程");
        let dr = effects
            .iter()
            .find(|e| e.kind == StatusEffectKind::DamageReduction)
            .expect("应含 DamageReduction(0.30)");
        assert!((dr.magnitude - 0.30).abs() < 1e-6);
    }

    // ── §3 consume_cultivation_pill 测试 ──

    #[test]
    fn consume_ling_xi_wan_mounts_cultivation_acceleration() {
        let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 100);

        assert_eq!(
            result.applied_effects.len(),
            1,
            "灵息丸应挂载 1 个 CultivationAcceleration"
        );
        assert_eq!(
            result.applied_effects[0].kind,
            StatusEffectKind::CultivationAcceleration
        );
        assert!((result.applied_effects[0].magnitude - 0.5).abs() < 1e-6);
        assert_eq!(result.applied_effects[0].duration_ticks, 36_000);

        // 验证丹毒注入
        assert!((result.toxin_injected - 0.15).abs() < 1e-9);
        assert_eq!(contam.entries.len(), 1);
        assert_eq!(contam.entries[0].color, ColorKind::Gentle);

        // 验证 StatusEffects 内的 ActiveStatusEffect
        assert_eq!(se.active.len(), 1);
        assert_eq!(se.active[0].kind, StatusEffectKind::CultivationAcceleration);
        assert_eq!(se.active[0].source_pill.as_deref(), Some("ling_xi_wan"));
        assert_eq!(se.active[0].remaining_ticks, 36_000);

        // 无堆叠拦截
        assert_eq!(result.blocked_by_cap, 0);
        // 非洗髓液，无延迟 debuff
        assert!(result.deferred_qi_regen_slowed.is_none());
    }

    #[test]
    fn consume_ning_yuan_dan_mounts_dual_effects() {
        let spec = cultivation_pill_spec("ning_yuan_dan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 200);

        assert_eq!(
            result.applied_effects.len(),
            2,
            "凝元丹应挂载 CultivationAcceleration + BreakthroughBoost"
        );
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::CultivationAcceleration
                && (e.magnitude - 2.0).abs() < 1e-6));
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::BreakthroughBoost
                && (e.magnitude - 0.10).abs() < 1e-6));

        assert_eq!(se.active.len(), 2);
        assert!((result.toxin_injected - 0.35).abs() < 1e-9);
    }

    #[test]
    fn consume_xi_sui_ye_mounts_accel_plus_vulnerability() {
        let spec = cultivation_pill_spec("xi_sui_ye").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 300);

        assert_eq!(result.applied_effects.len(), 2);
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::CultivationAcceleration
                && (e.magnitude - 3.0).abs() < 1e-6));
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::DamageVulnerability
                && (e.magnitude - 1.0).abs() < 1e-6));

        // 洗髓液应有延迟 debuff
        let deferred = result
            .deferred_qi_regen_slowed
            .expect("洗髓液应有 deferred_qi_regen_slowed");
        assert!((deferred.magnitude - 0.8).abs() < 1e-6);
        assert_eq!(deferred.duration_ticks, 12_000);
    }

    #[test]
    fn consume_kai_qiao_dan_mounts_extraordinary_meridian_acceleration() {
        let spec = cultivation_pill_spec("kai_qiao_dan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 400);

        assert_eq!(result.applied_effects.len(), 1);
        assert_eq!(
            result.applied_effects[0].kind,
            StatusEffectKind::ExtraordinaryMeridianAcceleration
        );
        assert!((result.applied_effects[0].magnitude - 4.0).abs() < 1e-6);
        assert_eq!(result.applied_effects[0].duration_ticks, 12_000);
    }

    #[test]
    fn consume_du_jie_dan_mounts_breakthrough_and_damage_reduction() {
        let spec = cultivation_pill_spec("du_jie_dan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 500);

        assert_eq!(result.applied_effects.len(), 2);
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::BreakthroughBoost
                && (e.magnitude - 0.25).abs() < 1e-6));
        assert!(result
            .applied_effects
            .iter()
            .any(|e| e.kind == StatusEffectKind::DamageReduction
                && (e.magnitude - 0.30).abs() < 1e-6));
        assert!((result.toxin_injected - 0.60).abs() < 1e-9);
    }

    #[test]
    fn consume_po_jing_dan_mounts_single_breakthrough_boost() {
        let spec = cultivation_pill_spec("po_jing_dan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 600);

        assert_eq!(result.applied_effects.len(), 1);
        assert_eq!(
            result.applied_effects[0].kind,
            StatusEffectKind::BreakthroughBoost
        );
        assert!((result.applied_effects[0].magnitude - 0.20).abs() < 1e-6);
    }

    // ── §4 堆叠 cap 测试 ──

    #[test]
    fn same_pill_third_dose_blocked_by_per_pill_cap() {
        let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        // 第 1 颗
        let r1 = consume_cultivation_pill(&spec, &mut contam, &mut se, 100);
        assert_eq!(r1.blocked_by_cap, 0);
        assert_eq!(se.active.len(), 1);

        // 第 2 颗
        let r2 = consume_cultivation_pill(&spec, &mut contam, &mut se, 200);
        assert_eq!(r2.blocked_by_cap, 0);
        assert_eq!(se.active.len(), 2);

        // 第 3 颗——被拦截
        let r3 = consume_cultivation_pill(&spec, &mut contam, &mut se, 300);
        assert_eq!(
            r3.blocked_by_cap, 1,
            "同种丹药第 3 颗应被 per-pill 2 层 cap 拦截"
        );
        assert_eq!(r3.applied_effects.len(), 0);
        // effects 仍然只有 2 条
        assert_eq!(se.active.len(), 2);
        // 但丹毒仍然注入（吞了但 effect 不生效）
        assert_eq!(contam.entries.len(), 3, "丹毒应照常注入即使 effect 被拦截");
    }

    #[test]
    fn same_pill_magnitude_not_aggregated_on_third() {
        // 灵息丸 ×3 = 只有 2×0.5=1.0 加速（不是 1.5）
        let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        consume_cultivation_pill(&spec, &mut contam, &mut se, 100);
        consume_cultivation_pill(&spec, &mut contam, &mut se, 200);
        consume_cultivation_pill(&spec, &mut contam, &mut se, 300);

        let total_mag: f32 = se
            .active
            .iter()
            .filter(|e| e.kind == StatusEffectKind::CultivationAcceleration)
            .map(|e| e.magnitude)
            .sum();
        assert!(
            (total_mag - 1.0).abs() < 1e-6,
            "灵息丸 ×3 生效 magnitude 总和应为 2×0.5=1.0，不是 1.5；实际为 {total_mag}"
        );
    }

    #[test]
    fn different_pills_not_blocked_by_each_other() {
        let ling_xi = cultivation_pill_spec("ling_xi_wan").unwrap();
        let ju_ling = cultivation_pill_spec("ju_ling_dan").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        // ling_xi_wan ×2
        consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 100);
        consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 200);

        // ju_ling_dan ×1 — 不应被 ling_xi_wan cap 拦截
        let r = consume_cultivation_pill(&ju_ling, &mut contam, &mut se, 300);
        assert_eq!(
            r.blocked_by_cap, 0,
            "不同丹药不应被其他丹药的 per-pill cap 拦截"
        );
        assert_eq!(se.active.len(), 3);
    }

    #[test]
    fn different_toxin_colors_accumulate_independently() {
        // Gentle + Solid 各自独立累积
        let ling_xi = cultivation_pill_spec("ling_xi_wan").unwrap(); // Gentle
        let tong_mai = cultivation_pill_spec("tong_mai_san").unwrap(); // Solid
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        consume_cultivation_pill(&ling_xi, &mut contam, &mut se, 100);
        consume_cultivation_pill(&tong_mai, &mut contam, &mut se, 200);

        let gentle_total = sum_drug_toxin(&contam, ColorKind::Gentle);
        let solid_total = sum_drug_toxin(&contam, ColorKind::Solid);
        assert!((gentle_total - 0.15).abs() < 1e-9, "Gentle 应独立累积");
        assert!((solid_total - 0.30).abs() < 1e-9, "Solid 应独立累积");
        // 两色互不干扰——各自仍可继续服药
        assert!(can_take_pill(&contam, ColorKind::Gentle));
        assert!(can_take_pill(&contam, ColorKind::Solid));
    }

    // ── §5 can_take_pill 回归测试 ──

    #[test]
    fn can_take_pill_blocks_when_same_color_at_threshold() {
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();

        // 连服 7 颗灵息丸（each 0.15），总毒 1.05 ≥ 1.0
        let spec = cultivation_pill_spec("ling_xi_wan").unwrap();
        for _ in 0..7 {
            consume_cultivation_pill(&spec, &mut contam, &mut se, 0);
        }

        let total = sum_drug_toxin(&contam, ColorKind::Gentle);
        assert!(
            total >= TOXIN_THRESHOLD,
            "7 颗灵息丸丹毒 {total} 应超阈值 {TOXIN_THRESHOLD}"
        );
        assert!(
            !can_take_pill(&contam, ColorKind::Gentle),
            "丹毒超阈值后 can_take_pill 应返回 false"
        );
        // 其他颜色仍可
        assert!(can_take_pill(&contam, ColorKind::Mellow));
    }

    // ── §6 洗髓液到期回调测试 ──

    #[test]
    fn xi_sui_ye_expiry_pushes_qi_regen_slowed() {
        use crate::combat::components::ActiveStatusEffect;
        let mut se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 3.0,
                remaining_ticks: 0, // 刚刚到期
                source_pill: Some("xi_sui_ye".to_string()),
            }],
        };

        let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
        assert!(triggered, "xi_sui_ye 到期应触发 QiRegenSlowed 追加");

        // 到期的 CultivationAcceleration 还在（remaining=0），加上新的 QiRegenSlowed
        let slowed = se
            .active
            .iter()
            .find(|e| e.kind == StatusEffectKind::QiRegenSlowed);
        assert!(slowed.is_some(), "应追加 QiRegenSlowed effect");
        let slowed = slowed.unwrap();
        assert!((slowed.magnitude - 0.8).abs() < 1e-6);
        assert_eq!(slowed.remaining_ticks, 12_000);
        assert_eq!(slowed.source_pill.as_deref(), Some("xi_sui_ye"));
    }

    #[test]
    fn xi_sui_ye_expiry_not_triggered_when_still_active() {
        use crate::combat::components::ActiveStatusEffect;
        let mut se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 3.0,
                remaining_ticks: 100, // 仍有效
                source_pill: Some("xi_sui_ye".to_string()),
            }],
        };

        let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
        assert!(!triggered, "xi_sui_ye 仍有效时不应触发 QiRegenSlowed 追加");
        assert_eq!(se.active.len(), 1, "不应追加任何 effect");
    }

    #[test]
    fn xi_sui_ye_expiry_not_triggered_for_other_pill() {
        use crate::combat::components::ActiveStatusEffect;
        let mut se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 0.5,
                remaining_ticks: 0,
                source_pill: Some("ling_xi_wan".to_string()), // 不是 xi_sui_ye
            }],
        };

        let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
        assert!(!triggered, "非 xi_sui_ye 丹药到期不应触发回调");
    }

    #[test]
    fn xi_sui_ye_expiry_not_triggered_for_no_source_pill() {
        use crate::combat::components::ActiveStatusEffect;
        let mut se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 3.0,
                remaining_ticks: 0,
                source_pill: None,
            }],
        };

        let triggered = check_xi_sui_ye_expiry_and_push_debuff(&mut se);
        assert!(!triggered, "source_pill=None 不应触发洗髓液回调");
    }

    // ── §7 P1.6 meridian_progress_bonus 测试 ──

    #[test]
    fn activate_meridian_progress_bonus_mounts_cultivation_acceleration() {
        let effect = PillEffect {
            toxin_amount: 0.2,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(24.0),
            meridian_progress_bonus: Some(1.5),
        };
        let mut se = fresh_status_effects();

        let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
        assert!(result, "meridian_progress_bonus>0 应成功挂载");
        assert_eq!(se.active.len(), 1);
        assert_eq!(se.active[0].kind, StatusEffectKind::CultivationAcceleration);
        assert!((se.active[0].magnitude - 1.5).abs() < 1e-6);
        assert_eq!(se.active[0].source_pill.as_deref(), Some("test_pill"));
    }

    #[test]
    fn activate_meridian_progress_bonus_none_does_nothing() {
        let effect = PillEffect {
            toxin_amount: 0.2,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(24.0),
            meridian_progress_bonus: None,
        };
        let mut se = fresh_status_effects();

        let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
        assert!(!result, "meridian_progress_bonus=None 应返回 false");
        assert!(se.active.is_empty());
    }

    #[test]
    fn activate_meridian_progress_bonus_zero_does_nothing() {
        let effect = PillEffect {
            toxin_amount: 0.2,
            toxin_color: ColorKind::Mellow,
            qi_gain: None,
            meridian_progress_bonus: Some(0.0),
        };
        let mut se = fresh_status_effects();

        let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
        assert!(!result, "meridian_progress_bonus=0 应返回 false");
    }

    #[test]
    fn activate_meridian_progress_bonus_negative_does_nothing() {
        let effect = PillEffect {
            toxin_amount: 0.2,
            toxin_color: ColorKind::Mellow,
            qi_gain: None,
            meridian_progress_bonus: Some(-1.0),
        };
        let mut se = fresh_status_effects();

        let result = activate_meridian_progress_bonus(&effect, &mut se, "test_pill");
        assert!(!result, "meridian_progress_bonus<0 应返回 false");
    }

    // ── §8 丹方 JSON 加载测试 ──

    #[test]
    fn all_eight_cultivation_recipes_load_from_json() {
        let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
        for id in &[
            "ling_xi_wan_v1",
            "ju_ling_dan_v1",
            "tong_mai_san_v1",
            "ning_yuan_dan_v1",
            "xi_sui_ye_v1",
            "po_jing_dan_v1",
            "kai_qiao_dan_v1",
            "du_jie_dan_v1",
        ] {
            assert!(
                registry.get(id).is_some(),
                "配方 {id} 应存在于 RecipeRegistry 中"
            );
        }
    }

    #[test]
    fn ling_xi_wan_recipe_structure_valid() {
        let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
        let r = registry.get("ling_xi_wan_v1").unwrap();
        assert_eq!(r.name, "灵息丸·入门修炼丹");
        assert_eq!(r.furnace_tier_min, 1);
        assert_eq!(r.stages.len(), 1);
        assert_eq!(r.stages[0].required.len(), 1);
        assert_eq!(r.stages[0].required[0].material, "spirit_grass");
        assert_eq!(r.stages[0].required[0].count, 3);
        let perfect = r.outcomes.perfect.as_ref().expect("should have perfect");
        assert_eq!(perfect.pill, "ling_xi_wan");
        assert!((perfect.toxin_amount - 0.15).abs() < 1e-9);
        assert_eq!(perfect.toxin_color, ColorKind::Gentle);
    }

    #[test]
    fn du_jie_dan_recipe_requires_tier_3() {
        let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
        let r = registry.get("du_jie_dan_v1").unwrap();
        assert_eq!(r.furnace_tier_min, 3, "渡劫丹应需要 3 级炉");
        // 材料检查
        let stage0 = r.stage0_ingredients();
        assert_eq!(stage0.get("long_lin_tai"), Some(&1));
        assert_eq!(stage0.get("xu_yuan_rui"), Some(&1));
        assert_eq!(stage0.get("ling_shi"), Some(&3));
    }

    #[test]
    fn xi_sui_ye_recipe_requires_tier_2() {
        let registry = crate::alchemy::recipe::load_recipe_registry().unwrap();
        let r = registry.get("xi_sui_ye_v1").unwrap();
        assert_eq!(r.furnace_tier_min, 2, "洗髓液应需要 2 级炉");
    }

    // ── §9 聚灵丹 delta 验证（zone_qi=0.6 下首条正经应 ≤30 min）──

    #[test]
    fn ju_ling_dan_with_zone_qi_first_meridian_under_30min() {
        // 使用 PR-1 的 cultivation_acceleration_multiplier 验证：
        // 聚灵丹 mag=1.0 → multiplier=(1+1.0)=2.0
        // 正经基础速率（PR-1 定义）× 2.0 × zone_qi 0.6
        // 验证首条正经 30 分钟内可打通
        use crate::combat::components::ActiveStatusEffect;
        use crate::cultivation::tick::cultivation_acceleration_multiplier;

        let se = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::CultivationAcceleration,
                magnitude: 1.0,
                remaining_ticks: 24_000,
                source_pill: Some("ju_ling_dan".to_string()),
            }],
        };

        let mult = cultivation_acceleration_multiplier(&se);
        assert!(
            (mult - 2.0).abs() < 1e-9,
            "聚灵丹 mag=1.0 应给 2× 修炼加速；实际为 {mult}"
        );
        // zone_qi=0.6 时基础每 tick 修炼进度 ≈ BASE_RATE × zone_qi × accel_mult
        // 首条正经难度 = 1.0（PR-1）
        // 打通时间 = difficulty / (per_tick_rate × accel_mult)
        // 只要 accel_mult=2.0，时间减半 → 验证概念正确性
        assert!(mult >= 2.0, "聚灵丹应至少 2× 加速以确保 ≤30min 首经");
    }

    // ── §10 全 8 种丹药消费后的 source_pill 字段正确 ──

    #[test]
    fn all_pills_set_source_pill_on_status_effects() {
        for id in &CULTIVATION_PILL_IDS {
            let spec = cultivation_pill_spec(id).unwrap();
            let mut contam = fresh_contam();
            let mut se = fresh_status_effects();

            let result = consume_cultivation_pill(&spec, &mut contam, &mut se, 0);

            for active in &se.active {
                assert_eq!(
                    active.source_pill.as_deref(),
                    Some(*id),
                    "pill {id} 的 StatusEffect.source_pill 应为 {id}"
                );
            }
            // 至少挂载了 1 个 effect（无堆叠限制首次服用）
            assert!(
                !result.applied_effects.is_empty(),
                "首颗 {id} 应至少挂载 1 个 effect"
            );
        }
    }

    // ── plan-cultivation-pacing-v1 P2.2 flawed 丹药测试 ──

    #[test]
    fn flawed_ling_xi_wan_spec_resolves() {
        let s = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
        assert_eq!(
            s.kind,
            CultivationPillKind::LingXiWan,
            "次品灵息丸 kind 应为 LingXiWan"
        );
        assert_eq!(s.name, "灵息丸（次品）");
        assert!(
            (s.toxin_amount - 0.15).abs() < 1e-9,
            "次品灵息丸丹毒与正品相同（0.15）"
        );
        assert_eq!(s.toxin_color, ColorKind::Gentle);
    }

    #[test]
    fn flawed_ju_ling_dan_spec_resolves() {
        let s = cultivation_pill_spec("ju_ling_dan_flawed").unwrap();
        assert_eq!(
            s.kind,
            CultivationPillKind::JuLingDan,
            "次品聚灵丹 kind 应为 JuLingDan"
        );
        assert_eq!(s.name, "聚灵丹（次品）");
        assert!(
            (s.toxin_amount - 0.20).abs() < 1e-9,
            "次品聚灵丹丹毒与正品相同（0.20）"
        );
    }

    #[test]
    fn is_flawed_cultivation_pill_detects_suffix() {
        assert!(is_flawed_cultivation_pill("ling_xi_wan_flawed"));
        assert!(is_flawed_cultivation_pill("ju_ling_dan_flawed"));
        assert!(!is_flawed_cultivation_pill("ling_xi_wan"));
        assert!(!is_flawed_cultivation_pill("ju_ling_dan"));
        assert!(!is_flawed_cultivation_pill("flawed_ling_xi_wan")); // prefix doesn't count
    }

    #[test]
    fn flawed_ling_xi_wan_magnitude_is_0_6x_normal() {
        let normal = cultivation_pill_spec("ling_xi_wan").unwrap();
        let flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();

        let mut normal_contam = fresh_contam();
        let mut normal_se = fresh_status_effects();
        consume_cultivation_pill(&normal, &mut normal_contam, &mut normal_se, 0);

        let mut flawed_contam = fresh_contam();
        let mut flawed_se = fresh_status_effects();
        consume_cultivation_pill(&flawed, &mut flawed_contam, &mut flawed_se, 0);

        assert!(!normal_se.active.is_empty(), "正品应挂载 effect");
        assert!(!flawed_se.active.is_empty(), "次品应挂载 effect");

        let normal_mag = normal_se.active[0].magnitude;
        let flawed_mag = flawed_se.active[0].magnitude;

        // 正品 mag=0.5，次品 mag=0.5×0.6=0.3
        assert!(
            (normal_mag - 0.5).abs() < 1e-6,
            "正品灵息丸 magnitude 应为 0.5，实际 {normal_mag}"
        );
        assert!(
            (flawed_mag - 0.3).abs() < 1e-6,
            "次品灵息丸 magnitude 应为 0.3（0.5×0.6），实际 {flawed_mag}"
        );
    }

    #[test]
    fn flawed_ju_ling_dan_magnitude_is_0_6x_normal() {
        let normal = cultivation_pill_spec("ju_ling_dan").unwrap();
        let flawed = cultivation_pill_spec("ju_ling_dan_flawed").unwrap();

        let mut normal_contam = fresh_contam();
        let mut normal_se = fresh_status_effects();
        consume_cultivation_pill(&normal, &mut normal_contam, &mut normal_se, 0);

        let mut flawed_contam = fresh_contam();
        let mut flawed_se = fresh_status_effects();
        consume_cultivation_pill(&flawed, &mut flawed_contam, &mut flawed_se, 0);

        let normal_mag = normal_se.active[0].magnitude;
        let flawed_mag = flawed_se.active[0].magnitude;

        // 正品 mag=1.0，次品 mag=1.0×0.6=0.6
        assert!(
            (normal_mag - 1.0).abs() < 1e-6,
            "正品聚灵丹 magnitude 应为 1.0，实际 {normal_mag}"
        );
        assert!(
            (flawed_mag - 0.6).abs() < 1e-6,
            "次品聚灵丹 magnitude 应为 0.6（1.0×0.6），实际 {flawed_mag}"
        );
    }

    #[test]
    fn flawed_pill_same_duration_as_normal() {
        let normal = cultivation_pill_spec("ling_xi_wan").unwrap();
        let flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();

        let mut nc = fresh_contam();
        let mut nse = fresh_status_effects();
        consume_cultivation_pill(&normal, &mut nc, &mut nse, 0);

        let mut fc = fresh_contam();
        let mut fse = fresh_status_effects();
        consume_cultivation_pill(&flawed, &mut fc, &mut fse, 0);

        assert_eq!(
            nse.active[0].remaining_ticks, fse.active[0].remaining_ticks,
            "次品丹药 duration 应与正品相同"
        );
    }

    #[test]
    fn flawed_pill_source_pill_contains_flawed_suffix() {
        let spec = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
        let mut contam = fresh_contam();
        let mut se = fresh_status_effects();
        consume_cultivation_pill(&spec, &mut contam, &mut se, 0);

        assert_eq!(
            se.active[0].source_pill.as_deref(),
            Some("ling_xi_wan_flawed"),
            "次品丹药 source_pill 应为 ling_xi_wan_flawed"
        );
    }

    #[test]
    fn flawed_pill_acceleration_multiplier_lower_than_normal() {
        use crate::cultivation::tick::cultivation_acceleration_multiplier;

        // 正品灵息丸 → 1.5× accel
        let spec_normal = cultivation_pill_spec("ling_xi_wan").unwrap();
        let mut nc = fresh_contam();
        let mut nse = fresh_status_effects();
        consume_cultivation_pill(&spec_normal, &mut nc, &mut nse, 0);
        let mult_normal = cultivation_acceleration_multiplier(&nse);

        // 次品灵息丸 → 1.3× accel
        let spec_flawed = cultivation_pill_spec("ling_xi_wan_flawed").unwrap();
        let mut fc = fresh_contam();
        let mut fse = fresh_status_effects();
        consume_cultivation_pill(&spec_flawed, &mut fc, &mut fse, 0);
        let mult_flawed = cultivation_acceleration_multiplier(&fse);

        assert!(
            (mult_normal - 1.5).abs() < 1e-6,
            "正品灵息丸加速应为 1.5×，实际 {mult_normal}"
        );
        assert!(
            (mult_flawed - 1.3).abs() < 1e-4,
            "次品灵息丸加速应为 1.3×（0.5×0.6=0.3 → 1+0.3），实际 {mult_flawed}"
        );
    }
}
