//! 丹道流派 Component —— DandaoStyle。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component};

/// 变异阶段阈值（cumulative_toxin 同 toxin_amount 量级 0.x）。
/// 基线：普通���药 toxin_amount 中位数 ~0.5/颗。正常修士终身约 100-150 颗（累计 ~50-75），
/// 永远不会触发微变线。只有刻意大量服药（2 倍以上）的丹道修士才进入变异轨道。
pub const MUTATION_STAGE_THRESHOLDS: [f64; 4] = [30.0, 100.0, 250.0, 500.0];

/// 丹道修习记��——首次炼丹/服药时 lazy insert。
#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DandaoStyle {
    /// 累计成功炼丹次数
    pub brew_count: u32,
    /// 累计服药次数
    pub pill_intake_count: u32,
    /// 历史累计丹毒总量（只增不减——排毒不降此值）
    pub cumulative_toxin: f64,
    /// 当前变异阶段 (0-4)
    pub mutation_stage: u8,
    /// 丹道熟练度（累计操作 tick）
    pub mastery_ticks: u64,
}

impl Default for DandaoStyle {
    fn default() -> Self {
        Self {
            brew_count: 0,
            pill_intake_count: 0,
            cumulative_toxin: 0.0,
            mutation_stage: 0,
            mastery_ticks: 0,
        }
    }
}

impl DandaoStyle {
    /// 推进累计丹毒并检查是否需要变异阶段提升。
    /// 返回 `Some(new_stage)` 表示刚刚跨越阈值。
    pub fn advance_toxin(&mut self, toxin_amount: f64) -> Option<u8> {
        let old_stage = self.mutation_stage;
        self.cumulative_toxin += toxin_amount;
        self.pill_intake_count += 1;

        let new_stage = Self::stage_for_toxin(self.cumulative_toxin);
        if new_stage > old_stage {
            self.mutation_stage = new_stage;
            Some(new_stage)
        } else {
            None
        }
    }

    /// 纯函数：给定累计丹毒值��计算应属阶段。
    pub fn stage_for_toxin(cumulative: f64) -> u8 {
        if cumulative >= MUTATION_STAGE_THRESHOLDS[3] {
            4
        } else if cumulative >= MUTATION_STAGE_THRESHOLDS[2] {
            3
        } else if cumulative >= MUTATION_STAGE_THRESHOLDS[1] {
            2
        } else if cumulative >= MUTATION_STAGE_THRESHOLDS[0] {
            1
        } else {
            0
        }
    }

    /// 记录一次成功炼丹。
    pub fn record_brew(&mut self) {
        self.brew_count += 1;
    }
}

/// 变异阶段枚举（用于匹配和序列化场景）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationStage {
    None = 0,
    Subtle = 1,
    Visible = 2,
    Heavy = 3,
    Bestial = 4,
}

impl From<u8> for MutationStage {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Subtle,
            2 => Self::Visible,
            3 => Self::Heavy,
            4 => Self::Bestial,
            _ => Self::Bestial,
        }
    }
}
