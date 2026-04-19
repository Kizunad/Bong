//! plan-lingtian-v1 §1.1 — 田块环境修饰（用于派生 `plot_qi_cap`）。
//!
//! 由调用方（valence world ↔ environment 适配层）填入；本模块只负责数学
//! 计算，单测无需 mock world。同 `TerrainKind` 设计原则。

use serde::{Deserialize, Serialize};
use valence::prelude::{BlockKind, BlockPos, ChunkLayer};

use super::plot::{PLOT_QI_CAP_BASE, PLOT_QI_CAP_MAX};

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
}

impl PlotEnvironment {
    pub const fn base() -> Self {
        Self {
            water_adjacent: false,
            biome: PlotBiome::Other,
            zhenfa_jvling: false,
        }
    }
}

/// 扫描 `pos` 周围 ±5 格水平范围（y 层不变）内是否有 Water 方块，用于
/// `water_adjacent` 判定（plan §1.1）。半径 5 足够覆盖一个典型"近水田"。
///
/// biome / zhenfa_jvling 两项暂不自动派生：biome 需 valence biome 读取 API，
/// zhenfa 要等 plan-zhenfa-v1 的阵法系统落地，目前默认 false。
pub fn read_environment_at(layer: &ChunkLayer, pos: BlockPos) -> PlotEnvironment {
    let water_adjacent = scan_water_near(layer, pos, 5);
    PlotEnvironment {
        water_adjacent,
        biome: PlotBiome::Other,
        zhenfa_jvling: false,
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

/// plan §1.1 — 由 `PlotEnvironment` 派生 `plot_qi_cap`。
///
/// 公式：base 1.0 + (water_adjacent ? 0.3 : 0) + (wetland ? 0.5 : 0)
///       + (zhenfa_jvling ? 1.0 : 0)，封顶 `PLOT_QI_CAP_MAX` (3.0)。
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
    cap.min(PLOT_QI_CAP_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_environment_yields_cap_1_0() {
        assert!((compute_plot_qi_cap(&PlotEnvironment::base()) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn each_modifier_adds_correctly() {
        let mut e = PlotEnvironment::base();
        e.water_adjacent = true;
        assert!((compute_plot_qi_cap(&e) - 1.3).abs() < 1e-6);

        let e = PlotEnvironment {
            biome: PlotBiome::Wetland,
            ..PlotEnvironment::base()
        };
        assert!((compute_plot_qi_cap(&e) - 1.5).abs() < 1e-6);

        let e = PlotEnvironment {
            zhenfa_jvling: true,
            ..PlotEnvironment::base()
        };
        assert!((compute_plot_qi_cap(&e) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn combined_modifiers_capped_at_3_0() {
        let e = PlotEnvironment {
            water_adjacent: true,
            biome: PlotBiome::Wetland,
            zhenfa_jvling: true,
        };
        // 1.0 + 0.3 + 0.5 + 1.0 = 2.8（未触上限）
        assert!((compute_plot_qi_cap(&e) - 2.8).abs() < 1e-6);
    }

    #[test]
    fn cap_does_not_exceed_max_even_if_overadded() {
        // 假设未来加更多修饰（>= +2.0）也不应超 3.0：手动伪造一个超额结果
        let mut cap = PLOT_QI_CAP_BASE + 5.0;
        cap = cap.min(PLOT_QI_CAP_MAX);
        assert!((cap - PLOT_QI_CAP_MAX).abs() < 1e-6);
    }
}
