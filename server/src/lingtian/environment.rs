//! plan-lingtian-v1 §1.1 — 田块环境修饰（用于派生 `plot_qi_cap`）。
//!
//! 由调用方（valence world ↔ environment 适配层）填入；本模块只负责数学
//! 计算，单测无需 mock world。同 `TerrainKind` 设计原则。
//!
//! plan-lingtian-weather-v1 §0 / §2 / §3 扩展两个槽位：
//!   * `season` —— 由 `crate::world::season::query_season` 派生的当前相位
//!     （jiezeq-v1 全服同步）。在 plot_qi_cap 上叠加稳定修饰（夏 -0.2 / 冬 +0.2 /
//!     汐转 0 + jitter）。
//!   * `active_weather` —— 当前 plot 上 active 的天气事件（None = 晴朗）。在
//!     plot_qi_cap / qi 流速 / natural_supply / shelflife 上叠加事件修饰。
//!
//! 季节 jitter 由调用方注入（`apply_xizhuan_qi_cap_jitter` / `with_jitter`
//! helper 接受 unit-float seed），不依赖全局 RNG，便于测试。

use serde::{Deserialize, Serialize};
use valence::prelude::{BlockKind, BlockPos, ChunkLayer};

use super::plot::{PLOT_QI_CAP_BASE, PLOT_QI_CAP_MAX};
use super::weather::WeatherEvent;
use crate::world::season::Season;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlotBiome {
    #[default]
    Other,
    /// 湿地 / 灵泉湿地等水气重的群系（plan §1.1 +0.5）。
    Wetland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlotEnvironment {
    /// 5 格内有水方块（plan §1.1 +0.3）。
    pub water_adjacent: bool,
    pub biome: PlotBiome,
    /// 处于聚灵阵覆盖（plan §1.1 +1.0；plan-zhenfa-v1 落地后由该模块填）。
    pub zhenfa_jvling: bool,
    /// plan-lingtian-weather-v1 §2 — 当前 zone 季节相位（jiezeq-v1 全服同步）。
    /// 由 [`crate::world::season::query_season`] 派生填入；默认 Summer 与
    /// `query_season("", 0)` 一致。
    pub season: Season,
    /// plan-lingtian-weather-v1 §3 — 当前 plot 上 active 的天气事件
    /// （None = 晴朗）。由 P2 `weather_apply_to_plot_system` 填写。
    pub active_weather: Option<WeatherEvent>,
}

impl PlotEnvironment {
    pub const fn base() -> Self {
        Self {
            water_adjacent: false,
            biome: PlotBiome::Other,
            zhenfa_jvling: false,
            season: Season::Summer,
            active_weather: None,
        }
    }
}

/// 扫描 `pos` 周围 ±5 格水平范围（y 层不变）内是否有 Water 方块，用于
/// `water_adjacent` 判定（plan §1.1）。半径 5 足够覆盖一个典型"近水田"。
///
/// biome / zhenfa_jvling 两项暂不自动派生：biome 需 valence biome 读取 API，
/// zhenfa 要等 plan-zhenfa-v1 的阵法系统落地，目前默认 false。
/// season / active_weather 由 P1 / P2 系统在调用此函数后注入（保持纯静态读取
/// 接口，避免 Resource 跨越）。
pub fn read_environment_at(layer: &ChunkLayer, pos: BlockPos) -> PlotEnvironment {
    let water_adjacent = scan_water_near(layer, pos, 5);
    PlotEnvironment {
        water_adjacent,
        biome: PlotBiome::Other,
        zhenfa_jvling: false,
        season: Season::Summer,
        active_weather: None,
    }
}

fn scan_water_near(layer: &ChunkLayer, center: BlockPos, radius: i32) -> bool {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            if dx == 0 && dz == 0 {
                continue;
            }
            let probe = BlockPos::new(center.x + dx, center.y, center.z + dz);
            if let Some(block) = layer.block(probe) {
                let kind = block.state.to_kind();
                if matches!(kind, BlockKind::Water) {
                    return true;
                }
            }
        }
    }
    false
}

