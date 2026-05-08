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
use crate::world::season::Season;

use super::environment::apply_xizhuan_supply_jitter;
use super::plot::LingtianPlot;
use super::weather::WeatherEvent;

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

    /// plan-lingtian-weather-v1 §5 / worldview §七 — 把 pressure 映射成档位时
    /// 临时降 N 档（阴霾期间 N=1）。
    ///
    /// `relax_steps=0` 等价于 [`classify`]；`relax_steps=1` 把 High→Mid → Low
    /// → None 各自降一档；多档放宽递归降。这反映"天道注视减弱"的语义：
    /// 同样 raw pressure 在阴霾下不触发同等档的负面效果。
    pub fn classify_with_relax(pressure: f32, relax_steps: u8) -> Self {
        let mut level = Self::classify(pressure);
        for _ in 0..relax_steps {
            level = match level {
                Self::High => Self::Mid,
                Self::Mid => Self::Low,
                Self::Low | Self::None => Self::None,
            };
        }
        level
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

/// plan-lingtian-weather-v1 §2 — 派生稳定（per-day）的 supply jitter unit。
///
/// 用 (zone, lingtian_day) 二元组 hash 映射到 `[-1, 1]`，保证：
/// - 同一 (zone, day) 多次调用返回同值（不抖动 zone_pressure）
/// - 不同 day 之间均匀分布（汐转期 RNG ±20% 在 32 day 内体现"反复"语义）
/// - 不依赖全局 RNG → 单测可重现
pub fn derive_supply_jitter(zone: &str, lingtian_tick: u64) -> f32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let day = lingtian_tick / LINGTIAN_TICKS_PER_DAY;
    let mut hasher = DefaultHasher::new();
    zone.hash(&mut hasher);
    day.hash(&mut hasher);
    let h = hasher.finish();
    // u64 → [0, 1] → [-1, 1]
    let unit01 = (h as f64) / (u64::MAX as f64);
    (unit01 * 2.0 - 1.0) as f32
}

/// plan-lingtian-weather-v1 §2 / §3 — 把季节 + 天气修饰应用到 zone 的 base
/// natural_supply。
///
/// 流程：
///   1. season modifier（相对增量）：summer -10% / winter +10% / 汐转 jitter ±20%
///      → `base × (1 + season_delta)`
///   2. weather modifier（硬覆盖倍率）：drought_wind ×0、ling_mist ×1.5、其他 ×1
///   3. clamp 到非负
///
/// `supply_jitter_unit` ∈ `[-1, 1]`：调用方负责生成（非汐转季节 amplitude=0
/// → jitter 不影响结果）。
pub fn effective_natural_supply(
    base: f32,
    season: Season,
    supply_jitter_unit: f32,
    weather: Option<WeatherEvent>,
) -> f32 {
    let season_delta = apply_xizhuan_supply_jitter(season, supply_jitter_unit);
    let after_season = base * (1.0 + season_delta);
    let weather_mult = weather
        .map(WeatherEvent::natural_supply_multiplier)
        .unwrap_or(1.0);
    (after_season * weather_mult).max(0.0)
}

