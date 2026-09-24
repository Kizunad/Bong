use super::*;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const TILE_SIZE: i32 = 2;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
struct RegistryFixtureEntry {
    name: String,
    export_type: String,
    safe_default: f32,
}

struct RasterFixture {
    provider: Option<TerrainProvider>,
    root: PathBuf,
}

impl RasterFixture {
    fn provider(&self) -> &TerrainProvider {
        self.provider
            .as_ref()
            .expect("fixture provider should be present until drop")
    }
}

impl Drop for RasterFixture {
    fn drop(&mut self) {
        self.provider.take();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn registry_fixture() -> Vec<RegistryFixtureEntry> {
    serde_json::from_str(include_str!("layer_registry_fixture.json"))
        .expect("layer registry fixture should be valid JSON")
}

/// Surface ceiling baked into every fixture column's surface span. Chosen
/// in-range so `sample_layer("height")` (which now reads the span ceiling)
/// has a deterministic expected value.
const FIXTURE_SURFACE_Y: i16 = 100;

fn test_biomes() -> BiomeRegistry {
    use valence::prelude::Biome;

    let mut biomes = BiomeRegistry::default();
    biomes.insert(
        Ident::new("plains").expect("valid test biome identifier"),
        Biome::default(),
    );
    biomes
}

fn invalid_cross_source_manifest() -> String {
    r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["unknown_surface_for_preflight"],
            "biome_palette": ["plains"],
            "tiles": [],
            "global_decoration_palette": [
                {
                    "global_id": 1,
                    "profile": "test",
                    "local_id": 1,
                    "name": "broken_deco",
                    "kind": "test",
                    "blocks": ["unknown_decoration_for_preflight"],
                    "size_range": [1, 1],
                    "rarity": 1.0,
                    "notes": "",
                    "nbt_templates": ["../escape.nbt", "decorations/test/missing.nbt"],
                    "anchor": "ground"
                }
            ]
        }"#
    .to_string()
}

fn build_fixture() -> RasterFixture {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).expect("test raster tile dir should be creatable");
    let tile_area = (TILE_SIZE * TILE_SIZE) as usize;

    // worldgen-v4 P0 §8.1 #1: every fixture column is a single solid span
    // (MIN_Y .. FIXTURE_SURFACE_Y); height.bin no longer exists on disk.
    write_spans_single_fixture(&tile_dir, tile_area, FIXTURE_SURFACE_Y);

    for (index, schema) in LAYER_SCHEMAS.iter().enumerate() {
        if schema.name == "height" {
            // height is folded into spans — never written as a raster.
            continue;
        }
        let path = tile_dir.join(format!("{}.bin", schema.name));
        match schema.export_type {
            LayerExportType::F32 => write_f32_layer(&path, test_f32_value(index), tile_area),
            LayerExportType::U8 => write_u8_layer(&path, test_u8_value(index), tile_area),
        }
    }

    let layers = LAYER_SCHEMAS
        .iter()
        .map(|schema| schema.name.to_string())
        .collect::<Vec<_>>();
    let tile = TileFields::load(&tile_dir, &root, &layers, tile_area)
        .expect("test raster fields should load");
    let mut tiles = HashMap::new();
    tiles.insert((0, 0), tile);

    let provider = TerrainProvider {
        tiles,
        tile_size: TILE_SIZE,
        world_bounds: Bounds2D {
            min_x: 0,
            max_x: TILE_SIZE - 1,
            min_z: 0,
            max_z: TILE_SIZE - 1,
        },
        surface_palette: vec![BlockState::STONE; 64],
        biome_palette: vec![BiomeId::DEFAULT; 64],
        default_wilderness_biome: BiomeId::DEFAULT,
        forest_wilderness_biome: BiomeId::DEFAULT,
        river_wilderness_biome: BiomeId::DEFAULT,
        pois: Vec::new(),
        anomaly_kinds: HashMap::new(),
        decoration_palette: Vec::new(),
        abyssal_tier_floor_y: HashMap::new(),
        fossil_bboxes: Vec::new(),
        placement_index: HashMap::new(),
        placement_block_count: 0,
        bot_fixture: None,
    };

    RasterFixture {
        provider: Some(provider),
        root,
    }
}

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

fn write_f32_layer(path: &Path, value: f32, tile_area: usize) {
    let mut bytes = Vec::with_capacity(tile_area * 4);
    for _ in 0..tile_area {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).expect("test f32 layer should be writable");
}

fn write_u8_layer(path: &Path, value: u8, tile_area: usize) {
    fs::write(path, vec![value; tile_area]).expect("test u8 layer should be writable");
}

/// Encode a slice of per-column span lists into the on-disk
/// (spans_count.bin, spans.bin) byte layout — the exact mirror of the
/// Python exporter (`encode_spans_arrays`). Unused slots get the sentinel.
fn encode_spans_bytes(columns: &[ColumnSpanList]) -> (Vec<u8>, Vec<u8>) {
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

/// Write spans_count.bin + spans.bin for a tile whose every column is one
/// solid span `(MIN_Y, surface_y)`.
fn write_spans_single_fixture(tile_dir: &Path, tile_area: usize, surface_y: i16) {
    let column: ColumnSpanList = smallvec::smallvec![(super::super::MIN_Y as i16, surface_y)];
    let columns = vec![column; tile_area];
    let (count_bytes, spans_bytes) = encode_spans_bytes(&columns);
    fs::write(tile_dir.join("spans_count.bin"), count_bytes)
        .expect("test spans_count.bin should be writable");
    fs::write(tile_dir.join("spans.bin"), spans_bytes).expect("test spans.bin should be writable");
}

fn test_f32_value(index: usize) -> f32 {
    1000.25 + index as f32
}

fn test_u8_value(index: usize) -> u8 {
    u8::try_from(index + 1).expect("test layer index should fit in u8")
}

fn assert_f32_eq(actual: f32, expected: f32, layer_name: &str) {
    assert!(
        (actual - expected).abs() < f32::EPSILON,
        "layer {layer_name} expected {expected}, got {actual}"
    );
}

#[test]
fn layer_names_size_matches_python_registry_fixture() {
    let fixture = registry_fixture();
    assert_eq!(TerrainProvider::layer_names().len(), fixture.len());

    for (schema, expected) in TerrainProvider::layer_names().iter().zip(fixture.iter()) {
        assert_eq!(schema.name, expected.name);
        match schema.export_type {
            LayerExportType::F32 => {
                assert_eq!(expected.export_type, "float32");
                assert_eq!(schema.safe_default_f32, Some(expected.safe_default));
                assert_eq!(schema.safe_default_u8, None);
            }
            LayerExportType::U8 => {
                assert_eq!(expected.export_type, "uint8");
                assert!(
                    expected.safe_default.is_finite(),
                    "uint8 layer {} safe_default should be finite",
                    expected.name
                );
                assert_eq!(
                    expected.safe_default.fract(),
                    0.0,
                    "uint8 layer {} safe_default should be an integer before casting",
                    expected.name
                );
                assert!(
                    (0.0..=u8::MAX as f32).contains(&expected.safe_default),
                    "uint8 layer {} safe_default should fit in u8",
                    expected.name
                );
                assert_eq!(schema.safe_default_f32, None);
                assert_eq!(schema.safe_default_u8, Some(expected.safe_default as u8));
            }
        }
    }
}

#[test]
fn layer_names_no_duplicates() {
    let mut names = HashSet::new();
    for schema in TerrainProvider::layer_names() {
        assert!(
            names.insert(schema.name),
            "duplicate terrain layer schema name {}",
            schema.name
        );
    }
}

#[test]
fn sample_layer_f32_known_layers_return_tile_values() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    for (index, schema) in TerrainProvider::layer_names().iter().enumerate() {
        if schema.export_type != LayerExportType::F32 {
            continue;
        }
        // worldgen-v4 P0 §8.1 #1: "height" has no standalone raster anymore
        // — it resolves to the surface span ceiling, asserted separately.
        if schema.name == "height" {
            let actual = provider
                .sample_layer_f32(1, 1, "height")
                .expect("height should resolve via spans");
            assert_f32_eq(actual, f32::from(FIXTURE_SURFACE_Y), "height");
            continue;
        }
        let actual = provider
            .sample_layer_f32(1, 1, schema.name)
            .expect("known f32 layer should return a value");
        assert_f32_eq(actual, test_f32_value(index), schema.name);
    }
}

#[test]
fn sample_layer_u8_known_layers_return_tile_values() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    for (index, schema) in TerrainProvider::layer_names().iter().enumerate() {
        if schema.export_type != LayerExportType::U8 {
            continue;
        }
        let actual = provider
            .sample_layer_u8(1, 1, schema.name)
            .expect("known u8 layer should return a value");
        assert_eq!(actual, test_u8_value(index), "layer {}", schema.name);
    }
}

#[test]
fn sample_layer_unknown_names_return_none() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    assert_eq!(provider.sample_layer_f32(1, 1, "missing_layer"), None);
    assert_eq!(provider.sample_layer_u8(1, 1, "missing_layer"), None);
    assert_eq!(provider.sample_layer(1, 1, "missing_layer"), None);
}

#[test]
fn sample_layer_rejects_export_type_mismatch() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    assert_eq!(provider.sample_layer_f32(1, 1, "surface_id"), None);
    assert_eq!(provider.sample_layer_u8(1, 1, "height"), None);
}

#[test]
fn sample_layer_wilderness_returns_schema_safe_defaults() {
    let provider = TerrainProvider::empty_for_tests();

    for schema in TerrainProvider::layer_names() {
        match schema.export_type {
            LayerExportType::F32 => {
                let actual = provider
                    .sample_layer_f32(2048, 2048, schema.name)
                    .expect("known wilderness f32 layer should return default");
                assert_f32_eq(
                    actual,
                    schema
                        .safe_default_f32
                        .expect("f32 schema should carry f32 default"),
                    schema.name,
                );
            }
            LayerExportType::U8 => {
                let actual = provider
                    .sample_layer_u8(2048, 2048, schema.name)
                    .expect("known wilderness u8 layer should return default");
                assert_eq!(
                    actual,
                    schema
                        .safe_default_u8
                        .expect("u8 schema should carry u8 default"),
                    "layer {}",
                    schema.name
                );
            }
        }
    }

    assert_eq!(
        provider.sample_layer(2048, 2048, "height"),
        None,
        "compatibility adapter should preserve missing-tile None semantics"
    );
    assert_eq!(
        provider.sample_layer(2048, 2048, "surface_id"),
        None,
        "compatibility adapter should preserve missing-tile None semantics"
    );
}

#[test]
fn sample_layer_out_of_tile_bounds_returns_schema_safe_defaults() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    assert_eq!(provider.sample_layer_f32(2, 0, "height"), Some(0.0));
    assert_eq!(provider.sample_layer_u8(2, 0, "surface_id"), Some(0));
    assert_eq!(provider.sample_layer(2, 0, "height"), None);
    assert_eq!(provider.sample_layer(2, 0, "surface_id"), None);
}

