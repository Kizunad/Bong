//! plan-lingtian-weather-v1 §3 — 天气事件枚举 + 生成器（P2 完整实装）。
//!
//! 五个变体覆盖 §3 表：
//! - **Thunderstorm**（雷暴）—— 夏 / 汐转主出现，2-4h；plot_qi 与 zone qi 流速
//!   ×1.5；plot_qi_cap 临时 -0.2；hook plan-tribulation-v1 渡劫稳定窗口（本 plan
//!   不实装 tribulation 逻辑，仅暴露状态供查询）
//! - **DroughtWind**（旱风）—— 夏季主出现，6-12h；plot_qi 衰减 ×2；natural_supply
//!   临时归零；shelflife 衰减 ×2
//! - **Blizzard**（风雪）—— 冬季主出现，12-24h；growth tick 暂停；雪线下移
//! - **HeavyHaze**（长阴霾）—— 冬季罕见极端 12-24h；天道注视密度阈值降 1 档
//!   （worldview §七）；growth tick 暂停
//! - **LingMist**（灵雾）—— 冬偶发 + 汐转主出现，1-2h；plot_qi_cap +0.2；
//!   natural_supply +50%；玩家"农忙"窗口
//!
//! § 3 生成器：每 game-day（1440 lingtian-tick）边界 RNG roll 一次；同 zone
//! 同时只能有一个 active 事件（避免叠加），事件持续到自然过期。
//!
//! 时间换算：1 game-day = 1440 lingtian-tick = 24 game-hour，1 game-hour =
//! 60 lingtian-tick（与 plan-lingtian-v1 §5.1 7d 窗口的 day = 1440 一致）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Event, EventWriter, Res, ResMut, Resource};

use crate::world::season::{Season, WorldSeasonState};

use super::pressure::LINGTIAN_TICKS_PER_DAY;
use super::qi_account::{LingtianTickAccumulator, DEFAULT_ZONE};
use super::systems::LingtianClock;

/// plan-lingtian-weather-v1 §3 — 天气事件类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherEvent {
    /// 雷暴（夏 / 汐转）—— qi 流速 ×1.5，雷暴是渡劫稳定窗口。
    Thunderstorm,
    /// 旱风（夏）—— qi 衰减 ×2，natural_supply 临时归零。
    DroughtWind,
    /// 风雪（冬）—— growth tick 暂停。
    Blizzard,
    /// 长阴霾（冬罕见 / 汐转）—— growth tick 暂停 + 密度阈值降 1 档。
    HeavyHaze,
    /// 灵雾（冬偶发 / 汐转）—— plot_qi_cap +0.2，natural_supply +50%。
    LingMist,
}

