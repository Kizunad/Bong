use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use serde::Deserialize;
use valence::prelude::{BiomeId, BiomeRegistry, BlockPos, BlockState, ChunkPos, Ident, Resource};

use super::nbt_registry::DecorationAnchor;
use super::wilderness;

// ---------------------------------------------------------------------------
// P1 — placement manifest serde structs (断链 #2, plan-terrain-wiring-v1)
// ---------------------------------------------------------------------------
//
// These mirror the format produced by worldgen's export_placement_manifest():
//   { "version": 1, "structures": [ { "nbt_path", "origin", "rotation",
//       "blocks": [ { "pos": [x,y,z], "block": "minecraft:...",
//                     "properties": { ... } } ] } ] }
//
// Server does NOT re-rotate (M3: worldgen rotates at export time).

/// A single pre-flattened block from the worldgen placement manifest.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementBlock {
    /// Absolute world position [x, y, z].
    pub pos: [i32; 3],
    /// Minecraft block name, e.g. `"minecraft:stone_bricks"`.
    pub block: String,
    /// Blockstate properties already rotated by worldgen (M3).
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// One authored structure (NBT paste or inline stamp) in the placement manifest.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementStructure {
    pub nbt_path: String,
    pub origin: [i32; 3],
    pub rotation: i32,
    pub blocks: Vec<PlacementBlock>,
}

/// Top-level placement manifest written by worldgen's `export_placement_manifest`.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementManifest {
    pub version: u32,
    pub structures: Vec<PlacementStructure>,
}

// Keep this Rust mirror in lockstep with
// worldgen/scripts/terrain_gen/fields.py::LAYER_REGISTRY.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayerExportType {
    F32,
    U8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerSchema {
    pub name: &'static str,
    pub export_type: LayerExportType,
    pub safe_default_f32: Option<f32>,
    pub safe_default_u8: Option<u8>,
}

const fn f32_layer(name: &'static str, safe_default: f32) -> LayerSchema {
    LayerSchema {
        name,
        export_type: LayerExportType::F32,
        safe_default_f32: Some(safe_default),
        safe_default_u8: None,
    }
}

const fn u8_layer(name: &'static str, safe_default: u8) -> LayerSchema {
    LayerSchema {
        name,
        export_type: LayerExportType::U8,
        safe_default_f32: None,
        safe_default_u8: Some(safe_default),
    }
}

const LAYER_SCHEMAS: &[LayerSchema] = &[
    f32_layer("height", 0.0),
    u8_layer("surface_id", 0),
    u8_layer("subsurface_id", 0),
    f32_layer("water_level", -1.0),
    u8_layer("biome_id", 0),
    f32_layer("feature_mask", 0.0),
    f32_layer("boundary_weight", 0.0),
    f32_layer("rift_axis_sdf", 99.0),
    f32_layer("portal_anchor_sdf", 999.0),
    f32_layer("rim_edge_mask", 0.0),
    f32_layer("fracture_mask", 0.0),
    // worldgen-v4 P0 §8.1 #1: cave_mask / ceiling_height / entrance_mask are
    // folded into the column-span representation (spans_count.bin + spans.bin)
    // and no longer exist as standalone rasters. They stay deleted here so the
    // registry mirror matches Python's LAYER_REGISTRY.
    f32_layer("neg_pressure", 0.0),
    f32_layer("ruin_density", 0.0),
    f32_layer("qi_density", 0.12),
    f32_layer("mofa_decay", 0.40),
    f32_layer("qi_vein_flow", 0.0),
    u8_layer("spirit_eye_candidates", 0),
    u8_layer("realm_collapse_mask", 0),
    // worldgen-v4 P0 §8.1 #12: sky_island_mask + underground_tier are RETAINED
    // semantic layers (the 5 灵草 environment locks key off them directly). The
    // geometric sky_island_base_y/thickness + cavern_floor_y are folded into
    // spans and deleted here.
    f32_layer("sky_island_mask", 0.0),
    u8_layer("underground_tier", 0),
    f32_layer("flora_density", 0.0),
    u8_layer("flora_variant_id", 0),
    f32_layer("ground_cover_density", 0.0),
    u8_layer("ground_cover_id", 0),
    u8_layer("zongmen_origin_id", 0),
    f32_layer("mineral_density", 0.0),
    u8_layer("mineral_kind", 0),
    u8_layer("fossil_bbox", 0),
    f32_layer("anomaly_intensity", 0.0),
    u8_layer("anomaly_kind", 0),
    u8_layer("tsy_presence", 0),
    u8_layer("tsy_origin_id", 0),
    u8_layer("tsy_depth_tier", 0),
];

fn layer_schema(layer_name: &str) -> Option<&'static LayerSchema> {
    LAYER_SCHEMAS
        .iter()
        .find(|schema| schema.name == layer_name)
}

// ---------------------------------------------------------------------------
// Column spans — worldgen-v4 P0 §8.1 #1 (Rust mirror of Python
// worldgen/scripts/terrain_gen/fields.py::ColumnSpans encoding).
//
// The vertical structure of a column is a small fixed-capacity list of *solid*
// inclusive `(floor_y, ceiling_y)` block ranges. This single representation
// replaces the old `height` field + the sky_island_base_y/thickness +
// cave_mask/ceiling_height/entrance_mask/cavern_floor_y patch layers.
//
// On-disk binary layout (mmap-friendly fixed stride, decided in §8.1 #1):
//   spans_count.bin : u8 per column, 0..=MAX_SPANS  (0 = full void column)
//   spans.bin       : MAX_SPANS slots per column, each slot = two little-endian
//                     i16 (floor_y, ceiling_y) = 4 bytes; column stride = 16 B.
//                     Unused slots are filled with the sentinel i16::MAX so the
//                     reader can mmap at `offset = col_idx * SPAN_STRIDE` and
//                     ignore trailing sentinels without a separate index.
//
// Convention (§8.1 #2): **span[0] is the surface/ground span**; its ceiling is
// the walkable surface and is what `query_surface()` returns ("最低段顶面").
// Extra spans — a cave-floor remnant below, a floating sky-isle above — follow
// in any order; only span[0] is privileged.
// ---------------------------------------------------------------------------

/// Max solid spans per column (matches Python `MAX_SPANS`).
pub const MAX_SPANS: usize = 4;
/// Sentinel marking an unused span slot (i16::MAX, matches Python `SPAN_SENTINEL`).
/// Production decode stops at the count byte and never reads sentinels; this
/// constant pins the encoding contract and is exercised by the span pin tests.
#[allow(dead_code)]
pub const SPAN_SENTINEL: i16 = i16::MAX;
/// Bytes per column in spans.bin: MAX_SPANS × (i16 floor + i16 ceiling).
pub const SPAN_STRIDE: usize = MAX_SPANS * 2 * 2;

/// Inline-storage list of `(floor_y, ceiling_y)` solid spans for one column.
pub type ColumnSpanList = smallvec::SmallVec<[(i16, i16); MAX_SPANS]>;

