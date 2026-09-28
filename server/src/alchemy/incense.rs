//! 炼丹香料效果。
//!
//! 香料是炼丹 session 的服务端权威状态。客户端只提交背包物品实例，
//! 具体效果由这里按模板 ID 决定，避免客户端伪造加成。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncenseEffect {
    /// 火候容差倍率，越高越容易守住火候。
    pub temp_band_scale: f64,
    /// 真元消耗倍率。
    pub qi_cost_scale: f64,
    /// 成丹真元收益倍率。
    pub qi_gain_scale: f64,
    /// 可持续的炼丹刻数。
    pub duration_ticks: u32,
    /// 客户端烟气展示色，格式为 RGB hex。
    pub smoke_color: String,
}

/// 炉上当前香料。剩余刻数为 0 时香已燃尽，但仍保留种类供快照展示。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveIncense {
    pub kind: String,
    pub remaining_ticks: u32,
    pub effect: IncenseEffect,
}

/// 记录实际燃烧区间，结算时不能让新点的香追溯影响此前炉次。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncenseBurn {
    pub start_tick: u32,
    pub burned_ticks: u32,
    pub effect: IncenseEffect,
}

/// 从权威物品模板 ID 读取香料效果。
pub fn effect_for_item(template_id: &str) -> Option<IncenseEffect> {
    let effect = match template_id {
        "incense_plain" => IncenseEffect {
            temp_band_scale: 1.0,
            qi_cost_scale: 1.0,
            qi_gain_scale: 1.0,
            duration_ticks: 1200,
            smoke_color: "#B8B4A7".into(),
        },
        "incense_clear_mind" => IncenseEffect {
            temp_band_scale: 1.25,
            qi_cost_scale: 1.0,
            qi_gain_scale: 1.0,
            duration_ticks: 240,
            smoke_color: "#A8D7C5".into(),
        },
        "incense_warm_ash" => IncenseEffect {
            temp_band_scale: 0.9,
            qi_cost_scale: 0.85,
            qi_gain_scale: 1.1,
            duration_ticks: 200,
            smoke_color: "#E6A56A".into(),
        },
        "incense_damp_wood" => IncenseEffect {
            temp_band_scale: 1.0,
            qi_cost_scale: 1.15,
            qi_gain_scale: 0.8,
            duration_ticks: 320,
            smoke_color: "#8B9A78".into(),
        },
        _ => return None,
    };
    Some(effect)
}
