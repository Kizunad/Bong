//! plan-tsy-zone-v1 §1.3 / §3.2 — 活坍缩渊（TSY）的玩家 presence + 裂缝 POI 数据结构。
//!
//! 仅类型/component 定义；drain tick / portal system / entry filter 在各自模块中实现。

use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, Component, DVec3};

use super::dimension::DimensionKind;

/// 跨位面锚点（dim + 位面内部坐标）。
///
/// `plan-tsy-zone-v1 §1.3` / `plan-tsy-dimension-v1 §3` — 出关 / 入场跨位面传送
/// 的目标全部以本类型表示，避免 (DimensionKind, DVec3) 二元组到处分裂。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DimensionAnchor {
    pub dimension: DimensionKind,
    /// 序列化为 `[f64; 3]`，与 schema/src/tsy.ts 的 wire 形态对齐。
    #[serde(with = "dvec3_array_serde")]
    pub pos: DVec3,
}

mod dvec3_array_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use valence::prelude::DVec3;

    pub fn serialize<S: Serializer>(value: &DVec3, ser: S) -> Result<S::Ok, S::Error> {
        [value.x, value.y, value.z].serialize(ser)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<DVec3, D::Error> {
        let arr: [f64; 3] = Deserialize::deserialize(de)?;
        Ok(DVec3::new(arr[0], arr[1], arr[2]))
    }
}

/// 玩家在 TSY 内的 ECS 状态。
///
/// 生命周期：
/// - 玩家踏进 Entry 裂缝 → `tsy_entry_portal_system` `commands.insert` 本组件
/// - 玩家踏进 Exit 裂缝 / 在 TSY 内死亡 → `commands.remove` 本组件
/// - 死亡结算（`plan-tsy-loot-v1` P1）会读 `entry_inventory_snapshot` 区分
///   "秘境所得" vs "原带物"
#[allow(dead_code)] // 字段全部由 portal / loot / lifecycle plan 消费；P0 仅落定义。
#[derive(Component, Debug, Clone)]
pub struct TsyPresence {
    /// 玩家所在的 TSY 系列 id（如 `"tsy_lingxu_01"`）—— 由 `Zone::tsy_family_id` 派生。
    pub family_id: String,
    /// 进入 tick（用于计算 duration、drain total 等）。
    pub entered_at_tick: u64,
    /// 入场时 inventory 内所有 instance_id；`plan-tsy-loot-v1` 死亡结算用此区分秘境所得。
    pub entry_inventory_snapshot: Vec<u64>,
    /// 出关锚点：传回到哪个位面 + 哪个坐标。
    /// 通常 = `(Overworld, 触发 Entry 裂缝的主世界坐标 + (0,1,0))`。
    /// 塌缩时若主世界锚点失效 → P2 lifecycle 决定 fallback（出生点 / 灵龛）。
    pub return_to: DimensionAnchor,
}

/// 裂缝 POI 朝向。Entry = 主世界 → TSY；Exit = TSY → 主世界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalDirection {
    Entry,
    Exit,
}

/// 裂缝 POI Component。
///
/// 实体附着方式：worldgen blueprint 或 `!tsy-spawn` 调试命令在 layer 上摆好 portal
/// 方块（vanilla nether_portal / end_portal 皮肤），中心位置 spawn 一个不可见的
/// marker entity（armor stand），挂 `Position` + 本组件。玩家靠近 → AABB 命中
/// marker → portal system 发 `DimensionTransferRequest`。
#[allow(dead_code)] // 字段由 tsy_portal.rs 消费；P0 单独 commit 落定义。
#[derive(Component, Debug, Clone)]
pub struct RiftPortal {
    /// 对应 TSY family id（如 `"tsy_lingxu_01"`）。Entry / Exit 共享 family 串联。
    pub family_id: String,
    /// 跨位面传送目标。
    pub target: DimensionAnchor,
    /// 激活半径（玩家距 marker 中心 ≤ 此值即触发）。MVP 默认 1.5。
    pub trigger_radius: f64,
    /// 主世界 → TSY，还是 TSY → 主世界。
    pub direction: PortalDirection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_anchor_round_trips_through_serde_json() {
        let anchor = DimensionAnchor {
            dimension: DimensionKind::Tsy,
            pos: DVec3::new(1.5, 80.0, -3.25),
        };
        let json = serde_json::to_string(&anchor).expect("serialize");
        // pos 必须是 [x, y, z] 数组形态以匹配 wire schema。
        assert!(json.contains("[1.5,80.0,-3.25]"), "got: {json}");
        let parsed: DimensionAnchor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, anchor);
    }

    #[test]
    fn dimension_anchor_default_dimension_overworld_round_trip() {
        let anchor = DimensionAnchor {
            dimension: DimensionKind::Overworld,
            pos: DVec3::new(0.0, 64.0, 0.0),
        };
        let json = serde_json::to_string(&anchor).expect("serialize");
        let parsed: DimensionAnchor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.dimension, DimensionKind::Overworld);
        assert_eq!(parsed.pos, DVec3::new(0.0, 64.0, 0.0));
    }

    #[test]
    fn tsy_presence_default_field_shape() {
        let presence = TsyPresence {
            family_id: "tsy_lingxu_01".to_string(),
            entered_at_tick: 100,
            entry_inventory_snapshot: vec![1, 2, 3],
            return_to: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 65.0, 0.0),
            },
        };
        assert_eq!(presence.family_id, "tsy_lingxu_01");
        assert_eq!(presence.entered_at_tick, 100);
        assert_eq!(presence.entry_inventory_snapshot.len(), 3);
        assert_eq!(presence.return_to.dimension, DimensionKind::Overworld);
    }

    #[test]
    fn rift_portal_can_be_constructed_for_entry_and_exit() {
        let entry = RiftPortal {
            family_id: "tsy_lingxu_01".to_string(),
            target: DimensionAnchor {
                dimension: DimensionKind::Tsy,
                pos: DVec3::new(50.0, 80.0, 50.0),
            },
            trigger_radius: 1.5,
            direction: PortalDirection::Entry,
        };
        assert_eq!(entry.direction, PortalDirection::Entry);
        assert_eq!(entry.target.dimension, DimensionKind::Tsy);

        let exit = RiftPortal {
            family_id: "tsy_lingxu_01".to_string(),
            target: DimensionAnchor {
                dimension: DimensionKind::Overworld,
                pos: DVec3::new(0.0, 66.0, 0.0),
            },
            trigger_radius: 1.5,
            direction: PortalDirection::Exit,
        };
        assert_eq!(exit.direction, PortalDirection::Exit);
        assert_eq!(exit.target.dimension, DimensionKind::Overworld);
    }
}
