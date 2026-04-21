//! plan-lingtian-v1 §5.1 — 密度阈值（zone_pressure）+ 天道注视。
//!
//! 公式：
//! ```text
//! zone_pressure = Σ (crop.kind.growth_cost.drain_per_tick × crop_count)
//!               − zone_natural_supply
//!               − Σ replenish_recent_7d
//! ```
//!
//! 阈值（plan §5.1 占位）：
//!   * `LOW`  = 0.3  → 天道 narration（冷漠古意）
//!   * `MID`  = 0.6  → 异变兽刷新率 +30%
//!   * `HIGH` = 1.0  → 该 zone 所有 plot_qi 瞬时清零 + 3×3 道伥（道伥 spawn
//!     由 npc 系统接消费 [`ZonePressureCrossed`] 事件）
//!
//! 7 天滚动窗口：1 day = 1440 lingtian-tick；7 days = 10080 lingtian-tick。
//! `record_replenish` 时追加；`compute_zone_pressure` 时按 clock prune。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

use crate::botany::PlantKindRegistry;

use super::plot::LingtianPlot;

pub const LINGTIAN_TICKS_PER_DAY: u64 = 1440;
pub const REPLENISH_WINDOW_LINGTIAN_TICKS: u64 = 7 * LINGTIAN_TICKS_PER_DAY;

pub const PRESSURE_LOW: f32 = 0.3;
pub const PRESSURE_MID: f32 = 0.6;
pub const PRESSURE_HIGH: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PressureLevel {
    #[default]
    None,
    Low,
    Mid,
    High,
}

impl PressureLevel {
    /// 由原始 pressure 数值映射档位。
    pub fn classify(pressure: f32) -> Self {
        if pressure >= PRESSURE_HIGH {
            Self::High
        } else if pressure >= PRESSURE_MID {
            Self::Mid
        } else if pressure >= PRESSURE_LOW {
            Self::Low
        } else {
            Self::None
        }
    }

    pub fn is_higher_than(self, other: Self) -> bool {
        self.rank() > other.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Low => 1,
            Self::Mid => 2,
            Self::High => 3,
        }
    }
}

#[derive(Debug, Default)]
pub struct ZonePressureState {
    /// (lingtian_tick_at_replenish, plot_qi_added) — 仅最近 7d 的条目。
    pub recent_replenish: Vec<(u64, f32)>,
    /// 上一次 compute 后的档位（用于"上升"边沿事件）。
    pub last_level: PressureLevel,
    /// 上一次 compute 后的原始 pressure（debug / monitor 用）。
    pub last_pressure: f32,
}

impl ZonePressureState {
    pub fn record_replenish(&mut self, lingtian_tick: u64, amount: f32) {
        self.recent_replenish.push((lingtian_tick, amount));
    }

    /// 删除 7d 窗口外的旧条目。
    pub fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(REPLENISH_WINDOW_LINGTIAN_TICKS);
        self.recent_replenish.retain(|(t, _)| *t >= cutoff);
    }

    pub fn replenish_total_7d(&self) -> f32 {
        self.recent_replenish.iter().map(|(_, a)| *a).sum()
    }
}

#[derive(Debug, Default, Resource)]
pub struct ZonePressureTracker {
    by_zone: HashMap<String, ZonePressureState>,
    /// 区域自然补给（plan §5.1 zone_natural_supply）。MVP 单 zone 默认 0.05/tick，
    /// 让中等密度（5-10 个 low-cost 作物）才触 LOW。
    natural_supply: HashMap<String, f32>,
}

impl ZonePressureTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_natural_supply(&mut self, zone: impl Into<String>, supply: f32) {
        self.natural_supply.insert(zone.into(), supply.max(0.0));
    }

    pub fn natural_supply_for(&self, zone: &str) -> f32 {
        self.natural_supply.get(zone).copied().unwrap_or(0.0)
    }

    pub fn state_mut(&mut self, zone: &str) -> &mut ZonePressureState {
        self.by_zone.entry(zone.to_string()).or_default()
    }

    pub fn state(&self, zone: &str) -> Option<&ZonePressureState> {
        self.by_zone.get(zone)
    }

    pub fn zones(&self) -> impl Iterator<Item = &String> {
        self.by_zone.keys()
    }
}

/// 计算指定 zone 的 pressure。
///
/// `plot_iter` 应该只迭这个 zone 的 plots（当前简化为单 DEFAULT_ZONE，
/// 即所有 plots 都属同一 zone）。
pub fn compute_zone_pressure<'a>(
    zone: &str,
    plot_iter: impl Iterator<Item = &'a LingtianPlot>,
    plant_registry: &PlantKindRegistry,
    tracker: &ZonePressureTracker,
) -> f32 {
    let demand: f32 = plot_iter
        .filter_map(|p| p.crop.as_ref())
        .filter_map(|c| plant_registry.get(&c.kind))
        .map(|k| k.growth_cost.drain_per_tick())
        .sum();
    let natural = tracker.natural_supply_for(zone);
    let replenish = tracker
        .state(zone)
        .map(|s| s.replenish_total_7d())
        .unwrap_or(0.0);
    demand - natural - replenish
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_thresholds() {
        assert_eq!(PressureLevel::classify(0.0), PressureLevel::None);
        assert_eq!(PressureLevel::classify(0.29), PressureLevel::None);
        assert_eq!(PressureLevel::classify(0.30), PressureLevel::Low);
        assert_eq!(PressureLevel::classify(0.59), PressureLevel::Low);
        assert_eq!(PressureLevel::classify(0.60), PressureLevel::Mid);
        assert_eq!(PressureLevel::classify(0.99), PressureLevel::Mid);
        assert_eq!(PressureLevel::classify(1.00), PressureLevel::High);
        assert_eq!(PressureLevel::classify(5.0), PressureLevel::High);
    }

    #[test]
    fn is_higher_than_orders_levels() {
        assert!(PressureLevel::Low.is_higher_than(PressureLevel::None));
        assert!(PressureLevel::Mid.is_higher_than(PressureLevel::Low));
        assert!(PressureLevel::High.is_higher_than(PressureLevel::Mid));
        assert!(!PressureLevel::Low.is_higher_than(PressureLevel::Mid));
        assert!(!PressureLevel::High.is_higher_than(PressureLevel::High));
    }

    #[test]
    fn prune_drops_entries_older_than_7d() {
        let mut s = ZonePressureState::default();
        s.record_replenish(100, 0.5);
        s.record_replenish(REPLENISH_WINDOW_LINGTIAN_TICKS + 200, 0.3);
        // now = REPLENISH_WINDOW + 300 → cutoff = 300 → 第一条（100）落出
        s.prune(REPLENISH_WINDOW_LINGTIAN_TICKS + 300);
        assert_eq!(s.recent_replenish.len(), 1);
        assert!((s.replenish_total_7d() - 0.3).abs() < 1e-6);
    }
}
