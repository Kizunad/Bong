use bong_server::world::terrain::TerrainProvider;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{BiomeRegistry, Ident};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_temp_dir() -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bong-raster-layer-query-{}-{nanos}-{counter}",
        std::process::id()
    ))
}

fn test_biomes() -> BiomeRegistry {
    use valence::prelude::Biome;

    let mut biomes = BiomeRegistry::default();
    biomes.insert(
        Ident::new("plains").expect("valid test biome identifier"),
        Biome::default(),
    );
    biomes
}

#[test]
fn public_load_rejects_invalid_and_missing_nbt_template_references() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("public loader fixture root should be creatable");
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["stone"],
            "biome_palette": ["plains"],
            "tiles": [],
            "global_decoration_palette": [{
                "global_id": 1,
                "profile": "test",
                "local_id": 1,
                "name": "broken_public_loader_deco",
                "kind": "test",
                "blocks": ["stone"],
                "size_range": [1, 1],
                "rarity": 1.0,
                "notes": "",
                "nbt_templates": ["../escape.nbt", "decorations/test/missing.nbt"],
                "anchor": "ground"
            }]
        }"#,
    )
    .expect("public loader manifest should be writable");

    let error = TerrainProvider::load(&manifest_path, &root, &test_biomes())
        .expect_err("the public loader must enforce the same strict NBT reference admission");
    assert!(
        error.contains("../escape.nbt") && error.contains("invalid template id"),
        "public loader must reject malformed template ids: {error}"
    );
    assert!(
        error.contains("decorations/test/missing.nbt")
            && error.contains("missing resident template"),
        "public loader must reject dangling resident-template references: {error}"
    );

    let _ = fs::remove_dir_all(&root);
}