impl WeatherEvent {
    /// IPC 序列化字符串（与 schema `WeatherEventKindV1` 对齐）。
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Thunderstorm => "thunderstorm",
            Self::DroughtWind => "drought_wind",
            Self::Blizzard => "blizzard",
            Self::HeavyHaze => "heavy_haze",
            Self::LingMist => "ling_mist",
        }
    }

    /// plan §3 — 事件期间是否暂停 plot growth tick（阴霾 / 风雪）。
    pub const fn blocks_growth_tick(self) -> bool {
        matches!(self, Self::Blizzard | Self::HeavyHaze)
    }

    /// plan §3 — 事件期间 plot_qi_cap 的额外修饰（在 Season 修饰之上叠加）。
    pub const fn plot_qi_cap_delta(self) -> f32 {
        match self {
            Self::Thunderstorm => -0.2,
            Self::LingMist => 0.2,
            Self::DroughtWind | Self::Blizzard | Self::HeavyHaze => 0.0,
        }
    }

    /// plan §3 — 事件期间 plot ↔ zone qi 流速倍率（在 Season 倍率上再乘）。
    pub const fn zone_flow_multiplier(self) -> f32 {
        match self {
            Self::Thunderstorm => 1.5,
            Self::DroughtWind | Self::Blizzard | Self::HeavyHaze | Self::LingMist => 1.0,
        }
    }

    /// plan §3 — 事件期间 plot_qi 衰减速率倍率（旱风 ×2）。
    pub const fn qi_decay_multiplier(self) -> f32 {
        match self {
            Self::DroughtWind => 2.0,
            _ => 1.0,
        }
    }

    /// plan §3 — 事件期间 natural_supply 的"硬覆盖"倍率：
    /// - DroughtWind：归零（×0）
    /// - LingMist：×1.5（+50%）
    /// - 其他：保持季节修饰，不强覆盖（×1.0）
    pub const fn natural_supply_multiplier(self) -> f32 {
        match self {
            Self::DroughtWind => 0.0,
            Self::LingMist => 1.5,
            Self::Thunderstorm | Self::Blizzard | Self::HeavyHaze => 1.0,
        }
    }

    /// plan §3 — 事件期间 shelflife 衰减倍率（旱风 ×2）。
    pub const fn shelflife_decay_multiplier(self) -> f32 {
        match self {
            Self::DroughtWind => 2.0,
            _ => 1.0,
        }
    }

    /// plan §5 / worldview §七 —— 事件期间 zone_pressure 阈值降档数（阴霾降 1 档）。
    pub const fn pressure_threshold_relax_steps(self) -> u8 {
        match self {
            Self::HeavyHaze => 1,
            _ => 0,
        }
    }

    /// 全部变体（用于 P2 RNG 表 + schema sample 对拍 + 单测枚举遍历）。
    pub const fn all() -> [Self; 5] {
        [
            Self::Thunderstorm,
            Self::DroughtWind,
            Self::Blizzard,
            Self::HeavyHaze,
            Self::LingMist,
        ]
    }

    /// plan §3 表 — 该事件在指定季节的 per game-day 触发概率。
    ///
    /// 雷暴 / 旱风：仅夏 + 汐转；风雪 / 阴霾：仅冬 + 汐转；灵雾：冬 + 汐转。
    /// 主季节 bonus 已含；汐转 bonus 已含（雷暴/旱风/风雪/阴霾 ×2，灵雾 ×3）。
    pub const fn daily_probability(self, season: Season) -> f32 {
        match (self, season) {
            (Self::Thunderstorm, Season::Summer) => 0.03,
            (Self::Thunderstorm, Season::SummerToWinter) => 0.02,
            (Self::Thunderstorm, Season::WinterToSummer) => 0.02,
            (Self::Thunderstorm, Season::Winter) => 0.0,

            (Self::DroughtWind, Season::Summer) => 0.06,
            (Self::DroughtWind, Season::SummerToWinter) => 0.04,
            (Self::DroughtWind, Season::WinterToSummer) => 0.04,
            (Self::DroughtWind, Season::Winter) => 0.0,

            (Self::Blizzard, Season::Winter) => 0.03,
            (Self::Blizzard, Season::SummerToWinter) => 0.06,
            (Self::Blizzard, Season::WinterToSummer) => 0.06,
            (Self::Blizzard, Season::Summer) => 0.0,

            (Self::HeavyHaze, Season::Winter) => 0.005,
            (Self::HeavyHaze, Season::SummerToWinter) => 0.01,
            (Self::HeavyHaze, Season::WinterToSummer) => 0.01,
            (Self::HeavyHaze, Season::Summer) => 0.0,

            (Self::LingMist, Season::Winter) => 0.01,
            (Self::LingMist, Season::SummerToWinter) => 0.03,
            (Self::LingMist, Season::WinterToSummer) => 0.03,
            (Self::LingMist, Season::Summer) => 0.0,
        }
    }

    /// 该事件能否在指定季节出现（即概率 > 0）。
    pub const fn can_occur_in(self, season: Season) -> bool {
        // const f32 比较：用绝对值 > epsilon 避开浮点判定的 != 0.0 边界。
        // daily_probability 返回都是离散 f32 常量，直接 > 0 安全。
        self.daily_probability(season) > 0.0
    }

    /// 持续时间范围（lingtian-tick）。1 game-hour = 60 lingtian-tick。
    pub const fn duration_range_lingtian_ticks(self) -> (u64, u64) {
        match self {
            Self::Thunderstorm => (120, 240), // 2-4h
            Self::DroughtWind => (360, 720),  // 6-12h
            Self::Blizzard => (720, 1440),    // 12-24h
            Self::HeavyHaze => (720, 1440),   // 12-24h
            Self::LingMist => (60, 120),      // 1-2h
        }
    }
}