/// 计算指定 zone 的 pressure（接 plan-lingtian-weather-v1 §2 / §3 季节-天气修饰）。
///
/// `plot_iter` 应该只迭这个 zone 的 plots（当前简化为单 DEFAULT_ZONE，
/// 即所有 plots 都属同一 zone）。
///
/// 季节-天气修饰只作用于 `natural_supply` 项；`replenish_total_7d` 来源是
/// 玩家补灵动作（plan-lingtian-v1），不应被天道修饰。
pub fn compute_zone_pressure<'a>(
    zone: &str,
    plot_iter: impl Iterator<Item = &'a LingtianPlot>,
    plant_registry: &PlantKindRegistry,
    tracker: &ZonePressureTracker,
    season: Season,
    supply_jitter_unit: f32,
    weather: Option<WeatherEvent>,
) -> f32 {
    let demand: f32 = plot_iter
        .filter_map(|p| p.crop.as_ref())
        .filter_map(|c| plant_registry.get(&c.kind))
        .map(|k| k.growth_cost.drain_per_tick())
        .sum();
    let base_supply = tracker.natural_supply_for(zone);
    let natural = effective_natural_supply(base_supply, season, supply_jitter_unit, weather);
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

    // -------- plan-lingtian-weather-v1 §6 P1 effective_natural_supply 单测 --------

    #[test]
    fn effective_supply_summer_drops_10_percent() {
        // base 0.5、Summer modifier -10% → 0.45（jitter 不影响：amplitude=0）
        for jitter in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let s = effective_natural_supply(0.5, Season::Summer, jitter, None);
            assert!((s - 0.45).abs() < 1e-6, "summer jitter={jitter}: {s}");
        }
    }

    #[test]
    fn effective_supply_winter_rises_10_percent() {
        for jitter in [-1.0, 0.0, 1.0] {
            let s = effective_natural_supply(0.5, Season::Winter, jitter, None);
            assert!((s - 0.55).abs() < 1e-6, "winter jitter={jitter}: {s}");
        }
    }

    #[test]
    fn effective_supply_xizhuan_jitter_swings_plus_minus_20_percent() {
        // 汐转：base 0.5、modifier=0、amplitude=0.20
        // jitter=-1 → 0.5 * (1 - 0.20) = 0.4
        // jitter=+1 → 0.5 * (1 + 0.20) = 0.6
        // jitter=0  → 0.5（基线）
        let low = effective_natural_supply(0.5, Season::SummerToWinter, -1.0, None);
        let mid = effective_natural_supply(0.5, Season::SummerToWinter, 0.0, None);
        let high = effective_natural_supply(0.5, Season::SummerToWinter, 1.0, None);
        assert!((low - 0.4).abs() < 1e-6, "汐转 low: {low}");
        assert!((mid - 0.5).abs() < 1e-6, "汐转 mid: {mid}");
        assert!((high - 0.6).abs() < 1e-6, "汐转 high: {high}");
    }

    #[test]
    fn effective_supply_drought_wind_zeros_supply() {
        // 旱风硬覆盖 ×0：无论季节 / jitter，base × 0 = 0
        for season in [
            Season::Summer,
            Season::Winter,
            Season::SummerToWinter,
            Season::WinterToSummer,
        ] {
            let s = effective_natural_supply(0.5, season, 0.0, Some(WeatherEvent::DroughtWind));
            assert!(s.abs() < 1e-6, "drought_wind {}: {s}", season.as_wire_str());
        }
    }

    #[test]
    fn effective_supply_ling_mist_boosts_50_percent() {
        // 灵雾硬覆盖 ×1.5：冬 0.5 × 1.1 × 1.5 = 0.825
        let s = effective_natural_supply(0.5, Season::Winter, 0.0, Some(WeatherEvent::LingMist));
        assert!((s - 0.825).abs() < 1e-6);
    }

    #[test]
    fn effective_supply_clamps_to_non_negative() {
        // 极端：base 1.0、汐转 jitter=-1 → 1.0 × (1 - 0.20) = 0.8（已经非负）
        // 真正能让其负的：base 0.0 already 0；这里手动构造 negative season_delta
        // 实际上 Season::natural_supply_modifier 限定为 ±10%，加 ±20% jitter
        // 上限 ±0.30 → 1 + (-0.3) = 0.7 始终非负。但接口承诺 max(0)，验证之。
        let s = effective_natural_supply(0.0, Season::Winter, 0.0, None);
        assert!(s >= 0.0);
    }

    #[test]
    fn derive_supply_jitter_within_unit_range() {
        // hash 派生的 jitter 必须落在 [-1, 1]
        for tick in [0u64, 1, LINGTIAN_TICKS_PER_DAY, 12345, u64::MAX / 2] {
            let j = derive_supply_jitter("default_zone", tick);
            assert!(
                (-1.0..=1.0).contains(&j),
                "derive_supply_jitter(zone, {tick}) = {j} 不在 [-1, 1]"
            );
        }
    }

    #[test]
    fn derive_supply_jitter_stable_within_same_day() {
        // 同 (zone, day) 多次调用必须返回同值（不抖动 zone_pressure）
        let day = 5;
        let t0 = day * LINGTIAN_TICKS_PER_DAY;
        let t_mid = t0 + LINGTIAN_TICKS_PER_DAY / 2;
        let t_end = t0 + LINGTIAN_TICKS_PER_DAY - 1;
        let a = derive_supply_jitter("default_zone", t0);
        let b = derive_supply_jitter("default_zone", t_mid);
        let c = derive_supply_jitter("default_zone", t_end);
        assert!((a - b).abs() < 1e-6 && (b - c).abs() < 1e-6, "{a} {b} {c}");
    }

    #[test]
    fn derive_supply_jitter_changes_across_days() {
        // 不同 day 应给不同 jitter（high 概率，非确定性）；至少 30 天里看到 ≥ 5 个
        // 不同 unique 值即可（避免 hash 巧合 → tolerant 的 spread test）。
        let mut uniques = std::collections::HashSet::new();
        for d in 0..30u64 {
            let j = derive_supply_jitter("default_zone", d * LINGTIAN_TICKS_PER_DAY);
            uniques.insert((j * 1e6) as i64);
        }
        assert!(
            uniques.len() >= 5,
            "30 天 derive_supply_jitter 期望分散但得到 {} 个 unique 值",
            uniques.len()
        );
    }

    #[test]
    fn natural_supply_in_tide_phase_random_within_plus_minus_20_percent() {
        // §6 P1 e2e — 汐转期 supply 在 ±20% 内浮动（hash 派生的 jitter
        // 跨 32 game-day 应当能既见 < base 又见 > base）。
        let base = 1.0;
        let mut min_eff = f32::INFINITY;
        let mut max_eff = f32::NEG_INFINITY;
        for d in 0..32u64 {
            let jitter = derive_supply_jitter("default_zone", d * LINGTIAN_TICKS_PER_DAY);
            let s = effective_natural_supply(base, Season::SummerToWinter, jitter, None);
            min_eff = min_eff.min(s);
            max_eff = max_eff.max(s);
        }
        // 32 day 跨度内 jitter 应当探到 ≥ ±0.10 范围
        assert!(
            min_eff <= 0.90,
            "32 day 内 min effective supply 应 ≤ 0.90，实际 {min_eff}"
        );
        assert!(
            max_eff >= 1.10,
            "32 day 内 max effective supply 应 ≥ 1.10，实际 {max_eff}"
        );
        // 且永远在 [0.80, 1.20] 内（汐转 ±20% 边界）
        assert!(
            min_eff >= 0.80 - 1e-6 && max_eff <= 1.20 + 1e-6,
            "汐转 supply 越界：min={min_eff} max={max_eff}"
        );
    }

    #[test]
    fn full_year_cycle_supply_summer_winter_delta() {
        // §6 P1 e2e "growth_curve_full_year_cycle_diff" 简化版：同 base supply、
        // 不同季节，差应当 ≈ ±20% (夏 -10% / 冬 +10%)。
        let summer = effective_natural_supply(1.0, Season::Summer, 0.0, None);
        let winter = effective_natural_supply(1.0, Season::Winter, 0.0, None);
        let delta = winter - summer;
        assert!(
            (delta - 0.2).abs() < 1e-6,
            "winter - summer effective_supply 应当 +0.2（即 1.1 - 0.9），实际 {delta}（summer={summer}, winter={winter}）"
        );
    }

    // -------- plan-lingtian-weather-v1 §6 P4 — 阴霾 ↔ 密度阈值耦合 --------

    #[test]
    fn classify_with_relax_zero_matches_classify() {
        for p in [0.0, 0.29, 0.30, 0.59, 0.60, 0.99, 1.0, 5.0] {
            assert_eq!(
                PressureLevel::classify_with_relax(p, 0),
                PressureLevel::classify(p),
                "relax=0 应等价于 classify, p={p}"
            );
        }
    }

    #[test]
    fn classify_with_relax_step_1_high_becomes_mid() {
        // raw 1.0 → classify=High → relax 1 → Mid
        assert_eq!(
            PressureLevel::classify_with_relax(1.0, 1),
            PressureLevel::Mid
        );
        assert_eq!(
            PressureLevel::classify_with_relax(2.5, 1),
            PressureLevel::Mid
        );
    }

    #[test]
    fn classify_with_relax_step_1_mid_becomes_low() {
        assert_eq!(
            PressureLevel::classify_with_relax(0.6, 1),
            PressureLevel::Low
        );
        assert_eq!(
            PressureLevel::classify_with_relax(0.99, 1),
            PressureLevel::Low
        );
    }

    #[test]
    fn classify_with_relax_step_1_low_becomes_none() {
        assert_eq!(
            PressureLevel::classify_with_relax(0.3, 1),
            PressureLevel::None
        );
        assert_eq!(
            PressureLevel::classify_with_relax(0.59, 1),
            PressureLevel::None
        );
    }

    #[test]
    fn classify_with_relax_step_1_none_stays_none() {
        assert_eq!(
            PressureLevel::classify_with_relax(0.0, 1),
            PressureLevel::None
        );
        assert_eq!(
            PressureLevel::classify_with_relax(0.29, 1),
            PressureLevel::None
        );
    }

    #[test]
    fn classify_with_relax_multi_steps_compounds() {
        // raw 1.0 → High → relax 2 → Low；relax 3 → None
        assert_eq!(
            PressureLevel::classify_with_relax(1.0, 2),
            PressureLevel::Low
        );
        assert_eq!(
            PressureLevel::classify_with_relax(1.0, 3),
            PressureLevel::None
        );
        assert_eq!(
            PressureLevel::classify_with_relax(1.0, 99),
            PressureLevel::None
        );
    }

    #[test]
    fn weather_event_haze_relax_steps_eq_one_others_zero() {
        // 与 weather.rs 的 pressure_threshold_relax_steps 对接：阴霾 1 / 其他 0
        assert_eq!(WeatherEvent::HeavyHaze.pressure_threshold_relax_steps(), 1);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::LingMist,
        ] {
            assert_eq!(ev.pressure_threshold_relax_steps(), 0);
        }
    }
}
