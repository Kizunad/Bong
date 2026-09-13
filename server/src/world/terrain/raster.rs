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
    // The bot harness independently verifies these authored values against the
    // raster bytes. Runtime only publishes kind/token, but strict manifest
    // admission must still acknowledge the producer's complete evidence shape.
    #[serde(rename = "surface_y")]
    _surface_y: i32,
    #[serde(rename = "support")]
    _support: String,
    #[serde(rename = "feet_y")]
    _feet_y: i32,
    #[serde(rename = "head_y")]
    _head_y: i32,
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
        let tile_area = match manifest.tile_size {
            tile_size if tile_size > 0 => match tile_size
                .checked_mul(tile_size)
                .and_then(|area| usize::try_from(area).ok())
            {
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
        let mut resolved_biomes = Vec::with_capacity(manifest.biome_palette.len());
        let mut biome_errors = Vec::new();
        for (index, name) in manifest.biome_palette.iter().enumerate() {
            match biome_id_from_name(name, biomes) {
                Ok(id) => resolved_biomes.push(id),
                Err(error) => biome_errors.push(format!(
                    "manifest: biome_palette #{} '{}': {error}",
                    index + 1,
                    name
                )),
            }
        }
        let biome_palette = if !biome_errors.is_empty() {
            diagnostics.extend(biome_errors);
            None
        } else if resolved_biomes.is_empty() {
            diagnostics.push("manifest: biome palette cannot be empty".to_string());
            None
        } else {
            Some(resolved_biomes)
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

        let raster_root = match std::fs::canonicalize(raster_dir) {
            Ok(root) => Some(root),
            Err(error) => {
                diagnostics.push(format!(
                    "raster: failed to anchor raster directory {}: {error}",
                    raster_dir.display()
                ));
                None
            }
        };
        let mut tiles = HashMap::with_capacity(manifest.tiles.len());
        if let (Some(tile_area), Some(raster_root)) = (tile_area, raster_root.as_deref()) {
            for tile in &manifest.tiles {
                for layer_name in &tile.layers {
                    if layer_schema(layer_name).is_none() {
                        diagnostics.push(format!(
                            "raster: tile ({},{}) '{}' declares unknown layer '{}', not present in the canonical layer registry",
                            tile.tile_x, tile.tile_z, tile.dir, layer_name
                        ));
                    }
                }
                let tile_dir = raster_dir.join(&tile.dir);
                match TileFields::load(&tile_dir, raster_root, &tile.layers, tile_area) {
                    Ok(tile_fields) => {
                        collect_palette_id_diagnostics(
                            tile,
                            "surface_id",
                            &tile_fields.surface_id,
                            "surface palette",
                            manifest.surface_palette.len(),
                            &mut diagnostics,
                        );
                        collect_palette_id_diagnostics(
                            tile,
                            "subsurface_id",
                            &tile_fields.subsurface_id,
                            "surface palette",
                            manifest.surface_palette.len(),
                            &mut diagnostics,
                        );
                        collect_palette_id_diagnostics(
                            tile,
                            "biome_id",
                            &tile_fields.biome_id,
                            "biome palette",
                            manifest.biome_palette.len(),
                            &mut diagnostics,
                        );
                        for (layer_name, bytes) in [
                            ("flora_variant_id", tile_fields.flora_variant_id.as_ref()),
                            ("ground_cover_id", tile_fields.ground_cover_id.as_ref()),
                        ] {
                            if let Some(bytes) = bytes {
                                collect_decoration_palette_id_diagnostics(
                                    tile,
                                    layer_name,
                                    bytes,
                                    decoration_palette.as_deref().unwrap_or(&[]),
                                    &mut diagnostics,
                                );
                            }
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

fn collect_palette_id_diagnostics(
    tile: &ManifestTile,
    layer_name: &str,
    bytes: &Mmap,
    palette_name: &str,
    palette_len: usize,
    diagnostics: &mut Vec<String>,
) {
    const MAX_EXAMPLES: usize = 8;
    let mut invalid_count = 0usize;
    let mut examples = Vec::new();
    for (index, value) in bytes.iter().copied().enumerate() {
        if usize::from(value) >= palette_len {
            invalid_count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(format!("index {index} has palette id {value}"));
            }
        }
    }
    if invalid_count > 0 {
        diagnostics.push(format!(
            "raster: tile ({},{}) '{}' layer {layer_name} has {invalid_count} ids outside {palette_name} length {palette_len}; first {}: {}",
            tile.tile_x,
            tile.tile_z,
            tile.dir,
            examples.len(),
            examples.join(", ")
        ));
    }
}

fn collect_decoration_palette_id_diagnostics(
    tile: &ManifestTile,
    layer_name: &str,
    bytes: &Mmap,
    palette: &[Option<Decoration>],
    diagnostics: &mut Vec<String>,
) {
    const MAX_EXAMPLES: usize = 8;
    let mut invalid_count = 0usize;
    let mut examples = Vec::new();
    for (index, value) in bytes.iter().copied().enumerate() {
        if value == 0 {
            continue;
        }
        if !palette.get(usize::from(value)).is_some_and(Option::is_some) {
            invalid_count += 1;
            if examples.len() < MAX_EXAMPLES {
                examples.push(format!(
                    "index {index} has unoccupied decoration id {value}"
                ));
            }
        }
    }
    if invalid_count > 0 {
        diagnostics.push(format!(
            "raster: tile ({},{}) '{}' layer {layer_name} has {invalid_count} ids without resident decoration palette entries; first {}: {}",
            tile.tile_x,
            tile.tile_z,
            tile.dir,
            examples.len(),
            examples.join(", ")
        ));
    }
}

impl TileFields {
    fn load(
        tile_dir: &Path,
        raster_root: &Path,
        layers: &[String],
        tile_area: usize,
    ) -> Result<Self, String> {
        let area4 = tile_area * 4;
        Ok(Self {
            // worldgen-v4 P0 §8.1 #1: spans_count.bin is u8/col (tile_area bytes);
            // spans.bin is SPAN_STRIDE bytes/col. Both replace height.bin.
            spans_count: map_required_layer(tile_dir, raster_root, "spans_count.bin", tile_area)?,
            spans: map_required_layer(tile_dir, raster_root, "spans.bin", tile_area * SPAN_STRIDE)?,
            surface_id: map_required_layer(tile_dir, raster_root, "surface_id.bin", tile_area)?,
            subsurface_id: map_required_layer(
                tile_dir,
                raster_root,
                "subsurface_id.bin",
                tile_area,
            )?,
            biome_id: map_required_layer(tile_dir, raster_root, "biome_id.bin", tile_area)?,
            water_level: map_required_layer(tile_dir, raster_root, "water_level.bin", area4)?,
            feature_mask: map_required_layer(tile_dir, raster_root, "feature_mask.bin", area4)?,
            boundary_weight: map_required_layer(
                tile_dir,
                raster_root,
                "boundary_weight.bin",
                area4,
            )?,
            rift_axis_sdf: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "rift_axis_sdf",
                area4,
            )?,
            portal_anchor_sdf: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "portal_anchor_sdf",
                area4,
            )?,
            rim_edge_mask: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "rim_edge_mask",
                area4,
            )?,
            fracture_mask: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "fracture_mask",
                area4,
            )?,
            neg_pressure: map_optional_layer(tile_dir, raster_root, layers, "neg_pressure", area4)?,
            ruin_density: map_optional_layer(tile_dir, raster_root, layers, "ruin_density", area4)?,
            qi_density: map_optional_layer(tile_dir, raster_root, layers, "qi_density", area4)?,
            mofa_decay: map_optional_layer(tile_dir, raster_root, layers, "mofa_decay", area4)?,
            qi_vein_flow: map_optional_layer(tile_dir, raster_root, layers, "qi_vein_flow", area4)?,
            spirit_eye_candidates: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "spirit_eye_candidates",
                tile_area,
            )?,
            realm_collapse_mask: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "realm_collapse_mask",
                tile_area,
            )?,
            sky_island_mask: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "sky_island_mask",
                area4,
            )?,
            underground_tier: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "underground_tier",
                tile_area,
            )?,
            flora_density: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "flora_density",
                area4,
            )?,
            flora_variant_id: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "flora_variant_id",
                tile_area,
            )?,
            ground_cover_density: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "ground_cover_density",
                area4,
            )?,
            ground_cover_id: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "ground_cover_id",
                tile_area,
            )?,
            zongmen_origin_id: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "zongmen_origin_id",
                tile_area,
            )?,
            mineral_density: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "mineral_density",
                area4,
            )?,
            mineral_kind: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "mineral_kind",
                tile_area,
            )?,
            fossil_bbox: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "fossil_bbox",
                tile_area,
            )?,
            anomaly_intensity: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "anomaly_intensity",
                area4,
            )?,
            anomaly_kind: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "anomaly_kind",
                tile_area,
            )?,
            tsy_presence: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "tsy_presence",
                tile_area,
            )?,
            tsy_origin_id: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "tsy_origin_id",
                tile_area,
            )?,
            tsy_depth_tier: map_optional_layer(
                tile_dir,
                raster_root,
                layers,
                "tsy_depth_tier",
                tile_area,
            )?,
        })
    }
}

