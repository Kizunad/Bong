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
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{bevy_ecs, Event, EventWriter, Res, ResMut, Resource};

use crate::world::season::{Season, WorldSeasonState};
use crate::world::zone::ZoneRegistry;

use super::pressure::LINGTIAN_TICKS_PER_DAY;
use super::qi_account::{LingtianTickAccumulator, DEFAULT_ZONE};
use super::systems::LingtianClock;
use super::weather_profile::{ZoneWeatherProfile, ZoneWeatherProfileRegistry};

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

/// plan §3 — 单个 zone 上当前 active 的天气事件 + 起始 / 过期 tick。
///
/// `started_at_lingtian_tick` 在事件 expired 后由 `prune_expired` 一并返回，
/// 让下游 bridge 能在 wire payload 上保留"started_at < expires_at"不变量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveWeatherEntry {
    pub event: WeatherEvent,
    pub started_at_lingtian_tick: u64,
    pub expires_at_lingtian_tick: u64,
}

/// plan §3 — 所有 zone 的 active 天气状态 + 上次 RNG roll 的 day（去重避免重复 roll）。
#[derive(Debug, Default, Resource)]
pub struct ActiveWeather {
    by_zone: HashMap<String, ActiveWeatherEntry>,
    /// 每个 zone 已 roll 过的 game-day 编号（lingtian-tick / LINGTIAN_TICKS_PER_DAY）。
    /// zone-aware generator 每 day 边界逐 zone roll，避免 A zone 的去重挡住 B zone。
    last_rolled_day_by_zone: HashMap<String, u64>,
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
        started_at_lingtian_tick: u64,
        expires_at_lingtian_tick: u64,
    ) {
        self.by_zone.insert(
            zone.into(),
            ActiveWeatherEntry {
                event,
                started_at_lingtian_tick,
                expires_at_lingtian_tick,
            },
        );
    }

    /// 移除已过期事件，返回被移除的 (zone, entry) 列表（供下游 narration /
    /// bridge 在 wire payload 上保留 `started_at` 用）。
    pub fn prune_expired(&mut self, now_lingtian_tick: u64) -> Vec<(String, ActiveWeatherEntry)> {
        let mut expired = Vec::new();
        self.by_zone.retain(|zone, e| {
            if e.expires_at_lingtian_tick <= now_lingtian_tick {
                expired.push((zone.clone(), *e));
                false
            } else {
                true
            }
        });
        expired
    }

    pub fn last_rolled_day(&self) -> Option<u64> {
        self.last_rolled_day_for(DEFAULT_ZONE)
    }

    pub fn set_last_rolled_day(&mut self, day: u64) {
        self.set_last_rolled_day_for(DEFAULT_ZONE, day);
    }

    pub fn last_rolled_day_for(&self, zone: &str) -> Option<u64> {
        self.last_rolled_day_by_zone.get(zone).copied()
    }

    pub fn set_last_rolled_day_for(&mut self, zone: impl Into<String>, day: u64) {
        self.last_rolled_day_by_zone.insert(zone.into(), day);
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
/// `Expired`：weather generator / apply system 检测到事件自然过期（active 已清除）。
///
/// `Expired` 同时携带 `started_at_lingtian_tick` 与 `expired_at_lingtian_tick`，
/// 让 bridge 派生的 wire payload 能保持 `started_at <= expires_at` 不变量
/// （消费方据此区分"自然过期"与"刚开始就 expire"，避免信息丢失）。
#[derive(Debug, Clone, PartialEq, Eq, Event)]
pub enum WeatherLifecycleEvent {
    Started {
        zone: String,
        event: WeatherEvent,
        started_at_lingtian_tick: u64,
        expires_at_lingtian_tick: u64,
    },
    Expired {
        zone: String,
        event: WeatherEvent,
        started_at_lingtian_tick: u64,
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
/// 同 zone 已有事件 → 跳过（不覆盖正在进行中的 weather）。否则按
/// `WeatherEvent::all()` 的固定枚举顺序（雷暴 / 旱风 / 风雪 / 阴霾 / 灵雾）
/// **first-hit short-circuit**：每个事件单独抽 RNG `next_f32()`，第一个
/// `< daily_probability` 的命中即返回，后续不再 roll。
///
/// 这意味着排在前面的事件实测概率 ≈ plan §3 数表，排在后面（如 LingMist）的
/// 实测概率略低于 plan 数（必须前 4 个全 miss 才能轮到）。如果未来需要严格
/// uniform sampling，可改成累加 weight → 单次 unit float 二分。
///
/// 该函数与 system 解耦，方便单测注入 RNG / 季节。
pub fn try_roll_weather_for_zone(
    zone: &str,
    season: Season,
    now_lingtian_tick: u64,
    active: &mut ActiveWeather,
    rng: &mut WeatherRng,
) -> Option<WeatherEvent> {
    try_roll_weather_for_zone_with_profile(
        zone,
        season,
        now_lingtian_tick,
        active,
        rng,
        &ZoneWeatherProfile::default(),
    )
}

/// plan-zone-weather-v1 P0 — profile-aware weather roll for one zone.
///
/// `force_event` bypasses RNG but still respects the "already active means no refresh"
/// invariant. Probability multipliers are applied per event and clamped into [0, 1].
pub fn try_roll_weather_for_zone_with_profile(
    zone: &str,
    season: Season,
    now_lingtian_tick: u64,
    active: &mut ActiveWeather,
    rng: &mut WeatherRng,
    profile: &ZoneWeatherProfile,
) -> Option<WeatherEvent> {
    if active.current(zone).is_some() {
        return None;
    }
    if let Some(ev) = profile.force_event {
        insert_weather_event(zone, ev, now_lingtian_tick, active, rng);
        return Some(ev);
    }
    for ev in WeatherEvent::all() {
        let p = profile.effective_probability(ev, season);
        if p <= 0.0 {
            continue;
        }
        if rng.next_f32() < p {
            insert_weather_event(zone, ev, now_lingtian_tick, active, rng);
            return Some(ev);
        }
    }
    None
}

fn insert_weather_event(
    zone: &str,
    event: WeatherEvent,
    now_lingtian_tick: u64,
    active: &mut ActiveWeather,
    rng: &mut WeatherRng,
) {
    let (min_dur, max_dur) = event.duration_range_lingtian_ticks();
    let dur = rng.next_u64_range(min_dur, max_dur);
    let expires_at = now_lingtian_tick.saturating_add(dur);
    active.insert(zone.to_string(), event, now_lingtian_tick, expires_at);
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
    for (zone, entry) in active.prune_expired(now) {
        lifecycle.send(WeatherLifecycleEvent::Expired {
            zone,
            event: entry.event,
            started_at_lingtian_tick: entry.started_at_lingtian_tick,
            expired_at_lingtian_tick: entry.expires_at_lingtian_tick,
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
            zone: DEFAULT_ZONE.to_string(),
            event,
            started_at_lingtian_tick: now,
            expires_at_lingtian_tick: entry.expires_at_lingtian_tick,
        });
    }
}

/// plan-zone-weather-v1 P0 — zone-aware generator.
///
/// 与单 zone MVP 共存：缺 `ZoneRegistry` 时退回 DEFAULT_ZONE；缺 profile registry
/// 时每个 zone 使用默认 profile，即等价于 lingtian-weather 原概率表。
#[derive(SystemParam)]
pub struct ZoneAwareWeatherGenerationParams<'w> {
    accumulator: Res<'w, LingtianTickAccumulator>,
    clock: Res<'w, LingtianClock>,
    season_state: Option<Res<'w, WorldSeasonState>>,
    zone_registry: Option<Res<'w, ZoneRegistry>>,
    profile_registry: Option<Res<'w, ZoneWeatherProfileRegistry>>,
}

pub fn weather_generator_system_zone_aware(
    params: ZoneAwareWeatherGenerationParams,
    mut active: ResMut<ActiveWeather>,
    mut lifecycle: EventWriter<WeatherLifecycleEvent>,
) {
    if params.accumulator.raw() != 0 {
        return;
    }
    let now = params.clock.lingtian_tick;
    let current_day = now / LINGTIAN_TICKS_PER_DAY;

    for (zone, entry) in active.prune_expired(now) {
        lifecycle.send(WeatherLifecycleEvent::Expired {
            zone,
            event: entry.event,
            started_at_lingtian_tick: entry.started_at_lingtian_tick,
            expired_at_lingtian_tick: entry.expires_at_lingtian_tick,
        });
    }

    let season = params
        .season_state
        .as_deref()
        .map(|s| s.current.season)
        .unwrap_or_default();
    let zone_names: Vec<String> = match params.zone_registry.as_ref() {
        Some(registry) => registry
            .zones
            .iter()
            .map(|zone| zone.name.clone())
            .collect(),
        None => vec![DEFAULT_ZONE.to_string()],
    };
    if zone_names.is_empty() {
        return;
    }

    for zone in zone_names {
        if active.last_rolled_day_for(zone.as_str()) == Some(current_day) {
            continue;
        }
        active.set_last_rolled_day_for(zone.clone(), current_day);
        let mut zone_rng = WeatherRng::new(weather_zone_day_seed(zone.as_str(), current_day));
        let profile = params
            .profile_registry
            .as_deref()
            .and_then(|registry| registry.get(zone.as_str()))
            .cloned()
            .unwrap_or_default();
        if let Some(event) = try_roll_weather_for_zone_with_profile(
            zone.as_str(),
            season,
            now,
            &mut active,
            &mut zone_rng,
            &profile,
        ) {
            let entry = active
                .current_entry(zone.as_str())
                .expect("just inserted by try_roll_weather_for_zone_with_profile");
            lifecycle.send(WeatherLifecycleEvent::Started {
                zone,
                event,
                started_at_lingtian_tick: now,
                expires_at_lingtian_tick: entry.expires_at_lingtian_tick,
            });
        }
    }
}

fn weather_zone_day_seed(zone: &str, current_day: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ current_day.rotate_left(17);
    for byte in zone.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash ^ current_day.wrapping_mul(0x9e37_79b9_7f4a_7c15).max(1)
}

/// plan §3 / §4.1 — game-day 边界跑一次：兜底 expire 清理（generator 已在
/// 同 day 边界先 prune 一次，此处通常是 no-op，作为 generator 跳过本 day
/// roll 时的二次防御）。
///
/// gate 与 weather_generator_system 同节拍（`accumulator.raw() == 0` 即 day
/// boundary），不在每 lingtian-tick 跑——避免 prune 重复扫描 + EventWriter
/// 死分支。
///
/// **未来 polish（P4+ 范围外）**：把"plot.environment.active_weather 跟随
/// `Res<ActiveWeather>` 的当前事件"合并到此 system，让
/// `compute_plot_qi_cap` / `qi_decay_multiplier` / `blocks_growth_tick` /
/// `shelflife_decay_multiplier` 真正在生产路径生效（当前仅 pressure 路径
/// 直接 `Res<ActiveWeather>::current()` 接通；plot 端 env.active_weather
/// 仍由测试构造，未由 system 写入）。
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
    for (zone, entry) in active.prune_expired(now) {
        lifecycle.send(WeatherLifecycleEvent::Expired {
            zone,
            event: entry.event,
            started_at_lingtian_tick: entry.started_at_lingtian_tick,
            expired_at_lingtian_tick: entry.expires_at_lingtian_tick,
        });
    }
}
