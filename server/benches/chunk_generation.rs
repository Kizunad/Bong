//! worldgen-v4 §8.1 #10 — per-chunk terrain generation budget.
//!
//! Measures the real chunk-fill hot loop: build an `UnloadedChunk`, then for all
//! 16×16 = 256 columns sample the mmap-backed raster (`TerrainProvider::sample`)
//! and lower its spans into block states (`column::fill_column`). This is the
//! exact inner loop `world::terrain::ensure_chunk_generated` runs per chunk
//! (the procedural decoration / structure / authored passes that run afterward
//! are placement-density-gated and contribute far less than the unconditional
//! column fill, so the span→block lowering is the right thing to budget).
//!
//! Budget (§8.1 #10): target < 25 ms/chunk, hard cap 30 ms. The v3 baseline noted
//! in `raster.rs` was "~30 ms/chunk @ 20 TPS"; the v4 span representation should
//! be at or under that. The measured wall-clock number is the v4 baseline this
//! bench freezes — a regression that doubles per-chunk cost will show immediately.

use bong_server::world::terrain::{
    bench_support::single_tile_fixture, column, MIN_Y, WORLD_HEIGHT,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use valence::prelude::UnloadedChunk;

fn fill_one_chunk(terrain: &bong_server::world::terrain::TerrainProvider) -> UnloadedChunk {
    let min_y = MIN_Y;
    let mut chunk = UnloadedChunk::with_height(WORLD_HEIGHT);
    for local_z in 0..16i32 {
        for local_x in 0..16i32 {
            // Chunk (0,0): world coords == local coords, all inside the tile.
            let sample = terrain.sample(local_x, local_z);
            column::fill_column(&mut chunk, local_x as u32, local_z as u32, min_y, &sample);
        }
    }
    chunk
}

fn bench_chunk_generation(c: &mut Criterion) {
    let fixture = single_tile_fixture();
    let terrain = fixture.provider();

    let mut group = c.benchmark_group("chunk_generation");
    // One chunk per iteration; criterion reports time/iter == time/chunk.
    group.bench_function("fill_chunk_16x16", |b| {
        b.iter(|| {
            let chunk = fill_one_chunk(black_box(terrain));
            black_box(chunk);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_chunk_generation);
criterion_main!(benches);