/// Decode a single column's spans from the raw `spans_count` + `spans` mmaps.
///
/// `count` slots beyond which everything must be the sentinel; we stop at the
/// count byte so a corrupt sentinel never leaks coordinates. The count byte is
/// clamped to `MAX_SPANS` defensively (a malformed exporter can't overflow the
/// fixed-capacity slot region).
fn decode_spans(count_bytes: &Mmap, spans_bytes: &Mmap, index: usize) -> ColumnSpanList {
    let count = (count_bytes[index] as usize).min(MAX_SPANS);
    let base = index * SPAN_STRIDE;
    let mut spans = ColumnSpanList::new();
    for slot in 0..count {
        let off = base + slot * 4;
        let floor_y = i16::from_le_bytes([spans_bytes[off], spans_bytes[off + 1]]);
        let ceiling_y = i16::from_le_bytes([spans_bytes[off + 2], spans_bytes[off + 3]]);
        spans.push((floor_y, ceiling_y));
    }
    spans
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Bounds2D {
    pub min_x: i32,
    pub max_x: i32,
    pub min_z: i32,
    pub max_z: i32,
}

#[allow(dead_code)]
impl Bounds2D {
    pub fn contains(&self, x: i32, z: i32) -> bool {
        x >= self.min_x && x <= self.max_x && z >= self.min_z && z <= self.max_z
    }
}

#[allow(dead_code)]
// `spans` is a SmallVec, so ColumnSample is Clone but not Copy.
#[derive(Clone, Debug)]
pub struct ColumnSample {
    /// Vertical structure of this column — worldgen-v4 P0 §8.1 #1. `spans[0]`
    /// is the surface/ground span; its ceiling is the walkable surface. Extra
    /// spans encode a cave-floor remnant (below) or a floating sky-isle (above).
    /// Empty = full void column. Replaces the old `height` + sky_island_base_y/
    /// thickness + cave_mask/ceiling_height/cavern_floor_y fields.
    pub spans: ColumnSpanList,
    pub surface_block: BlockState,
    pub subsurface_block: BlockState,
    pub biome_id: u8,
    pub biome: BiomeId,
    pub water_level: f32,
    pub feature_mask: f32,
    pub boundary_weight: f32,
    pub rift_axis_sdf: f32,
    pub portal_anchor_sdf: f32,
    pub rim_edge_mask: f32,
    pub fracture_mask: f32,
    pub neg_pressure: f32,
    pub ruin_density: f32,
    // --- xianxia semantic layers ---
    pub qi_density: f32,
    pub mofa_decay: f32,
    pub qi_vein_flow: f32,
    pub spirit_eye_candidates: u8,
    pub realm_collapse_mask: u8,
    // --- vertical-dimension SEMANTIC layers (retained; §8.1 #12) ---
    // The geometric sky_island_base_y/thickness + cavern_floor_y are folded
    // into `spans`; sky_island_mask + underground_tier survive because the 5
    // 灵草 environment locks key off them directly.
    /// 0..1 likelihood this column hosts a floating isle above. Gate on >= 0.2.
    pub sky_island_mask: f32,
    /// Deepest active cave tier at this column: 0 (none), 1 shallow, 2 middle, 3 deep.
    pub underground_tier: u8,
    // --- ecology layers ---
    /// 0..1 decoration placement probability.
    pub flora_density: f32,
    /// Global decoration id (0 = none; lookup via TerrainProvider::decoration).
    pub flora_variant_id: u8,
    /// 0..1 ground-cover (短草/花/枯木) placement probability. Independent
    /// from flora_density so a column can host both a feature decoration AND
    /// dense ground cover (e.g. elder_oak + meadow_grass).
    pub ground_cover_density: f32,
    /// Global decoration id for ground cover (0 = none). Same palette as
    /// flora_variant_id; convention is to point at kind="flower" specs.
    pub ground_cover_id: u8,
    /// Overworld sect-ruin origin discriminator; 0 means no sect origin.
    pub zongmen_origin_id: u8,
    /// 0..1 likelihood a mineral ore-block occupies this column.
    pub mineral_density: f32,
    /// Global mineral id written by the worldgen mineral palette; 0 = none.
    pub mineral_kind: u8,
    /// 0 none, 1 whalefall outer ribs/periphery, 2 mineral-rich core.
    pub fossil_bbox: u8,
    // --- event / anomaly layers ---
    /// 0..1 local anomaly strength (event system threshold ≈ 0.3).
    pub anomaly_intensity: f32,
    /// 0..5: 0 none, 1 spacetime_rift, 2 qi_turbulence,
    /// 3 blood_moon_anchor, 4 cursed_echo, 5 wild_formation.
    pub anomaly_kind: u8,
    // --- TSY-specific layers (plan-tsy-worldgen-v1 §4.1) ---
    /// 1 if column is inside a TSY family AABB, else 0. Only present on TSY-dim
    /// rasters; overworld manifest never writes this layer (default = 0).
    pub tsy_presence: u8,
    /// 1=daneng_luoluo / 2=zongmen_yiji / 3=zhanchang_chendian /
    /// 4=gaoshou_sichu / 0=none.
    pub tsy_origin_id: u8,
    /// 1=shallow / 2=mid / 3=deep / 0=none.
    pub tsy_depth_tier: u8,
}

impl ColumnSample {
    /// Walkable surface Y = ceiling of the surface span (`spans[0]`), the lowest
    /// solid span's top face (§8.1 #2). Returns `MIN_Y` for a full void column so
    /// downstream consumers still get a sane (floor-of-world) anchor.
    pub fn surface_y(&self) -> i32 {
        self.spans
            .first()
            .map(|(_floor, ceiling)| i32::from(*ceiling))
            .unwrap_or(super::MIN_Y)
    }

    /// True when this column has no solid blocks at all (full void).
    #[allow(dead_code)]
    pub fn is_void(&self) -> bool {
        self.spans.is_empty()
    }

    /// The surface span itself `(floor_y, ceiling_y)`, if any.
    fn surface_span(&self) -> Option<(i32, i32)> {
        self.spans
            .first()
            .map(|(floor, ceiling)| (i32::from(*floor), i32::from(*ceiling)))
    }

    /// Floating sky-isle span `(bottom_y, top_y)`, derived from any span that
    /// sits strictly above the surface span with a real air gap. Mirrors the
    /// old `sky_island_span_for_sample` gate (an isle floats above the ground).
    pub fn sky_island_span(&self) -> Option<(i32, i32)> {
        let (_surface_floor, surface_ceiling) = self.surface_span()?;
        self.spans
            .iter()
            .skip(1)
            .map(|(floor, ceiling)| (i32::from(*floor), i32::from(*ceiling)))
            .filter(|(floor, ceiling)| *floor > surface_ceiling + 1 && *ceiling > *floor)
            // Highest such span is the isle (caves are below the surface, not above).
            .max_by_key(|(floor, _ceiling)| *floor)
    }

    /// Carved cave void `(carve_floor, carve_ceiling)` between the surface span
    /// and a floor remnant directly below it. `None` when the column is solid
    /// down to bedrock (no carve). Mirrors the old cave carve geometry: the void
    /// is the inclusive air gap between the floor remnant and the surface cap.
    pub fn cave_carve(&self) -> Option<(i32, i32)> {
        let (surface_floor, _surface_ceiling) = self.surface_span()?;
        // The floor remnant is the highest span strictly below the surface span.
        let remnant_ceiling = self
            .spans
            .iter()
            .skip(1)
            .map(|(_floor, ceiling)| i32::from(*ceiling))
            .filter(|ceiling| *ceiling < surface_floor)
            .max()?;
        let carve_floor = remnant_ceiling + 1;
        let carve_ceiling = surface_floor - 1;
        if carve_ceiling >= carve_floor {
            Some((carve_floor, carve_ceiling))
        } else {
            None
        }
    }

    /// True when this column is carved open by a cave void (has a floor remnant).
    pub fn has_carved_cave(&self) -> bool {
        self.cave_carve().is_some()
    }

    /// Top face of the deepest cavern floor remnant — where a plant rooted in a
    /// cave would sit. `None` when there is no carved cave. Replaces the old
    /// `cavern_floor_y` field (§8.1 #1).
    pub fn cavern_floor_y(&self) -> Option<i32> {
        let (surface_floor, _surface_ceiling) = self.surface_span()?;
        self.spans
            .iter()
            .skip(1)
            .map(|(_floor, ceiling)| i32::from(*ceiling))
            .filter(|ceiling| *ceiling < surface_floor)
            .max()
    }

    pub fn is_peaks_biome(&self) -> bool {
        matches!(self.biome_id, 1 | 9)
    }

    pub fn is_marsh_biome(&self) -> bool {
        matches!(self.biome_id, 2 | 10)
    }

    pub fn is_rift_biome(&self) -> bool {
        self.biome_id == 3
    }

    pub fn is_spawn_biome(&self) -> bool {
        matches!(self.biome_id, 4 | 11)
    }

    pub fn is_wastes_biome(&self) -> bool {
        self.biome_id == 6
    }
}

#[derive(Debug)]
pub struct TerrainProvider {
    tiles: HashMap<(i32, i32), TileFields>,
    tile_size: i32,
    #[allow(dead_code)]
    pub world_bounds: Bounds2D,
    surface_palette: Vec<BlockState>,
    pub biome_palette: Vec<BiomeId>,
    default_wilderness_biome: BiomeId,
    forest_wilderness_biome: BiomeId,
    river_wilderness_biome: BiomeId,
    // --- narrative / event metadata read once from manifest ---
    pois: Vec<Poi>,
    anomaly_kinds: HashMap<u8, String>,
    /// Global decoration palette: index by global id (0-slot is unused placeholder).
    decoration_palette: Vec<Option<Decoration>>,
    abyssal_tier_floor_y: HashMap<u8, f32>,
    fossil_bboxes: Vec<FossilBbox>,
    /// P1 — authored structure blocks pre-bucketed by ChunkPos (断链 #2).
    /// Empty when no placement_manifest.json sidecar is present (向后兼容).
    placement_index: HashMap<ChunkPos, Vec<(BlockPos, BlockState)>>,
    /// Total number of authored placement blocks loaded (for startup logging).
    placement_block_count: usize,
    /// Test-only provenance metadata from Bot-owned raster manifests. Production
    /// manifests omit it; when present it is validated before any ready marker is emitted.
    bot_fixture: Option<BotRasterFixture>,
}

impl Resource for TerrainProvider {}

/// Per-dimension `TerrainProvider` map (plan-tsy-dimension-v1 §2.2).
///
/// Inserted alongside the legacy `TerrainProvider` resource so existing
/// overworld-only consumers keep compiling. New / TSY-aware consumers should
/// take `Option<Res<TerrainProviders>>` and route via `DimensionKind`.
///
/// `tsy` is `Option` while `plan-tsy-worldgen-v1` is still pre-active and the
/// TSY raster manifest is not yet produced; once worldgen lands the field
/// becomes mandatory (§6 contract).
pub struct TerrainProviders {
    pub overworld: TerrainProvider,
    #[allow(dead_code)]
    pub tsy: Option<TerrainProvider>,
}

impl Resource for TerrainProviders {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TerrainLoadError {
    diagnostics: Vec<String>,
}

impl TerrainLoadError {
    fn new<I>(diagnostics: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        let diagnostics = diagnostics.into_iter().collect::<BTreeSet<_>>();
        Self {
            diagnostics: diagnostics.into_iter().collect(),
        }
    }

    pub(crate) fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

impl std::fmt::Display for TerrainLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "terrain raster failed startup preflight:\n- {}",
            self.diagnostics.join("\n- ")
        )
    }
}

