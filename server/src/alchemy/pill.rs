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

use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};
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
}