#[test]
fn sample_layer_compatibility_adapter_exposes_both_export_types() {
    let fixture = build_fixture();
    let provider = fixture.provider();

    let surface_index = TerrainProvider::layer_names()
        .iter()
        .position(|schema| schema.name == "surface_id")
        .expect("surface_id schema should exist");

    // worldgen-v4 P0 §8.1 #1: "height" now resolves to the surface span's
    // ceiling (FIXTURE_SURFACE_Y), not a standalone height.bin value.
    assert_eq!(
        provider.sample_layer(1, 1, "height"),
        Some(f32::from(FIXTURE_SURFACE_Y)),
        "sample_layer(\"height\") should return the span ceiling now that \
             height.bin is folded into spans"
    );
    assert_eq!(
        provider.sample_layer(1, 1, "surface_id"),
        Some(f32::from(test_u8_value(surface_index)))
    );
}

// -----------------------------------------------------------------------
// worldgen-v4 P0 §8.1 #1 — RasterManifest version validation
// -----------------------------------------------------------------------

#[test]
fn manifest_version_accepts_expected_and_rejects_others() {
    let path = Path::new("/tmp/manifest.json");
    // The current span encoding (v2) is accepted.
    assert!(
        validate_manifest_version(EXPECTED_RASTER_MANIFEST_VERSION, path).is_ok(),
        "v{EXPECTED_RASTER_MANIFEST_VERSION} (span encoding) must load"
    );
    // A pre-span v1 manifest (height.bin, no spans.bin) must be rejected so
    // the reader never mmaps the wrong layout.
    let err = validate_manifest_version(1, path)
        .expect_err("v1 (pre-span height.bin layout) must be rejected, not silently loaded");
    assert!(
        err.contains("unsupported version 1") && err.contains("v2"),
        "the error must name the bad version and the expected one for diagnosis; got: {err}"
    );
    // A future v3 is also rejected (forward-incompat is loud too).
    assert!(
        validate_manifest_version(3, path).is_err(),
        "an unknown future version must be rejected, not best-effort loaded"
    );
}

#[test]
fn manifest_missing_version_field_fails_to_parse() {
    // `version` has no serde default — a manifest that omits it (e.g. a
    // hand-edited or truncated file) must error at parse time rather than
    // defaulting to 0 and slipping past the version gate.
    let json = r#"{
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["minecraft:stone"],
            "biome_palette": ["minecraft:plains"],
            "tiles": []
        }"#;
    let parsed: Result<RasterManifest, _> = serde_json::from_str(json);
    assert!(
        parsed.is_err(),
        "a manifest with no `version` field must fail to deserialize (no default), \
             so a missing version can never be mistaken for a supported one"
    );
}

#[test]
fn bot_fixture_metadata_is_optional_and_validated_before_ready_use() {
    let base = r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["minecraft:stone"],
            "biome_palette": ["minecraft:plains"],
            "tiles": []
        }"#;
    let ordinary: RasterManifest =
        serde_json::from_str(base).expect("production manifest without bot_fixture must parse");
    assert!(
        validate_bot_fixture(ordinary.bot_fixture, Path::new("manifest.json"))
            .expect("absent fixture metadata must remain compatible")
            .is_none()
    );

    let with_fixture = base.replace(
            "\n        }",
            ",\n            \"bot_fixture\": {\"kind\":\"ambient-surface-v1\",\"token\":\"0123456789abcdef\",\"surface_y\":72,\"support\":\"grass_block\",\"feet_y\":73,\"head_y\":74}\n        }",
        );
    let fixture_manifest: RasterManifest =
        serde_json::from_str(&with_fixture).expect("valid bot_fixture metadata must deserialize");
    let fixture = validate_bot_fixture(fixture_manifest.bot_fixture, Path::new("manifest.json"))
        .expect("valid bot fixture must pass validation")
        .expect("fixture must remain present");
    assert_eq!(fixture.kind, "ambient-surface-v1");
    assert_eq!(fixture.token, "0123456789abcdef");

    let producer_fixture = serde_json::json!({
        "kind": "ambient-surface-v1",
        "token": "0123456789abcdef",
        "surface_y": 72,
        "support": "grass_block",
        "feet_y": 73,
        "head_y": 74,
    });
    for required_field in ["surface_y", "support", "feet_y", "head_y"] {
        let mut missing = producer_fixture.clone();
        missing
            .as_object_mut()
            .expect("fixture must be an object")
            .remove(required_field);
        let error = serde_json::from_value::<ManifestBotFixture>(missing)
            .expect_err("every producer evidence field must remain required");
        assert!(
            error.to_string().contains(required_field),
            "missing-field diagnostics must identify {required_field}: {error}"
        );
    }

    for (field, wrong_value) in [
        ("surface_y", serde_json::json!("72")),
        ("support", serde_json::json!(72)),
        ("feet_y", serde_json::json!("73")),
        ("head_y", serde_json::json!("74")),
    ] {
        let mut wrong_type = producer_fixture.clone();
        wrong_type[field] = wrong_value;
        let error = serde_json::from_value::<ManifestBotFixture>(wrong_type)
            .expect_err("producer evidence fields must retain their JSON types");
        assert!(
            error.to_string().contains("invalid type"),
            "wrong-type diagnostics for {field} must explain the type mismatch: {error}"
        );
    }

    let mut unknown = producer_fixture.clone();
    unknown["future_unreviewed_field"] = serde_json::json!(true);
    let error = serde_json::from_value::<ManifestBotFixture>(unknown)
        .expect_err("unreviewed nested fixture fields must fail closed");
    assert!(error.to_string().contains("future_unreviewed_field"));

    for (kind, token) in [
        ("other", "0123456789abcdef"),
        ("ambient-surface-v1", "short"),
        ("ambient-surface-v1", "0123456789abcde\n"),
        ("ambient-surface-v1", "0123456789abcde!"),
    ] {
        let fixture: ManifestBotFixture = serde_json::from_value(serde_json::json!({
            "kind": kind,
            "token": token,
            "surface_y": 72,
            "support": "grass_block",
            "feet_y": 73,
            "head_y": 74,
        }))
        .expect("invalid kind/token must still satisfy the strict producer schema");
        assert!(
                validate_bot_fixture(Some(fixture), Path::new("manifest.json")).is_err(),
                "invalid fixture kind/token must fail before any ready marker: kind={kind:?} token={token:?}"
            );
    }
}

#[test]
fn manifest_novice_poi_coordinates_survive_deserialize_and_runtime_mapping() {
    let json = r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["minecraft:stone"],
            "biome_palette": ["minecraft:plains"],
            "tiles": [],
            "pois": [
                {
                    "zone": "spawn",
                    "kind": "novice_forge_station",
                    "name": "破败炼器台",
                    "pos_xyz": [224.0, 71.0, -240.0],
                    "tags": ["poi_novice", "poi_type:forge_station", "selection:strict_radius_1500"]
                },
                {
                    "zone": "spawn",
                    "kind": "novice_alchemy_furnace",
                    "name": "凡铁丹炉",
                    "pos_xyz": [0.0, 72.0, -200.0],
                    "tags": ["poi_novice", "poi_type:alchemy_furnace", "selection:relaxed_radius_2000"]
                },
                {
                    "zone": "spawn",
                    "kind": "novice_scroll_hidden",
                    "name": "残卷藏匿点",
                    "pos_xyz": [176.0, 72.0, -96.0],
                    "tags": ["poi_novice", "poi_type:scroll_hidden", "selection:strict_radius_1500"]
                }
            ]
        }"#;

    let manifest: RasterManifest =
        serde_json::from_str(json).expect("valid raster manifest fixture");
    let pois = manifest_pois_into_runtime(manifest.pois);

    assert_eq!(pois.len(), 3);
    assert_eq!(pois[0].pos_xyz, [224.0, 71.0, -240.0]);
    assert_eq!(pois[1].pos_xyz, [0.0, 72.0, -200.0]);
    assert_eq!(pois[2].pos_xyz, [176.0, 72.0, -96.0]);
    assert!(pois
        .iter()
        .all(|poi| poi.tags.iter().any(|tag| tag == "poi_novice")));
}

#[test]
fn production_manifest_metadata_is_known_but_future_fields_fail_closed() {
    let mut json = serde_json::json!({
        "version": 2,
        "backend": "raster",
        "world_name": "test_world",
        "tile_size": 1,
        "spans_encoding": {"max_spans": 4},
        "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
        "surface_palette": ["stone"],
        "biome_palette": ["plains"],
        "tiles": [{
            "tile_x": 0,
            "tile_z": 0,
            "dir": "tile_0_0",
            "zones": ["spawn"],
            "layers": [],
            "spans": {"count_file": "spans_count.bin"}
        }],
        "pois": [],
        "zones": [],
        "collapsed_zones": [],
        "semantic_layers": {},
        "structure_layers": {},
        "vertical_layers": {},
        "profiles_ecology": {},
        "qi_density_source": {},
        "qi_budget_report": {},
        "anomaly_kinds": {},
        "abyssal_tier_floor_y": {},
        "ascension_pits": [],
        "corpse_mounds": [],
        "global_decoration_palette": [{
            "global_id": 1,
            "profile": "test",
            "local_id": 1,
            "name": "test",
            "kind": "test",
            "blocks": ["stone"],
            "size_range": [1, 1],
            "rarity": 1.0,
            "notes": "",
            "nbt_templates": [],
            "anchor": "ground"
        }],
        "fossil_bboxes": [{
            "zone": "spawn",
            "name": "test",
            "center_xz": [0, 0],
            "center_y": 0,
            "min_x": 0,
            "max_x": 0,
            "min_z": 0,
            "max_z": 0,
            "max_units": 1,
            "mask_values": {"outer": 1},
            "minerals": {"outer": ["test"]}
        }],
        "notes": {},
    });

    serde_json::from_value::<RasterManifest>(json.clone())
        .expect("every field emitted by the production raster exporter must remain admitted");

    json["future_unreviewed_field"] = serde_json::json!(true);
    let error = serde_json::from_value::<RasterManifest>(json)
        .expect_err("an unreviewed producer field must fail closed instead of being ignored");
    assert!(
        error.to_string().contains("future_unreviewed_field"),
        "unknown-field diagnostics must identify the producer key: {error}"
    );
}

#[test]
fn manifest_with_version_field_parses() {
    // Sanity: the same shape WITH version=2 parses, proving the field is the
    // only thing the previous case was missing.
    let json = r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["minecraft:stone"],
            "biome_palette": ["minecraft:plains"],
            "tiles": []
        }"#;
    let manifest: RasterManifest =
        serde_json::from_str(json).expect("a v2 manifest with all required fields must parse");
    assert_eq!(manifest.version, 2);
    assert_eq!(manifest.tile_size, 1);
}

