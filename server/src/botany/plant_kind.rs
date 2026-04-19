//! plan-botany-v1 §1 — `PlantKind` 元定义。
//!
//! 共享给 `lingtian`（人工种植）+ 未来野生采集流。`cultivable` flag 区分
//! 可种 vs 野生 only（plan-lingtian-v1 §1.2.4 / §2 表）。

use serde::Deserialize;

pub type PlantId = String;

/// plan-lingtian-v1 §1.3：plot_qi 消耗档位。
///
/// 与 §2 表"plot_qi 消耗"列对应（低/中/高/低中/中高），转成数值倍率。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowthCost {
    Low,
    LowMid,
    Mid,
    MidHigh,
    High,
}

impl GrowthCost {
    /// 每 lingtian-tick 的 base_drain 系数（plan-lingtian-v1 §1.3）。
    pub fn drain_per_tick(self) -> f32 {
        match self {
            GrowthCost::Low => 0.002,
            GrowthCost::LowMid => 0.004,
            GrowthCost::Mid => 0.006,
            GrowthCost::MidHigh => 0.009,
            GrowthCost::High => 0.012,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlantRarity {
    Common,
    Region,
    Rare,
}

impl PlantRarity {
    /// 收获时种子掉率（plan-lingtian-v1 §1.2.4）。
    pub fn seed_drop_rate(self) -> f32 {
        match self {
            PlantRarity::Common => 0.30,
            PlantRarity::Region => 0.20,
            PlantRarity::Rare => 0.10,
        }
    }
}

/// 单个植物物种。覆盖野生 + 灵田两路。
#[derive(Debug, Clone, Deserialize)]
pub struct PlantKind {
    pub id: PlantId,
    pub display_name: String,
    /// `true` → 可入 lingtian（生成种子 item）；`false` → 野生 only。
    /// plan-lingtian-v1 §2 表"野生 only"清单。
    pub cultivable: bool,
    pub growth_cost: GrowthCost,
    /// 在 lingtian 下成熟所需 lingtian-tick 数。`LingtianTick` 周期 1 min（plan §4），
    /// 故 8h = 480 ticks、16h = 960 ticks、480h = 28800 ticks。
    pub growth_duration_ticks: u32,
    pub rarity: PlantRarity,
    #[serde(default)]
    pub description: String,
}