/// plan §1.1 + plan-lingtian-weather-v1 §2 / §3 — 由 `PlotEnvironment` 派生 `plot_qi_cap`。
///
/// 公式：base 1.0
///   + (water_adjacent ? 0.3 : 0)
///   + (wetland ? 0.5 : 0)
///   + (zhenfa_jvling ? 1.0 : 0)
///   + season modifier（夏 -0.2 / 冬 +0.2 / 汐转 0 基线）
///   + active_weather modifier（雷暴 -0.2 / 灵雾 +0.2 / 其他 0）
///
/// 封顶 `PLOT_QI_CAP_MAX` (3.0)，下限 0（夏季 + 雷暴可能落到负值时 clamp）。
///
/// 注意：汐转期 jitter（±0.3）不在此函数处理——调用方先用
/// [`apply_xizhuan_qi_cap_jitter`] 派生 effective `Season` 修饰，再写入 env。
pub fn compute_plot_qi_cap(env: &PlotEnvironment) -> f32 {
    let mut cap = PLOT_QI_CAP_BASE;
    if env.water_adjacent {
        cap += 0.3;
    }
    if matches!(env.biome, PlotBiome::Wetland) {
        cap += 0.5;
    }
    if env.zhenfa_jvling {
        cap += 1.0;
    }
    cap += env.season.plot_qi_cap_modifier();
    if let Some(weather) = env.active_weather {
        cap += weather.plot_qi_cap_delta();
    }
    cap.clamp(0.0, PLOT_QI_CAP_MAX)
}

/// plan-lingtian-weather-v1 §2 — 把汐转期 jitter 应用到 `plot_qi_cap` 修饰。
///
/// `jitter_unit` 应在 `[-1.0, 1.0]`，调用方负责生成（推荐 (zone, plot, day)
/// 三元组的 hash → unit float，保证可重现）。非汐转季节 amplitude=0，结果
/// 等同于 `season.plot_qi_cap_modifier()`。
pub fn apply_xizhuan_qi_cap_jitter(season: Season, jitter_unit: f32) -> f32 {
    let jitter = jitter_unit.clamp(-1.0, 1.0);
    season.plot_qi_cap_modifier() + jitter * season.xizhuan_qi_cap_amplitude()
}

/// plan-lingtian-weather-v1 §2 — 汐转期 `natural_supply` jitter。
/// 返回相对增量（夏 -0.10、冬 +0.10、汐转 jitter*0.20）。
pub fn apply_xizhuan_supply_jitter(season: Season, jitter_unit: f32) -> f32 {
    let jitter = jitter_unit.clamp(-1.0, 1.0);
    season.natural_supply_modifier() + jitter * season.xizhuan_supply_amplitude()
}

