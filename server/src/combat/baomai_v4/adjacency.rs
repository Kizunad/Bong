//! plan-baomai-v4 §1.2 — 经脉邻接常量图。
//!
//! 定义经脉间的物理邻接关系（基于 worldview §四:293 正经按肢分布 + 奇经连通）。
//! 仅同肢经脉互为邻接（简化模型，不走 TCM 全拓扑）。
//! P1 scar_circuit 系统直接引用本模块，P4/P5 裂读/互噬也依赖。
#![allow(dead_code)]

use crate::cultivation::components::MeridianId;

/// 经脉邻接对。仅同肢经脉互为邻接（简化模型）。
pub const MERIDIAN_ADJACENCY: &[(MeridianId, MeridianId)] = &[
    // 左臂·手三阴（气）
    (MeridianId::Lung, MeridianId::Heart),
    (MeridianId::Heart, MeridianId::Pericardium),
    (MeridianId::Lung, MeridianId::Pericardium),
    // 右臂·手三阳（力）
    (MeridianId::LargeIntestine, MeridianId::SmallIntestine),
    (MeridianId::SmallIntestine, MeridianId::TripleEnergizer),
    (MeridianId::LargeIntestine, MeridianId::TripleEnergizer),
    // 左腿·足三阴（韧）
    (MeridianId::Spleen, MeridianId::Kidney),
    (MeridianId::Kidney, MeridianId::Liver),
    (MeridianId::Spleen, MeridianId::Liver),
    // 右腿·足三阳（速）
    (MeridianId::Stomach, MeridianId::Bladder),
    (MeridianId::Bladder, MeridianId::Gallbladder),
    (MeridianId::Stomach, MeridianId::Gallbladder),
    // 任督桥（躯干纵轴，连通上下）
    (MeridianId::Ren, MeridianId::Du),
];

/// 判断两条经脉是否物理邻接。对称——are_adjacent(a, b) == are_adjacent(b, a)。
pub fn are_adjacent(a: MeridianId, b: MeridianId) -> bool {
    if a == b {
        return false;
    }
    MERIDIAN_ADJACENCY
        .iter()
        .any(|(x, y)| (*x == a && *y == b) || (*x == b && *y == a))
}

/// 返回与 `id` 邻接的所有经脉。
pub fn adjacent_pairs_for(id: MeridianId) -> Vec<MeridianId> {
    MERIDIAN_ADJACENCY
        .iter()
        .filter_map(|(a, b)| {
            if *a == id {
                Some(*b)
            } else if *b == id {
                Some(*a)
            } else {
                None
            }
        })
        .collect()
}
