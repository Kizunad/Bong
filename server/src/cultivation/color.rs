//! QiColorEvolutionTick — 真元色演化（plan §2 / §1.1）。
//!
//! 简化模型：每个玩家维护 `PracticeLog`（Component），记录各色的练习权重。
//! 按窗口内比例判定：
//!   * 任一项 > 60% → main = 该色
//!   * 次项 > 25% → secondary
//!   * ≥3 项 > 15% → is_chaotic = true
//!   * 全部 < 25% → is_hunyuan = true
//!
//! P1：实际"练习事件"来源（打坐/战斗动作/丹药）由上层后续接入，这里只提供
//! tick + 纯函数 + PracticeLog Component 作为接口。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, Query};

use super::components::{ColorKind, QiColor};

/// 玩家修习累积日志 — 权重值可由 gameplay 系统增加，tick 会慢慢衰减。
#[derive(Debug, Default, Clone, Component, Serialize, Deserialize)]
pub struct PracticeLog {
    pub weights: HashMap<ColorKind, f64>,
    pub decay_per_tick: f64,
}

impl PracticeLog {
    pub fn add(&mut self, color: ColorKind, amount: f64) {
        *self.weights.entry(color).or_insert(0.0) += amount;
    }

    pub fn decay(&mut self) {
        if self.decay_per_tick <= 0.0 {
            return;
        }
        for w in self.weights.values_mut() {
            *w = (*w - self.decay_per_tick).max(0.0);
        }
        self.weights.retain(|_, w| *w > 0.0);
    }

    pub fn total(&self) -> f64 {
        self.weights.values().sum()
    }
}

/// 纯函数：基于日志权重演化 QiColor（plan §2 QiColorEvolutionTick 规则）。
pub fn evolve_qi_color(log: &PracticeLog, out: &mut QiColor) {
    let total = log.total();
    if total <= 0.0 {
        return;
    }
    let mut sorted: Vec<(ColorKind, f64)> =
        log.weights.iter().map(|(k, v)| (*k, v / total)).collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let over15 = sorted.iter().filter(|(_, r)| *r > 0.15).count();
    let all_under_25 = sorted.iter().all(|(_, r)| *r < 0.25);

    // 混元：所有项均 < 25%
    if all_under_25 {
        out.is_hunyuan = true;
        out.is_chaotic = false;
        out.secondary = None;
        return;
    }
    // 杂色：≥3 项 > 15%
    if over15 >= 3 {
        out.is_chaotic = true;
        out.is_hunyuan = false;
        out.secondary = None;
        return;
    }

    out.is_chaotic = false;
    out.is_hunyuan = false;
    if let Some(&(main_k, main_r)) = sorted.first() {
        if main_r > 0.60 {
            out.main = main_k;
        }
    }
    if let Some(&(sec_k, sec_r)) = sorted.get(1) {
        if sec_r > 0.25 {
            out.secondary = Some(sec_k);
        } else {
            out.secondary = None;
        }
    } else {
        out.secondary = None;
    }
}

pub fn qi_color_evolution_tick(mut players: Query<(&mut PracticeLog, &mut QiColor)>) {
    for (mut log, mut color) in players.iter_mut() {
        log.decay();
        evolve_qi_color(&log, &mut color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominant_color_becomes_main() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 70.0);
        log.add(ColorKind::Heavy, 30.0);
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert_eq!(c.main, ColorKind::Sharp);
        assert_eq!(c.secondary, Some(ColorKind::Heavy));
        assert!(!c.is_chaotic);
        assert!(!c.is_hunyuan);
    }

    #[test]
    fn three_over_15_percent_triggers_chaotic() {
        let mut log = PracticeLog::default();
        log.add(ColorKind::Sharp, 40.0);
        log.add(ColorKind::Heavy, 30.0);
        log.add(ColorKind::Mellow, 30.0);
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert!(c.is_chaotic);
    }

    #[test]
    fn uniform_under_25_triggers_hunyuan() {
        let mut log = PracticeLog::default();
        for k in [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
        ] {
            log.add(k, 20.0);
        }
        let mut c = QiColor::default();
        evolve_qi_color(&log, &mut c);
        assert!(c.is_hunyuan);
        assert!(!c.is_chaotic);
    }

    #[test]
    fn decay_drops_weights_to_zero() {
        let mut log = PracticeLog {
            decay_per_tick: 1.0,
            ..Default::default()
        };
        log.add(ColorKind::Sharp, 3.0);
        for _ in 0..5 {
            log.decay();
        }
        assert_eq!(log.total(), 0.0);
    }

    #[test]
    fn empty_log_leaves_color_untouched() {
        let log = PracticeLog::default();
        let mut c = QiColor {
            main: ColorKind::Violent,
            ..Default::default()
        };
        evolve_qi_color(&log, &mut c);
        assert_eq!(c.main, ColorKind::Violent);
    }
}
