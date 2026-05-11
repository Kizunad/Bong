use std::fmt;

use valence::prelude::{BlockPos, BlockState, ChunkLayer};

pub const BONG_BLOCK_STATE_START: u16 = 24135;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceError {
    NonBongBlock(BlockState),
    ChunkNotLoaded(BlockPos),
}

impl fmt::Display for PlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonBongBlock(state) => write!(f, "state {state:?} is not a Bong custom block"),
            Self::ChunkNotLoaded(pos) => {
                write!(f, "chunk for block position {pos:?} is not loaded")
            }
        }
    }
}

pub fn place_bong_block(
    chunk_layer: &mut ChunkLayer,
    pos: BlockPos,
    block: BlockState,
) -> Result<(), PlaceError> {
    if !is_bong_block(block) {
        return Err(PlaceError::NonBongBlock(block));
    }
    if chunk_layer.block(pos).is_none() {
        return Err(PlaceError::ChunkNotLoaded(pos));
    }

    chunk_layer.set_block(pos, block);
    Ok(())
}

pub fn remove_bong_block(chunk_layer: &mut ChunkLayer, pos: BlockPos) -> Option<BlockState> {
    let previous = chunk_layer.block(pos).map(|block| block.state)?;
    if !is_bong_block(previous) {
        return None;
    }

    chunk_layer.set_block(pos, BlockState::AIR);
    Some(previous)
}

pub fn is_bong_block(state: BlockState) -> bool {
    state.to_raw() >= BONG_BLOCK_STATE_START
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Entity, UnloadedChunk};
    use valence::testing::ScenarioSingleClient;

    fn test_layer() -> (App, Entity) {
        let scenario = ScenarioSingleClient::new();
        let mut app = scenario.app;
        crate::world::dimension::mark_test_layer_as_overworld(&mut app);
        app.world_mut()
            .get_mut::<ChunkLayer>(scenario.layer)
            .expect("test layer should carry ChunkLayer")
            .insert_chunk([0, 0], UnloadedChunk::new());
        (app, scenario.layer)
    }

    #[test]
    fn place_and_read_back() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(3, 64, 3);
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        place_bong_block(&mut layer, pos, BlockState::BONG_ZHENFA_NODE)
            .expect("custom block should place into loaded chunk");

        assert_eq!(
            layer.block(pos).map(|block| block.state),
            Some(BlockState::BONG_ZHENFA_NODE)
        );
    }

    #[test]
    fn place_rejects_vanilla_block() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(3, 64, 3);
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");

        assert_eq!(
            place_bong_block(&mut layer, pos, BlockState::STONE),
            Err(PlaceError::NonBongBlock(BlockState::STONE))
        );
        assert_eq!(
            layer.block(pos).map(|block| block.state),
            Some(BlockState::AIR)
        );
    }

    #[test]
    fn remove_returns_previous_custom_block() {
        let (mut app, layer_entity) = test_layer();
        let pos = BlockPos::new(3, 64, 3);
        let mut layer = app
            .world_mut()
            .get_mut::<ChunkLayer>(layer_entity)
            .expect("test layer should carry ChunkLayer");
        place_bong_block(&mut layer, pos, BlockState::BONG_ZHENFA_NODE)
            .expect("custom block should place into loaded chunk");

        assert_eq!(
            remove_bong_block(&mut layer, pos),
            Some(BlockState::BONG_ZHENFA_NODE)
        );
        assert_eq!(
            layer.block(pos).map(|block| block.state),
            Some(BlockState::AIR)
        );
    }

    #[test]
    fn is_bong_block_true_for_custom() {
        assert!(is_bong_block(BlockState::BONG_ZHENFA_NODE));
        assert!(is_bong_block(BlockState::BONG_ZHENFA_EYE));
    }

    #[test]
    fn is_bong_block_false_for_vanilla() {
        assert!(!is_bong_block(BlockState::AIR));
        assert!(!is_bong_block(BlockState::STONE));
    }
}