fn map_required_layer(
    tile_dir: &Path,
    raster_root: &Path,
    file_name: &str,
    expected_len: usize,
) -> Result<Mmap, String> {
    let path = tile_dir.join(file_name);
    map_file_under_root(&path, raster_root, expected_len)
}

fn map_optional_layer(
    tile_dir: &Path,
    raster_root: &Path,
    layers: &[String],
    layer_name: &str,
    expected_len: usize,
) -> Result<Option<Mmap>, String> {
    if !layers.iter().any(|layer| layer == layer_name) {
        return Ok(None);
    }
    map_file_under_root(
        &tile_dir.join(format!("{layer_name}.bin")),
        raster_root,
        expected_len,
    )
    .map(Some)
}

fn map_file_under_root(
    path: &Path,
    raster_root: &Path,
    expected_len: usize,
) -> Result<Mmap, String> {
    let file = super::nbt_io::open_regular_file_under_root(path, raster_root)
        .map_err(|error| format!("failed to open raster layer {}: {error}", path.display()))?;
    map_open_file(path, file, expected_len)
}

#[cfg(test)]
fn map_file(path: &Path, expected_len: usize) -> Result<Mmap, String> {
    let file = super::nbt_io::open_regular_file_no_follow(path)
        .map_err(|error| format!("failed to open raster layer {}: {error}", path.display()))?;
    map_open_file(path, file, expected_len)
}

