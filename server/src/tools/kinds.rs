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