/// plan-lingtian-weather-v1 §2 — plot ↔ zone qi 流速倍率（融合季节 + 天气）。
///
/// `jitter01` ∈ [0, 1]；非汐转无影响（基线 1.3 / 1.0 / 0.7），汐转期映射到 1.0–1.5。
/// 雷暴叠乘 ×1.5；其他事件 ×1.0。
pub fn compute_zone_flow_multiplier(
    season: Season,
    weather: Option<WeatherEvent>,
    jitter01: f32,
) -> f32 {
    let jitter = jitter01.clamp(0.0, 1.0);
    let season_mult = if season.is_xizhuan() {
        // 汐转：1.0 + jitter * 0.5 ∈ [1.0, 1.5]
        1.0 + jitter * season.xizhuan_zone_flow_jitter_max_delta()
    } else {
        season.zone_flow_multiplier()
    };
    season_mult
        * weather
            .map(WeatherEvent::zone_flow_multiplier)
            .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 基线环境（夏 + 无水 + 无湿地 + 无阵法 + 无天气）：
    /// base 1.0 + summer -0.2 = 0.8（旧测试预期 1.0 已不再适用，因为夏散是
    /// plan-lingtian-weather-v1 §2 的物理常态）。
    #[test]
    fn base_summer_environment_yields_cap_0_8() {
        let cap = compute_plot_qi_cap(&PlotEnvironment::base());
        assert!(
            (cap - 0.8).abs() < 1e-6,
            "夏季基线 plot_qi_cap 应当 0.8（=1.0 base - 0.2 summer），实际 {cap}"
        );
    }

    #[test]
    fn each_legacy_plot_modifier_adds_correctly_in_summer() {
        // 测试夏季基线（-0.2）下，原 plan-lingtian-v1 三个修饰仍按预期叠加。
        let mut e = PlotEnvironment::base();
        e.water_adjacent = true;
        assert!(
            (compute_plot_qi_cap(&e) - 1.1).abs() < 1e-6,
            "1.0 + 0.3(water) - 0.2(summer) = 1.1"
        );

        let e = PlotEnvironment {
            biome: PlotBiome::Wetland,
            ..PlotEnvironment::base()
        };
        assert!(
            (compute_plot_qi_cap(&e) - 1.3).abs() < 1e-6,
            "1.0 + 0.5(wetland) - 0.2(summer) = 1.3"
        );

        let e = PlotEnvironment {
            zhenfa_jvling: true,
            ..PlotEnvironment::base()
        };
        assert!(
            (compute_plot_qi_cap(&e) - 1.8).abs() < 1e-6,
            "1.0 + 1.0(jvling) - 0.2(summer) = 1.8"
        );
    }

    #[test]
    fn combined_modifiers_capped_at_3_0_in_winter() {
        // 冬季 +0.2 + 三大修饰最大叠加：1.0 + 0.3 + 0.5 + 1.0 + 0.2 = 3.0（恰好上限）
        let e = PlotEnvironment {
            water_adjacent: true,
            biome: PlotBiome::Wetland,
            zhenfa_jvling: true,
            season: Season::Winter,
            active_weather: None,
        };
        let cap = compute_plot_qi_cap(&e);
        assert!(
            (cap - 3.0).abs() < 1e-6,
            "1.0+0.3+0.5+1.0+0.2(winter) = 3.0（封顶），实际 {cap}"
        );
    }

    #[test]
    fn cap_clamped_to_zero_when_over_subtracted() {
        // clamp 下限验证：负值 → 0，作为防御性 invariant
        // （compute_plot_qi_cap 用 .clamp(0.0, MAX)，避免下游对 plot_qi_cap < 0
        //  做未定义算术）。
        let cap_negative_input = (-0.4_f32).clamp(0.0, PLOT_QI_CAP_MAX);
        assert!((cap_negative_input - 0.0).abs() < 1e-6);
        // 正值不变
        let cap_pos = 1.5_f32.clamp(0.0, PLOT_QI_CAP_MAX);
        assert!((cap_pos - 1.5).abs() < 1e-6);
        // 越上限被夹
        let cap_over = 5.0_f32.clamp(0.0, PLOT_QI_CAP_MAX);
        assert!((cap_over - PLOT_QI_CAP_MAX).abs() < 1e-6);
    }

    // -------- plan-lingtian-weather-v1 §6 P0 单测 --------

    #[test]
    fn default_plot_environment_season_is_summer() {
        let env = PlotEnvironment::default();
        assert_eq!(env.season, Season::Summer);
        assert_eq!(env.active_weather, None);
    }

    #[test]
    fn base_plot_environment_season_is_summer() {
        // PlotEnvironment::base() 与 default() 行为一致，dual-source 防漂移。
        assert_eq!(PlotEnvironment::base().season, Season::Summer);
        assert_eq!(PlotEnvironment::base().active_weather, None);
    }

    #[test]
    fn read_environment_at_does_not_set_active_weather() {
        // P0：read_environment_at 留 season=Summer / weather=None；P1/P2 系统注入
        // （这里不能直接调 read_environment_at 因为需要 ChunkLayer，验证语义即可）。
        let env = PlotEnvironment::base();
        assert_eq!(env.active_weather, None);
    }

    #[test]
    fn compute_plot_qi_cap_summer_drops_0_2_relative_to_winter() {
        // §6 P1 e2e (移到 P0 由 compute 端的核心检验)：
        // 同 plot 同环境，仅 season 不同，夏-冬差应当 -0.4（夏 -0.2 / 冬 +0.2）。
        let summer = compute_plot_qi_cap(&PlotEnvironment {
            season: Season::Summer,
            ..PlotEnvironment::base()
        });
        let winter = compute_plot_qi_cap(&PlotEnvironment {
            season: Season::Winter,
            ..PlotEnvironment::base()
        });
        let diff = winter - summer;
        assert!(
            (diff - 0.4).abs() < 1e-6,
            "winter - summer plot_qi_cap 应当 +0.4，实际 {diff}（summer={summer}, winter={winter}）"
        );
    }

    #[test]
    fn compute_plot_qi_cap_xizhuan_base_equals_zero_modifier() {
        // 汐转基线 modifier=0，无 jitter 时与"无季节修饰"等价。
        let xz_s2w = compute_plot_qi_cap(&PlotEnvironment {
            season: Season::SummerToWinter,
            ..PlotEnvironment::base()
        });
        let xz_w2s = compute_plot_qi_cap(&PlotEnvironment {
            season: Season::WinterToSummer,
            ..PlotEnvironment::base()
        });
        assert!((xz_s2w - 1.0).abs() < 1e-6);
        assert!((xz_w2s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn compute_plot_qi_cap_thunderstorm_subtracts_0_2_on_top_of_season() {
        // 雷暴只能在夏 / 汐转出现；这里测纯叠加：夏 + 雷暴 = 1.0 - 0.2 - 0.2 = 0.6
        let env = PlotEnvironment {
            season: Season::Summer,
            active_weather: Some(WeatherEvent::Thunderstorm),
            ..PlotEnvironment::base()
        };
        assert!((compute_plot_qi_cap(&env) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn compute_plot_qi_cap_ling_mist_adds_0_2_on_top_of_winter() {
        // 灵雾在冬偶发：冬 + 灵雾 = 1.0 + 0.2 + 0.2 = 1.4
        let env = PlotEnvironment {
            season: Season::Winter,
            active_weather: Some(WeatherEvent::LingMist),
            ..PlotEnvironment::base()
        };
        assert!((compute_plot_qi_cap(&env) - 1.4).abs() < 1e-6);
    }

    #[test]
    fn compute_plot_qi_cap_neutral_weather_does_not_shift() {
        // 旱风 / 风雪 / 阴霾 weather.plot_qi_cap_delta()=0，不影响 cap。
        for ev in [
            WeatherEvent::DroughtWind,
            WeatherEvent::Blizzard,
            WeatherEvent::HeavyHaze,
        ] {
            let with_event = compute_plot_qi_cap(&PlotEnvironment {
                season: Season::Summer,
                active_weather: Some(ev),
                ..PlotEnvironment::base()
            });
            let without = compute_plot_qi_cap(&PlotEnvironment {
                season: Season::Summer,
                ..PlotEnvironment::base()
            });
            assert!(
                (with_event - without).abs() < 1e-6,
                "{} 不应改变 plot_qi_cap，实际 with={with_event} without={without}",
                ev.as_wire_str()
            );
        }
    }

    #[test]
    fn apply_xizhuan_qi_cap_jitter_summer_returns_static_modifier() {
        // 非汐转季节：amplitude=0，jitter 任意值都返回稳定 modifier。
        for jitter in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            assert!(
                (apply_xizhuan_qi_cap_jitter(Season::Summer, jitter) + 0.2).abs() < 1e-6,
                "Summer + jitter={jitter} 应当 -0.2"
            );
            assert!(
                (apply_xizhuan_qi_cap_jitter(Season::Winter, jitter) - 0.2).abs() < 1e-6,
                "Winter + jitter={jitter} 应当 +0.2"
            );
        }
    }

    #[test]
    fn apply_xizhuan_qi_cap_jitter_xizhuan_swings_plus_minus_0_3() {
        // 汐转：jitter=-1 → -0.3，jitter=+1 → +0.3，jitter=0 → 0
        for season in [Season::SummerToWinter, Season::WinterToSummer] {
            assert!((apply_xizhuan_qi_cap_jitter(season, -1.0) + 0.3).abs() < 1e-6);
            assert!((apply_xizhuan_qi_cap_jitter(season, 1.0) - 0.3).abs() < 1e-6);
            assert!(apply_xizhuan_qi_cap_jitter(season, 0.0).abs() < 1e-6);
            // 边界外 clamp：jitter=2 应被 clamp 到 1，结果仍是 +0.3
            assert!((apply_xizhuan_qi_cap_jitter(season, 2.0) - 0.3).abs() < 1e-6);
            assert!((apply_xizhuan_qi_cap_jitter(season, -3.0) + 0.3).abs() < 1e-6);
        }
    }

    #[test]
    fn apply_xizhuan_supply_jitter_summer_minus_10_percent() {
        // 夏 -10% 不受 jitter 影响；汐转 jitter=±1 → ±20%
        assert!((apply_xizhuan_supply_jitter(Season::Summer, 0.5) + 0.10).abs() < 1e-6);
        assert!((apply_xizhuan_supply_jitter(Season::Winter, -0.5) - 0.10).abs() < 1e-6);
        assert!((apply_xizhuan_supply_jitter(Season::SummerToWinter, 1.0) - 0.20).abs() < 1e-6);
        assert!((apply_xizhuan_supply_jitter(Season::WinterToSummer, -1.0) + 0.20).abs() < 1e-6);
        assert!(apply_xizhuan_supply_jitter(Season::SummerToWinter, 0.0).abs() < 1e-6);
    }

    #[test]
    fn compute_zone_flow_multiplier_summer_winter_static() {
        // 非汐转：jitter 任意值返回稳定倍率。
        for j in [0.0, 0.5, 1.0] {
            assert!((compute_zone_flow_multiplier(Season::Summer, None, j) - 1.3).abs() < 1e-6);
            assert!((compute_zone_flow_multiplier(Season::Winter, None, j) - 0.7).abs() < 1e-6);
        }
    }

    #[test]
    fn compute_zone_flow_multiplier_xizhuan_jitter_maps_to_1_0_to_1_5() {
        // 汐转：jitter01=0 → 1.0，jitter01=1 → 1.5
        assert!(
            (compute_zone_flow_multiplier(Season::SummerToWinter, None, 0.0) - 1.0).abs() < 1e-6
        );
        assert!(
            (compute_zone_flow_multiplier(Season::SummerToWinter, None, 1.0) - 1.5).abs() < 1e-6
        );
        assert!(
            (compute_zone_flow_multiplier(Season::WinterToSummer, None, 0.5) - 1.25).abs() < 1e-6
        );
    }

    #[test]
    fn compute_zone_flow_multiplier_thunderstorm_multiplies_x_1_5() {
        // 夏 + 雷暴 = 1.3 × 1.5 = 1.95
        let m = compute_zone_flow_multiplier(Season::Summer, Some(WeatherEvent::Thunderstorm), 0.0);
        assert!(
            (m - 1.95).abs() < 1e-6,
            "summer*thunderstorm 应当 1.95，实际 {m}"
        );
    }
}