#[cfg(test)]
type AfterMmapHook = Box<dyn FnOnce(&std::fs::File)>;

#[cfg(test)]
thread_local! {
    static MAP_OPEN_FILE_AFTER_MMAP_TEST_HOOK: std::cell::RefCell<Option<AfterMmapHook>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
fn set_map_open_file_after_mmap_test_hook(hook: impl FnOnce(&std::fs::File) + 'static) {
    MAP_OPEN_FILE_AFTER_MMAP_TEST_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

fn map_open_file(path: &Path, file: File, expected_len: usize) -> Result<Mmap, String> {
    let metadata_before = file
        .metadata()
        .map_err(|error| format!("failed to stat raster layer {}: {error}", path.display()))?;
    if metadata_before.len() as usize != expected_len {
        return Err(format!(
            "raster layer {} has {} bytes, expected {}",
            path.display(),
            metadata_before.len(),
            expected_len
        ));
    }
    file.try_lock_shared().map_err(|error| {
        format!(
            "failed to acquire shared lock for raster layer {}: {error}",
            path.display()
        )
    })?;
    let mmap = unsafe { memmap2::MmapOptions::new().len(expected_len).map(&file) }
        .map_err(|error| format!("failed to mmap raster layer {}: {error}", path.display()))?;
    #[cfg(test)]
    MAP_OPEN_FILE_AFTER_MMAP_TEST_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(&file);
        }
    });
    let metadata_after = file
        .metadata()
        .map_err(|error| format!("failed to restat raster layer {}: {error}", path.display()))?;
    if metadata_before.len() != metadata_after.len()
        || metadata_before.modified().ok() != metadata_after.modified().ok()
    {
        return Err(format!(
            "raster layer {} changed while being mapped",
            path.display()
        ));
    }
    Ok(mmap)
}

#[cfg(test)]
fn read_only_mmap(bytes: &[u8]) -> Result<Mmap, std::io::Error> {
    let mut mmap = memmap2::MmapMut::map_anon(bytes.len())?;
    mmap.copy_from_slice(bytes);
    mmap.make_read_only()
}

#[cfg(test)]
fn anonymous_mmap_for_tests(bytes: &[u8]) -> Mmap {
    read_only_mmap(bytes).expect("anonymous test raster mmap should allocate")
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
        if !raw.anchor.is_empty()
            && !matches!(raw.anchor.as_str(), "ground" | "embedded" | "hanging")
        {
            diagnostics.push(format!(
                "decoration '{}' (global_id {}) has invalid anchor '{}' (expected ground, embedded, or hanging)",
                raw.name, raw.global_id, raw.anchor
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
#[path = "raster_tests.rs"]
mod tests;
