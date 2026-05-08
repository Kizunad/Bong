//! plan-lingtian-weather-v1 §3 — 天气事件枚举（短时事件，数小时 in-game）。
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
//! 持续时间 / 触发概率 / apply 系统在 P2 实装；本模块（P0）只锁定 enum + 枚举级
//! 修饰常量，给下游 grep 抓手对齐用。

use serde::{Deserialize, Serialize};

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
}