// ============================================================================
// ActiveWeather Resource + WeatherRng
// ============================================================================

/// plan §3 — 单个 zone 上当前 active 的天气事件 + 过期 tick。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWeatherEntry {
    pub event: WeatherEvent,
    pub expires_at_lingtian_tick: u64,
}

/// plan §3 — 所有 zone 的 active 天气状态 + 上次 RNG roll 的 day（去重避免重复 roll）。
#[derive(Debug, Default, Resource)]
pub struct ActiveWeather {
    by_zone: HashMap<String, ActiveWeatherEntry>,
    /// 已 roll 过的 game-day 编号（lingtian-tick / LINGTIAN_TICKS_PER_DAY）。
    /// generator 每 day 边界跨过时才 roll，避免一 day 内多次 roll。
    last_rolled_day: Option<u64>,
}

impl ActiveWeather {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self, zone: &str) -> Option<WeatherEvent> {
        self.by_zone.get(zone).map(|e| e.event)
    }

    pub fn current_entry(&self, zone: &str) -> Option<&ActiveWeatherEntry> {
        self.by_zone.get(zone)
    }

    pub fn insert(
        &mut self,
        zone: impl Into<String>,
        event: WeatherEvent,
        expires_at_lingtian_tick: u64,
    ) {
        self.by_zone.insert(
            zone.into(),
            ActiveWeatherEntry {
                event,
                expires_at_lingtian_tick,
            },
        );
    }

    /// 移除已过期事件，返回被移除的 (zone, event) 列表（供下游 narration / log 用）。
    pub fn prune_expired(&mut self, now_lingtian_tick: u64) -> Vec<(String, WeatherEvent)> {
        let mut expired = Vec::new();
        self.by_zone.retain(|zone, e| {
            if e.expires_at_lingtian_tick <= now_lingtian_tick {
                expired.push((zone.clone(), e.event));
                false
            } else {
                true
            }
        });
        expired
    }

    pub fn last_rolled_day(&self) -> Option<u64> {
        self.last_rolled_day
    }

    pub fn set_last_rolled_day(&mut self, day: u64) {
        self.last_rolled_day = Some(day);
    }

    pub fn is_empty(&self) -> bool {
        self.by_zone.is_empty()
    }

    pub fn zones(&self) -> impl Iterator<Item = &String> {
        self.by_zone.keys()
    }
}

/// plan §3 / §4.4 — 天气事件生命周期 Bevy event（generator → redis bridge）。
///
/// `Started`：generator 刚成功 roll 出一个新事件（active 已写入）。
/// `Expired`：weather_apply_to_plot_system 检测到事件自然过期（active 已清除）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Event)]
pub enum WeatherLifecycleEvent {
    Started {
        event: WeatherEvent,
        started_at_lingtian_tick: u64,
        expires_at_lingtian_tick: u64,
    },
    Expired {
        event: WeatherEvent,
        expired_at_lingtian_tick: u64,
    },
}

/// plan §3 — 天气专用 RNG 资源（独立于 LingtianHarvestRng，避免 RNG 状态串扰）。
/// xorshift64 + f32 fraction，与 LingtianHarvestRng 同算法保证测试可重现。
#[derive(Debug, Resource)]
pub struct WeatherRng {
    state: u64,
}

impl WeatherRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        ((x & 0x00FF_FFFF) as f32) / (0x0100_0000_u32 as f32)
    }

    /// 在 `[min, max]` 闭区间内均匀采样 u64。
    pub fn next_u64_range(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(min <= max);
        let span = max - min;
        if span == 0 {
            return min;
        }
        // f32 精度足够 (24 bit fraction) 覆盖 duration 范围（< 2048 ticks）
        let unit = self.next_f32(); // [0, 1)
        let offset = (unit * (span + 1) as f32) as u64;
        min + offset.min(span)
    }
}

impl Default for WeatherRng {
    fn default() -> Self {
        Self::new(0xCAFE_F00D_DEAD_BEEF)
    }
}

// ============================================================================
// 生成器 / 应用 systems
// ============================================================================

