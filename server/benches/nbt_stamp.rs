//! worldgen-v4 §8.1 #10 — NBT decoration stamp memcpy micro-bench.
//!
//! `DecorationNbtRegistry::stamp` is the per-placement hot path the flora /
//! structure decoration passes call once a resident `.nbt` variant is chosen. The
//! contract (raster.rs / nbt_registry.rs docs) is that it is **memcpy-level**: it
//! only walks the resident block list and lowers palette entries — no IO, no gzip
//! inflate (that happened once at load). The budget is < 1 ms per stamp.
//!
//! This loads the real `server/structures/decorations/**` assets via
//! `load_default()` (the production load path) and stamps real resident templates
//! through both the `Rotation::None` fast path and a rotated per-block path, so a
//! regression that reintroduces per-stamp IO or an accidental inflate shows
//! immediately.

use bong_server::world::terrain::nbt_registry::{
    DecorationAnchor, DecorationNbtRegistry, Rotation,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use valence::prelude::BlockPos;

/// Pick a representative resident template id — prefer a larger structure (more
/// blocks = more memcpy work) so the bench is not dominated by a 1-block stub.
/// Falls back to the first resident id; panics only if the registry is empty
/// (which would mean the decoration assets vanished — a real regression).
fn pick_largest_template(reg: &DecorationNbtRegistry) -> String {
    reg.iter()
        .max_by_key(|(_, s)| s.blocks.len())
        .map(|(id, _)| id.to_string())
        .expect("decoration registry must hold at least one resident template")
}

fn bench_nbt_stamp(c: &mut Criterion) {
    let reg = DecorationNbtRegistry::load_default();
    assert!(
        !reg.is_empty(),
        "load_default must find the committed decoration .nbt assets under \
         server/structures/decorations/ — an empty registry means the assets \
         regressed, not a real benchmark"
    );
    let template_id = pick_largest_template(&reg);
    let block_count = reg
        .get(&template_id)
        .map(|s| s.blocks.len())
        .unwrap_or_default();
    let surface = BlockPos::new(128, 96, -64);

    let mut group = c.benchmark_group("nbt_stamp");
    group.throughput(criterion::Throughput::Elements(block_count as u64));

    // Fast path: Rotation::None stamps positions verbatim from one base origin.
    group.bench_function("stamp_ground_none", |b| {
        b.iter(|| {
            let out = reg.stamp(
                black_box(&template_id),
                black_box(surface),
                DecorationAnchor::Ground,
                Rotation::None,
            );
            black_box(out);
        });
    });

    // Rotated path: per-block rotation about Y (the heavier branch).
    group.bench_function("stamp_ground_cw90", |b| {
        b.iter(|| {
            let out = reg.stamp(
                black_box(&template_id),
                black_box(surface),
                DecorationAnchor::Ground,
                Rotation::Cw90,
            );
            black_box(out);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_nbt_stamp);
criterion_main!(benches);
