use valence::prelude::ChunkPos;

#[derive(Clone, Copy)]
pub(super) struct ChunkBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_z: i32,
    pub max_z: i32,
}

impl ChunkBounds {
    pub fn from_chunk_pos(pos: ChunkPos) -> Self {
        Self {
            min_x: pos.x * 16,
            max_x: pos.x * 16 + 15,
            min_z: pos.z * 16,
            max_z: pos.z * 16 + 15,
        }
    }

    pub fn contains(self, world_x: i32, world_z: i32) -> bool {
        (self.min_x..=self.max_x).contains(&world_x) && (self.min_z..=self.max_z).contains(&world_z)
    }

    pub fn contains_with_margin(self, world_x: i32, world_z: i32, margin: i32) -> bool {
        world_x >= self.min_x - margin
            && world_x <= self.max_x + margin
            && world_z >= self.min_z - margin
            && world_z <= self.max_z + margin
    }
}