// -----------------------------------------------------------------------
// worldgen-v4 P6 §8.1 — ManifestDecoration nbt_templates / anchor contract.
// Dual-pinned against the Python exporter (profiles/base.py decoration_payload
// emits "nbt_templates" + "anchor"). Changing either side must break a test.
// -----------------------------------------------------------------------

#[test]
fn manifest_decoration_with_nbt_fields_deserializes() {
    // The exact shape the Python `decoration_payload` emits for an NBT-driven
    // decoration: a list of template paths plus an anchor string.
    let json = r#"{
            "global_id": 5,
            "profile": "qingyun_peaks",
            "local_id": 2,
            "name": "ling_yu_tree",
            "kind": "tree",
            "blocks": ["minecraft:oak_log", "minecraft:oak_leaves"],
            "size_range": [4, 8],
            "rarity": 0.5,
            "notes": "灵峰玉树",
            "nbt_templates": ["decorations/tree/ling_yu_tree_v1.nbt", "decorations/tree/ling_yu_tree_v2.nbt"],
            "anchor": "hanging"
        }"#;
    let deco: ManifestDecoration =
        serde_json::from_str(json).expect("NBT-driven decoration must deserialize");
    assert_eq!(
        deco.nbt_templates,
        vec![
            "decorations/tree/ling_yu_tree_v1.nbt".to_string(),
            "decorations/tree/ling_yu_tree_v2.nbt".to_string(),
        ],
        "nbt_templates must round-trip the authored variant path list in order"
    );
    assert_eq!(
        deco.anchor, "hanging",
        "anchor string must round-trip verbatim before it is lowered to the enum"
    );
    // The string lowers to the typed enum the runtime stamp uses.
    assert_eq!(
        DecorationAnchor::from_manifest(&deco.anchor),
        DecorationAnchor::Hanging,
        "manifest anchor 'hanging' must lower to DecorationAnchor::Hanging"
    );
}

#[test]
fn manifest_decoration_without_nbt_fields_defaults_to_procedural_ground() {
    // Backward compat: a pre-P6 manifest decoration carries no nbt_templates
    // and no anchor. It must deserialize into the procedural path (empty
    // templates) anchored at Ground — never a parse error, never a panic.
    let json = r#"{
            "global_id": 1,
            "profile": "wilderness",
            "local_id": 1,
            "name": "wild_grass",
            "kind": "flower",
            "blocks": ["grass"],
            "size_range": [1, 1],
            "rarity": 0.65,
            "notes": "野草"
        }"#;
    let deco: ManifestDecoration =
        serde_json::from_str(json).expect("legacy decoration without NBT fields must parse");
    assert!(
        deco.nbt_templates.is_empty(),
        "absent nbt_templates must default to empty (stays on the procedural path), got {:?}",
        deco.nbt_templates
    );
    assert_eq!(
        deco.anchor, "",
        "absent anchor defaults to empty string at the manifest layer"
    );
    assert_eq!(
        DecorationAnchor::from_manifest(&deco.anchor),
        DecorationAnchor::Ground,
        "an empty anchor must lower to the Ground default so legacy specs still place"
    );
}

#[test]
fn resolve_surface_palette_aggregates_every_invalid_entry() {
    let names = vec![
        "stone".to_string(),
        "unknown_surface_one".to_string(),
        "minecraft:dirt".to_string(),
        "unknown_surface_two".to_string(),
    ];
    let manifest_path = Path::new("/tmp/test-manifest.json");

    let error = resolve_surface_palette(&names, manifest_path)
        .expect_err("all unsupported surface keys must reject manifest admission");
    assert!(error.contains("test-manifest.json"));
    assert!(error.contains("surface_palette #2"));
    assert!(error.contains("unknown_surface_one"));
    assert!(error.contains("surface_palette #3"));
    assert!(error.contains("minecraft:dirt"));
    assert!(error.contains("surface_palette #4"));
    assert!(error.contains("unknown_surface_two"));
    assert_eq!(
        error.matches("unknown surface palette block").count(),
        3,
        "one valid key must not hide or multiply the three invalid diagnostics: {error}"
    );
}

#[test]
fn resolve_decoration_palette_aggregates_ids_blocks_and_empty_entries() {
    fn decoration(global_id: u32, name: &str, blocks: &[&str]) -> ManifestDecoration {
        ManifestDecoration {
            global_id,
            profile: "test".into(),
            local_id: 1,
            name: name.into(),
            kind: "test".into(),
            blocks: blocks.iter().map(|block| (*block).to_string()).collect(),
            size_range: [1, 1],
            rarity: 1.0,
            notes: String::new(),
            nbt_templates: Vec::new(),
            anchor: String::new(),
        }
    }

    let error = resolve_decoration_palette(
        vec![
            decoration(0, "zero", &["stone"]),
            decoration(1, "first", &["unknown_deco_one", "unknown_deco_two"]),
            decoration(1, "duplicate", &["stone"]),
            decoration(2, "empty", &[]),
            decoration(256, "too_large", &["stone"]),
        ],
        Path::new("/tmp/test-manifest.json"),
    )
    .expect_err("every invalid decoration entry must reject admission atomically");

    assert!(error.contains("zero") && error.contains("invalid global_id 0"));
    assert!(error.contains("too_large") && error.contains("invalid global_id 256"));
    assert!(error.contains("duplicate decoration global_id 1"));
    assert!(error.contains("unknown_deco_one"));
    assert!(error.contains("unknown_deco_two"));
    assert!(error.contains("empty") && error.contains("at least one procedural block"));
    assert_eq!(
        error.matches("unknown surface palette block").count(),
        2,
        "both invalid block keys must be diagnosed without returning a partial palette: {error}"
    );
}

fn write_required_raster_tile(tile_dir: &Path, surface_ids: &[u8], subsurface_ids: &[u8]) {
    assert_eq!(
        surface_ids.len(),
        subsurface_ids.len(),
        "surface and subsurface fixtures must describe the same tile area"
    );
    fs::create_dir_all(tile_dir).expect("raster tile fixture directory should be creatable");
    write_spans_single_fixture(tile_dir, surface_ids.len(), FIXTURE_SURFACE_Y);
    fs::write(tile_dir.join("surface_id.bin"), surface_ids)
        .expect("surface id fixture should be writable");
    fs::write(tile_dir.join("subsurface_id.bin"), subsurface_ids)
        .expect("subsurface id fixture should be writable");
    write_u8_layer(&tile_dir.join("biome_id.bin"), 0, surface_ids.len());
    write_f32_layer(&tile_dir.join("water_level.bin"), -1.0, surface_ids.len());
    write_f32_layer(&tile_dir.join("feature_mask.bin"), 0.0, surface_ids.len());
    write_f32_layer(
        &tile_dir.join("boundary_weight.bin"),
        0.0,
        surface_ids.len(),
    );
}

fn write_loadable_raster_manifest(path: &Path, surface_palette: &[&str], tile_dir: &str) {
    let palette = surface_palette
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        path,
        format!(
            r#"{{
                    "version": 2,
                    "tile_size": 2,
                    "world_bounds": {{"min_x":0,"max_x":1,"min_z":0,"max_z":1}},
                    "surface_palette": [{palette}],
                    "biome_palette": ["plains"],
                    "tiles": [{{
                        "tile_x": 0,
                        "tile_z": 0,
                        "dir": "{tile_dir}",
                        "layers": []
                    }}]
                }}"#
        ),
    )
    .expect("raster manifest fixture should be writable");
}

