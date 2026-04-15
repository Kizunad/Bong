//! 服药 → 污染 注入（plan-alchemy-v1 §2）。
//!
//! 复用 `cultivation::Contamination / ContamSource` — 不新增字段。
//! 代谢速率天然由 MeridianSystem `sum_rate × integrity`（contamination_tick 做）决定。

use serde::{Deserialize, Serialize};

use crate::cultivation::components::{ColorKind, ContamSource, Contamination, Cultivation};

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

/// plan §2.1 服药流程：
/// 1. 查 pill 效果
/// 2. 污染 push 一条 ContamSource（attacker_id=None，标识丹毒）
/// 3. 应用效果（qi_gain 等）
///
/// 返回实际生效的 qi_gain（供调用侧广播）。
pub fn consume_pill(
    effect: &PillEffect,
    contam: &mut Contamination,
    cultivation: &mut Cultivation,
    now_tick: u64,
) -> f64 {
    contam.entries.push(ContamSource {
        amount: effect.toxin_amount,
        color: effect.toxin_color,
        attacker_id: None,
        introduced_at: now_tick,
    });
    match effect.qi_gain {
        Some(q) => {
            let before = cultivation.qi_current;
            cultivation.qi_current = (before + q).min(cultivation.qi_max);
            cultivation.qi_current - before
        }
        None => 0.0,
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

    #[test]
    fn consume_pill_appends_contam_and_restores_qi() {
        let mut contam = fresh_contam();
        let mut cult = Cultivation {
            qi_current: 0.0,
            qi_max: 100.0,
            ..Default::default()
        };
        let effect = PillEffect {
            toxin_amount: 0.3,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(24.0),
            meridian_progress_bonus: None,
        };
        let gained = consume_pill(&effect, &mut contam, &mut cult, 10);
        assert_eq!(gained, 24.0);
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
        let effect = PillEffect {
            toxin_amount: 0.3,
            toxin_color: ColorKind::Mellow,
            qi_gain: Some(50.0),
            meridian_progress_bonus: None,
        };
        let gained = consume_pill(&effect, &mut contam, &mut cult, 0);
        assert_eq!(gained, 10.0);
        assert_eq!(cult.qi_current, 100.0);
    }

    #[test]
    fn can_take_pill_blocks_when_same_color_exceeds_threshold() {
        let mut contam = fresh_contam();
        contam.entries.push(ContamSource {
            amount: 0.6,
            color: ColorKind::Mellow,
            attacker_id: None,
            introduced_at: 0,
        });
        contam.entries.push(ContamSource {
            amount: 0.5,
            color: ColorKind::Mellow,
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
            attacker_id: None,
            introduced_at: 0,
        });
        assert_eq!(overdose_penalty(&contam, ColorKind::Violent), 0.0);
    }
}
