//! P1 — authored structure placement pass (plan-terrain-wiring-v1).
//!
//! Reads the pre-bucketed placement index built from `placement_manifest.json`
//! sidecar and stamps blocks into the in-progress chunk.  The pass is
//! zone-agnostic: it simply looks up the given `ChunkPos` in the index and
//! writes whatever blocks are stored there.  This guarantees that future zones
//! (P3 王印台, etc.) require **zero new server code** — only worldgen changes.
//!
//! # Insertion point
//! Called after `structures::decorate_chunk` and before biome fill so
//! authored buildings override generic procedural decorations while density
//! masking (P0 worldgen side) already zeroed flora in the compound footprint.
//!
//! # Block replacement policy
//! Authored manifest blocks are authored structures — they always win over the
//! terrain/decoration that was already placed.  We write unconditionally into
//! the chunk because P0's `compound_flatten` already levelled the surface and
//! the density mask removed flora.

use valence::prelude::{Chunk, ChunkPos, UnloadedChunk};

use super::raster::TerrainProvider;

/// Stamp all authored structure blocks for `chunk_pos` into `chunk`.
///
/// Blocks outside the Y range `[min_y, min_y + chunk.height())` are silently
/// dropped — the manifest may contain blocks that fall above the world ceiling
/// or in the bedrock layer.
///
/// This function is zone-agnostic: it uses only `ChunkPos` for lookup, so any
/// zone with a `placement_manifest.json` entry is handled automatically.
pub(super) fn place_authored_structures(
    chunk: &mut UnloadedChunk,
    chunk_pos: ChunkPos,
    min_y: i32,
    provider: &TerrainProvider,
) {
    let blocks = provider.placement_blocks_for_chunk(chunk_pos);
    if blocks.is_empty() {
        return;
    }

    let height = chunk.height() as i32;

    for &(block_pos, block_state) in blocks {
        // Verify the block actually belongs to this chunk (defensive check).
        if block_pos.x.div_euclid(16) != chunk_pos.x || block_pos.z.div_euclid(16) != chunk_pos.z {
            continue;
        }

        let local_y = block_pos.y - min_y;
        if local_y < 0 || local_y >= height {
            continue;
        }

        let local_x = block_pos.x.rem_euclid(16) as u32;
        let local_z = block_pos.z.rem_euclid(16) as u32;

        chunk.set_block_state(local_x, local_y as u32, local_z, block_state);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use valence::prelude::{BlockPos, BlockState, ChunkPos};

    use crate::world::terrain::{raster::TerrainProvider, MIN_Y, WORLD_HEIGHT};

    // ----- helpers ----------------------------------------------------------

    fn make_chunk() -> UnloadedChunk {
        UnloadedChunk::with_height(WORLD_HEIGHT)
    }

    fn block_at(chunk: &UnloadedChunk, local_x: u32, world_y: i32, local_z: u32) -> BlockState {
        let local_y = (world_y - MIN_Y) as u32;
        chunk.block_state(local_x, local_y, local_z)
    }

    // Build a placement index with a single block and return a provider.
    fn provider_with_one_block(
        chunk_x: i32,
        chunk_z: i32,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        state: BlockState,
    ) -> TerrainProvider {
        let pos = BlockPos::new(world_x, world_y, world_z);
        let cp = ChunkPos::new(chunk_x, chunk_z);
        let mut index = HashMap::new();
        index.insert(cp, vec![(pos, state)]);
        TerrainProvider::with_placement_index_for_tests(index)
    }

    // ----- contract: happy path ----------------------------------------

    /// P1 contract: a block in the manifest for chunk (0,0) lands in the
    /// chunk at the correct local coords.
    #[test]
    fn authored_block_is_stamped_into_correct_chunk_position() {
        let world_x = 5;
        let world_y = 82;
        let world_z = 9;
        let provider =
            provider_with_one_block(0, 0, world_x, world_y, world_z, BlockState::STONE_BRICKS);
        let mut chunk = make_chunk();

        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);

        assert_eq!(
            block_at(&chunk, world_x as u32, world_y, world_z as u32),
            BlockState::STONE_BRICKS,
            "authored block should be stamped at local ({world_x},{world_y},{world_z})"
        );
    }

    // ----- cross-chunk bucketing: each chunk stamps only its own blocks -----

    /// A structure spanning two chunks must appear in both, never bleed across.
    #[test]
    fn cross_chunk_structure_stamps_each_chunk_independently() {
        // Two blocks: one in chunk (0,0), one in chunk (1,0)
        let pos_a = BlockPos::new(15, 82, 0); // world_x=15 → chunk_x=0
        let pos_b = BlockPos::new(16, 82, 0); // world_x=16 → chunk_x=1

        let cp_a = ChunkPos::new(0, 0);
        let cp_b = ChunkPos::new(1, 0);

        let mut index = HashMap::new();
        index.insert(cp_a, vec![(pos_a, BlockState::STONE_BRICKS)]);
        index.insert(cp_b, vec![(pos_b, BlockState::MOSSY_COBBLESTONE)]);

        let provider = TerrainProvider::with_placement_index_for_tests(index);

        let mut chunk_a = make_chunk();
        place_authored_structures(&mut chunk_a, cp_a, MIN_Y, &provider);
        let mut chunk_b = make_chunk();
        place_authored_structures(&mut chunk_b, cp_b, MIN_Y, &provider);

        // chunk_a has STONE_BRICKS at local_x=15
        assert_eq!(
            block_at(&chunk_a, 15, 82, 0),
            BlockState::STONE_BRICKS,
            "chunk_a should contain pos_a block (x=15)"
        );
        // chunk_a must NOT contain pos_b
        assert_eq!(
            block_at(&chunk_a, 0, 82, 0),
            BlockState::AIR,
            "chunk_a must not contain pos_b block (x=16 belongs to chunk_b)"
        );

        // chunk_b has MOSSY_COBBLESTONE at local_x=0
        assert_eq!(
            block_at(&chunk_b, 0, 82, 0),
            BlockState::MOSSY_COBBLESTONE,
            "chunk_b should contain pos_b block (x=16, local_x=0)"
        );
        // chunk_b must NOT contain pos_a
        assert_eq!(
            block_at(&chunk_b, 15, 82, 0),
            BlockState::AIR,
            "chunk_b must not contain pos_a block (x=15 belongs to chunk_a)"
        );
    }

    // ----- large structure: >4 chunks -----------------------------------

    /// A structure spanning 5 chunks in X: only the matching chunk gets each
    /// block column stamped, and no column appears in the wrong chunk.
    #[test]
    fn large_structure_spanning_more_than_four_chunks_stamps_correct_columns() {
        let mut index: HashMap<ChunkPos, Vec<(BlockPos, BlockState)>> = HashMap::new();

        // Place one block in each of chunks (0,0)…(4,0)
        for cx in 0_i32..5 {
            let world_x = cx * 16; // first column of each chunk
            let cp = ChunkPos::new(cx, 0);
            index.insert(
                cp,
                vec![(BlockPos::new(world_x, 64, 0), BlockState::COBBLESTONE)],
            );
        }

        let provider = TerrainProvider::with_placement_index_for_tests(index);

        for cx in 0_i32..5 {
            let cp = ChunkPos::new(cx, 0);
            let mut chunk = make_chunk();
            place_authored_structures(&mut chunk, cp, MIN_Y, &provider);

            // The block placed in this chunk must be present
            assert_eq!(
                block_at(&chunk, 0, 64, 0),
                BlockState::COBBLESTONE,
                "chunk ({cx},0) should have its authored block at local_x=0"
            );
            // Blocks NOT in this chunk must not appear
            for cx2 in 0_i32..5 {
                if cx2 == cx {
                    continue;
                }
                let other_world_x = cx2 * 16;
                let local_x = other_world_x.rem_euclid(16) as u32;
                // Only meaningful when local_x would differ
                if local_x != 0 {
                    assert_eq!(
                        block_at(&chunk, local_x, 64, 0),
                        BlockState::AIR,
                        "chunk ({cx},0) must not contain block from chunk ({cx2},0)"
                    );
                }
            }
        }
    }

    // ----- Y bounds clamping --------------------------------------------

    /// Blocks below min_y must be silently dropped (not panic).
    #[test]
    fn blocks_below_min_y_are_dropped_without_panic() {
        let world_y = MIN_Y - 1; // below chunk floor
        let provider = provider_with_one_block(0, 0, 0, world_y, 0, BlockState::STONE_BRICKS);
        let mut chunk = make_chunk();
        // Must not panic
        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);
        // Chunk is all-air (nothing was stamped)
        assert_eq!(
            block_at(&chunk, 0, MIN_Y, 0),
            BlockState::AIR,
            "block below min_y must be silently dropped"
        );
    }

    /// Blocks above world height must be silently dropped (not panic).
    #[test]
    fn blocks_above_world_height_are_dropped_without_panic() {
        let world_y = MIN_Y + WORLD_HEIGHT as i32; // one above ceiling
        let provider = provider_with_one_block(0, 0, 0, world_y, 0, BlockState::STONE_BRICKS);
        let mut chunk = make_chunk();
        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);
        // Last valid Y is MIN_Y + WORLD_HEIGHT - 1; check that is still AIR
        assert_eq!(
            block_at(&chunk, 0, MIN_Y + WORLD_HEIGHT as i32 - 1, 0),
            BlockState::AIR,
            "block above world ceiling must be silently dropped"
        );
    }

    // ----- no-placement: absent sidecar → chunk generated normally ------

    /// When placement_index is empty (no sidecar loaded), chunk generation
    /// must succeed without panicking.
    #[test]
    fn empty_placement_index_produces_no_authored_blocks() {
        let provider = TerrainProvider::empty_for_tests();
        let mut chunk = make_chunk();
        // Must not panic and must leave chunk untouched
        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);
        // Spot-check a few positions
        assert_eq!(
            block_at(&chunk, 8, 64, 8),
            BlockState::AIR,
            "empty index must leave chunk all-air"
        );
    }

    // ----- authored block overwrites existing terrain --------------------

    /// Authored buildings override what was already written into the chunk
    /// (column fill + flora + decorations all run before this pass).
    #[test]
    fn authored_block_overwrites_existing_chunk_contents() {
        let world_x = 3;
        let world_y = 64;
        let world_z = 3;
        let provider =
            provider_with_one_block(0, 0, world_x, world_y, world_z, BlockState::STONE_BRICKS);
        let mut chunk = make_chunk();

        // Pre-fill the target position with STONE (simulating terrain fill)
        let local_y = (world_y - MIN_Y) as u32;
        chunk.set_block_state(world_x as u32, local_y, world_z as u32, BlockState::STONE);

        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);

        assert_eq!(
            block_at(&chunk, world_x as u32, world_y, world_z as u32),
            BlockState::STONE_BRICKS,
            "authored block must overwrite existing terrain block"
        );
    }

    // ----- boundary: block exactly at chunk edge ------------------------

    /// A block at world_x = chunk_x * 16 + 15 (last column of chunk) must land
    /// at local_x = 15, not bleed into the adjacent chunk.
    #[test]
    fn block_at_chunk_x_boundary_lands_in_correct_chunk() {
        let world_x = 15; // last column of chunk (0,0): chunk_x * 16 + 15 = 15
        let world_z = 15;
        let provider = provider_with_one_block(0, 0, world_x, 64, world_z, BlockState::COBBLESTONE);
        let mut chunk = make_chunk();

        place_authored_structures(&mut chunk, ChunkPos::new(0, 0), MIN_Y, &provider);

        assert_eq!(
            block_at(&chunk, 15, 64, 15),
            BlockState::COBBLESTONE,
            "block at chunk edge (local_x=15) must land correctly"
        );
    }
}