/// plan-lingtian-weather-v1 §5 / worldview §八 — 是否当前是"夏季雷暴稳定渡劫窗口"。
///
/// 雷暴在 Summer 与 汐转都可能出现，但 plan §5 把"夏季雷暴"明确作为
/// plan-tribulation-v1 的稳定窗口（汐转期 RNG ×2，事件分布更乱，不稳定）。
/// 本函数是 plan-tribulation-v1 / 调用方查询当前是否处于稳定窗口的薄 helper。
///
/// 不实装 tribulation 触发逻辑——本 plan 仅暴露状态供查询。
pub fn is_stable_tribulation_window(season: Season, active_weather: Option<WeatherEvent>) -> bool {
    matches!(season, Season::Summer) && matches!(active_weather, Some(WeatherEvent::Thunderstorm))
}

/// plan-lingtian-weather-v1 §5 — 当前是否为"汐转期"（plan-narrative-v1 hint 触发条件）。
///
/// 给 plan-narrative-v1 用：天道情绪在汐转期更易触发暗示性 narration（worldview §八）。
pub fn is_xizhuan_phase(season: Season) -> bool {
    season.is_xizhuan()
}

/// plan §3 — 在指定 zone 上"试 roll"一次天气事件；命中 → 写入 `active`。
///
/// 同 zone 已有事件 → 跳过（不覆盖正在进行中的 weather）。否则按 §3 表概率
/// 在 5 个 valid 事件中按"概率独立检查（不归一化）"逐个测试，命中第一个就停。
///
/// 该函数与 system 解耦，方便单测。
pub fn try_roll_weather_for_zone(
    zone: &str,
    season: Season,
    now_lingtian_tick: u64,
    active: &mut ActiveWeather,
    rng: &mut WeatherRng,
) -> Option<WeatherEvent> {
    if active.current(zone).is_some() {
        return None;
    }
    for ev in WeatherEvent::all() {
        let p = ev.daily_probability(season);
        if p <= 0.0 {
            continue;
        }
        if rng.next_f32() < p {
            let (min_dur, max_dur) = ev.duration_range_lingtian_ticks();
            let dur = rng.next_u64_range(min_dur, max_dur);
            let expires_at = now_lingtian_tick.saturating_add(dur);
            active.insert(zone.to_string(), ev, expires_at);
            return Some(ev);
        }
    }
    None
}

/// plan §3 — 每 game-day（1440 lingtian-tick）边界跨过时 RNG roll 一次。
/// 同一 day 多次调用幂等（`last_rolled_day` 去重）。同一 zone 已有 active
/// 事件时跳过（不覆盖）。
///
/// 仅在 `LingtianTickAccumulator` 刚归零时跑（与 pressure / growth 同节拍），
/// 离线时 accumulator 不推进 → 不 roll，回线续播 game-day boundary 自然恢复。
pub fn weather_generator_system(
    accumulator: Res<LingtianTickAccumulator>,
    clock: Res<LingtianClock>,
    season_state: Option<Res<WorldSeasonState>>,
    mut active: ResMut<ActiveWeather>,
    mut rng: ResMut<WeatherRng>,
    mut lifecycle: EventWriter<WeatherLifecycleEvent>,
) {
    if accumulator.raw() != 0 {
        return;
    }
    let now = clock.lingtian_tick;
    let current_day = now / LINGTIAN_TICKS_PER_DAY;
    if active.last_rolled_day() == Some(current_day) {
        return;
    }
    active.set_last_rolled_day(current_day);

    // 先清过期事件（emit Expired），再 roll 新事件。
    for (_zone, event) in active.prune_expired(now) {
        lifecycle.send(WeatherLifecycleEvent::Expired {
            event,
            expired_at_lingtian_tick: now,
        });
    }

    let season = season_state
        .as_deref()
        .map(|s| s.current.season)
        .unwrap_or_default();
    // 单 zone MVP：默认 zone 使用全局季节状态。
    if let Some(event) = try_roll_weather_for_zone(DEFAULT_ZONE, season, now, &mut active, &mut rng)
    {
        let entry = active
            .current_entry(DEFAULT_ZONE)
            .expect("just inserted by try_roll_weather_for_zone");
        lifecycle.send(WeatherLifecycleEvent::Started {
            event,
            started_at_lingtian_tick: now,
            expires_at_lingtian_tick: entry.expires_at_lingtian_tick,
        });
    }
}

