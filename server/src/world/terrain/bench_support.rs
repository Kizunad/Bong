//! Bench / test fixture builder for the worldgen-v4 §8.1 #10 performance gate.
//!
//! The criterion benches under `server/benches/` are separate crates: they can
//! only see the `bong_server` lib's public API, and a `TerrainProvider` is mmap
//! backed (it must read real files off disk). Rather than duplicate the on-disk
//! v2 raster layout inside the bench, this module writes a minimal but **real**
//! single-tile raster and loads it through the production [`TerrainProvider::load`]
//! path, so the bench measures the genuine `sample` → [`column::fill_column`]
//! chunk-generation hot loop end to end.
//!
//! This is fixture support only — no runtime system references it; it is `pub`
//! purely so the out-of-crate bench can call it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use valence::prelude::{Biome, BiomeRegistry, Ident};

use super::raster::{ColumnSpanList, TerrainProvider, MAX_SPANS, SPAN_SENTINEL, SPAN_STRIDE};
use super::MIN_Y;

/// Tile size for the bench fixture. 16 == exactly one chunk footprint, so a
/// chunk at `(0, 0)` reads entirely from this single tile (no wilderness
/// fallthrough), exercising the real mmap-backed `sample` for all 256 columns.
pub const BENCH_TILE_SIZE: i32 = 16;

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A bench fixture: an mmap-backed [`TerrainProvider`] plus the temp dir it lives
/// in. Dropping it releases the mmaps (by dropping the provider first) and then
/// removes the temp dir.
pub struct BenchTerrain {
    provider: Option<TerrainProvider>,
    root: PathBuf,
}

impl BenchTerrain {
    /// Borrow the loaded provider for sampling.
    pub fn provider(&self) -> &TerrainProvider {
        self.provider
            .as_ref()
            .expect("bench provider is present until drop")
    }
}

