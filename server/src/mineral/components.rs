//! plan-mineral-v1 §7 — `MineralOreNode` 组件 + `MineralOreIndex` 资源。
//!
//! 矿脉单方块（worldgen 写入）：每方块 = 一个 `MineralOreNode` 实体，挂当前
//! 方块位置 + mineral_id + 剩余储量。`MineralOreIndex` 是 (DimensionKind, BlockPos)→Entity 的
//! 反查表，让 `BlockBreakEvent` 监听器 O(1) 拿到矿脉数据。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, BlockPos, Component, Resource};

use super::types::MineralId;
use crate::world::dimension::DimensionKind;

/// 单方块矿脉节点 — 挂在 worldgen 创建的 ore-block 实体上。
///
/// `remaining_units` 当前为 1（每方块独立 entity）。多方块矿脉 = 多个
/// `MineralOreNode` entity；plan §2.1 矿脉有限性的"耗尽"语义 = 把 entity 移除
/// + 标记 `MineralOreIndex` 移除该 BlockPos。
#[derive(Debug, Clone, Component)]
pub struct MineralOreNode {
    pub mineral_id: MineralId,
    pub position: BlockPos,
    pub remaining_units: u32,
}

impl MineralOreNode {
    pub fn new(mineral_id: MineralId, position: BlockPos) -> Self {
        Self {
            mineral_id,
            position,
            remaining_units: 1,
        }
    }

    pub fn with_units(mineral_id: MineralId, position: BlockPos, units: u32) -> Self {
        Self {
            mineral_id,
            position,
            remaining_units: units,
        }
    }
}

/// (DimensionKind, BlockPos) → Entity 反查表 — `BlockBreakEvent` listener 用。
///
/// 由 worldgen spawn 矿脉时同步插入；耗尽 / despawn 时同步移除。
#[derive(Debug, Default, Resource)]
pub struct MineralOreIndex {
    by_pos: HashMap<(DimensionKind, BlockPos), valence::prelude::Entity>,
}

impl MineralOreIndex {
    pub fn insert(
        &mut self,
        dimension: DimensionKind,
        pos: BlockPos,
        entity: valence::prelude::Entity,
    ) {
        self.by_pos.insert((dimension, pos), entity);
    }

    pub fn lookup(
        &self,
        dimension: DimensionKind,
        pos: BlockPos,
    ) -> Option<valence::prelude::Entity> {
        self.by_pos.get(&(dimension, pos)).copied()
    }

    pub fn remove(
        &mut self,
        dimension: DimensionKind,
        pos: BlockPos,
    ) -> Option<valence::prelude::Entity> {
        self.by_pos.remove(&(dimension, pos))
    }

    pub fn len(&self) -> usize {
        self.by_pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_pos.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (DimensionKind, BlockPos, valence::prelude::Entity)> + '_ {
        self.by_pos
            .iter()
            .map(|((dimension, pos), ent)| (*dimension, *pos, *ent))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Entity};

    #[test]
    fn ore_node_default_unit_is_one() {
        let node = MineralOreNode::new(MineralId::FanTie, BlockPos::new(0, 64, 0));
        assert_eq!(node.remaining_units, 1);
        assert_eq!(node.mineral_id, MineralId::FanTie);
    }

    #[test]
    fn ore_node_with_explicit_units_keeps_value() {
        let node = MineralOreNode::with_units(MineralId::SuiTie, BlockPos::new(8, 32, 8), 5);
        assert_eq!(node.remaining_units, 5);
    }

    #[test]
    fn index_insert_and_lookup() {
        // Use a fresh app to allocate real entities (Entity::PLACEHOLDER may collide).
        let mut app = App::new();
        let e1: Entity = app.world_mut().spawn_empty().id();
        let e2: Entity = app.world_mut().spawn_empty().id();

        let mut idx = MineralOreIndex::default();
        let p1 = BlockPos::new(0, 64, 0);
        let p2 = BlockPos::new(1, 64, 0);
        idx.insert(DimensionKind::Overworld, p1, e1);
        idx.insert(DimensionKind::Tsy, p1, e2);
        idx.insert(DimensionKind::Overworld, p2, e2);

        assert_eq!(idx.lookup(DimensionKind::Overworld, p1), Some(e1));
        assert_eq!(idx.lookup(DimensionKind::Tsy, p1), Some(e2));
        assert_eq!(idx.lookup(DimensionKind::Overworld, p2), Some(e2));
        assert_eq!(
            idx.lookup(DimensionKind::Overworld, BlockPos::new(99, 99, 99)),
            None
        );
        assert_eq!(idx.len(), 3);
    }

    #[test]
    fn index_remove_returns_entity() {
        let mut app = App::new();
        let e: Entity = app.world_mut().spawn_empty().id();

        let mut idx = MineralOreIndex::default();
        let p = BlockPos::new(0, 64, 0);
        idx.insert(DimensionKind::Overworld, p, e);

        assert_eq!(idx.remove(DimensionKind::Overworld, p), Some(e));
        assert!(idx.is_empty());
        assert_eq!(idx.remove(DimensionKind::Overworld, p), None);
    }
}