#[test]
fn load_preflighted_rejects_empty_biome_palette_and_invalid_tile_sizes() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    for (tile_size, expected) in [
        ("0", "tile_size must be positive"),
        ("-1", "tile_size must be positive"),
        ("2147483647", "tile_size squared overflowed"),
    ] {
        fs::write(
            &manifest_path,
            format!(
                r#"{{
                "version":2,"tile_size":{tile_size},
                "world_bounds":{{"min_x":0,"max_x":0,"min_z":0,"max_z":0}},
                "surface_palette":["stone"],"biome_palette":["plains"],"tiles":[]
            }}"#
            ),
        )
        .unwrap();
        let error = TerrainProvider::load_preflighted(
            &manifest_path,
            &root,
            &test_biomes(),
            &super::super::nbt_registry::DecorationNbtRegistry::empty(),
        )
        .expect_err("invalid tile_size must reject admission");
        assert!(error.to_string().contains(expected), "{error}");
    }
    fs::write(
        &manifest_path,
        r#"{
            "version":2,"tile_size":1,
            "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette":["stone"],"biome_palette":[],"tiles":[]
        }"#,
    )
    .unwrap();
    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("empty biome palette must reject admission");
    assert!(
        error.to_string().contains("biome palette cannot be empty"),
        "{error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_preflighted_rejects_empty_surface_palette() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("empty-palette fixture root should be creatable");
    let manifest_path = root.join("manifest.json");
    write_loadable_raster_manifest(&manifest_path, &[], "tile_0_0");

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("an empty surface palette must fail before runtime sampling");
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic == "manifest: surface palette cannot be empty"),
        "empty-palette admission must be explicit: {error}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_preflighted_aggregates_every_surface_and_subsurface_palette_id_overflow() {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    write_required_raster_tile(&tile_dir, &[0, 2, 3, 0], &[4, 0, 5, 0]);
    let manifest_path = root.join("manifest.json");
    write_loadable_raster_manifest(&manifest_path, &["stone", "dirt"], "tile_0_0");

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("every palette id outside [0, palette_len) must reject provider admission");
    let palette_diagnostics = error
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.contains("surface palette length 2"))
        .collect::<Vec<_>>();
    assert_eq!(
        palette_diagnostics.len(),
        2,
        "surface and subsurface failures must each produce one bounded summary: {error}"
    );
    for expected in [
            "surface_id has 2 ids outside surface palette length 2; first 2: index 1 has palette id 2, index 2 has palette id 3",
            "subsurface_id has 2 ids outside surface palette length 2; first 2: index 0 has palette id 4, index 2 has palette id 5",
        ] {
            assert!(
                palette_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains(expected)
                        && diagnostic.contains("tile (0,0) 'tile_0_0'")),
                "bounded palette diagnostic must identify layer, count, examples, and tile for {expected:?}: {error}"
            );
        }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_preflighted_aggregates_every_unknown_biome() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],
                "biome_palette":["missing_one","missing_two"],
                "tiles":[]
            }"#,
    )
    .unwrap();

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("every unknown biome must reject aggregate preflight");
    assert!(error.to_string().contains("biome_palette #1 'missing_one'"));
    assert!(error.to_string().contains("biome_palette #2 'missing_two'"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_preflighted_rejects_sparse_decoration_palette_references() {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    write_required_raster_tile(&tile_dir, &[0], &[0]);
    fs::write(tile_dir.join("flora_variant_id.bin"), [2]).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
            &manifest_path,
            r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],"biome_palette":["plains"],
                "global_decoration_palette":[
                    {"global_id":1,"profile":"test","local_id":1,"name":"one","kind":"flower","blocks":["grass"],"size_range":[1,1],"rarity":1.0,"notes":"","nbt_templates":[],"anchor":"ground"},
                    {"global_id":3,"profile":"test","local_id":3,"name":"three","kind":"flower","blocks":["poppy"],"size_range":[1,1],"rarity":1.0,"notes":"","nbt_templates":[],"anchor":"ground"}
                ],
                "tiles":[{"tile_x":0,"tile_z":0,"dir":"tile_0_0","layers":["flora_variant_id"]}]
            }"#,
        )
        .unwrap();

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("a raster id naming an empty sparse palette slot must fail");
    assert!(error.to_string().contains("unoccupied decoration id 2"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_preflighted_rejects_biome_flora_and_ground_cover_palette_overflow() {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    write_required_raster_tile(&tile_dir, &[0, 0, 0, 0], &[0, 0, 0, 0]);
    fs::write(tile_dir.join("biome_id.bin"), [0, 1, 0, 0]).unwrap();
    fs::write(tile_dir.join("flora_variant_id.bin"), [0, 0, 2, 0]).unwrap();
    fs::write(tile_dir.join("ground_cover_id.bin"), [0, 0, 0, 3]).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
            "version":2,"tile_size":2,
            "world_bounds":{"min_x":0,"max_x":1,"min_z":0,"max_z":1},
            "surface_palette":["stone"],"biome_palette":["plains"],
            "global_decoration_palette":[{
                "global_id":1,"profile":"test","local_id":1,"name":"grass",
                "kind":"flower","blocks":["grass"],"size_range":[1,1],
                "rarity":1.0,"notes":"","nbt_templates":[],"anchor":"ground"
            }],
            "tiles":[{"tile_x":0,"tile_z":0,"dir":"tile_0_0",
                      "layers":["flora_variant_id","ground_cover_id"]}]
        }"#,
    )
    .unwrap();
    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("all palette-indexed raster layers must reject invalid foreign keys");
    for expected in ["biome_id", "flora_variant_id", "ground_cover_id"] {
        assert!(
            error.to_string().contains(expected),
            "missing {expected}: {error}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn raster_layer_is_an_immutable_startup_snapshot() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).unwrap();
    let path = root.join("layer.bin");
    fs::write(&path, [1_u8, 2, 3, 4]).unwrap();
    let snapshot = map_file(&path, 4).expect("valid layer must snapshot");
    let replacement = root.join("replacement.bin");
    fs::write(&replacement, [9_u8, 9, 9, 9]).unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert_eq!(
        &snapshot[..],
        &[1, 2, 3, 4],
        "post-preflight disk mutation must not change admitted bytes"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn raster_layer_rejects_short_file_at_preflight() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).unwrap();
    let path = root.join("layer.bin");
    fs::write(&path, [1_u8, 2, 3]).unwrap();
    let error = map_file(&path, 4).expect_err("short raster layer must be rejected at preflight");
    assert!(
        error.contains("has 3 bytes, expected 4"),
        "expected length-mismatch diagnostic, got: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn raster_layer_rejects_file_mutated_while_being_mapped() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).unwrap();
    let path = root.join("layer.bin");
    fs::write(&path, [1_u8, 2, 3, 4]).unwrap();
    let hook_path = path.clone();
    set_map_open_file_after_mmap_test_hook(move |_file| {
        use std::fs::OpenOptions;
        use std::io::Write;
        let mut writer =
            std::io::BufWriter::new(OpenOptions::new().write(true).open(&hook_path).unwrap());
        writer.write_all(&[5_u8, 6, 7, 8, 9, 10, 11, 12]).unwrap();
        writer.flush().unwrap();
    });
    let error = map_file(&path, 4).expect_err("in-place mutation during map must be rejected");
    assert!(
        error.contains("changed while being mapped"),
        "expected stability diagnostic, got: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn load_preflighted_rejects_tile_paths_outside_raster_root() {
    use std::os::unix::fs::symlink;

    let root = unique_temp_dir();
    let outside = unique_temp_dir();
    write_required_raster_tile(&outside, &[0], &[0]);
    fs::create_dir_all(&root).unwrap();
    symlink(&outside, root.join("escaped_tile")).unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],"biome_palette":["plains"],
                "tiles":[{"tile_x":0,"tile_z":0,"dir":"escaped_tile","layers":[]}]
            }"#,
    )
    .unwrap();

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("symlinked tile ancestor must not escape the raster root");
    assert!(error.to_string().contains("outside trusted root"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(unix)]
#[test]
fn load_preflighted_rejects_optional_layer_file_outside_raster_root() {
    use std::os::unix::fs::symlink;

    let root = unique_temp_dir();
    let outside = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).unwrap();
    fs::create_dir_all(&outside).unwrap();
    write_required_raster_tile(&tile_dir, &[0], &[0]);
    fs::write(outside.join("rift_axis_sdf.bin"), [0_u8; 4]).unwrap();
    symlink(
        outside.join("rift_axis_sdf.bin"),
        tile_dir.join("rift_axis_sdf.bin"),
    )
    .unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],"biome_palette":["plains"],
                "tiles":[{"tile_x":0,"tile_z":0,"dir":"tile_0_0","layers":["rift_axis_sdf"]}]
            }"#,
    )
    .unwrap();

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("optional raster symlink outside the root must fail closed");
    assert!(
        error.to_string().contains("outside trusted root"),
        "optional layer escape must preserve trusted-root diagnostics: {error}"
    );
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn load_preflighted_rejects_unknown_manifest_layer_names() {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    write_required_raster_tile(&tile_dir, &[0], &[0]);
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],"biome_palette":["plains"],
                "tiles":[{"tile_x":0,"tile_z":0,"dir":"tile_0_0","layers":["flroa_density"]}]
            }"#,
    )
    .unwrap();

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("unknown manifest layer names must reject provider admission");
    assert!(
        error.to_string().contains("unknown layer 'flroa_density'"),
        "unknown layer diagnostics must name the exact producer typo: {error}"
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn load_preflighted_rejects_optional_layer_reparse_escape() {
    // Windows reparse-point coverage is exercised by the same loader path;
    // the no-follow handle admission must reject an external target.
    let root = unique_temp_dir();
    let outside = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).unwrap();
    write_required_raster_tile(&tile_dir, &[0], &[0]);
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("rift_axis_sdf.bin"), [0_u8; 4]).unwrap();
    std::os::windows::fs::symlink_file(
        outside.join("rift_axis_sdf.bin"),
        tile_dir.join("rift_axis_sdf.bin"),
    )
    .unwrap();
    let manifest_path = root.join("manifest.json");
    fs::write(
        &manifest_path,
        r#"{
                "version":2,"tile_size":1,
                "world_bounds":{"min_x":0,"max_x":0,"min_z":0,"max_z":0},
                "surface_palette":["stone"],"biome_palette":["plains"],
                "tiles":[{"tile_x":0,"tile_z":0,"dir":"tile_0_0","layers":["rift_axis_sdf"]}]
            }"#,
    )
    .unwrap();
    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("Windows reparse-point layer escape must fail closed");
    assert!(error.to_string().contains("outside trusted root"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn load_preflighted_accepts_surface_ids_at_the_upper_valid_boundary() {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    write_required_raster_tile(&tile_dir, &[0, 1, 1, 0], &[1, 0, 1, 0]);
    let manifest_path = root.join("manifest.json");
    write_loadable_raster_manifest(&manifest_path, &["stone", "dirt"], "tile_0_0");

    let provider = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect("palette id == palette_len - 1 is valid for both raster layers");
    assert_eq!(provider.sample(1, 0).surface_block, BlockState::DIRT);
    assert_eq!(provider.sample(0, 0).subsurface_block, BlockState::DIRT);
    drop(provider);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_preflighted_aggregates_surface_decoration_template_and_placement_errors() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("cross-source preflight root should be creatable");
    let manifest_path = root.join("manifest.json");
    fs::write(&manifest_path, invalid_cross_source_manifest())
        .expect("cross-source manifest should be writable");
    fs::write(
        root.join("placement_manifest.json"),
        r#"{
                "version": 1,
                "structures": [{
                    "nbt_path": "test.nbt",
                    "origin": [0, 64, 0],
                    "rotation": 0,
                    "blocks": [
                        {"pos":[0,64,0],"block":"unknown_placement_for_preflight"},
                        {"pos":[1,64,0],"block":"oak_log","properties":{"axis":"north"}}
                    ]
                }]
            }"#,
    )
    .expect("invalid placement fixture should be writable");

    let error = TerrainProvider::load_preflighted(
        &manifest_path,
        &root,
        &test_biomes(),
        &super::super::nbt_registry::DecorationNbtRegistry::empty(),
    )
    .expect_err("all four invalid authored sources must reject provider construction");
    let diagnostics = error.diagnostics();
    assert!(diagnostics.windows(2).all(|pair| pair[0] <= pair[1]));
    for expected in [
        "surface: ",
        "decoration: ",
        "nbt-reference: ",
        "placement: ",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.starts_with(expected)),
            "cross-source preflight must report the {expected:?} source: {error}"
        );
    }
    for authored_value in [
        "unknown_surface_for_preflight",
        "unknown_decoration_for_preflight",
        "../escape.nbt",
        "decorations/test/missing.nbt",
        "unknown_placement_for_preflight",
        "axis",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains(authored_value)),
            "diagnostics must identify invalid authored value {authored_value:?}: {error}"
        );
    }
    assert_eq!(
        diagnostics,
        TerrainProvider::load_preflighted(
            &manifest_path,
            &root,
            &test_biomes(),
            &super::super::nbt_registry::DecorationNbtRegistry::empty(),
        )
        .expect_err("repeated preflight must remain fatal")
        .diagnostics(),
        "startup diagnostics must be deterministic across repeated preflights"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn decoration_palette_helper_lowers_nbt_fields() {
    // A manifest decoration carrying NBT fields lowers them into the runtime
    // Decoration while preserving the authored template order and anchor.
    // templates + lowered anchor, and `is_nbt_driven()` must agree.
    let json = r#"{
            "version": 2,
            "tile_size": 1,
            "world_bounds": {"min_x":0,"max_x":0,"min_z":0,"max_z":0},
            "surface_palette": ["minecraft:stone"],
            "biome_palette": ["minecraft:plains"],
            "tiles": [],
            "global_decoration_palette": [
                {
                    "global_id": 1,
                    "profile": "rift_valley",
                    "local_id": 1,
                    "name": "grave_mound",
                    "kind": "grave_mound",
                    "blocks": ["dirt", "mossy_cobblestone", "oak_sign"],
                    "size_range": [2, 4],
                    "rarity": 0.4,
                    "notes": "荒冢",
                    "nbt_templates": ["decorations/grave_mound/grave_mound_v1.nbt"],
                    "anchor": "embedded"
                }
            ]
        }"#;
    let manifest: RasterManifest =
        serde_json::from_str(json).expect("manifest with NBT-driven decoration must parse");
    let mut palette = resolve_decoration_palette(
        manifest.global_decoration_palette,
        Path::new("/tmp/test-manifest.json"),
    )
    .expect("valid NBT-driven decoration palette must lower");
    let deco = palette[1]
        .take()
        .expect("global_id 1 must occupy palette slot 1");
    assert!(
        deco.is_nbt_driven(),
        "a decoration with a template path must report is_nbt_driven()"
    );
    assert_eq!(
        deco.nbt_templates,
        vec!["decorations/grave_mound/grave_mound_v1.nbt".to_string()],
        "the public Decoration must carry the manifest's template path list"
    );
    assert_eq!(
        deco.resolved_blocks,
        vec![
            BlockState::DIRT,
            BlockState::MOSSY_COBBLESTONE,
            BlockState::OAK_SIGN
        ],
        "manifest block ordering must lower unchanged into the procedural consumer palette"
    );
    assert_eq!(
            deco.anchor,
            DecorationAnchor::Embedded,
            "manifest anchor 'embedded' must lower to DecorationAnchor::Embedded on the public Decoration"
        );
}