/// plan §3 / §4.1 — 每 lingtian-tick 跑一次：清过期事件 + 发广播 hook。
///
/// 当前阶段（P2）只做 expire 清理；P4 在此处接 plot_qi_cap weather 修饰
/// （遍历 zone 内所有 plot 应用 `weather.plot_qi_cap_delta()` 临时改 cap，
/// 事件结束时回退）。
pub fn weather_apply_to_plot_system(
    accumulator: Res<LingtianTickAccumulator>,
    clock: Res<LingtianClock>,
    mut active: ResMut<ActiveWeather>,
    mut lifecycle: EventWriter<WeatherLifecycleEvent>,
) {
    if accumulator.raw() != 0 {
        return;
    }
    let now = clock.lingtian_tick;
    for (_zone, event) in active.prune_expired(now) {
        lifecycle.send(WeatherLifecycleEvent::Expired {
            event,
            expired_at_lingtian_tick: now,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_wire_str_round_trip_round_for_all_variants() {
        // schema/serde 对拍：每个 variant 都有专属 wire 字符串，反序列回原值。
        for ev in WeatherEvent::all() {
            let wire = ev.as_wire_str();
            let json = format!("\"{}\"", wire);
            let back: WeatherEvent =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{wire}: {e}"));
            assert_eq!(back, ev, "{wire} round-trip 失败");
        }
    }

    #[test]
    fn weather_blocks_growth_tick_only_blizzard_and_haze() {
        assert!(WeatherEvent::Blizzard.blocks_growth_tick());
        assert!(WeatherEvent::HeavyHaze.blocks_growth_tick());
        assert!(!WeatherEvent::Thunderstorm.blocks_growth_tick());
        assert!(!WeatherEvent::DroughtWind.blocks_growth_tick());
        assert!(!WeatherEvent::LingMist.blocks_growth_tick());
    }

    #[test]
    fn weather_plot_qi_cap_delta_thunderstorm_minus_0_2() {
        assert!((WeatherEvent::Thunderstorm.plot_qi_cap_delta() + 0.2).abs() < 1e-6);
    }

    #[test]
    fn weather_plot_qi_cap_delta_ling_mist_plus_0_2() {
        assert!((WeatherEvent::LingMist.plot_qi_cap_delta() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn weather_plot_qi_cap_delta_neutral_events_zero() {
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
        ] {
            assert_eq!(
                ev.plot_qi_cap_delta(),
                0.0,
                "{} should be neutral",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn weather_zone_flow_thunderstorm_1_5() {
        assert!((WeatherEvent::Thunderstorm.zone_flow_multiplier() - 1.5).abs() < 1e-6);
        // 其他事件不直接影响 zone_flow（落在 Season 上）。
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.zone_flow_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_qi_decay_drought_wind_doubles() {
        assert!((WeatherEvent::DroughtWind.qi_decay_multiplier() - 2.0).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.qi_decay_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_natural_supply_drought_zero_ling_mist_1_5() {
        assert!(WeatherEvent::DroughtWind.natural_supply_multiplier().abs() < 1e-6);
        assert!((WeatherEvent::LingMist.natural_supply_multiplier() - 1.5).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
        ] {
            assert!((ev.natural_supply_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_shelflife_drought_wind_doubles() {
        assert!((WeatherEvent::DroughtWind.shelflife_decay_multiplier() - 2.0).abs() < 1e-6);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!((ev.shelflife_decay_multiplier() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn weather_pressure_threshold_relax_haze_only() {
        assert_eq!(WeatherEvent::HeavyHaze.pressure_threshold_relax_steps(), 1);
        for ev in [
            WeatherEvent::Thunderstorm,
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::LingMist,
        ] {
            assert_eq!(
                ev.pressure_threshold_relax_steps(),
                0,
                "{} should not relax pressure",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn weather_all_returns_five_distinct_variants() {
        let all = WeatherEvent::all();
        assert_eq!(all.len(), 5);
        let mut set = std::collections::HashSet::new();
        for ev in all {
            set.insert(ev);
        }
        assert_eq!(set.len(), 5, "WeatherEvent::all() 必须返回 5 个不同变体");
    }

    // -------- plan-lingtian-weather-v1 §6 P2 — 概率表 + 季节耦合 --------

    #[test]
    fn weather_thunderstorm_only_in_summer_or_tide() {
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::Summer) > 0.0);
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::SummerToWinter) > 0.0);
        assert!(WeatherEvent::Thunderstorm.daily_probability(Season::WinterToSummer) > 0.0);
        assert_eq!(
            WeatherEvent::Thunderstorm.daily_probability(Season::Winter),
            0.0,
            "雷暴不在冬季出现"
        );
        assert!(!WeatherEvent::Thunderstorm.can_occur_in(Season::Winter));
    }

    #[test]
    fn weather_drought_wind_only_in_summer_or_tide() {
        assert!(WeatherEvent::DroughtWind.daily_probability(Season::Summer) > 0.0);
        assert!(WeatherEvent::DroughtWind.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::DroughtWind.daily_probability(Season::Winter),
            0.0
        );
    }

    #[test]
    fn weather_blizzard_only_in_winter_or_tide() {
        assert!(WeatherEvent::Blizzard.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::Blizzard.daily_probability(Season::SummerToWinter) > 0.0);
        assert!(WeatherEvent::Blizzard.daily_probability(Season::WinterToSummer) > 0.0);
        assert_eq!(
            WeatherEvent::Blizzard.daily_probability(Season::Summer),
            0.0,
            "风雪不在夏季出现"
        );
    }

    #[test]
    fn weather_heavy_haze_only_in_winter_or_tide() {
        assert!(WeatherEvent::HeavyHaze.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::HeavyHaze.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::HeavyHaze.daily_probability(Season::Summer),
            0.0
        );
    }

    #[test]
    fn weather_ling_mist_only_in_winter_or_tide() {
        assert!(WeatherEvent::LingMist.daily_probability(Season::Winter) > 0.0);
        assert!(WeatherEvent::LingMist.daily_probability(Season::SummerToWinter) > 0.0);
        assert_eq!(
            WeatherEvent::LingMist.daily_probability(Season::Summer),
            0.0
        );
    }

    #[test]
    fn weather_tide_doubles_base_thunderstorm_rng() {
        // §3 表 — 雷暴：base 1% / Summer × 3 = 3% / 汐转 × 2 = 2%
        // 夏 0.03 ≠ 汐转 0.02；汐转 = base 1% × 2
        let summer_p = WeatherEvent::Thunderstorm.daily_probability(Season::Summer);
        let xizhuan_p = WeatherEvent::Thunderstorm.daily_probability(Season::SummerToWinter);
        assert!((summer_p - 0.03).abs() < 1e-6);
        assert!((xizhuan_p - 0.02).abs() < 1e-6);
        // 汐转 prob = base 1% × 2 = 2%
        assert!(xizhuan_p > 0.0 && xizhuan_p < summer_p);
    }

    #[test]
    fn weather_tide_triples_base_ling_mist_rng() {
        // §3 表 — 灵雾：base 1% / Winter / 汐转 × 3 = 3%
        let winter_p = WeatherEvent::LingMist.daily_probability(Season::Winter);
        let xizhuan_p = WeatherEvent::LingMist.daily_probability(Season::SummerToWinter);
        assert!((winter_p - 0.01).abs() < 1e-6);
        assert!((xizhuan_p - 0.03).abs() < 1e-6);
        assert!(xizhuan_p > winter_p, "灵雾汐转应该 > 冬");
    }

    #[test]
    fn weather_duration_ranges_match_plan_table() {
        // §3 表 — 持续时间区间核验（lingtian-tick）：1 game-hour = 60 lingtian-tick
        assert_eq!(
            WeatherEvent::Thunderstorm.duration_range_lingtian_ticks(),
            (120, 240)
        );
        assert_eq!(
            WeatherEvent::DroughtWind.duration_range_lingtian_ticks(),
            (360, 720)
        );
        assert_eq!(
            WeatherEvent::Blizzard.duration_range_lingtian_ticks(),
            (720, 1440)
        );
        assert_eq!(
            WeatherEvent::HeavyHaze.duration_range_lingtian_ticks(),
            (720, 1440)
        );
        assert_eq!(
            WeatherEvent::LingMist.duration_range_lingtian_ticks(),
            (60, 120)
        );
    }

    // -------- ActiveWeather Resource --------

    #[test]
    fn active_weather_insert_and_current_round_trip() {
        let mut active = ActiveWeather::new();
        active.insert("zone_a", WeatherEvent::Thunderstorm, 200);
        assert_eq!(active.current("zone_a"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(active.current("zone_b"), None);
    }

    #[test]
    fn active_weather_event_remaining_ticks_decrements() {
        // event_remaining_ticks 直观语义：expires_at - now_tick 单调下降
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 1000);
        let entry = active.current_entry("z").expect("just inserted");
        assert_eq!(entry.expires_at_lingtian_tick, 1000);
        // remaining at tick=100 → 900；tick=500 → 500；tick=999 → 1
        for now in [100u64, 500, 999] {
            let remaining = entry.expires_at_lingtian_tick.saturating_sub(now);
            assert_eq!(remaining, 1000 - now);
        }
    }

    #[test]
    fn active_weather_event_expires_clears_active_weather() {
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 100);
        assert!(active.current("z").is_some());
        // tick 100 → expires_at <= now → 清除
        let removed = active.prune_expired(100);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].1, WeatherEvent::Thunderstorm);
        assert!(active.current("z").is_none());
        // 二次 prune 应当无变化
        let removed2 = active.prune_expired(200);
        assert!(removed2.is_empty());
    }

    #[test]
    fn active_weather_prune_keeps_unexpired() {
        let mut active = ActiveWeather::new();
        active.insert("z1", WeatherEvent::Thunderstorm, 200);
        active.insert("z2", WeatherEvent::LingMist, 50);
        active.prune_expired(100);
        // z2 expired (50 <= 100)，z1 still alive (200 > 100)
        assert_eq!(active.current("z1"), Some(WeatherEvent::Thunderstorm));
        assert_eq!(active.current("z2"), None);
    }

    // -------- WeatherRng --------

    #[test]
    fn weather_rng_deterministic_with_same_seed() {
        let mut a = WeatherRng::new(42);
        let mut b = WeatherRng::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_f32(), b.next_f32(), "seed=42 必须 deterministic");
        }
    }

    #[test]
    fn weather_rng_next_f32_within_unit_range() {
        let mut rng = WeatherRng::new(7);
        for _ in 0..100 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v), "next_f32={v} 越界");
        }
    }

    #[test]
    fn weather_rng_next_u64_range_within_bounds() {
        let mut rng = WeatherRng::new(13);
        for _ in 0..100 {
            let v = rng.next_u64_range(360, 720);
            assert!(
                (360..=720).contains(&v),
                "next_u64_range(360, 720) = {v} 越界"
            );
        }
    }

    #[test]
    fn weather_rng_next_u64_range_collapsed_min_eq_max() {
        let mut rng = WeatherRng::new(9);
        assert_eq!(rng.next_u64_range(100, 100), 100);
    }

    // -------- try_roll_weather_for_zone --------

    #[test]
    fn try_roll_skips_when_zone_already_has_event() {
        let mut active = ActiveWeather::new();
        active.insert("z", WeatherEvent::Thunderstorm, 500);
        let mut rng = WeatherRng::new(1);
        let res = try_roll_weather_for_zone("z", Season::Summer, 0, &mut active, &mut rng);
        assert_eq!(res, None, "已有 active 事件时不应再 roll");
        assert_eq!(active.current("z"), Some(WeatherEvent::Thunderstorm));
    }

    #[test]
    fn try_roll_seasons_winter_skips_summer_only_events() {
        // 冬季 roll：雷暴 / 旱风 prob=0 → 永远跳过；可能命中 风雪 / 阴霾 / 灵雾
        let mut hit_summer_only = 0;
        let mut hit_winter_valid = 0;
        for seed in 1u64..200 {
            let mut active = ActiveWeather::new();
            let mut rng = WeatherRng::new(seed);
            if let Some(ev) =
                try_roll_weather_for_zone("z", Season::Winter, 0, &mut active, &mut rng)
            {
                if matches!(ev, WeatherEvent::Thunderstorm | WeatherEvent::DroughtWind) {
                    hit_summer_only += 1;
                } else {
                    hit_winter_valid += 1;
                }
            }
        }
        assert_eq!(hit_summer_only, 0, "冬季不应触发夏限定事件");
        // winter prob 总和 ≈ 4.5%，200 次 seed 至少命中数次
        assert!(hit_winter_valid > 0, "200 次 seed 应至少触发一次冬天事件");
    }

    #[test]
    fn try_roll_summer_only_triggers_summer_or_tide_events() {
        let mut hit_winter_only = 0;
        let mut hit_summer_valid = 0;
        for seed in 1u64..200 {
            let mut active = ActiveWeather::new();
            let mut rng = WeatherRng::new(seed);
            if let Some(ev) =
                try_roll_weather_for_zone("z", Season::Summer, 0, &mut active, &mut rng)
            {
                if matches!(
                    ev,
                    WeatherEvent::Blizzard | WeatherEvent::HeavyHaze | WeatherEvent::LingMist
                ) {
                    hit_winter_only += 1;
                } else {
                    hit_summer_valid += 1;
                }
            }
        }
        assert_eq!(hit_winter_only, 0, "夏季不应触发冬限定事件");
        assert!(hit_summer_valid > 0);
    }

    #[test]
    fn try_roll_inserts_event_with_duration_in_range() {
        // 强 RNG 注入：用一个会命中第一个 valid 事件的种子（雷暴 0.03）
        let mut active = ActiveWeather::new();
        // 找一个能命中的种子（暴力扫一定能找到）
        let mut hit_seed = None;
        for seed in 1u64..200 {
            let mut rng = WeatherRng::new(seed);
            let mut probe = ActiveWeather::new();
            if try_roll_weather_for_zone("z", Season::Summer, 1000, &mut probe, &mut rng).is_some()
            {
                hit_seed = Some(seed);
                break;
            }
        }
        let seed = hit_seed.expect("200 次种子至少命中一次");
        let mut rng = WeatherRng::new(seed);
        let ev = try_roll_weather_for_zone("z", Season::Summer, 1000, &mut active, &mut rng)
            .expect("命中 seed");
        let entry = active.current_entry("z").expect("event inserted");
        let (min_d, max_d) = ev.duration_range_lingtian_ticks();
        let dur = entry.expires_at_lingtian_tick - 1000;
        assert!(
            (min_d..=max_d).contains(&dur),
            "{ev:?} duration {dur} 不在 [{min_d}, {max_d}]"
        );
    }

    // -------- plan-lingtian-weather-v1 §6 P4 hooks --------

    #[test]
    fn is_stable_tribulation_window_only_summer_thunderstorm() {
        // 唯一返回 true 的组合：Summer + Thunderstorm
        assert!(is_stable_tribulation_window(
            Season::Summer,
            Some(WeatherEvent::Thunderstorm)
        ));
        // 其他季节 + 雷暴：false
        for season in [
            Season::Winter,
            Season::SummerToWinter,
            Season::WinterToSummer,
        ] {
            assert!(
                !is_stable_tribulation_window(season, Some(WeatherEvent::Thunderstorm)),
                "汐转 / 冬 + 雷暴不应当稳定，{season:?}"
            );
        }
        // Summer + 其他事件：false
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
            WeatherEvent::LingMist,
        ] {
            assert!(!is_stable_tribulation_window(Season::Summer, Some(ev)));
        }
        // 无事件：false
        assert!(!is_stable_tribulation_window(Season::Summer, None));
    }

    #[test]
    fn is_xizhuan_phase_helper_matches_season_is_xizhuan() {
        // narrative hint helper：跟 Season::is_xizhuan() 同语义。
        assert!(!is_xizhuan_phase(Season::Summer));
        assert!(!is_xizhuan_phase(Season::Winter));
        assert!(is_xizhuan_phase(Season::SummerToWinter));
        assert!(is_xizhuan_phase(Season::WinterToSummer));
    }
}