impl Drop for BenchTerrain {
    fn drop(&mut self) {
        // Drop the provider (and its mmaps) before deleting the backing files.
        self.provider.take();
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Build a single-tile raster fixture with a **representative mix** of column
/// shapes so `column::fill_column`'s branches are all exercised by the bench:
///
/// * plain solid columns (`MIN_Y..surface`) — the common case,
/// * a sky-isle column (extra span floating above the surface) — `sky_island`
///   branch + the `sky_island_mask >= 0.2` gate,
/// * a carved-cave column (floor remnant below the surface span) — `cave_carve`
///   branch,
/// * a water column (surface dipped below `water_level`) — the water-fill branch.
///
/// The shape is keyed off `(local_x + local_z) % 8` so roughly an eighth of the
/// 256 columns hit each non-trivial branch — a realistic blend, not a degenerate
/// all-flat plane that would understate the per-chunk write cost.
pub fn single_tile_fixture() -> BenchTerrain {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).expect("bench raster tile dir should be creatable");

    let tile_area = (BENCH_TILE_SIZE * BENCH_TILE_SIZE) as usize;
    let surface_y: i16 = 96;

    // --- spans (the vertical source of truth) ---
    let mut columns: Vec<ColumnSpanList> = Vec::with_capacity(tile_area);
    let mut sky_mask: Vec<f32> = Vec::with_capacity(tile_area);
    let mut water_level: Vec<f32> = Vec::with_capacity(tile_area);
    for local_z in 0..BENCH_TILE_SIZE {
        for local_x in 0..BENCH_TILE_SIZE {
            let bucket = ((local_x + local_z) as usize) % 8;
            let mut spans: ColumnSpanList = smallvec::smallvec![(MIN_Y as i16, surface_y)];
            let mut mask = 0.0_f32;
            let mut water = -1.0_f32;
            match bucket {
                1 => {
                    // Sky isle floating above the surface (air gap > 1).
                    spans.push((surface_y + 24, surface_y + 30));
                    mask = 0.8;
                }
                2 => {
                    // Carved cave: a floor remnant strictly below the surface span,
                    // so `cave_carve` reports the air gap between them.
                    spans[0] = (surface_y - 12, surface_y);
                    spans.push((MIN_Y as i16, surface_y - 20));
                }
                3 => {
                    // Shallow basin filled with water above the dipped surface.
                    spans[0] = (MIN_Y as i16, surface_y - 8);
                    water = f32::from(surface_y - 3);
                }
                _ => {}
            }
            columns.push(spans);
            sky_mask.push(mask);
            water_level.push(water);
        }
    }
    let (count_bytes, spans_bytes) = encode_spans(&columns);
    write_bytes(&tile_dir, "spans_count.bin", &count_bytes);
    write_bytes(&tile_dir, "spans.bin", &spans_bytes);

    // --- required scalar layers ---
    // surface/subsurface id index into surface_palette (grass / dirt below).
    write_u8(&tile_dir, "surface_id.bin", 0, tile_area);
    write_u8(&tile_dir, "subsurface_id.bin", 1, tile_area);
    write_u8(&tile_dir, "biome_id.bin", 1, tile_area); // peaks biome
    write_f32_each(&tile_dir, "water_level.bin", &water_level);
    write_f32(&tile_dir, "feature_mask.bin", 0.0, tile_area);
    write_f32(&tile_dir, "boundary_weight.bin", 1.0, tile_area);

    // --- optional layers the bench cares about (sky-isle gate) ---
    write_f32_each(&tile_dir, "sky_island_mask.bin", &sky_mask);

    let layers = vec![
        "spans_count".to_string(),
        "spans".to_string(),
        "surface_id".to_string(),
        "subsurface_id".to_string(),
        "biome_id".to_string(),
        "water_level".to_string(),
        "feature_mask".to_string(),
        "boundary_weight".to_string(),
        "sky_island_mask".to_string(),
    ];

    let manifest = bench_manifest_json(&layers);
    let manifest_path = root.join("manifest.json");
    fs::write(&manifest_path, manifest).expect("bench manifest.json should be writable");

    // `load` resolves manifest biome names through the registry, so the fixture
    // must register the names it references (an empty registry would reject
    // "plains"). Production gets this from valence's DefaultPlugins biome init.
    let mut biomes = BiomeRegistry::default();
    biomes.insert(
        Ident::new("plains").expect("valid biome ident"),
        Biome::default(),
    );
    biomes.insert(
        Ident::new("windswept_hills").expect("valid biome ident"),
        Biome::default(),
    );
    let provider = TerrainProvider::load(&manifest_path, &root, &biomes)
        .expect("bench raster fixture should load through the production path");

    BenchTerrain {
        provider: Some(provider),
        root,
    }
}

/// Mirror of the Python `encode_spans_arrays` / the in-crate test encoder: one
/// count byte per column + `MAX_SPANS` `(i16 floor, i16 ceiling)` slots, unused
/// slots filled with the sentinel.
fn encode_spans(columns: &[ColumnSpanList]) -> (Vec<u8>, Vec<u8>) {
    let mut count_bytes = Vec::with_capacity(columns.len());
    let mut spans_bytes = Vec::with_capacity(columns.len() * SPAN_STRIDE);
    for column in columns {
        let n = column.len().min(MAX_SPANS);
        count_bytes.push(n as u8);
        for slot in 0..MAX_SPANS {
            let (floor_y, ceiling_y) = if slot < n {
                column[slot]
            } else {
                (SPAN_SENTINEL, SPAN_SENTINEL)
            };
            spans_bytes.extend_from_slice(&floor_y.to_le_bytes());
            spans_bytes.extend_from_slice(&ceiling_y.to_le_bytes());
        }
    }
    (count_bytes, spans_bytes)
}

fn bench_manifest_json(layers: &[String]) -> String {
    let layer_list = layers
        .iter()
        .map(|l| format!("\"{l}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let max = BENCH_TILE_SIZE - 1;
    format!(
        r#"{{
  "version": 2,
  "tile_size": {BENCH_TILE_SIZE},
  "world_bounds": {{ "min_x": 0, "max_x": {max}, "min_z": 0, "max_z": {max} }},
  "surface_palette": ["grass_block", "dirt", "stone", "water"],
  "biome_palette": ["plains", "windswept_hills"],
  "tiles": [
    {{ "tile_x": 0, "tile_z": 0, "dir": "tile_0_0", "layers": [{layer_list}] }}
  ]
}}"#
    )
}

fn write_bytes(tile_dir: &Path, name: &str, bytes: &[u8]) {
    fs::write(tile_dir.join(name), bytes)
        .unwrap_or_else(|e| panic!("bench fixture {name} should be writable: {e}"));
}

fn write_u8(tile_dir: &Path, name: &str, value: u8, count: usize) {
    write_bytes(tile_dir, name, &vec![value; count]);
}

fn write_f32(tile_dir: &Path, name: &str, value: f32, count: usize) {
    let mut bytes = Vec::with_capacity(count * 4);
    for _ in 0..count {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    write_bytes(tile_dir, name, &bytes);
}

fn write_f32_each(tile_dir: &Path, name: &str, values: &[f32]) {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for v in values {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    write_bytes(tile_dir, name, &bytes);
}

fn unique_temp_dir() -> PathBuf {
    let counter = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bong-bench-raster-{}-{nanos}-{counter}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_loads_and_samples_every_chunk_column() {
        let fixture = single_tile_fixture();
        let provider = fixture.provider();
        // Every column in the chunk must sample from the tile (no void) and
        // resolve to a sane surface span.
        let mut saw_sky = false;
        let mut saw_cave = false;
        let mut saw_water = false;
        for z in 0..16 {
            for x in 0..16 {
                let sample = provider.sample(x, z);
                assert!(
                    !sample.spans.is_empty(),
                    "bench fixture column ({x},{z}) must have at least the surface span"
                );
                if sample.sky_island_span().is_some() && sample.sky_island_mask >= 0.2 {
                    saw_sky = true;
                }
                if sample.has_carved_cave() {
                    saw_cave = true;
                }
                if sample.water_level >= 0.0 {
                    saw_water = true;
                }
            }
        }
        assert!(saw_sky, "fixture must include at least one sky-isle column");
        assert!(
            saw_cave,
            "fixture must include at least one carved-cave column"
        );
        assert!(saw_water, "fixture must include at least one water column");
    }
}
