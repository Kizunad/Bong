use valence::prelude::{
    ident, App, BiomeRegistry, BlockState, Commands, DimensionTypeRegistry, LayerBundle, Res,
    Server, Startup, UnloadedChunk,
};

const TEST_AREA_CHUNKS: i32 = 16;
const CHUNK_WIDTH: i32 = 16;
const BEDROCK_Y: i32 = 64;
const GRASS_Y: i32 = BEDROCK_Y + 1;

pub fn register(app: &mut App) {
    tracing::info!("[bong][world] registering world setup system");
    app.add_systems(Startup, setup_world);
}

fn setup_world(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    tracing::info!("[bong][world] creating overworld test area (16x16 chunks)");

    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    for chunk_z in 0..TEST_AREA_CHUNKS {
        for chunk_x in 0..TEST_AREA_CHUNKS {
            layer
                .chunk
                .insert_chunk([chunk_x, chunk_z], UnloadedChunk::new());
        }
    }

    let block_extent = TEST_AREA_CHUNKS * CHUNK_WIDTH;

    for z in 0..block_extent {
        for x in 0..block_extent {
            layer
                .chunk
                .set_block([x, BEDROCK_Y, z], BlockState::BEDROCK);
            layer
                .chunk
                .set_block([x, GRASS_Y, z], BlockState::GRASS_BLOCK);
        }
    }

    commands.spawn(layer);
}
