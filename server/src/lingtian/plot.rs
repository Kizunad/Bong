//! plan-lingtian-v1 §1.1 — 田块模型 (`LingtianPlot`)。
//!
//! 田块是世界中的方块挂载组件（v1 暂以独立 Entity 形式，后续 BlockEntity
//! 持久化在 plan-persistence-v1 接管）。本切片只提供组件 + 方法，**不含**
//! tick / system / 网络 IPC。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, BlockPos, Component, Entity};

use crate::botany::PlantId;

/// 单个田块（Plot）。
///
/// `plot_qi` 为田块独立灵气池，与所在 zone 的 `spirit_qi` 双向流动
/// （plan §1.3 / §1.4）。
#[derive(Debug, Clone, Component)]
pub struct LingtianPlot {
    pub pos: BlockPos,
    pub zone: String,
    pub owner: Option<Entity>,
    pub crop: Option<CropInstance>,
    pub plot_qi: f32,
    pub plot_qi_cap: f32,
    pub harvest_count: u32,
    /// plan §1.4 补灵冷却（72-168h）的基准时刻，单位 server tick。
    pub last_replenish_at: u64,
    /// plan-alchemy-recycle-v1 §3：plot 级杂染，范围 [0, 1]。
    pub dye_contamination: f32,
}

/// 田块基线 cap（plan §1.1 — 1.0 / 水源 +0.3 / 湿地 +0.5 / 聚灵阵 +1.0，封顶 3.0）。
pub const PLOT_QI_CAP_BASE: f32 = 1.0;
pub const PLOT_QI_CAP_MAX: f32 = 3.0;

/// plan §1.6：累计 N_RENEW 次收获后进入"贫瘠"，必须翻新。
pub const N_RENEW: u32 = 5;
pub const DYE_CONTAMINATION_WARNING_THRESHOLD: f32 = 0.3;
pub const DYE_CONTAMINATION_DECAY_PER_LINGTIAN_TICK: f32 = 0.001;
pub const DYE_CONTAMINATION_QUALITY_PENALTY_MAX: f32 = 0.3;

impl LingtianPlot {
    pub fn new(pos: BlockPos, owner: Option<Entity>) -> Self {
        Self {
            pos,
            zone: String::new(),
            owner,
            crop: None,
            plot_qi: 0.0,
            plot_qi_cap: PLOT_QI_CAP_BASE,
            harvest_count: 0,
            last_replenish_at: 0,
            dye_contamination: 0.0,
        }
    }

    pub fn with_zone(mut self, zone: impl Into<String>) -> Self {
        self.zone = zone.into();
        self
    }

    pub fn is_empty(&self) -> bool {
        self.crop.is_none()
    }

    /// plan §1.6：贫瘠 = 收获满 N_RENEW 次但未翻新。
    pub fn is_barren(&self) -> bool {
        self.harvest_count >= N_RENEW
    }

    /// 翻新：清空 crop / plot_qi / harvest_count，但保留位置与 cap。
    pub fn renew(&mut self) {
        self.crop = None;
        self.plot_qi = 0.0;
        self.harvest_count = 0;
        self.dye_contamination = 0.0;
    }

    /// 灵气注入，封顶 cap，溢出量返回（用于 plan §1.4 回馈环境）。
    pub fn deposit_qi(&mut self, amount: f32) -> f32 {
        debug_assert!(amount >= 0.0);
        let new_qi = self.plot_qi + amount;
        if new_qi <= self.plot_qi_cap {
            self.plot_qi = new_qi;
            0.0
        } else {
            let overflow = new_qi - self.plot_qi_cap;
            self.plot_qi = self.plot_qi_cap;
            overflow
        }
    }

    pub fn add_dye_contamination(&mut self, amount: f32) -> f32 {
        let before = self.dye_contamination;
        self.dye_contamination = (self.dye_contamination + amount.max(0.0)).clamp(0.0, 1.0);
        self.dye_contamination - before
    }

    pub fn decay_dye_contamination(&mut self) {
        self.dye_contamination =
            (self.dye_contamination - DYE_CONTAMINATION_DECAY_PER_LINGTIAN_TICK).max(0.0);
    }

    pub fn contamination_quality_multiplier(&self) -> f32 {
        1.0 - self.dye_contamination.clamp(0.0, 1.0) * DYE_CONTAMINATION_QUALITY_PENALTY_MAX
    }

    pub fn has_dye_contamination_warning(&self) -> bool {
        self.dye_contamination >= DYE_CONTAMINATION_WARNING_THRESHOLD
    }
}

/// 当前作物实例 — 与 `PlantKind` 共用 `PlantId`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropInstance {
    pub kind: PlantId,
    /// `[0, 1]`，>= 1 即熟。
    pub growth: f32,
    /// 生长过程累积品质修饰（plan §1.3 quality_multiplier 累加）。
    pub quality_accum: f32,
}

impl CropInstance {
    pub fn new(kind: PlantId) -> Self {
        Self {
            kind,
            growth: 0.0,
            quality_accum: 0.0,
        }
    }

    pub fn is_ripe(&self) -> bool {
        self.growth >= 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_pos() -> BlockPos {
        BlockPos::new(0, 64, 0)
    }

    #[test]
    fn deposit_caps_and_returns_overflow() {
        let mut plot = LingtianPlot::new(dummy_pos(), None);
        assert_eq!(plot.deposit_qi(0.5), 0.0);
        assert_eq!(plot.plot_qi, 0.5);
        // 灌满 + 溢出
        let overflow = plot.deposit_qi(2.0);
        assert!((plot.plot_qi - PLOT_QI_CAP_BASE).abs() < 1e-6);
        assert!((overflow - 1.5).abs() < 1e-6);
    }

    #[test]
    fn barren_after_n_renew_harvests() {
        let mut plot = LingtianPlot::new(dummy_pos(), None);
        assert!(!plot.is_barren());
        plot.harvest_count = N_RENEW;
        assert!(plot.is_barren());
        plot.renew();
        assert!(!plot.is_barren());
        assert_eq!(plot.harvest_count, 0);
        assert_eq!(plot.dye_contamination, 0.0);
    }

    #[test]
    fn ripe_at_growth_one() {
        let mut crop = CropInstance::new("ci_she_hao".into());
        assert!(!crop.is_ripe());
        crop.growth = 1.0;
        assert!(crop.is_ripe());
    }

    #[test]
    fn dye_contamination_clamps_decays_and_flags_warning() {
        let mut plot = LingtianPlot::new(dummy_pos(), None);
        assert_eq!(plot.add_dye_contamination(0.31), 0.31);
        assert!(plot.has_dye_contamination_warning());
        assert!((plot.contamination_quality_multiplier() - 0.907).abs() < 1e-6);
        plot.decay_dye_contamination();
        assert!((plot.dye_contamination - 0.309).abs() < 1e-6);
        plot.add_dye_contamination(2.0);
        assert_eq!(plot.dye_contamination, 1.0);
    }
}
