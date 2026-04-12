//! ContaminationTick（plan §2.1）— 异种真元排异。
//!
//! 每 tick：
//!   * 对每条污染记录 `ContamSource`，按排异效率扣减 `amount`
//!   * 自身真元按 `排异量 × DRAIN_RATIO`（10:15 亏损）扣
//!   * qi_current 不够时，对随机经脉施加裂痕（P1: 施加到首条已打通经脉）
//!   * `amount <= 0` 的条目移除
//!   * 所有条目都清空 + qi/经络全毁 → emit `CultivationDeathTrigger::ContaminationOverflow`

use valence::prelude::{Entity, EventWriter, Query};

use super::components::{Contamination, CrackCause, Cultivation, MeridianCrack, MeridianSystem};
use super::death_hooks::{CultivationDeathCause, CultivationDeathTrigger};
use super::tick::CultivationClock;
use valence::prelude::Res;

/// plan §0-3 10:15 排异亏损比。
pub const DRAIN_RATIO: f64 = 1.5;
/// 每 tick 基础排异速率。
pub const BASE_PURGE_RATE: f64 = 0.1;

/// 纯函数：推进一条 contam 的排异。返回 (排异量, 真元消耗, 是否清空)。
pub fn purge_step(
    contam: &mut super::components::ContamSource,
    qi_budget: f64,
) -> (f64, f64, bool) {
    let want_purge = BASE_PURGE_RATE.min(contam.amount);
    let want_cost = want_purge * DRAIN_RATIO;
    let actual_cost = want_cost.min(qi_budget);
    let actual_purge = if want_cost > 0.0 {
        actual_cost / DRAIN_RATIO
    } else {
        0.0
    };
    contam.amount = (contam.amount - actual_purge).max(0.0);
    let cleared = contam.amount <= 1e-9;
    (actual_purge, actual_cost, cleared)
}

pub fn contamination_tick(
    clock: Res<CultivationClock>,
    mut deaths: EventWriter<CultivationDeathTrigger>,
    mut players: Query<(
        Entity,
        &mut Cultivation,
        &mut Contamination,
        &mut MeridianSystem,
    )>,
) {
    let now = clock.tick;
    for (entity, mut cultivation, mut contam, mut meridians) in players.iter_mut() {
        if contam.entries.is_empty() {
            continue;
        }
        let mut any_qi_deficit = false;
        // 按 amount 从大到小处理
        contam.entries.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for entry in contam.entries.iter_mut() {
            let budget = cultivation.qi_current.max(0.0);
            let (_purge, cost, _cleared) = purge_step(entry, budget);
            cultivation.qi_current -= cost;
            if cultivation.qi_current < 0.0 {
                any_qi_deficit = true;
                // 对首条已打通经脉添加裂痕
                if let Some(m) = meridians.iter_mut().find(|m| m.opened) {
                    m.cracks.push(MeridianCrack {
                        severity: 0.1,
                        healing_progress: 0.0,
                        cause: CrackCause::Backfire,
                        created_at: now,
                    });
                    m.integrity = (m.integrity - 0.05).max(0.0);
                }
                cultivation.qi_current = 0.0;
            }
        }

        contam.entries.retain(|e| e.amount > 1e-9);

        // 致死检查：经络全毁 + qi=0 + 仍残留污染（暂用简单判据）
        let all_broken = meridians.iter().all(|m| m.integrity <= 0.0 || !m.opened);
        if any_qi_deficit && all_broken && !contam.entries.is_empty() {
            deaths.send(CultivationDeathTrigger {
                entity,
                cause: CultivationDeathCause::ContaminationOverflow,
                context: serde_json::json!({
                    "remaining": contam.entries.len(),
                    "tick": now,
                }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{ColorKind, ContamSource};

    #[test]
    fn purge_consumes_qi_at_10_to_15_ratio() {
        let mut c = ContamSource {
            amount: 1.0,
            color: ColorKind::Sharp,
            introduced_at: 0,
        };
        let (purge, cost, _) = purge_step(&mut c, 100.0);
        assert!((cost / purge - DRAIN_RATIO).abs() < 1e-9);
    }

    #[test]
    fn purge_clamped_by_qi_budget() {
        let mut c = ContamSource {
            amount: 1.0,
            color: ColorKind::Sharp,
            introduced_at: 0,
        };
        let (_purge, cost, _) = purge_step(&mut c, 0.05);
        assert!(cost <= 0.05 + 1e-9);
    }

    #[test]
    fn purge_clears_when_amount_reaches_zero() {
        let mut c = ContamSource {
            amount: 0.05,
            color: ColorKind::Sharp,
            introduced_at: 0,
        };
        let (_, _, cleared) = purge_step(&mut c, 100.0);
        assert!(cleared);
    }
}