#[test]
fn decoration_palette_rejects_explicit_invalid_anchor() {
    let raw = ManifestDecoration {
        global_id: 1,
        profile: "test".into(),
        local_id: 1,
        name: "bad_anchor".into(),
        kind: "crystal".into(),
        blocks: vec!["stone".into()],
        size_range: [1, 1],
        rarity: 1.0,
        notes: String::new(),
        nbt_templates: Vec::new(),
        anchor: "hangng".into(),
    };
    let error = resolve_decoration_palette(vec![raw], Path::new("manifest.json"))
        .expect_err("explicit invalid anchors must not become ground placements");
    assert!(error.contains("invalid anchor 'hangng'"), "{error}");
}

#[test]
fn decoration_without_templates_is_not_nbt_driven() {
    let deco = Decoration {
        global_id: 1,
        profile: "wilderness".into(),
        local_id: 1,
        name: "wild_grass".into(),
        kind: "flower".into(),
        blocks: vec!["grass".into()],
        resolved_blocks: vec![BlockState::GRASS],
        size_range: [1, 1],
        rarity: 0.65,
        notes: String::new(),
        nbt_templates: vec![],
        anchor: DecorationAnchor::Ground,
    };
    assert!(
        !deco.is_nbt_driven(),
        "a decoration with no templates must stay procedural (is_nbt_driven() == false)"
    );
}

#[test]
fn decoration_template_preflight_aggregates_missing_and_invalid_ids() {
    let provider = TerrainProvider {
        decoration_palette: vec![
            None,
            Some(Decoration {
                global_id: 1,
                profile: "spawn".into(),
                local_id: 1,
                name: "broken_tree".into(),
                kind: "tree".into(),
                blocks: vec!["oak_log".into()],
                resolved_blocks: vec![BlockState::OAK_LOG],
                size_range: [1, 2],
                rarity: 0.2,
                notes: String::new(),
                nbt_templates: vec![
                    "../escape.nbt".into(),
                    "decorations/tree/missing.nbt".into(),
                ],
                anchor: DecorationAnchor::Ground,
            }),
        ],
        ..TerrainProvider::empty_for_tests()
    };
    let diagnostics = provider
        .validate_decoration_templates(&super::super::nbt_registry::DecorationNbtRegistry::empty())
        .expect_err("both malformed and dangling template ids must reject startup");
    assert_eq!(
        diagnostics.len(),
        2,
        "all template reference errors aggregate"
    );
    assert!(diagnostics
        .iter()
        .any(|d| d.contains("../escape.nbt") && d.contains("invalid")));
    assert!(diagnostics
        .iter()
        .any(|d| d.contains("missing.nbt") && d.contains("missing resident")));
}

// -----------------------------------------------------------------------
// P1 — PlacementManifest serde contract tests (断链 #2,
//       plan-terrain-wiring-v1 §P1 "契约对拍" requirement)
// -----------------------------------------------------------------------

/// Fixture JSON matches worldgen export_placement_manifest format exactly.
/// This is the dual-pin test: changing either side must break this.
#[test]
fn placement_manifest_fixture_deserialises_correctly() {
    let manifest: PlacementManifest =
        serde_json::from_str(include_str!("placement_manifest_fixture.json"))
            .expect("placement_manifest_fixture.json must be valid PlacementManifest JSON");

    assert_eq!(manifest.version, 1, "manifest version should be 1");
    assert_eq!(
        manifest.structures.len(),
        2,
        "fixture should contain exactly 2 structures"
    );

    let s0 = &manifest.structures[0];
    assert_eq!(s0.nbt_path, "server/structures/dan_zong/great_hall.nbt");
    assert_eq!(s0.origin, [128, 82, 256]);
    assert_eq!(s0.rotation, 0);
    assert_eq!(s0.blocks.len(), 5);

    // First block: no properties
    let b0 = &s0.blocks[0];
    assert_eq!(b0.pos, [128, 82, 256]);
    assert_eq!(b0.block, "minecraft:stone_bricks");
    assert!(b0.properties.is_empty(), "first block has no properties");

    // Fourth block: has 'moisture' property
    let b3 = &s0.blocks[3];
    assert_eq!(b3.block, "minecraft:farmland");
    assert_eq!(b3.properties.get("moisture"), Some(&"7".to_string()));

    // stamp_radial structure
    let s1 = &manifest.structures[1];
    assert!(
        s1.nbt_path.starts_with("<stamp_radial:"),
        "second structure is a stamp_radial"
    );
    assert_eq!(s1.rotation, 90);
}

/// PlacementManifest with empty structures array must deserialise fine.
#[test]
fn placement_manifest_empty_structures_deserialises_ok() {
    let json = r#"{"version":1,"structures":[]}"#;
    let pm: PlacementManifest =
        serde_json::from_str(json).expect("empty structures array must deserialise");
    assert_eq!(pm.structures.len(), 0);
}

/// PlacementBlock without "properties" key must default to empty map.
#[test]
fn placement_block_missing_properties_defaults_to_empty_map() {
    let json = r#"{"version":1,"structures":[{"nbt_path":"x","origin":[0,0,0],"rotation":0,"blocks":[{"pos":[1,2,3],"block":"minecraft:stone"}]}]}"#;
    let pm: PlacementManifest = serde_json::from_str(json)
        .expect("placement manifest missing properties key must deserialise");
    let b = &pm.structures[0].blocks[0];
    assert!(
        b.properties.is_empty(),
        "missing properties key must default to empty HashMap"
    );
}

// ----- block_state_from_placement -------------------------------------------

/// Known bare and namespaced block names resolve through the same strict path.
#[test]
fn block_state_from_placement_resolves_known_names() {
    let bare = block_state_from_placement("stone_bricks", &HashMap::new())
        .expect("bare stone_bricks must resolve");
    let namespaced = block_state_from_placement("minecraft:stone_bricks", &HashMap::new())
        .expect("minecraft: prefix must be accepted once");
    assert_eq!(bare, namespaced);
}

#[test]
fn block_state_from_placement_rejects_unknown_blocks_and_properties() {
    assert!(block_state_from_placement("minecraft:unknown_block_xyz", &HashMap::new()).is_err());

    let mut unknown_name = HashMap::new();
    unknown_name.insert("nonexistent_prop".to_string(), "x".to_string());
    assert!(block_state_from_placement("oak_log", &unknown_name).is_err());

    let mut unknown_value = HashMap::new();
    unknown_value.insert("axis".to_string(), "not_a_value".to_string());
    assert!(block_state_from_placement("oak_log", &unknown_value).is_err());

    let mut inapplicable = HashMap::new();
    inapplicable.insert("axis".to_string(), "x".to_string());
    assert!(block_state_from_placement("stone", &inapplicable).is_err());

    let mut invalid_for_property = HashMap::new();
    invalid_for_property.insert("axis".to_string(), "north".to_string());
    assert!(block_state_from_placement("oak_log", &invalid_for_property).is_err());
}

#[test]
fn block_state_from_placement_applies_valid_properties() {
    let mut props = HashMap::new();
    props.insert("axis".to_string(), "x".to_string());
    let with_axis_x =
        block_state_from_placement("oak_log", &props).expect("axis=x is valid for oak_log");
    let without_props =
        block_state_from_placement("oak_log", &HashMap::new()).expect("oak_log resolves");
    assert_ne!(with_axis_x, without_props);
}

#[test]
fn build_placement_index_buckets_blocks_atomically() {
    let manifest = PlacementManifest {
        version: 1,
        structures: vec![PlacementStructure {
            nbt_path: "test.nbt".to_string(),
            origin: [0, 64, 0],
            rotation: 0,
            blocks: vec![
                PlacementBlock {
                    pos: [0, 64, 0],
                    block: "stone_bricks".to_string(),
                    properties: HashMap::new(),
                },
                PlacementBlock {
                    pos: [16, 64, 0],
                    block: "stone_bricks".to_string(),
                    properties: HashMap::new(),
                },
            ],
        }],
    };

    let (index, total) = build_placement_index(manifest).expect("all blocks are valid");
    assert_eq!(total, 2);
    assert_eq!(index.len(), 2);
    assert_eq!(index[&ChunkPos::new(0, 0)].len(), 1);
    assert_eq!(index[&ChunkPos::new(1, 0)].len(), 1);
}

#[test]
fn build_placement_index_aggregates_invalid_blocks_without_partial_success() {
    let mut bad_property = HashMap::new();
    bad_property.insert("axis".to_string(), "north".to_string());
    let manifest = PlacementManifest {
        version: 1,
        structures: vec![PlacementStructure {
            nbt_path: "test.nbt".to_string(),
            origin: [0, 64, 0],
            rotation: 0,
            blocks: vec![
                PlacementBlock {
                    pos: [0, 64, 0],
                    block: "minecraft:stone_bricks".to_string(),
                    properties: HashMap::new(),
                },
                PlacementBlock {
                    pos: [1, 64, 0],
                    block: "minecraft:unknown_block_for_test".to_string(),
                    properties: HashMap::new(),
                },
                PlacementBlock {
                    pos: [2, 64, 0],
                    block: "minecraft:oak_log".to_string(),
                    properties: bad_property,
                },
            ],
        }],
    };

    let diagnostics = build_placement_index(manifest)
        .expect_err("one valid block must not hide two invalid authored blocks");
    assert_eq!(
        diagnostics.len(),
        2,
        "both invalid entries must be reported"
    );
    assert!(diagnostics[0].contains("unknown_block_for_test"));
    assert!(diagnostics[1].contains("axis"));
}

#[test]
fn load_placement_index_accepts_only_a_missing_sidecar_as_empty() {
    let (index, total) =
        load_placement_index(Path::new("/nonexistent/path/placement_manifest.json"))
            .expect("NotFound is the only backward-compatible sidecar case");
    assert!(index.is_empty());
    assert_eq!(total, 0);
}