impl std::error::Error for TerrainLoadError {}

impl TerrainProviders {
    /// Look up the provider for the given dimension. Returns `None` for TSY
    /// when no TSY manifest is loaded (transitional state until worldgen plan
    /// ships).
    #[allow(dead_code)]
    pub fn for_dimension(
        &self,
        kind: crate::world::dimension::DimensionKind,
    ) -> Option<&TerrainProvider> {
        use crate::world::dimension::DimensionKind;
        match kind {
            DimensionKind::Overworld => Some(&self.overworld),
            DimensionKind::Tsy => self.tsy.as_ref(),
        }
    }
}

#[derive(Debug)]
struct TileFields {
    // worldgen-v4 P0 §8.1 #1: spans_count.bin (u8/col) + spans.bin (16B/col)
    // replace height.bin + the six deleted vertical patch layers.
    spans_count: Mmap,
    spans: Mmap,
    surface_id: Mmap,
    subsurface_id: Mmap,
    biome_id: Mmap,
    water_level: Mmap,
    feature_mask: Mmap,
    boundary_weight: Mmap,
    rift_axis_sdf: Option<Mmap>,
    portal_anchor_sdf: Option<Mmap>,
    rim_edge_mask: Option<Mmap>,
    fracture_mask: Option<Mmap>,
    neg_pressure: Option<Mmap>,
    ruin_density: Option<Mmap>,
    // Semantic / vertical / ecology / anomaly layers — all optional so older
    // manifests without them still load cleanly.
    qi_density: Option<Mmap>,
    mofa_decay: Option<Mmap>,
    qi_vein_flow: Option<Mmap>,
    spirit_eye_candidates: Option<Mmap>,
    realm_collapse_mask: Option<Mmap>,
    sky_island_mask: Option<Mmap>,
    underground_tier: Option<Mmap>,
    flora_density: Option<Mmap>,
    flora_variant_id: Option<Mmap>,
    ground_cover_density: Option<Mmap>,
    ground_cover_id: Option<Mmap>,
    zongmen_origin_id: Option<Mmap>,
    mineral_density: Option<Mmap>,
    mineral_kind: Option<Mmap>,
    fossil_bbox: Option<Mmap>,
    anomaly_intensity: Option<Mmap>,
    anomaly_kind: Option<Mmap>,
    // plan-tsy-worldgen-v1 §4.1 — TSY-only layers, all uint8 (tile_area sized).
    tsy_presence: Option<Mmap>,
    tsy_origin_id: Option<Mmap>,
    tsy_depth_tier: Option<Mmap>,
}

/// worldgen-v4 P0 §8.1 #1 — manifest schema version the Rust reader expects.
/// v2 introduced the span column encoding (spans_count.bin + spans.bin) that
/// replaced height.bin + the deleted vertical patch layers; a v1 manifest has
/// no spans on disk and would mmap garbage, so the loader must reject it loudly
/// instead of letting serde default the field and read a bad layout.
const EXPECTED_RASTER_MANIFEST_VERSION: u32 = 2;

