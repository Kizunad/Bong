//! NPC 空间索引。
//!
//! 本模块只索引 `NpcMarker` 实体。玩家 / 灵田 / ore 等非 NPC 查询暂不纳入，
//! 避免把本 plan 的性能修复扩成通用空间库。

use std::collections::HashMap;

use big_brain::prelude::BigBrainSet;
use valence::prelude::{
    bevy_ecs, App, DVec3, Despawned, Entity, IntoSystemConfigs, Position, PreUpdate, Query, ResMut,
    Resource, With, Without,
};

use crate::npc::spawn::NpcMarker;

#[derive(Clone, Debug, Resource)]
pub struct NpcSpatialIndex {
    cell_size: f64,
    cells: HashMap<(i32, i32), Vec<Entity>>,
    positions: HashMap<Entity, DVec3>,
}

impl Default for NpcSpatialIndex {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CELL_SIZE)
    }
}

impl NpcSpatialIndex {
    pub const DEFAULT_CELL_SIZE: f64 = 32.0;

    pub fn new(cell_size: f64) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
            positions: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn rebuild_from_iter(&mut self, npcs: impl IntoIterator<Item = (Entity, DVec3)>) {
        self.cells.clear();
        self.positions.clear();

        for (entity, position) in npcs {
            let key = self.cell_key(position);
            self.cells.entry(key).or_default().push(entity);
            self.positions.insert(entity, position);
        }
    }

    #[allow(dead_code)]
    pub fn position_of(&self, entity: Entity) -> Option<DVec3> {
        self.positions.get(&entity).copied()
    }

    pub fn neighbors_within(&self, center: DVec3, radius: f64) -> Vec<Entity> {
        if self.positions.is_empty() {
            return Vec::new();
        }

        let radius = radius.max(0.0);
        let radius_sq = radius * radius;
        let min = self.cell_key(DVec3::new(center.x - radius, center.y, center.z - radius));
        let max = self.cell_key(DVec3::new(center.x + radius, center.y, center.z + radius));
        let mut out = Vec::new();

        for cx in min.0..=max.0 {
            for cz in min.1..=max.1 {
                let Some(bucket) = self.cells.get(&(cx, cz)) else {
                    continue;
                };
                for &entity in bucket {
                    if self
                        .positions
                        .get(&entity)
                        .is_some_and(|pos| planar_distance_sq(*pos, center) <= radius_sq)
                    {
                        out.push(entity);
                    }
                }
            }
        }

        out
    }

    fn cell_key(&self, position: DVec3) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.z / self.cell_size).floor() as i32,
        )
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(NpcSpatialIndex::default()).add_systems(
        PreUpdate,
        rebuild_npc_spatial_index_system.before(BigBrainSet::Scorers),
    );
}

#[allow(clippy::type_complexity)]
pub fn rebuild_npc_spatial_index_system(
    mut index: ResMut<NpcSpatialIndex>,
    npcs: Query<(Entity, &Position), (With<NpcMarker>, Without<Despawned>)>,
) {
    index.rebuild_from_iter(npcs.iter().map(|(entity, pos)| (entity, pos.get())));
}

fn planar_distance_sq(left: DVec3, right: DVec3) -> f64 {
    let dx = left.x - right.x;
    let dz = left.z - right.z;
    dx * dx + dz * dz
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw(index)
    }

    fn set(values: Vec<Entity>) -> HashSet<Entity> {
        values.into_iter().collect()
    }

    #[test]
    fn rebuild_empty_clears_previous_cells() {
        let mut index = NpcSpatialIndex::default();
        index.rebuild_from_iter([(entity(1), DVec3::new(1.0, 64.0, 1.0))]);
        assert_eq!(index.len(), 1);

        index.rebuild_from_iter([]);

        assert!(index.is_empty());
        assert!(
            index
                .neighbors_within(DVec3::new(1.0, 64.0, 1.0), 8.0)
                .is_empty(),
            "empty rebuild must leave no stale neighbors"
        );
    }

    #[test]
    fn single_entity_can_be_queried_by_radius() {
        let mut index = NpcSpatialIndex::default();
        let npc = entity(1);
        index.rebuild_from_iter([(npc, DVec3::new(4.0, 64.0, 4.0))]);

        assert_eq!(
            index.neighbors_within(DVec3::new(0.0, 64.0, 0.0), 6.0),
            vec![npc],
            "NPC inside planar radius should be returned"
        );
        assert!(
            index
                .neighbors_within(DVec3::new(0.0, 64.0, 0.0), 5.0)
                .is_empty(),
            "NPC outside exact planar radius must be filtered after cell lookup"
        );
    }

    #[test]
    fn query_crosses_positive_cell_boundary() {
        let mut index = NpcSpatialIndex::new(32.0);
        let left = entity(1);
        let right = entity(2);
        index.rebuild_from_iter([
            (left, DVec3::new(31.0, 64.0, 0.0)),
            (right, DVec3::new(33.0, 64.0, 0.0)),
        ]);

        assert_eq!(
            set(index.neighbors_within(DVec3::new(32.0, 64.0, 0.0), 2.0)),
            set(vec![left, right]),
            "radius query must include both sides of a cell boundary"
        );
    }

    #[test]
    fn query_crosses_negative_cell_boundary() {
        let mut index = NpcSpatialIndex::new(32.0);
        let left = entity(1);
        let right = entity(2);
        index.rebuild_from_iter([
            (left, DVec3::new(-33.0, 64.0, 0.0)),
            (right, DVec3::new(-31.0, 64.0, 0.0)),
        ]);

        assert_eq!(
            set(index.neighbors_within(DVec3::new(-32.0, 64.0, 0.0), 2.0)),
            set(vec![left, right]),
            "floor-based negative cells must still be queried on both sides"
        );
    }

    #[test]
    fn query_uses_xz_distance_only() {
        let mut index = NpcSpatialIndex::default();
        let npc = entity(1);
        index.rebuild_from_iter([(npc, DVec3::new(3.0, 180.0, 4.0))]);

        assert_eq!(
            index.neighbors_within(DVec3::new(0.0, 64.0, 0.0), 5.0),
            vec![npc],
            "NPC neighbor checks intentionally ignore Y, matching existing hotspot behavior"
        );
    }

    #[test]
    fn query_does_not_duplicate_entities_across_cells() {
        let mut index = NpcSpatialIndex::new(8.0);
        let npc = entity(1);
        index.rebuild_from_iter([(npc, DVec3::new(8.0, 64.0, 8.0))]);

        assert_eq!(
            index.neighbors_within(DVec3::new(8.0, 64.0, 8.0), 20.0),
            vec![npc],
            "each NPC lives in one bucket, so wide queries must not duplicate it"
        );
    }

    #[test]
    fn position_lookup_tracks_latest_rebuild() {
        let mut index = NpcSpatialIndex::default();
        let npc = entity(1);
        index.rebuild_from_iter([(npc, DVec3::new(1.0, 64.0, 1.0))]);
        index.rebuild_from_iter([(npc, DVec3::new(9.0, 64.0, 9.0))]);

        assert_eq!(
            index.position_of(npc),
            Some(DVec3::new(9.0, 64.0, 9.0)),
            "rebuild must replace old positions instead of appending stale data"
        );
    }
}