#[test]
fn load_placement_index_rejects_malformed_schema_version_and_unknown_fields() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");

    fs::write(&path, b"not valid json { { {").expect("write malformed fixture");
    assert!(load_placement_index(&path).is_err());

    fs::write(&path, r#"{"version":2,"structures":[]}"#).expect("write version fixture");
    assert!(load_placement_index(&path).is_err());

    fs::write(&path, r#"{"version":1,"structures":[],"extra":true}"#)
        .expect("write unknown-field fixture");
    assert!(load_placement_index(&path).is_err());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_placement_index_rejects_invalid_utf8_directory_and_nested_unknown_fields() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");

    fs::write(&path, [0xff, 0xfe, 0xfd]).expect("write invalid utf-8 fixture");
    let utf8_error = load_placement_index(&path)
        .expect_err("present non-UTF-8 sidecar must fail instead of degrading to empty");
    assert!(utf8_error.contains("failed to read placement sidecar"));

    fs::remove_file(&path).expect("remove utf-8 fixture");
    fs::create_dir(&path).expect("create directory at sidecar path");
    let directory_error = load_placement_index(&path)
        .expect_err("a directory at the configured sidecar path must be fatal");
    assert!(directory_error.contains("must be a regular file"));

    fs::remove_dir(&path).expect("remove sidecar directory fixture");
    fs::write(
            &path,
            r#"{"version":1,"structures":[{"nbt_path":"x","origin":[0,0,0],"rotation":0,"blocks":[{"pos":[0,0,0],"block":"minecraft:stone","properties":{},"extra":true}]}]}"#,
        )
        .expect("write nested unknown-field fixture");
    let nested_error = load_placement_index(&path)
        .expect_err("unknown fields in nested placement blocks must fail schema admission");
    assert!(nested_error.contains("failed to parse placement sidecar"));
    assert!(nested_error.contains("unknown field `extra`"));

    fs::remove_file(&path).expect("remove block unknown-field fixture");
    fs::write(
            &path,
            r#"{"version":1,"structures":[{"nbt_path":"x","origin":[0,0,0],"rotation":0,"blocks":[],"extra":true}]}"#,
        )
        .expect("write structure unknown-field fixture");
    let structure_error = load_placement_index(&path)
        .expect_err("unknown fields in placement structures must fail schema admission");
    assert!(structure_error.contains("failed to parse placement sidecar"));
    assert!(structure_error.contains("unknown field `extra`"));

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn load_placement_index_rejects_dangling_symlink_and_special_node() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");

    symlink(root.join("missing-target.json"), &path)
        .expect("create dangling placement sidecar symlink");
    let symlink_error = load_placement_index(&path)
        .expect_err("a dangling sidecar symlink exists and must not use NotFound fallback");
    assert!(
        symlink_error.contains("without following symlinks")
            || symlink_error.to_ascii_lowercase().contains("symbolic link"),
        "dangling symlink diagnostic must explain the no-follow admission failure: {symlink_error}"
    );

    fs::remove_file(&path).expect("remove dangling symlink fixture");
    let listener = UnixListener::bind(&path).expect("bind special sidecar socket fixture");
    let special_error = load_placement_index(&path)
        .expect_err("a special sidecar node must fail before any blocking read is attempted");
    assert!(
        special_error.contains("must be a regular file") && special_error.contains("special node"),
        "special-node diagnostic must explain the file-type contract: {special_error}"
    );

    drop(listener);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn load_placement_index_rejects_fresh_symlink_and_fifo_inputs_without_blocking() {
    use std::os::unix::fs::symlink;
    use std::process::Command;

    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");
    let external = root.join("external.json");
    fs::write(&external, r#"{"version":1,"structures":[]}"#)
        .expect("write external sidecar target");
    symlink(&external, &path).expect("create placement sidecar symlink");

    let symlink_error = load_placement_index(&path)
        .expect_err("no-follow open must reject a symlink before reading its target");
    assert!(
        symlink_error.contains("without following symlinks")
            || symlink_error.to_ascii_lowercase().contains("symbolic link"),
        "fresh symlink admission must fail closed: {symlink_error}"
    );

    fs::remove_file(&path).expect("remove symlink fixture");
    let status = Command::new("mkfifo")
        .arg(&path)
        .status()
        .expect("run mkfifo for input fixture");
    assert!(status.success(), "mkfifo input fixture must succeed");
    let fifo_error = load_placement_index(&path)
        .expect_err("nonblocking no-follow open must reject a FIFO without waiting for a writer");
    assert!(
        fifo_error.contains("regular file") && fifo_error.contains("special node"),
        "fresh FIFO admission must identify the regular-file contract: {fifo_error}"
    );

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn load_placement_index_reads_the_opened_descriptor_after_path_replacement() {
    use std::os::unix::fs::symlink;

    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");
    let external = root.join("replacement.json");
    fs::write(&path, r#"{"version":1,"structures":[]}"#).expect("write original valid sidecar");
    fs::write(&external, b"replacement must not be read").expect("write replacement target");

    let hook_path = path.clone();
    let hook_external = external.clone();
    super::super::nbt_io::set_open_regular_file_after_open_test_hook(move || {
        fs::remove_file(&hook_path).expect("unlink opened sidecar path");
        symlink(&hook_external, &hook_path).expect("replace sidecar path with symlink");
    });

    let (index, total) = load_placement_index(&path)
        .expect("loader must read the already-open original file, not reopen the replacement");
    assert!(index.is_empty());
    assert_eq!(total, 0);
    assert!(
        fs::symlink_metadata(&path)
            .expect("replacement path should exist")
            .file_type()
            .is_symlink(),
        "test must actually replace the path after descriptor open"
    );

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn load_placement_index_reads_the_opened_descriptor_after_fifo_replacement() {
    use std::process::Command;

    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");
    fs::write(&path, r#"{"version":1,"structures":[]}"#).expect("write original valid sidecar");

    let hook_path = path.clone();
    super::super::nbt_io::set_open_regular_file_after_open_test_hook(move || {
        fs::remove_file(&hook_path).expect("unlink opened sidecar path");
        let status = Command::new("mkfifo")
            .arg(&hook_path)
            .status()
            .expect("run mkfifo for replacement fixture");
        assert!(status.success(), "mkfifo replacement fixture must succeed");
    });

    let (index, total) = load_placement_index(&path)
        .expect("loader must not reopen and block on a FIFO replacement");
    assert!(index.is_empty());
    assert_eq!(total, 0);
    assert!(
        !fs::symlink_metadata(&path)
            .expect("FIFO replacement path should exist")
            .is_file(),
        "test must actually replace the path with a non-regular node"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_placement_index_from_valid_fixture_produces_all_blocks() {
    let root = unique_temp_dir();
    fs::create_dir_all(&root).expect("temp dir should be creatable");
    let path = root.join("placement_manifest.json");
    fs::write(&path, include_str!("placement_manifest_fixture.json"))
        .expect("fixture should be writable");
    let (index, total) = load_placement_index(&path).expect("valid sidecar loads");
    let _ = fs::remove_dir_all(&root);
    assert_eq!(
        total, 7,
        "the valid fixture's seven authored blocks all resolve"
    );
    assert!(!index.is_empty());
}

// ----- authored NBT palette zero-drop contract ------------------------------------------------

/// Every block name authored in dan_zong / wangyintai structures must be indexed
/// without any drops.  If block_from_name doesn't cover a name, build_placement_index
/// rejects it, preventing a partial placement index (a structural hole).
///
/// This test creates a synthetic manifest containing one block entry for each name
/// in the authored palette and asserts that ALL are indexed (drop count == 0).
///
/// The palette list is kept in sync with `blocks::tests::AUTHORED_STRUCTURE_BLOCKS`.
#[test]
fn authored_nbt_palette_zero_drop_in_build_placement_index() {
    use super::super::blocks::tests::AUTHORED_STRUCTURE_BLOCKS;

    let blocks: Vec<PlacementBlock> = AUTHORED_STRUCTURE_BLOCKS
        .iter()
        .enumerate()
        .map(|(i, name)| PlacementBlock {
            pos: [i as i32, 64, 0],
            block: format!("minecraft:{name}"),
            properties: HashMap::new(),
        })
        .collect();

    let authored_count = blocks.len();
    let manifest = PlacementManifest {
        version: 1,
        structures: vec![PlacementStructure {
            nbt_path: "<test_authored_palette>".to_string(),
            origin: [0, 64, 0],
            rotation: 0,
            blocks,
        }],
    };

    let (_, total) =
        build_placement_index(manifest).expect("the authored NBT palette must all resolve");
    assert_eq!(
            total,
            authored_count,
            "Authored NBT palette has {authored_count} distinct block names but only {total} \
             resolved (drop count = {}). Add missing names to server/assets/worldgen/block_catalog.toml.",
            authored_count.saturating_sub(total)
        );
}

// -----------------------------------------------------------------------
// worldgen-v4 P0 §8.1 #1 — span encode/decode pin + behavior equivalence
// -----------------------------------------------------------------------

/// Decode helper that mmaps a freshly written pair of span buffers so the
/// pin tests exercise the real `decode_spans` mmap path (offset arithmetic
/// + sentinel handling), not an in-memory shortcut.
fn decode_via_disk(columns: &[ColumnSpanList], col_idx: usize) -> ColumnSpanList {
    let dir = unique_temp_dir();
    fs::create_dir_all(&dir).expect("temp span dir should be creatable");
    let (count_bytes, spans_bytes) = encode_spans_bytes(columns);
    let count_path = dir.join("spans_count.bin");
    let spans_path = dir.join("spans.bin");
    fs::write(&count_path, &count_bytes).expect("write count");
    fs::write(&spans_path, &spans_bytes).expect("write spans");
    let count_mmap = map_file(&count_path, columns.len()).expect("map count");
    let spans_mmap = map_file(&spans_path, columns.len() * SPAN_STRIDE).expect("map spans");
    let decoded = decode_spans(&count_mmap, &spans_mmap, col_idx);
    drop(count_mmap);
    drop(spans_mmap);
    let _ = fs::remove_dir_all(&dir);
    decoded
}

#[test]
fn span_decode_roundtrip_all_column_shapes() {
    // Four representative shapes (§8.1 #1): normal single span, floating
    // sky-isle (2 spans), carved cave (surface cap + floor remnant), and a
    // full 4-span column. Each must decode back byte-identically.
    let normal: ColumnSpanList = smallvec::smallvec![(-64, 72)];
    let sky_isle: ColumnSpanList = smallvec::smallvec![(-64, 74), (260, 272)];
    let cave: ColumnSpanList = smallvec::smallvec![(70, 74), (-64, 40)];
    let four: ColumnSpanList = smallvec::smallvec![(70, 74), (-64, 40), (120, 130), (200, 210)];
    let columns = vec![normal.clone(), sky_isle.clone(), cave.clone(), four.clone()];

    assert_eq!(
        decode_via_disk(&columns, 0).as_slice(),
        normal.as_slice(),
        "normal single span must roundtrip"
    );
    assert_eq!(
        decode_via_disk(&columns, 1).as_slice(),
        sky_isle.as_slice(),
        "sky-isle 2 spans must roundtrip in order (surface then isle)"
    );
    assert_eq!(
        decode_via_disk(&columns, 2).as_slice(),
        cave.as_slice(),
        "cave (surface cap + floor remnant) must roundtrip in order"
    );
    assert_eq!(
        decode_via_disk(&columns, 3).as_slice(),
        four.as_slice(),
        "a full MAX_SPANS=4 column must roundtrip every slot"
    );
}

#[test]
fn span_decode_void_column_is_empty() {
    // count byte 0 → no spans, regardless of slot bytes. surface_y falls
    // back to MIN_Y (full void).
    let void: ColumnSpanList = smallvec::smallvec![];
    let columns = vec![void];
    let decoded = decode_via_disk(&columns, 0);
    assert!(
        decoded.is_empty(),
        "void column (count=0) decodes to zero spans, got {decoded:?}"
    );
}

#[test]
fn span_decode_stops_at_count_and_ignores_trailing_sentinels() {
    // A 1-span column has its remaining 3 slots sentinel-filled on disk;
    // decode must stop at the count byte and never leak the sentinels as
    // real coordinates.
    let single: ColumnSpanList = smallvec::smallvec![(-64, 50)];
    let columns = vec![single.clone()];
    // Verify the on-disk trailing slots really are the sentinel.
    let (_count, spans_bytes) = encode_spans_bytes(&columns);
    let slot1_floor = i16::from_le_bytes([spans_bytes[4], spans_bytes[5]]);
    assert_eq!(
        slot1_floor, SPAN_SENTINEL,
        "slot 1 floor should be the sentinel for a 1-span column"
    );
    let decoded = decode_via_disk(&columns, 0);
    assert_eq!(
        decoded.len(),
        1,
        "decode must honor the count byte (1), not the sentinel slots"
    );
    assert_eq!(decoded[0], (-64, 50));
}

#[test]
fn span_decode_clamps_corrupt_count_to_max_spans() {
    // A malformed exporter could write a count byte above MAX_SPANS; the
    // decoder must clamp it so it never reads past the fixed slot region.
    let dir = unique_temp_dir();
    fs::create_dir_all(&dir).expect("temp dir");
    // One column, all four slots populated, but a corrupt count = 9.
    let full: ColumnSpanList = smallvec::smallvec![(0, 1), (5, 6), (10, 11), (20, 21)];
    let (_good_count, spans_bytes) = encode_spans_bytes(&[full]);
    fs::write(dir.join("spans_count.bin"), [9u8]).expect("write corrupt count");
    fs::write(dir.join("spans.bin"), &spans_bytes).expect("write spans");
    let count_mmap = map_file(&dir.join("spans_count.bin"), 1).expect("map count");
    let spans_mmap = map_file(&dir.join("spans.bin"), SPAN_STRIDE).expect("map spans");
    let decoded = decode_spans(&count_mmap, &spans_mmap, 0);
    drop(count_mmap);
    drop(spans_mmap);
    let _ = fs::remove_dir_all(&dir);
    assert_eq!(
        decoded.len(),
        MAX_SPANS,
        "corrupt count=9 must clamp to MAX_SPANS={MAX_SPANS}, never overrun the slot region"
    );
}

/// Build a provider from explicit per-column (spans, water_level, biome_id)
/// so behavior-equivalence assertions read the real mmap → ColumnSample →
/// query_surface path. Tile is `n × 1` (columns laid along x at z=0).
fn build_spans_provider(cols: &[(ColumnSpanList, f32, u8)]) -> RasterFixture {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).expect("tile dir");
    let tile_size = cols.len() as i32;
    let area = cols.len();

    let span_cols: Vec<ColumnSpanList> = cols.iter().map(|(s, _, _)| s.clone()).collect();
    let (count_bytes, spans_bytes) = encode_spans_bytes(&span_cols);
    fs::write(tile_dir.join("spans_count.bin"), count_bytes).expect("count");
    fs::write(tile_dir.join("spans.bin"), spans_bytes).expect("spans");

    // Required non-span layers.
    let mut water = Vec::with_capacity(area * 4);
    for (_, w, _) in cols {
        water.extend_from_slice(&w.to_le_bytes());
    }
    fs::write(tile_dir.join("water_level.bin"), water).expect("water");
    let biomes: Vec<u8> = cols.iter().map(|(_, _, b)| *b).collect();
    fs::write(tile_dir.join("biome_id.bin"), &biomes).expect("biome");
    write_u8_layer(&tile_dir.join("surface_id.bin"), 0, area);
    write_u8_layer(&tile_dir.join("subsurface_id.bin"), 0, area);
    write_f32_layer(&tile_dir.join("feature_mask.bin"), 0.0, area);
    write_f32_layer(&tile_dir.join("boundary_weight.bin"), 0.0, area);

    let tile = TileFields::load(&tile_dir, &root, &[], area).expect("spans tile should load");
    let mut tiles = HashMap::new();
    tiles.insert((0, 0), tile);
    let provider = TerrainProvider {
        tiles,
        tile_size,
        world_bounds: Bounds2D {
            min_x: 0,
            max_x: tile_size - 1,
            min_z: 0,
            max_z: 0,
        },
        surface_palette: vec![BlockState::STONE; 4],
        biome_palette: vec![BiomeId::DEFAULT; 32],
        default_wilderness_biome: BiomeId::DEFAULT,
        forest_wilderness_biome: BiomeId::DEFAULT,
        river_wilderness_biome: BiomeId::DEFAULT,
        pois: Vec::new(),
        anomaly_kinds: HashMap::new(),
        decoration_palette: Vec::new(),
        abyssal_tier_floor_y: HashMap::new(),
        fossil_bboxes: Vec::new(),
        placement_index: HashMap::new(),
        placement_block_count: 0,
        bot_fixture: None,
    };
    RasterFixture {
        provider: Some(provider),
        root,
    }
}

/// Build a provider that also carries the §8.1 #12 SEMANTIC layers
/// (sky_island_mask + underground_tier) so the 5 灵草 env-locks can be
/// exercised through the REAL span → ColumnSample → env_sky_island path.
/// Each column: (spans, sky_island_mask, underground_tier).
fn build_botany_provider(cols: &[(ColumnSpanList, f32, u8)]) -> RasterFixture {
    let root = unique_temp_dir();
    let tile_dir = root.join("tile_0_0");
    fs::create_dir_all(&tile_dir).expect("tile dir");
    let tile_size = cols.len() as i32;
    let area = cols.len();

    let span_cols: Vec<ColumnSpanList> = cols.iter().map(|(s, _, _)| s.clone()).collect();
    let (count_bytes, spans_bytes) = encode_spans_bytes(&span_cols);
    fs::write(tile_dir.join("spans_count.bin"), count_bytes).expect("count");
    fs::write(tile_dir.join("spans.bin"), spans_bytes).expect("spans");

    write_f32_layer(&tile_dir.join("water_level.bin"), -1.0, area);
    write_u8_layer(&tile_dir.join("biome_id.bin"), 0, area);
    write_u8_layer(&tile_dir.join("surface_id.bin"), 0, area);
    write_u8_layer(&tile_dir.join("subsurface_id.bin"), 0, area);
    write_f32_layer(&tile_dir.join("feature_mask.bin"), 0.0, area);
    write_f32_layer(&tile_dir.join("boundary_weight.bin"), 0.0, area);

    // The semantic layers the 5 灵草 lock off (§8.1 #12 — retained, not folded).
    let mut sky_mask = Vec::with_capacity(area * 4);
    for (_, m, _) in cols {
        sky_mask.extend_from_slice(&m.to_le_bytes());
    }
    fs::write(tile_dir.join("sky_island_mask.bin"), sky_mask).expect("sky mask");
    let tiers: Vec<u8> = cols.iter().map(|(_, _, t)| *t).collect();
    fs::write(tile_dir.join("underground_tier.bin"), &tiers).expect("tier");
    // qi_vein_flow is needed by yuan_ni_hong_yu; give it a high constant.
    write_f32_layer(&tile_dir.join("qi_vein_flow.bin"), 1.0, area);

    let optional = vec![
        "sky_island_mask".to_string(),
        "underground_tier".to_string(),
        "qi_vein_flow".to_string(),
    ];
    let tile = TileFields::load(&tile_dir, &root, &optional, area).expect("botany tile loads");
    let mut tiles = HashMap::new();
    tiles.insert((0, 0), tile);
    let provider = TerrainProvider {
        tiles,
        tile_size,
        world_bounds: Bounds2D {
            min_x: 0,
            max_x: tile_size - 1,
            min_z: 0,
            max_z: 0,
        },
        surface_palette: vec![BlockState::STONE; 4],
        biome_palette: vec![BiomeId::DEFAULT; 32],
        default_wilderness_biome: BiomeId::DEFAULT,
        forest_wilderness_biome: BiomeId::DEFAULT,
        river_wilderness_biome: BiomeId::DEFAULT,
        pois: Vec::new(),
        anomaly_kinds: HashMap::new(),
        decoration_palette: Vec::new(),
        abyssal_tier_floor_y: HashMap::new(),
        fossil_bboxes: Vec::new(),
        placement_index: HashMap::new(),
        placement_block_count: 0,
        bot_fixture: None,
    };
    RasterFixture {
        provider: Some(provider),
        root,
    }
}

/// worldgen-v4 P0 §8.1 #12 — the 5 灵草 lock off sky_island_mask +
/// underground_tier; the span refactor must NOT drift their generation
/// positions. Old (round-height) and new (span) representations must agree on
/// every column for all five, exercising the real span → env_sky_island path
/// (botany/registry.rs:181-223 EnvLock specs).
#[test]
fn spirit_herbs_env_locks_unchanged_after_span_refactor() {
    use crate::botany::env_lock::check_env_lock;
    use crate::botany::registry::{DecorationLock, EnvLock, SkyIsleSurface};

    // 5 columns laid along x at z=0. Each crafted to make EXACTLY one herb's
    // primary geometry lock pass, isolating sky-isle Top/Bottom and the three
    // underground tiers:
    //   x=0 yun_ding_lan   — sky isle present (mask>=0.2, base<9000)        → Top
    //   x=1 xuan_gen_wei   — sky isle present (thickness>0)                 → Bottom
    //   x=2 ying_yuan_gu   — underground_tier 1
    //   x=3 xuan_rong_tai  — underground_tier 2
    //   x=4 yuan_ni_hong_yu— underground_tier 3 (+ qi_vein_flow constant 1.0)
    let isle: ColumnSpanList = smallvec::smallvec![(-64, 72), (260, 280)];
    let cols: Vec<(ColumnSpanList, f32, u8)> = vec![
        (isle.clone(), 0.5, 0), // sky isle, no tier
        (isle.clone(), 0.5, 0), // sky isle, no tier
        (smallvec::smallvec![(-64, 60)], 0.0, 1),
        (smallvec::smallvec![(-64, 60)], 0.0, 2),
        (smallvec::smallvec![(-64, 60)], 0.0, 3),
    ];
    let fixture = build_botany_provider(&cols);
    let provider = fixture.provider();
    let zone = crate::world::zone::Zone {
        name: "botany_test".to_string(),
        dimension: crate::world::dimension::DimensionKind::Overworld,
        bounds: (
            valence::prelude::DVec3::new(0.0, 0.0, 0.0),
            valence::prelude::DVec3::new(16.0, 320.0, 16.0),
        ),
        spirit_qi: 0.0,
        danger_level: 1,
        active_events: vec![],
        patrol_anchors: vec![],
        blocked_tiles: vec![],
        qi_equilibrium: 0.0,
        qi_inflow_per_min: 0.0,
    };
    let manifest = crate::botany::env_lock::DecorationManifest::from_terrain_provider(provider);

    // First, prove env_sky_island derives a real (base_y, thickness) from the
    // span (the §8.1 #12 swap point) — base 260, thickness 20.
    use crate::botany::env_lock::EnvLayerSampler;
    assert_eq!(
        provider.env_sky_island(0, 0),
        Some((260.0, 20.0)),
        "sky-isle (base_y, thickness) must come from the span (260, 280), not a \
             deleted base_y/thickness field"
    );

    // Each herb's primary geometry lock at its intended column → PASS.
    let yun_ding_lan = EnvLock::SkyIslandMask {
        min: 0.2,
        surface: SkyIsleSurface::Top,
    };
    let xuan_gen_wei = EnvLock::SkyIslandMask {
        min: 0.2,
        surface: SkyIsleSurface::Bottom,
    };
    assert!(
        check_env_lock(yun_ding_lan, 0, 0, provider, &zone, &manifest),
        "yun_ding_lan (sky-isle Top) must pass on the isle column via the span path"
    );
    assert!(
        check_env_lock(xuan_gen_wei, 1, 0, provider, &zone, &manifest),
        "xuan_gen_wei (sky-isle Bottom) must pass on the isle column"
    );
    assert!(
        check_env_lock(
            EnvLock::UndergroundTier { tier: 1 },
            2,
            0,
            provider,
            &zone,
            &manifest
        ),
        "ying_yuan_gu (tier 1) must pass on the tier-1 column"
    );
    assert!(
        check_env_lock(
            EnvLock::UndergroundTier { tier: 2 },
            3,
            0,
            provider,
            &zone,
            &manifest
        ),
        "xuan_rong_tai (tier 2) must pass on the tier-2 column"
    );
    assert!(
        check_env_lock(
            EnvLock::UndergroundTier { tier: 3 },
            4,
            0,
            provider,
            &zone,
            &manifest
        ),
        "yuan_ni_hong_yu (tier 3) must pass on the tier-3 column"
    );

    // And each lock must FAIL where its semantic layer is absent — proving the
    // span refactor did not silently make every column pass (position drift).
    assert!(
        !check_env_lock(yun_ding_lan, 2, 0, provider, &zone, &manifest),
        "sky-isle Top must NOT pass on a non-isle underground column"
    );
    assert!(
        !check_env_lock(
            EnvLock::UndergroundTier { tier: 3 },
            2,
            0,
            provider,
            &zone,
            &manifest
        ),
        "tier-3 lock must NOT pass on a tier-1 column (no position drift)"
    );
    // qi_vein_flow lock (part of yuan_ni_hong_yu) reads its own layer, set 1.0.
    assert!(
        check_env_lock(
            EnvLock::QiVeinFlow { min: 0.5 },
            4,
            0,
            provider,
            &zone,
            &manifest
        ),
        "yuan_ni_hong_yu qi_vein_flow lock must pass with the constant 1.0 layer"
    );
    let _ = DecorationLock::One("yuan_ni_ebony"); // keep the import meaningful
}

#[test]
fn behavior_equivalence_surface_water_biome_match_v3_golden() {
    use super::super::SurfaceProvider;
    // Frozen v3 golden sample (subset of worldgen/fixtures/v3_surface_baseline.json).
    // These are the REAL v3 carved surfaces (rift/fracture/neg/entrance
    // sculpting baked in), NOT round(height) — the Python golden records the
    // exact same numbers via v3_surface_top_y (test_v3_behavior_baseline.py).
    // For a carved column the surface span ceiling already == carved top_y, so
    // the byte path here (mmap → ColumnSample → query_surface) reproduces the
    // carved surface. water_level<0 → no water.
    // Exact frozen golden rows from worldgen/fixtures/v3_surface_baseline.json:
    //   normal  (60,100)  surface 307, no water,  biome 9   (no carve)
    //   water   (100,100) surface 44,  water 44,  biome 10  (no carve)
    //   sky_isle(100,100) surface 75,  no water,  biome 4   (height 74.5 →
    //                     f32 round-half-away = 75, NOT banker's 74)
    //   cave    (100,100) surface 63,  no water,  biome 5   (entrance sink 4)
    //   abyssal (100,100) surface 57,  no water,  biome 5   (neg 9 + entrance 4)
    let cols: Vec<(ColumnSpanList, f32, u8)> = vec![
        (smallvec::smallvec![(-64, 307)], -1.0, 9),
        (smallvec::smallvec![(-64, 44)], 44.0, 10),
        // sky_isle: ground 75 + isle span above; surface stays 75.
        (smallvec::smallvec![(-64, 75), (260, 272)], -1.0, 4),
        // cave: carved surface cap ceiling 63 + a floor remnant below.
        (smallvec::smallvec![(58, 63), (-64, 30)], -1.0, 5),
        // abyssal: carved surface cap 57 (neg+entrance) + remnant.
        (smallvec::smallvec![(40, 57), (-64, 20)], -1.0, 5),
    ];
    let expected: [(i32, Option<i32>, u8); 5] = [
        (307, None, 9),
        (44, Some(44), 10),
        (75, None, 4),
        (63, None, 5),
        (57, None, 5),
    ];

    let fixture = build_spans_provider(&cols);
    let provider = fixture.provider();

    for (x, (want_y, want_water, want_biome)) in expected.iter().enumerate() {
        let surface = provider.query_surface(x as i32, 0);
        let sample = provider.sample(x as i32, 0);
        assert_eq!(
            surface.y, *want_y,
            "column {x}: query_surface().y should equal the carved v3 surface \
                 (golden surface_y={want_y}), got {}",
            surface.y
        );
        let got_water = if surface.water_y == i32::MIN {
            None
        } else {
            Some(surface.water_y)
        };
        assert_eq!(
            got_water, *want_water,
            "column {x}: water_y should match v3 golden ({want_water:?}), got {got_water:?}"
        );
        assert_eq!(
            sample.biome_id, *want_biome,
            "column {x}: biome_id should match v3 golden ({want_biome}), got {}",
            sample.biome_id
        );
    }
}

/// Anti-circular literal anchors for the carved surfaces above. v4 Rust no
/// longer carves (the rift/fracture/neg/entrance sculpt moved into the Python
/// span shim — `surface_y_for_sample` now just reads `spans[0].ceiling`), so
/// the carve FORMULA is pinned by hand-calced literals on the Python side
/// (`worldgen/tests/test_v3_behavior_baseline.py::HandCalcedV3CarveAnchors`).
/// Here we pin the CONSUMER contract: a span whose ceiling already equals the
/// hand-computed carved top_y must surface at exactly that Y through the real
/// mmap → ColumnSample → query_surface path, so a span-decode regression撞红.
#[test]
fn carved_surface_spans_query_to_hand_calced_top_y() {
    use super::super::SurfaceProvider;
    // Each row: a surface span whose ceiling is the hand-computed v3 carved
    // top_y (matching the Python literals), plus the expected query_surface.
    //   rift     80 - round((1-0.5)*22 + 0.25*4)=12          → 68
    //   fracture 100 - int(f32(0.90-0.7)*300=59.99→59)       → 41
    //   neg      90 - round(0.5*14)=7                         → 83
    //   entrance 72 - round(0.45*10)=round(4.5)=5             → 67
    //   stacked  95 - 9(rift) - 30(frac) - 4(neg) - 2(ent)   → 50
    let cols: Vec<(ColumnSpanList, f32, u8)> = vec![
        (smallvec::smallvec![(-64, 68)], -1.0, 3),
        (smallvec::smallvec![(-64, 41)], -1.0, 3),
        (smallvec::smallvec![(-64, 83)], -1.0, 6),
        (smallvec::smallvec![(-64, 67)], -1.0, 5),
        (smallvec::smallvec![(-64, 50)], -1.0, 3),
    ];
    let expected = [68, 41, 83, 67, 50];
    let fixture = build_spans_provider(&cols);
    let provider = fixture.provider();
    for (x, want) in expected.iter().enumerate() {
        let surface = provider.query_surface(x as i32, 0);
        assert_eq!(
            surface.y, *want,
            "column {x}: carved-surface span must query to the hand-calced \
                 v3 top_y {want}, got {}",
            surface.y
        );
    }
}

#[test]
fn column_sample_helpers_derive_geometry_from_spans() {
    // Sky-isle span and cave carve must be recoverable from a decoded
    // ColumnSample, mirroring the Python shim's folding (consumer contract
    // for flora/botany/lifecycle).
    let cols: Vec<(ColumnSpanList, f32, u8)> = vec![
        (smallvec::smallvec![(-64, 74), (260, 272)], -1.0, 4),
        (smallvec::smallvec![(60, 67), (-64, 30)], -1.0, 5),
        (smallvec::smallvec![(-64, 80)], -1.0, 9),
    ];
    let fixture = build_spans_provider(&cols);
    let provider = fixture.provider();

    let isle = provider.sample(0, 0);
    assert_eq!(isle.surface_y(), 74, "surface = lowest span ceiling");
    assert_eq!(
        isle.sky_island_span(),
        Some((260, 272)),
        "high span above the surface is the isle"
    );
    assert!(!isle.has_carved_cave(), "isle column has no cave void");

    let cave = provider.sample(1, 0);
    assert_eq!(cave.surface_y(), 67, "cave surface = surface cap ceiling");
    assert_eq!(
        cave.cave_carve(),
        Some((31, 59)),
        "carve void = (remnant_ceiling+1, surface_floor-1) = (31, 59)"
    );
    assert_eq!(
        cave.cavern_floor_y(),
        Some(30),
        "cavern floor anchor = floor remnant ceiling (30)"
    );
    assert_eq!(cave.sky_island_span(), None, "cave column has no isle");

    let plain = provider.sample(2, 0);
    assert_eq!(plain.surface_y(), 80);
    assert!(!plain.has_carved_cave());
    assert_eq!(plain.sky_island_span(), None);
    assert!(!plain.is_void());
}