/// Reject any raster manifest whose schema version is not the one the span
/// reader understands. A mismatched manifest (e.g. v1 with height.bin and no
/// spans.bin) would mmap the wrong on-disk layout, so this fails loudly rather
/// than letting the reader produce silent garbage. Mirrors the PlacementManifest
/// `version` field convention, but actually enforced.
fn validate_manifest_version(version: u32, manifest_path: &Path) -> Result<(), String> {
    if version == EXPECTED_RASTER_MANIFEST_VERSION {
        return Ok(());
    }
    Err(format!(
        "terrain raster manifest {} has unsupported version {version} \
         (this server expects v{EXPECTED_RASTER_MANIFEST_VERSION}, the span column \
         encoding — regenerate the rasters with worldgen-v4)",
        manifest_path.display(),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RasterManifest {
    /// Manifest schema version (written by worldgen raster_export). Validated in
    /// `load()` against `EXPECTED_RASTER_MANIFEST_VERSION`. No serde default — a
    /// manifest missing the field fails to parse rather than silently passing.
    version: u32,
    // These fields are emitted by worldgen for preview/provenance consumers but
    // are not used by runtime terrain sampling. Listing them explicitly keeps
    // deny_unknown_fields useful for catching producer schema drift.
    #[serde(default, rename = "backend")]
    _backend: Option<String>,
    #[serde(default, rename = "world_name")]
    _world_name: Option<String>,
    tile_size: i32,
    #[serde(default, rename = "spans_encoding")]
    _spans_encoding: Option<serde_json::Value>,
    world_bounds: ManifestBounds,
    surface_palette: Vec<String>,
    biome_palette: Vec<String>,
    tiles: Vec<ManifestTile>,
    #[serde(default)]
    pois: Vec<ManifestPoi>,
    #[serde(default, rename = "zones")]
    _zones: Vec<serde_json::Value>,
    #[serde(default, rename = "collapsed_zones")]
    _collapsed_zones: Vec<serde_json::Value>,
    #[serde(default, rename = "semantic_layers")]
    _semantic_layers: Option<serde_json::Value>,
    #[serde(default, rename = "structure_layers")]
    _structure_layers: Option<serde_json::Value>,
    #[serde(default, rename = "vertical_layers")]
    _vertical_layers: Option<serde_json::Value>,
    #[serde(default, rename = "profiles_ecology")]
    _profiles_ecology: Option<serde_json::Value>,
    #[serde(default, rename = "qi_density_source")]
    _qi_density_source: Option<serde_json::Value>,
    #[serde(default, rename = "qi_budget_report")]
    _qi_budget_report: Option<serde_json::Value>,
    #[serde(default)]
    anomaly_kinds: HashMap<String, String>,
    #[serde(default)]
    abyssal_tier_floor_y: HashMap<String, f32>,
    #[serde(default, rename = "ascension_pits")]
    _ascension_pits: Vec<serde_json::Value>,
    #[serde(default, rename = "corpse_mounds")]
    _corpse_mounds: Vec<serde_json::Value>,
    #[serde(default)]
    global_decoration_palette: Vec<ManifestDecoration>,
    #[serde(default)]
    fossil_bboxes: Vec<ManifestFossilBbox>,
    #[serde(default, rename = "notes")]
    _notes: Option<serde_json::Value>,
    #[serde(default)]
    bot_fixture: Option<ManifestBotFixture>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestBotFixture {
    kind: String,
    token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BotRasterFixture {
    pub kind: String,
    pub token: String,
}

fn validate_bot_fixture(
    fixture: Option<ManifestBotFixture>,
    manifest_path: &Path,
) -> Result<Option<BotRasterFixture>, String> {
    let Some(fixture) = fixture else {
        return Ok(None);
    };
    let valid_kind = fixture.kind == "ambient-surface-v1";
    let valid_token = (16..=128).contains(&fixture.token.len())
        && fixture
            .token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if !valid_kind || !valid_token {
        return Err(format!(
            "terrain raster manifest {} has invalid bot_fixture metadata",
            manifest_path.display()
        ));
    }
    Ok(Some(BotRasterFixture {
        kind: fixture.kind,
        token: fixture.token,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestTile {
    tile_x: i32,
    tile_z: i32,
    dir: String,
    #[serde(default, rename = "zones")]
    _zones: Vec<String>,
    layers: Vec<String>,
    #[serde(default, rename = "spans")]
    _spans: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestPoi {
    zone: String,
    kind: String,
    name: String,
    pos_xyz: [f32; 3],
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    unlock: String,
    #[serde(default)]
    qi_affinity: f32,
    #[serde(default)]
    danger_bias: i32,
}

fn manifest_pois_into_runtime(raw_pois: Vec<ManifestPoi>) -> Vec<Poi> {
    raw_pois
        .into_iter()
        .map(|raw| Poi {
            zone: raw.zone,
            kind: raw.kind,
            name: raw.name,
            pos_xyz: raw.pos_xyz,
            tags: raw.tags,
            unlock: raw.unlock,
            qi_affinity: raw.qi_affinity,
            danger_bias: raw.danger_bias,
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestDecoration {
    global_id: u32,
    profile: String,
    #[serde(default)]
    local_id: u32,
    name: String,
    kind: String,
    #[serde(default)]
    blocks: Vec<String>,
    #[serde(default)]
    size_range: [i32; 2],
    #[serde(default)]
    rarity: f32,
    #[serde(default)]
    notes: String,
    // worldgen-v4 P6 §8.1 — NBT-driven placement. `#[serde(default)]` keeps old
    // manifests (no nbt_templates / no anchor) deserializing into the procedural
    // path: empty templates ⇒ procedural, anchor "" ⇒ Ground (see Decoration).
    #[serde(default, rename = "nbt_templates")]
    nbt_templates: Vec<String>,
    #[serde(default, rename = "anchor")]
    anchor: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFossilBbox {
    zone: String,
    name: String,
    center_xz: [i32; 2],
    center_y: i32,
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    #[serde(default)]
    max_units: u32,
    #[serde(default, rename = "mask_values")]
    _mask_values: Option<serde_json::Value>,
    #[serde(default, rename = "minerals")]
    _minerals: Option<serde_json::Value>,
}

// --- Public read-only views of manifest data ----------------------------

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Poi {
    pub zone: String,
    pub kind: String,
    pub name: String,
    pub pos_xyz: [f32; 3],
    pub tags: Vec<String>,
    pub unlock: String,
    pub qi_affinity: f32,
    pub danger_bias: i32,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Decoration {
    pub global_id: u32,
    pub profile: String,
    pub local_id: u32,
    pub name: String,
    pub kind: String,
    pub blocks: Vec<String>,
    pub(crate) resolved_blocks: Vec<BlockState>,
    pub size_range: [i32; 2],
    pub rarity: f32,
    pub notes: String,
    /// worldgen-v4 P6 §8.1 — relative paths (under `server/structures/`) of the
    /// authored NBT variants for this decoration. Empty ⇒ this decoration stays
    /// on the §8.1 #9 procedural path (mega_tree / decoration.rs / ground cover /
    /// aquatic / single-block flower); non-empty ⇒ the server stamps one variant
    /// chosen deterministically per placement from
    /// [`crate::world::terrain::nbt_registry::DecorationNbtRegistry`].
    pub nbt_templates: Vec<String>,
    /// How a stamped NBT template is positioned relative to the column surface.
    /// Parsed from the manifest `anchor` string at load (unknown / empty →
    /// [`DecorationAnchor::Ground`]).
    pub anchor: DecorationAnchor,
}

impl Decoration {
    /// True when this decoration is placed by stamping an authored NBT variant
    /// (vs. the procedural geometry path). A decoration with at least one
    /// template is NBT-driven; an empty list keeps it procedural.
    #[allow(dead_code)]
    pub fn is_nbt_driven(&self) -> bool {
        !self.nbt_templates.is_empty()
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FossilBbox {
    pub zone: String,
    pub name: String,
    pub center_xz: [i32; 2],
    pub center_y: i32,
    pub min_x: i32,
    pub max_x: i32,
    pub min_z: i32,
    pub max_z: i32,
    pub max_units: u32,
}

impl TerrainProvider {
    #[cfg(test)]
    pub(crate) fn empty_for_tests() -> Self {
        Self {
            tiles: HashMap::new(),
            tile_size: 16,
            world_bounds: Bounds2D {
                min_x: 0,
                max_x: 15,
                min_z: 0,
                max_z: 15,
            },
            surface_palette: vec![BlockState::STONE],
            biome_palette: vec![BiomeId::DEFAULT],
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
        }
    }

    /// Build the smallest mmap-backed provider that can materialize a real
    /// whalefall fossil node. The fixture intentionally accepts only one raster
    /// column: callers that need a wider terrain surface should use the on-disk
    /// raster fixture instead of silently relying on wilderness fallbacks.
    #[cfg(test)]
    pub(crate) fn with_fossil_for_tests(fossil: FossilBbox, mask: u8) -> Self {
        assert_ne!(mask, 0, "fossil test provider requires a non-zero mask");
        assert_eq!(
            fossil.min_x, fossil.max_x,
            "fossil test provider supports exactly one x column"
        );
        assert_eq!(
            fossil.min_z, fossil.max_z,
            "fossil test provider supports exactly one z column"
        );

        let mut spans = vec![0_u8; SPAN_STRIDE];
        for slot in 0..MAX_SPANS {
            let offset = slot * 4;
            spans[offset..offset + 2].copy_from_slice(&SPAN_SENTINEL.to_le_bytes());
            spans[offset + 2..offset + 4].copy_from_slice(&SPAN_SENTINEL.to_le_bytes());
        }
        let floor_y = i16::try_from(super::MIN_Y).expect("terrain MIN_Y must fit the span format");
        spans[0..2].copy_from_slice(&floor_y.to_le_bytes());
        spans[2..4].copy_from_slice(&64_i16.to_le_bytes());
        let zero_f32 = 0.0_f32.to_le_bytes();

        let tile = TileFields {
            spans_count: anonymous_mmap_for_tests(&[1]),
            spans: anonymous_mmap_for_tests(&spans),
            surface_id: anonymous_mmap_for_tests(&[0]),
            subsurface_id: anonymous_mmap_for_tests(&[0]),
            biome_id: anonymous_mmap_for_tests(&[0]),
            water_level: anonymous_mmap_for_tests(&zero_f32),
            feature_mask: anonymous_mmap_for_tests(&zero_f32),
            boundary_weight: anonymous_mmap_for_tests(&zero_f32),
            rift_axis_sdf: None,
            portal_anchor_sdf: None,
            rim_edge_mask: None,
            fracture_mask: None,
            neg_pressure: None,
            ruin_density: None,
            qi_density: None,
            mofa_decay: None,
            qi_vein_flow: None,
            spirit_eye_candidates: None,
            realm_collapse_mask: None,
            sky_island_mask: None,
            underground_tier: None,
            flora_density: None,
            flora_variant_id: None,
            ground_cover_density: None,
            ground_cover_id: None,
            zongmen_origin_id: None,
            mineral_density: None,
            mineral_kind: None,
            fossil_bbox: Some(anonymous_mmap_for_tests(&[mask])),
            anomaly_intensity: None,
            anomaly_kind: None,
            tsy_presence: None,
            tsy_origin_id: None,
            tsy_depth_tier: None,
        };
        let tile_key = (fossil.min_x, fossil.min_z);

        Self {
            tiles: HashMap::from([(tile_key, tile)]),
            tile_size: 1,
            world_bounds: Bounds2D {
                min_x: fossil.min_x,
                max_x: fossil.max_x,
                min_z: fossil.min_z,
                max_z: fossil.max_z,
            },
            fossil_bboxes: vec![fossil],
            ..Self::empty_for_tests()
        }
    }

    /// Build a `TerrainProvider` that already has a populated placement index.
    /// Used by P1 unit tests to verify bucket lookup without touching disk.
    #[cfg(test)]
    pub(crate) fn with_placement_index_for_tests(
        index: HashMap<ChunkPos, Vec<(BlockPos, BlockState)>>,
    ) -> Self {
        let count: usize = index.values().map(|v| v.len()).sum();
        Self {
            placement_index: index,
            placement_block_count: count,
            ..Self::empty_for_tests()
        }
    }

    pub fn load(
        manifest_path: &Path,
        raster_dir: &Path,
        biomes: &BiomeRegistry,
    ) -> Result<Self, String> {
        let nbt_preflight = super::nbt_registry::DecorationNbtRegistry::prepare_default();
        let mut diagnostics = nbt_preflight
            .diagnostics()
            .iter()
            .map(|diagnostic| format!("nbt: {diagnostic}"))
            .collect::<Vec<_>>();
        let provider = match Self::load_preflighted(
            manifest_path,
            raster_dir,
            biomes,
            nbt_preflight.candidate(),
        ) {
            Ok(provider) => Some(provider),
            Err(error) => {
                diagnostics.extend(error.diagnostics().iter().cloned());
                None
            }
        };
        if diagnostics.is_empty() {
            Ok(provider.expect("diagnostic-free raster preflight must produce a provider"))
        } else {
            Err(TerrainLoadError::new(diagnostics).to_string())
        }
    }

    pub(crate) fn load_preflighted(
        manifest_path: &Path,
        raster_dir: &Path,
        biomes: &BiomeRegistry,
        registry: &super::nbt_registry::DecorationNbtRegistry,
    ) -> Result<Self, TerrainLoadError> {
        let manifest_text = std::fs::read_to_string(manifest_path).map_err(|error| {
            TerrainLoadError::new([format!(
                "manifest: failed to read terrain raster manifest {}: {error}",
                manifest_path.display()
            )])
        })?;
        let manifest: RasterManifest = serde_json::from_str(&manifest_text).map_err(|error| {
            TerrainLoadError::new([format!(
                "manifest: failed to parse terrain raster manifest {}: {error}",
                manifest_path.display()
            )])
        })?;

        let mut diagnostics = Vec::new();
        if let Err(error) = validate_manifest_version(manifest.version, manifest_path) {
            diagnostics.push(format!("manifest: {error}"));
        }
        let bot_fixture = match validate_bot_fixture(manifest.bot_fixture, manifest_path) {
            Ok(fixture) => Some(fixture),
            Err(error) => {
                diagnostics.push(format!("manifest: {error}"));
                None
            }
        };
        let tile_area = match usize::try_from(manifest.tile_size) {
            Ok(tile_size) if tile_size > 0 => match tile_size.checked_mul(tile_size) {
                Some(tile_area) => Some(tile_area),
                None => {
                    diagnostics.push(
                        "manifest: tile_size squared overflowed while loading rasters".to_string(),
                    );
                    None
                }
            },
            _ => {
                diagnostics.push("manifest: tile_size must be positive".to_string());
                None
            }
        };
        let surface_palette =
            match resolve_surface_palette(&manifest.surface_palette, manifest_path) {
                Ok(palette) if !palette.is_empty() => Some(palette),
                Ok(_) => {
                    diagnostics.push("manifest: surface palette cannot be empty".to_string());
                    None
                }
                Err(error) => {
                    diagnostics.extend(prefix_multiline_diagnostics("surface", &error));
                    None
                }
            };
        let biome_palette = match manifest
            .biome_palette
            .iter()
            .map(|name| biome_id_from_name(name, biomes))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(palette) if !palette.is_empty() => Some(palette),
            Ok(_) => {
                diagnostics.push("manifest: biome palette cannot be empty".to_string());
                None
            }
            Err(error) => {
                diagnostics.push(format!("manifest: {error}"));
                None
            }
        };

        collect_decoration_template_diagnostics(
            &manifest.global_decoration_palette,
            registry,
            &mut diagnostics,
        );
        let decoration_palette = match resolve_decoration_palette(
            manifest.global_decoration_palette.clone(),
            manifest_path,
        ) {
            Ok(palette) => Some(palette),
            Err(error) => {
                diagnostics.extend(prefix_multiline_diagnostics("decoration", &error));
                None
            }
        };

        let sidecar_path = raster_dir.join("placement_manifest.json");
        let placement = match load_placement_index(&sidecar_path) {
            Ok(placement) => Some(placement),
            Err(error) => {
                diagnostics.extend(prefix_multiline_diagnostics("placement", &error));
                None
            }
        };

        let mut tiles = HashMap::with_capacity(manifest.tiles.len());
        if let Some(tile_area) = tile_area {
            for tile in &manifest.tiles {
                let tile_dir = raster_dir.join(&tile.dir);
                match TileFields::load(&tile_dir, &tile.layers, tile_area) {
                    Ok(tile_fields) => {
                        if !manifest.surface_palette.is_empty() {
                            collect_surface_palette_id_diagnostics(
                                tile,
                                &tile_fields,
                                manifest.surface_palette.len(),
                                &mut diagnostics,
                            );
                        }
                        tiles.insert((tile.tile_x, tile.tile_z), tile_fields);
                    }
                    Err(error) => diagnostics.push(format!("raster: {error}")),
                }
            }
        }

        if !diagnostics.is_empty() {
            return Err(TerrainLoadError::new(diagnostics));
        }

        let surface_palette = surface_palette.expect("validated surface palette must be present");
        let biome_palette = biome_palette.expect("validated biome palette must be present");
        let decoration_palette =
            decoration_palette.expect("validated decoration palette must be present");
        let (placement_index, placement_block_count) =
            placement.expect("validated placement index must be present");
        let default_wilderness_biome = biome_palette[0];
        let forest_wilderness_biome = biome_palette
            .get(7)
            .copied()
            .unwrap_or(default_wilderness_biome);
        let river_wilderness_biome = biome_palette
            .get(8)
            .copied()
            .unwrap_or(default_wilderness_biome);

        let pois = manifest_pois_into_runtime(manifest.pois);
        let anomaly_kinds = manifest
            .anomaly_kinds
            .into_iter()
            .filter_map(|(k, v)| k.parse::<u8>().ok().map(|id| (id, v)))
            .collect::<HashMap<u8, String>>();
        let abyssal_tier_floor_y = manifest
            .abyssal_tier_floor_y
            .into_iter()
            .filter_map(|(k, v)| k.parse::<u8>().ok().map(|tier| (tier, v)))
            .collect::<HashMap<u8, f32>>();
        let fossil_bboxes = manifest
            .fossil_bboxes
            .into_iter()
            .map(|raw| FossilBbox {
                zone: raw.zone,
                name: raw.name,
                center_xz: raw.center_xz,
                center_y: raw.center_y,
                min_x: raw.min_x,
                max_x: raw.max_x,
                min_z: raw.min_z,
                max_z: raw.max_z,
                max_units: raw.max_units,
            })
            .collect::<Vec<_>>();

        Ok(Self {
            tiles,
            tile_size: manifest.tile_size,
            world_bounds: Bounds2D {
                min_x: manifest.world_bounds.min_x,
                max_x: manifest.world_bounds.max_x,
                min_z: manifest.world_bounds.min_z,
                max_z: manifest.world_bounds.max_z,
            },
            surface_palette,
            biome_palette,
            default_wilderness_biome,
            forest_wilderness_biome,
            river_wilderness_biome,
            pois,
            anomaly_kinds,
            decoration_palette,
            abyssal_tier_floor_y,
            fossil_bboxes,
            placement_index,
            placement_block_count,
            bot_fixture: bot_fixture.expect("validated bot fixture result must be present"),
        })
    }

    /// Zone-scoped POI list from the worldgen blueprint.
    #[allow(dead_code)]
    pub fn pois(&self) -> &[Poi] {
        &self.pois
    }

    /// P1 — authored structure blocks for the given chunk position.
    ///
    /// Returns a slice of `(BlockPos, BlockState)` pairs pre-bucketed at load
    /// time. Empty when no placement manifest was found (向后兼容 / old manifests).
    pub fn placement_blocks_for_chunk(&self, chunk_pos: ChunkPos) -> &[(BlockPos, BlockState)] {
        self.placement_index
            .get(&chunk_pos)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Total authored placement blocks loaded from the sidecar (for logging).
    pub fn placement_block_count(&self) -> usize {
        self.placement_block_count
    }

    pub fn bot_fixture(&self) -> Option<&BotRasterFixture> {
        self.bot_fixture.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn set_bot_fixture_for_tests(&mut self, fixture: BotRasterFixture) {
        self.bot_fixture = Some(fixture);
    }

    /// Look up a decoration by its global id (0 → None).
    #[allow(dead_code)]
    pub fn decoration(&self, global_id: u8) -> Option<&Decoration> {
        self.decoration_palette
            .get(global_id as usize)
            .and_then(|o| o.as_ref())
    }

    /// Strict startup preflight for all manifest-owned block references and NBT
    /// template ids. The provider already stores lowered decoration states, so
    /// runtime flora placement cannot silently drop an unknown name.
    pub fn validate_decoration_templates(
        &self,
        registry: &super::nbt_registry::DecorationNbtRegistry,
    ) -> Result<(), Vec<String>> {
        let raw = self
            .decorations()
            .map(|decoration| ManifestDecoration {
                global_id: decoration.global_id,
                profile: decoration.profile.clone(),
                local_id: decoration.local_id,
                name: decoration.name.clone(),
                kind: decoration.kind.clone(),
                blocks: decoration.blocks.clone(),
                size_range: decoration.size_range,
                rarity: decoration.rarity,
                notes: decoration.notes.clone(),
                nbt_templates: decoration.nbt_templates.clone(),
                anchor: decoration.anchor.as_manifest().to_string(),
            })
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        collect_decoration_template_diagnostics(&raw, registry, &mut diagnostics);
        diagnostics.sort();
        diagnostics.dedup();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    #[allow(dead_code)]
    pub fn decorations(&self) -> impl Iterator<Item = &Decoration> {
        self.decoration_palette.iter().filter_map(Option::as_ref)
    }

    #[allow(dead_code)]
    pub fn decoration_by_name(&self, name: &str) -> Option<&Decoration> {
        self.decorations()
            .find(|decoration| decoration.name == name)
    }

    /// Total number of decorations in the global palette.
    #[allow(dead_code)]
    pub fn decoration_count(&self) -> usize {
        self.decoration_palette
            .iter()
            .filter(|d| d.is_some())
            .count()
    }

    #[allow(dead_code)]
    pub fn fossil_bboxes(&self) -> &[FossilBbox] {
        &self.fossil_bboxes
    }

    #[allow(dead_code)]
    pub fn sample_fossil_bbox(&self, world_x: i32, world_z: i32) -> u8 {
        self.sample(world_x, world_z).fossil_bbox
    }

    /// Human-readable name for an anomaly_kind enum value.
    #[allow(dead_code)]
    pub fn anomaly_name(&self, kind: u8) -> Option<&str> {
        self.anomaly_kinds.get(&kind).map(String::as_str)
    }

    /// Floor y for an abyssal tier (1..=3). None for tier 0 or unknown.
    #[allow(dead_code)]
    pub fn abyssal_tier_floor(&self, tier: u8) -> Option<f32> {
        self.abyssal_tier_floor_y.get(&tier).copied()
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn sample(&self, world_x: i32, world_z: i32) -> ColumnSample {
        let tile_x = world_x.div_euclid(self.tile_size);
        let tile_z = world_z.div_euclid(self.tile_size);

        let Some(tile) = self.tiles.get(&(tile_x, tile_z)) else {
            return wilderness::sample(
                world_x,
                world_z,
                self.default_wilderness_biome,
                self.forest_wilderness_biome,
                self.river_wilderness_biome,
            );
        };

        let local_x = world_x.rem_euclid(self.tile_size) as usize;
        let local_z = world_z.rem_euclid(self.tile_size) as usize;
        let index = local_z * self.tile_size as usize + local_x;

        let surface_index = read_u8(&tile.surface_id, index) as usize;
        let subsurface_index = read_u8(&tile.subsurface_id, index) as usize;
        let biome_id = read_u8(&tile.biome_id, index);
        let biome = self
            .biome_palette
            .get(biome_id as usize)
            .copied()
            .unwrap_or(self.default_wilderness_biome);

        ColumnSample {
            spans: decode_spans(&tile.spans_count, &tile.spans, index),
            surface_block: *self
                .surface_palette
                .get(surface_index)
                .unwrap_or(&BlockState::STONE),
            subsurface_block: *self
                .surface_palette
                .get(subsurface_index)
                .unwrap_or(&BlockState::STONE),
            biome_id,
            biome,
            water_level: read_f32(&tile.water_level, index),
            feature_mask: read_f32(&tile.feature_mask, index),
            boundary_weight: read_f32(&tile.boundary_weight, index),
            rift_axis_sdf: read_optional_f32(&tile.rift_axis_sdf, index, 99.0),
            portal_anchor_sdf: read_optional_f32(&tile.portal_anchor_sdf, index, 999.0),
            rim_edge_mask: read_optional_f32(&tile.rim_edge_mask, index, 0.0),
            fracture_mask: read_optional_f32(&tile.fracture_mask, index, 0.0),
            neg_pressure: read_optional_f32(&tile.neg_pressure, index, 0.0),
            ruin_density: read_optional_f32(&tile.ruin_density, index, 0.0),
            qi_density: read_optional_f32(&tile.qi_density, index, 0.12),
            mofa_decay: read_optional_f32(&tile.mofa_decay, index, 0.40),
            qi_vein_flow: read_optional_f32(&tile.qi_vein_flow, index, 0.0),
            spirit_eye_candidates: read_optional_u8(&tile.spirit_eye_candidates, index, 0),
            realm_collapse_mask: read_optional_u8(&tile.realm_collapse_mask, index, 0),
            sky_island_mask: read_optional_f32(&tile.sky_island_mask, index, 0.0),
            underground_tier: read_optional_u8(&tile.underground_tier, index, 0),
            flora_density: read_optional_f32(&tile.flora_density, index, 0.0),
            flora_variant_id: read_optional_u8(&tile.flora_variant_id, index, 0),
            ground_cover_density: read_optional_f32(&tile.ground_cover_density, index, 0.0),
            ground_cover_id: read_optional_u8(&tile.ground_cover_id, index, 0),
            zongmen_origin_id: read_optional_u8(&tile.zongmen_origin_id, index, 0),
            mineral_density: read_optional_f32(&tile.mineral_density, index, 0.0),
            mineral_kind: read_optional_u8(&tile.mineral_kind, index, 0),
            fossil_bbox: read_optional_u8(&tile.fossil_bbox, index, 0),
            anomaly_intensity: read_optional_f32(&tile.anomaly_intensity, index, 0.0),
            anomaly_kind: read_optional_u8(&tile.anomaly_kind, index, 0),
            tsy_presence: read_optional_u8(&tile.tsy_presence, index, 0),
            tsy_origin_id: read_optional_u8(&tile.tsy_origin_id, index, 0),
            tsy_depth_tier: read_optional_u8(&tile.tsy_depth_tier, index, 0),
        }
    }

    #[allow(dead_code)]
    pub fn layer_names() -> &'static [LayerSchema] {
        LAYER_SCHEMAS
    }

    #[allow(dead_code)]
    pub fn sample_layer_f32(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        let schema = layer_schema(layer_name)?;
        let fallback = schema.safe_default_f32?;
        let Some((tile, index)) = self.tile_and_index(world_x, world_z) else {
            return Some(fallback);
        };

        Some(read_tile_layer_f32(tile, index, layer_name, fallback))
    }

    #[allow(dead_code)]
    pub fn sample_layer_u8(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<u8> {
        let schema = layer_schema(layer_name)?;
        let fallback = schema.safe_default_u8?;
        let Some((tile, index)) = self.tile_and_index(world_x, world_z) else {
            return Some(fallback);
        };

        Some(read_tile_layer_u8(tile, index, layer_name, fallback))
    }

    pub fn sample_layer(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        let schema = layer_schema(layer_name)?;
        let (tile, index) = self.tile_and_index(world_x, world_z)?;
        match schema.export_type {
            LayerExportType::F32 => Some(read_tile_layer_f32(
                tile,
                index,
                layer_name,
                schema
                    .safe_default_f32
                    .expect("f32 schema should carry f32 default"),
            )),
            LayerExportType::U8 => Some(f32::from(read_tile_layer_u8(
                tile,
                index,
                layer_name,
                schema
                    .safe_default_u8
                    .expect("u8 schema should carry u8 default"),
            ))),
        }
    }

    fn tile_and_index(&self, world_x: i32, world_z: i32) -> Option<(&TileFields, usize)> {
        let tile_x = world_x.div_euclid(self.tile_size);
        let tile_z = world_z.div_euclid(self.tile_size);
        let tile = self.tiles.get(&(tile_x, tile_z))?;
        let local_x = world_x.rem_euclid(self.tile_size) as usize;
        let local_z = world_z.rem_euclid(self.tile_size) as usize;
        let index = local_z * self.tile_size as usize + local_x;
        Some((tile, index))
    }
}

fn read_tile_layer_f32(tile: &TileFields, index: usize, layer_name: &str, fallback: f32) -> f32 {
    match layer_name {
        // height.bin no longer exists on disk (folded into spans, §8.1 #1).
        // The registry still lists `height` so the Python↔Rust mirror matches;
        // route generic height queries to the surface span ceiling.
        "height" => decode_spans(&tile.spans_count, &tile.spans, index)
            .first()
            .map(|(_floor, ceiling)| f32::from(*ceiling))
            .unwrap_or(fallback),
        "water_level" => read_f32(&tile.water_level, index),
        "feature_mask" => read_f32(&tile.feature_mask, index),
        "boundary_weight" => read_f32(&tile.boundary_weight, index),
        "rift_axis_sdf" => read_optional_f32(&tile.rift_axis_sdf, index, fallback),
        "portal_anchor_sdf" => read_optional_f32(&tile.portal_anchor_sdf, index, fallback),
        "rim_edge_mask" => read_optional_f32(&tile.rim_edge_mask, index, fallback),
        "fracture_mask" => read_optional_f32(&tile.fracture_mask, index, fallback),
        "neg_pressure" => read_optional_f32(&tile.neg_pressure, index, fallback),
        "ruin_density" => read_optional_f32(&tile.ruin_density, index, fallback),
        "qi_density" => read_optional_f32(&tile.qi_density, index, fallback),
        "mofa_decay" => read_optional_f32(&tile.mofa_decay, index, fallback),
        "qi_vein_flow" => read_optional_f32(&tile.qi_vein_flow, index, fallback),
        "sky_island_mask" => read_optional_f32(&tile.sky_island_mask, index, fallback),
        "flora_density" => read_optional_f32(&tile.flora_density, index, fallback),
        "ground_cover_density" => read_optional_f32(&tile.ground_cover_density, index, fallback),
        "mineral_density" => read_optional_f32(&tile.mineral_density, index, fallback),
        "anomaly_intensity" => read_optional_f32(&tile.anomaly_intensity, index, fallback),
        _ => unreachable!("schema export type should match f32 layer"),
    }
}

fn read_tile_layer_u8(tile: &TileFields, index: usize, layer_name: &str, fallback: u8) -> u8 {
    match layer_name {
        "surface_id" => read_u8(&tile.surface_id, index),
        "subsurface_id" => read_u8(&tile.subsurface_id, index),
        "biome_id" => read_u8(&tile.biome_id, index),
        "spirit_eye_candidates" => read_optional_u8(&tile.spirit_eye_candidates, index, fallback),
        "realm_collapse_mask" => read_optional_u8(&tile.realm_collapse_mask, index, fallback),
        "underground_tier" => read_optional_u8(&tile.underground_tier, index, fallback),
        "flora_variant_id" => read_optional_u8(&tile.flora_variant_id, index, fallback),
        "ground_cover_id" => read_optional_u8(&tile.ground_cover_id, index, fallback),
        "zongmen_origin_id" => read_optional_u8(&tile.zongmen_origin_id, index, fallback),
        "mineral_kind" => read_optional_u8(&tile.mineral_kind, index, fallback),
        "fossil_bbox" => read_optional_u8(&tile.fossil_bbox, index, fallback),
        "anomaly_kind" => read_optional_u8(&tile.anomaly_kind, index, fallback),
        "tsy_presence" => read_optional_u8(&tile.tsy_presence, index, fallback),
        "tsy_origin_id" => read_optional_u8(&tile.tsy_origin_id, index, fallback),
        "tsy_depth_tier" => read_optional_u8(&tile.tsy_depth_tier, index, fallback),
        _ => unreachable!("schema export type should match u8 layer"),
    }
}

fn collect_surface_palette_id_diagnostics(
    tile: &ManifestTile,
    fields: &TileFields,
    palette_len: usize,
    diagnostics: &mut Vec<String>,
) {
    for (layer_name, bytes) in [
        ("surface_id", &fields.surface_id),
        ("subsurface_id", &fields.subsurface_id),
    ] {
        for (index, value) in bytes.iter().copied().enumerate() {
            if usize::from(value) >= palette_len {
                diagnostics.push(format!(
                    "raster: tile ({},{}) '{}' layer {layer_name} index {index} has palette id {value}, but surface palette length is {palette_len}",
                    tile.tile_x, tile.tile_z, tile.dir
                ));
            }
        }
    }
}

impl TileFields {
    fn load(tile_dir: &Path, layers: &[String], tile_area: usize) -> Result<Self, String> {
        let area4 = tile_area * 4;
        Ok(Self {
            // worldgen-v4 P0 §8.1 #1: spans_count.bin is u8/col (tile_area bytes);
            // spans.bin is SPAN_STRIDE bytes/col. Both replace height.bin.
            spans_count: map_required_layer(tile_dir, "spans_count.bin", tile_area)?,
            spans: map_required_layer(tile_dir, "spans.bin", tile_area * SPAN_STRIDE)?,
            surface_id: map_required_layer(tile_dir, "surface_id.bin", tile_area)?,
            subsurface_id: map_required_layer(tile_dir, "subsurface_id.bin", tile_area)?,
            biome_id: map_required_layer(tile_dir, "biome_id.bin", tile_area)?,
            water_level: map_required_layer(tile_dir, "water_level.bin", area4)?,
            feature_mask: map_required_layer(tile_dir, "feature_mask.bin", area4)?,
            boundary_weight: map_required_layer(tile_dir, "boundary_weight.bin", area4)?,
            rift_axis_sdf: map_optional_layer(tile_dir, layers, "rift_axis_sdf", area4)?,
            portal_anchor_sdf: map_optional_layer(tile_dir, layers, "portal_anchor_sdf", area4)?,
            rim_edge_mask: map_optional_layer(tile_dir, layers, "rim_edge_mask", area4)?,
            fracture_mask: map_optional_layer(tile_dir, layers, "fracture_mask", area4)?,
            neg_pressure: map_optional_layer(tile_dir, layers, "neg_pressure", area4)?,
            ruin_density: map_optional_layer(tile_dir, layers, "ruin_density", area4)?,
            qi_density: map_optional_layer(tile_dir, layers, "qi_density", area4)?,
            mofa_decay: map_optional_layer(tile_dir, layers, "mofa_decay", area4)?,
            qi_vein_flow: map_optional_layer(tile_dir, layers, "qi_vein_flow", area4)?,
            spirit_eye_candidates: map_optional_layer(
                tile_dir,
                layers,
                "spirit_eye_candidates",
                tile_area,
            )?,
            realm_collapse_mask: map_optional_layer(
                tile_dir,
                layers,
                "realm_collapse_mask",
                tile_area,
            )?,
            sky_island_mask: map_optional_layer(tile_dir, layers, "sky_island_mask", area4)?,
            underground_tier: map_optional_layer(tile_dir, layers, "underground_tier", tile_area)?,
            flora_density: map_optional_layer(tile_dir, layers, "flora_density", area4)?,
            flora_variant_id: map_optional_layer(tile_dir, layers, "flora_variant_id", tile_area)?,
            ground_cover_density: map_optional_layer(
                tile_dir,
                layers,
                "ground_cover_density",
                area4,
            )?,
            ground_cover_id: map_optional_layer(tile_dir, layers, "ground_cover_id", tile_area)?,
            zongmen_origin_id: map_optional_layer(
                tile_dir,
                layers,
                "zongmen_origin_id",
                tile_area,
            )?,
            mineral_density: map_optional_layer(tile_dir, layers, "mineral_density", area4)?,
            mineral_kind: map_optional_layer(tile_dir, layers, "mineral_kind", tile_area)?,
            fossil_bbox: map_optional_layer(tile_dir, layers, "fossil_bbox", tile_area)?,
            anomaly_intensity: map_optional_layer(tile_dir, layers, "anomaly_intensity", area4)?,
            anomaly_kind: map_optional_layer(tile_dir, layers, "anomaly_kind", tile_area)?,
            tsy_presence: map_optional_layer(tile_dir, layers, "tsy_presence", tile_area)?,
            tsy_origin_id: map_optional_layer(tile_dir, layers, "tsy_origin_id", tile_area)?,
            tsy_depth_tier: map_optional_layer(tile_dir, layers, "tsy_depth_tier", tile_area)?,
        })
    }
}

fn map_required_layer(
    tile_dir: &Path,
    file_name: &str,
    expected_len: usize,
) -> Result<Mmap, String> {
    let path = tile_dir.join(file_name);
    map_file(&path, expected_len)
}

fn map_optional_layer(
    tile_dir: &Path,
    layers: &[String],
    layer_name: &str,
    expected_len: usize,
) -> Result<Option<Mmap>, String> {
    if !layers.iter().any(|layer| layer == layer_name) {
        return Ok(None);
    }
    map_file(&tile_dir.join(format!("{layer_name}.bin")), expected_len).map(Some)
}

fn map_file(path: &Path, expected_len: usize) -> Result<Mmap, String> {
    let file = File::open(path)
        .map_err(|error| format!("failed to open raster layer {}: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to stat raster layer {}: {error}", path.display()))?;
    if metadata.len() as usize != expected_len {
        return Err(format!(
            "raster layer {} has {} bytes, expected {}",
            path.display(),
            metadata.len(),
            expected_len
        ));
    }

    unsafe { Mmap::map(&file) }
        .map_err(|error| format!("failed to mmap raster layer {}: {error}", path.display()))
}

#[cfg(test)]
fn anonymous_mmap_for_tests(bytes: &[u8]) -> Mmap {
    let mut mmap = memmap2::MmapMut::map_anon(bytes.len())
        .expect("anonymous test raster mmap should allocate");
    mmap.copy_from_slice(bytes);
    mmap.make_read_only()
        .expect("anonymous test raster mmap should become read-only")
}

fn read_u8(bytes: &Mmap, index: usize) -> u8 {
    bytes[index]
}

fn read_f32(bytes: &Mmap, index: usize) -> f32 {
    let offset = index * 4;
    let slice = &bytes[offset..offset + 4];
    f32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]])
}

fn read_optional_f32(bytes: &Option<Mmap>, index: usize, fallback: f32) -> f32 {
    bytes
        .as_ref()
        .map(|mmap| read_f32(mmap, index))
        .unwrap_or(fallback)
}

fn read_optional_u8(bytes: &Option<Mmap>, index: usize, fallback: u8) -> u8 {
    bytes
        .as_ref()
        .map(|mmap| read_u8(mmap, index))
        .unwrap_or(fallback)
}

fn prefix_multiline_diagnostics(prefix: &str, error: &str) -> Vec<String> {
    let mut lines = error.lines();
    let context = lines.next().unwrap_or(error);
    let details = lines
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(|detail| format!("{prefix}: {context}: {detail}"))
        .collect::<Vec<_>>();
    if details.is_empty() {
        vec![format!("{prefix}: {error}")]
    } else {
        details
    }
}

fn collect_decoration_template_diagnostics(
    decorations: &[ManifestDecoration],
    registry: &super::nbt_registry::DecorationNbtRegistry,
    diagnostics: &mut Vec<String>,
) {
    for decoration in decorations {
        for (template_index, template_id) in decoration.nbt_templates.iter().enumerate() {
            if template_id.is_empty()
                || template_id.starts_with('/')
                || template_id.contains("..")
                || !template_id.starts_with("decorations/")
                || !template_id.ends_with(".nbt")
            {
                diagnostics.push(format!(
                    "nbt-reference: decoration '{}' (global_id {}) nbt_templates #{} has invalid template id '{}'",
                    decoration.name,
                    decoration.global_id,
                    template_index + 1,
                    template_id
                ));
            } else if !registry.contains(template_id) {
                diagnostics.push(format!(
                    "nbt-reference: decoration '{}' (global_id {}) nbt_templates #{} references missing resident template '{}'",
                    decoration.name,
                    decoration.global_id,
                    template_index + 1,
                    template_id
                ));
            }
        }
    }
}

fn resolve_decoration_palette(
    raw_decorations: Vec<ManifestDecoration>,
    manifest_path: &Path,
) -> Result<Vec<Option<Decoration>>, String> {
    let mut diagnostics = Vec::new();
    let max_id = raw_decorations
        .iter()
        .filter_map(|decoration| {
            if decoration.global_id == 0 || decoration.global_id > u8::MAX.into() {
                diagnostics.push(format!(
                    "decoration '{}' has invalid global_id {} (ids must be in 1..={})",
                    decoration.name,
                    decoration.global_id,
                    u8::MAX
                ));
                None
            } else {
                Some(decoration.global_id)
            }
        })
        .max()
        .unwrap_or(0);
    let mut palette: Vec<Option<Decoration>> = vec![None; max_id as usize + 1];

    for raw in raw_decorations {
        if raw.global_id == 0 || raw.global_id > u8::MAX.into() {
            continue;
        }
        let id = raw.global_id as usize;
        let mut resolved_blocks = Vec::with_capacity(raw.blocks.len());
        for (block_index, block_name) in raw.blocks.iter().enumerate() {
            match block_state_from_name(block_name) {
                Ok(state) => resolved_blocks.push(state),
                Err(error) => diagnostics.push(format!(
                    "decoration '{}' (global_id {}) block #{}: {error}",
                    raw.name,
                    raw.global_id,
                    block_index + 1
                )),
            }
        }
        if raw.blocks.is_empty() {
            diagnostics.push(format!(
                "decoration '{}' (global_id {}) must declare at least one procedural block",
                raw.name, raw.global_id
            ));
        }
        if palette[id].is_some() {
            diagnostics.push(format!(
                "duplicate decoration global_id {} at decoration '{}'",
                raw.global_id, raw.name
            ));
            continue;
        }
        palette[id] = Some(Decoration {
            global_id: raw.global_id,
            profile: raw.profile,
            local_id: raw.local_id,
            name: raw.name,
            kind: raw.kind,
            blocks: raw.blocks,
            resolved_blocks,
            size_range: raw.size_range,
            rarity: raw.rarity,
            notes: raw.notes,
            nbt_templates: raw.nbt_templates,
            anchor: DecorationAnchor::from_manifest(&raw.anchor),
        });
    }

    if diagnostics.is_empty() {
        Ok(palette)
    } else {
        Err(format!(
            "terrain raster manifest {} has invalid decoration palette:\n- {}",
            manifest_path.display(),
            diagnostics.join("\n- ")
        ))
    }
}

fn block_state_from_name(name: &str) -> Result<BlockState, String> {
    super::blocks::block_from_name(name).ok_or_else(|| {
        format!(
            "unknown surface palette block '{name}' (not declared in the canonical terrain block catalog)"
        )
    })
}

fn resolve_surface_palette(
    names: &[String],
    manifest_path: &Path,
) -> Result<Vec<BlockState>, String> {
    let mut states = Vec::with_capacity(names.len());
    let mut diagnostics = Vec::new();
    for (index, name) in names.iter().enumerate() {
        match block_state_from_name(name) {
            Ok(state) => states.push(state),
            Err(error) => diagnostics.push(format!("surface_palette #{}: {error}", index + 1)),
        }
    }
    if diagnostics.is_empty() {
        Ok(states)
    } else {
        Err(format!(
            "terrain raster manifest {} has invalid surface palette:\n- {}",
            manifest_path.display(),
            diagnostics.join("\n- ")
        ))
    }
}

fn biome_id_from_name(name: &str, biomes: &BiomeRegistry) -> Result<BiomeId, String> {
    let ident = Ident::new(name).map_err(|error| {
        format!("invalid biome identifier '{name}' in terrain raster manifest: {error}")
    })?;
    biomes
        .index_of(ident.as_str_ident())
        .ok_or_else(|| format!("unknown biome '{name}' in terrain raster manifest"))
}

pub fn raster_dir_from_manifest_path(manifest_path: &Path) -> Result<PathBuf, String> {
    manifest_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "manifest path {} has no parent directory",
                manifest_path.display()
            )
        })
}

// ---------------------------------------------------------------------------
// P1 — placement manifest loading helpers
// ---------------------------------------------------------------------------

const EXPECTED_PLACEMENT_MANIFEST_VERSION: u32 = 1;

type PlacementIndex = HashMap<ChunkPos, Vec<(BlockPos, BlockState)>>;
type PlacementLoadResult = Result<(PlacementIndex, usize), String>;
type PlacementBuildResult = Result<(PlacementIndex, usize), Vec<String>>;

/// Load `placement_manifest.json` and pre-bucket all authored blocks by
/// `ChunkPos`. Only a genuinely missing sidecar is backward-compatible; a file
/// that exists but cannot be read, decoded, parsed, versioned, or lowered is a
/// fatal startup error.
pub(crate) fn load_placement_index(sidecar_path: &Path) -> PlacementLoadResult {
    let mut file = match super::nbt_io::open_regular_file_no_follow(sidecar_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((HashMap::new(), 0));
        }
        Err(error) => {
            return Err(format!(
                "failed to open placement sidecar {} as a regular file without following symlinks: {error}",
                sidecar_path.display()
            ));
        }
    };

    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|error| {
        format!(
            "failed to read placement sidecar {}: {error}",
            sidecar_path.display()
        )
    })?;

    let manifest: PlacementManifest = serde_json::from_str(&text).map_err(|error| {
        format!(
            "failed to parse placement sidecar {}: {error}",
            sidecar_path.display()
        )
    })?;
    if manifest.version != EXPECTED_PLACEMENT_MANIFEST_VERSION {
        return Err(format!(
            "placement sidecar {} has unsupported version {} (expected {})",
            sidecar_path.display(),
            manifest.version,
            EXPECTED_PLACEMENT_MANIFEST_VERSION
        ));
    }

    build_placement_index(manifest).map_err(|diagnostics| {
        format!(
            "placement sidecar {} failed validation:\n- {}",
            sidecar_path.display(),
            diagnostics.join("\n- ")
        )
    })
}

/// Convert a placement manifest into a `ChunkPos`-keyed lookup table. Validation
/// is atomic: every invalid block/property is collected in deterministic source
/// order and no partial index is returned.
pub fn build_placement_index(manifest: PlacementManifest) -> PlacementBuildResult {
    let mut candidate: HashMap<ChunkPos, Vec<(BlockPos, BlockState)>> = HashMap::new();
    let mut candidate_total = 0;
    let mut diagnostics = Vec::new();

    for (structure_index, structure) in manifest.structures.into_iter().enumerate() {
        for (block_index, block) in structure.blocks.into_iter().enumerate() {
            let [x, y, z] = block.pos;
            let mut properties = block.properties.iter().collect::<Vec<_>>();
            properties.sort_by(|left, right| left.0.cmp(right.0));
            let property_pairs = properties
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()));
            let block_state =
                match super::blocks::block_state_with_properties(&block.block, property_pairs) {
                    Ok(state) => state,
                    Err(error) => {
                        diagnostics.push(format!(
                            "structure #{}, block #{} '{}' at [{x},{y},{z}]: {error}",
                            structure_index + 1,
                            block_index + 1,
                            block.block
                        ));
                        continue;
                    }
                };
            let block_pos = BlockPos::new(x, y, z);
            let chunk_pos = ChunkPos::new(x.div_euclid(16), z.div_euclid(16));
            candidate
                .entry(chunk_pos)
                .or_default()
                .push((block_pos, block_state));
            candidate_total += 1;
        }
    }

    if diagnostics.is_empty() {
        Ok((candidate, candidate_total))
    } else {
        Err(diagnostics)
    }
}

/// Strict public lowering helper for placement/NBT-style names. The optional
/// `minecraft:` namespace is accepted once; all validation is delegated to the
/// shared catalog property lowerer.
#[cfg(test)]
pub fn block_state_from_placement(
    name: &str,
    properties: &HashMap<String, String>,
) -> Result<BlockState, super::blocks::BlockStateResolveError> {
    let mut properties = properties.iter().collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(right.0));
    super::blocks::block_state_with_properties(
        name,
        properties
            .into_iter()
            .map(|(property, value)| (property.as_str(), value.as_str())),
    )
}

#[cfg(test)]
mod tests {
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
        let tile = TileFields::load(&tile_dir, &layers, tile_area)
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
        fs::write(tile_dir.join("spans.bin"), spans_bytes)
            .expect("test spans.bin should be writable");
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
            ",\n            \"bot_fixture\": {\"kind\":\"ambient-surface-v1\",\"token\":\"0123456789abcdef\"}\n        }",
        );
        let fixture_manifest: RasterManifest = serde_json::from_str(&with_fixture)
            .expect("valid bot_fixture metadata must deserialize");
        let fixture =
            validate_bot_fixture(fixture_manifest.bot_fixture, Path::new("manifest.json"))
                .expect("valid bot fixture must pass validation")
                .expect("fixture must remain present");
        assert_eq!(fixture.kind, "ambient-surface-v1");
        assert_eq!(fixture.token, "0123456789abcdef");

        for (kind, token) in [
            ("other", "0123456789abcdef"),
            ("ambient-surface-v1", "short"),
            ("ambient-surface-v1", "0123456789abcde\n"),
            ("ambient-surface-v1", "0123456789abcde!"),
        ] {
            let fixture = ManifestBotFixture {
                kind: kind.to_string(),
                token: token.to_string(),
            };
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
            .filter(|diagnostic| diagnostic.contains("surface palette length is 2"))
            .collect::<Vec<_>>();
        assert_eq!(
            palette_diagnostics.len(),
            4,
            "two bad surface ids and two bad subsurface ids must all survive aggregation: {error}"
        );
        for expected in [
            "surface_id index 1 has palette id 2",
            "surface_id index 2 has palette id 3",
            "subsurface_id index 0 has palette id 4",
            "subsurface_id index 2 has palette id 5",
        ] {
            assert!(
                palette_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.contains(expected)
                        && diagnostic.contains("tile (0,0) 'tile_0_0'")),
                "palette-id diagnostic must identify tile, layer, index, value, and palette length for {expected:?}: {error}"
            );
        }

        let _ = fs::remove_dir_all(&root);
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
            deco.anchor,
            DecorationAnchor::Embedded,
            "manifest anchor 'embedded' must lower to DecorationAnchor::Embedded on the public Decoration"
        );
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
            .validate_decoration_templates(
                &super::super::nbt_registry::DecorationNbtRegistry::empty(),
            )
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
        assert!(
            block_state_from_placement("minecraft:unknown_block_xyz", &HashMap::new()).is_err()
        );

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
            special_error.contains("must be a regular file")
                && special_error.contains("special node"),
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
        let fifo_error = load_placement_index(&path).expect_err(
            "nonblocking no-follow open must reject a FIFO without waiting for a writer",
        );
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

        let tile = TileFields::load(&tile_dir, &[], area).expect("spans tile should load");
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
        let tile = TileFields::load(&tile_dir, &optional, area).expect("botany tile loads");
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
}
