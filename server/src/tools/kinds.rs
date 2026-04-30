use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolKind {
    CaiYaoDao,
    BaoChu,
    CaoLian,
    DunQiJia,
    GuaDao,
    GuHaiQian,
    BingJiaShouTao,
}

impl ToolKind {
    pub fn item_id(self) -> &'static str {
        match self {
            Self::CaiYaoDao => "cai_yao_dao",
            Self::BaoChu => "bao_chu",
            Self::CaoLian => "cao_lian",
            Self::DunQiJia => "dun_qi_jia",
            Self::GuaDao => "gua_dao",
            Self::GuHaiQian => "gu_hai_qian",
            Self::BingJiaShouTao => "bing_jia_shou_tao",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::CaiYaoDao => "采药刀",
            Self::BaoChu => "刨锄",
            Self::CaoLian => "草镰",
            Self::DunQiJia => "钝气夹",
            Self::GuaDao => "刮刀",
            Self::GuHaiQian => "骨骸钳",
            Self::BingJiaShouTao => "冰甲手套",
        }
    }

    /// 凡器可临时用于战斗，但只是凡铁/木石档工具，倍率低于入门铁剑(1.2x)。
    pub fn combat_damage_multiplier(self) -> f32 {
        match self {
            Self::CaiYaoDao => 1.08,
            Self::BaoChu => 1.07,
            Self::CaoLian => 1.10,
            Self::DunQiJia => 1.02,
            Self::GuaDao => 1.06,
            Self::GuHaiQian => 1.05,
            Self::BingJiaShouTao => 1.03,
        }
    }

    /// 凡器统一按 100 次基础使用折算为 normalized durability（100 bp = 1%）。
    pub fn durability_cost_basis_points_per_use(self) -> u16 {
        match self {
            Self::CaiYaoDao
            | Self::BaoChu
            | Self::CaoLian
            | Self::DunQiJia
            | Self::GuaDao
            | Self::GuHaiQian
            | Self::BingJiaShouTao => 100,
        }
    }

    pub fn durability_cost_ratio_per_use(self) -> f64 {
        f64::from(self.durability_cost_basis_points_per_use()) / 10_000.0
    }
}

pub const ALL_TOOL_KINDS: [ToolKind; 7] = [
    ToolKind::CaiYaoDao,
    ToolKind::BaoChu,
    ToolKind::CaoLian,
    ToolKind::DunQiJia,
    ToolKind::GuaDao,
    ToolKind::GuHaiQian,
    ToolKind::BingJiaShouTao,
];
