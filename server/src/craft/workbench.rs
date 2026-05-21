//! plan-workbench-recipes-v1 §P0.1 — 制作台方块定义 + 放置/交互/拆除系统。
//!
//! `WorkbenchBlock` 是 ECS entity 上的 component，代表世界中放置的制作台方块。
//! 玩家右键制作台 → Server 发 `WorkbenchOpenPayload` → Client 打开 WorkbenchScreen。
//!
//! **放置**：消耗背包中 `workbench_item` × 1 → spawn block + entity。
//! **拆除**：左键长按 3s → 回收 `workbench_item` × 1 → despawn。
//! **交互**：右键 → 发送 payload 给客户端。
//!
//! §8.1 #3 决议：不设 per-chunk 数量限制（制作台是凡物，材料成本已限制滥放）。

use valence::prelude::{bevy_ecs, Component, Entity};

/// 制作台方块 ECS component。
///
/// 放置时 spawn 到方块 entity 上，拆除时 despawn 整个 entity。
#[derive(Debug, Clone, Component, PartialEq)]
pub struct WorkbenchBlock {
    /// 放置该制作台的玩家 entity。
    pub placed_by: Entity,
    /// 放置时的 tick 时戳。
    pub placed_at_tick: u64,
}

/// 制作台交互 payload（server → client），通知客户端打开 WorkbenchScreen。
///
/// 当前仅携带制作台 entity 信息；后续 PR-3 client 端消费此 payload。
#[derive(Debug, Clone, PartialEq)]
pub struct WorkbenchOpenPayload {
    /// 制作台 entity。
    pub workbench_entity: Entity,
    /// 制作台世界坐标（方块坐标）。
    pub position: [i32; 3],
}

/// 制作台最大交互距离（方块数）。
///
/// §P2.4 & §8.1 #2：recipe.station == Some(Workbench) 时，玩家必须在此距离内。
pub const WORKBENCH_INTERACT_RANGE: f64 = 3.0;

/// 制作台物品 template_id。
pub const WORKBENCH_ITEM_TEMPLATE: &str = "workbench_item";

/// 检查玩家位置是否在制作台 3 格交互范围内。
///
/// 使用曼哈顿距离（与 MC 交互距离一致）。
pub fn is_within_workbench_range(player_pos: [f64; 3], workbench_pos: [i32; 3]) -> bool {
    let dx = (player_pos[0] - workbench_pos[0] as f64).abs();
    let dy = (player_pos[1] - workbench_pos[1] as f64).abs();
    let dz = (player_pos[2] - workbench_pos[2] as f64).abs();
    // 使用 Chebyshev 距离（MC 式方块交互范围）
    dx.max(dy).max(dz) <= WORKBENCH_INTERACT_RANGE
}

// ===== 系统占位（PR-2 范围内仅定义签名和数据结构，
// 完整 Bevy system 实现依赖 chunk / block 交互 API，属 PR-3/PR-4）=====

// NOTE: handle_workbench_place / handle_workbench_interact / handle_workbench_break
// 作为完整 Bevy system 需要访问 ChunkLayer / BlockState / Inventory 等实际运行时
// 资源，当前 PR-2 不引入运行时依赖。这些 system 的签名和行为在 plan §P0.1 / §P2.3
// 已定义，将在 PR-3 实装。本模块先暴露数据结构和纯函数供 session 校验使用。

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::App;

    fn test_entity() -> Entity {
        let mut app = App::new();
        app.world_mut().spawn_empty().id()
    }

    #[test]
    fn workbench_block_component_stores_placed_by_and_tick() {
        let entity = test_entity();
        let wb = WorkbenchBlock {
            placed_by: entity,
            placed_at_tick: 12345,
        };
        assert_eq!(wb.placed_by, entity);
        assert_eq!(wb.placed_at_tick, 12345);
    }

    #[test]
    fn workbench_open_payload_carries_entity_and_position() {
        let entity = test_entity();
        let payload = WorkbenchOpenPayload {
            workbench_entity: entity,
            position: [10, 64, -20],
        };
        assert_eq!(payload.workbench_entity, entity);
        assert_eq!(payload.position, [10, 64, -20]);
    }

    #[test]
    fn workbench_interact_range_is_three_blocks() {
        assert_eq!(
            WORKBENCH_INTERACT_RANGE, 3.0,
            "workbench interact range must be exactly 3.0 blocks per plan §P2.4"
        );
    }

    #[test]
    fn workbench_item_template_is_correct() {
        assert_eq!(
            WORKBENCH_ITEM_TEMPLATE, "workbench_item",
            "workbench item template ID must match server/assets/items/core.toml"
        );
    }

    // ============= is_within_workbench_range =============

    #[test]
    fn within_range_at_origin() {
        assert!(is_within_workbench_range([0.0, 0.0, 0.0], [0, 0, 0]));
    }

    #[test]
    fn within_range_at_boundary() {
        // Exactly 3 blocks away on x axis
        assert!(is_within_workbench_range([3.0, 0.0, 0.0], [0, 0, 0]));
        // Exactly 3 blocks away on all axes (Chebyshev = 3)
        assert!(is_within_workbench_range([3.0, 3.0, 3.0], [0, 0, 0]));
    }

    #[test]
    fn out_of_range_just_beyond_boundary() {
        assert!(
            !is_within_workbench_range([3.1, 0.0, 0.0], [0, 0, 0]),
            "3.1 blocks on x axis should be out of range"
        );
    }

    #[test]
    fn within_range_negative_coords() {
        assert!(is_within_workbench_range([-2.0, 5.0, -3.0], [-5, 5, -3]));
    }

    #[test]
    fn out_of_range_far_away() {
        assert!(!is_within_workbench_range([100.0, 64.0, 200.0], [0, 64, 0]));
    }

    #[test]
    fn within_range_fractional_player_pos() {
        // Player at 2.9 blocks from workbench at origin
        assert!(is_within_workbench_range([2.9, 0.5, 1.0], [0, 0, 0]));
    }
}
