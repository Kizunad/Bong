use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use serde::Deserialize;
use valence::prelude::{BiomeId, BiomeRegistry, BlockState, Ident, Resource};

use super::wilderness;

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
    f32_layer("cave_mask", 0.0),
    f32_layer("ceiling_height", 0.0),
    f32_layer("entrance_mask", 0.0),
    f32_layer("neg_pressure", 0.0),
    f32_layer("ruin_density", 0.0),
    f32_layer("qi_density", 0.12),
    f32_layer("mofa_decay", 0.40),
    f32_layer("qi_vein_flow", 0.0),
    u8_layer("spirit_eye_candidates", 0),
    u8_layer("realm_collapse_mask", 0),
    f32_layer("sky_island_mask", 0.0),
    f32_layer("sky_island_base_y", 9999.0),
    f32_layer("sky_island_thickness", 0.0),
    u8_layer("underground_tier", 0),
    f32_layer("cavern_floor_y", 9999.0),
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
#[derive(Clone, Copy, Debug)]
pub struct ColumnSample {
    pub height: f32,
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
    pub cave_mask: f32,
    pub ceiling_height: f32,
    pub entrance_mask: f32,
    pub fracture_mask: f32,
    pub neg_pressure: f32,
    pub ruin_density: f32,
    // --- xianxia semantic layers ---
    pub qi_density: f32,
    pub mofa_decay: f32,
    pub qi_vein_flow: f32,
    pub spirit_eye_candidates: u8,
    pub realm_collapse_mask: u8,
    // --- vertical-dimension layers (2D rasters encoding 3D structure) ---
    /// 0..1 likelihood this column hosts a floating isle above. Gate on >= 0.2.
    pub sky_island_mask: f32,
    /// World-y of isle bottom face. 9999.0 sentinel = "no isle here".
    pub sky_island_base_y: f32,
    /// Isle thickness in blocks (carved downward from base_y).
    pub sky_island_thickness: f32,
    /// Deepest active cave tier at this column: 0 (none), 1 shallow, 2 middle, 3 deep.
    pub underground_tier: u8,
    /// World-y of deepest cavern floor. 9999.0 sentinel = no cavern.
    pub cavern_floor_y: f32,
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
    height: Mmap,
    surface_id: Mmap,
    subsurface_id: Mmap,
    biome_id: Mmap,
    water_level: Mmap,
    feature_mask: Mmap,
    boundary_weight: Mmap,
    rift_axis_sdf: Option<Mmap>,
    portal_anchor_sdf: Option<Mmap>,
    rim_edge_mask: Option<Mmap>,
    cave_mask: Option<Mmap>,
    ceiling_height: Option<Mmap>,
    entrance_mask: Option<Mmap>,
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
    sky_island_base_y: Option<Mmap>,
    sky_island_thickness: Option<Mmap>,
    underground_tier: Option<Mmap>,
    cavern_floor_y: Option<Mmap>,
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

#[derive(Debug, Deserialize)]
struct RasterManifest {
    tile_size: i32,
    world_bounds: ManifestBounds,
    surface_palette: Vec<String>,
    biome_palette: Vec<String>,
    tiles: Vec<ManifestTile>,
    #[serde(default)]
    pois: Vec<ManifestPoi>,
    #[serde(default)]
    anomaly_kinds: HashMap<String, String>,
    #[serde(default)]
    abyssal_tier_floor_y: HashMap<String, f32>,
    #[serde(default)]
    global_decoration_palette: Vec<ManifestDecoration>,
    #[serde(default)]
    fossil_bboxes: Vec<ManifestFossilBbox>,
}

#[derive(Debug, Deserialize)]
struct ManifestBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

#[derive(Debug, Deserialize)]
struct ManifestTile {
    tile_x: i32,
    tile_z: i32,
    dir: String,
    layers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
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
    pub size_range: [i32; 2],
    pub rarity: f32,
    pub notes: String,
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
        }
    }

    pub fn load(
        manifest_path: &Path,
        raster_dir: &Path,
        biomes: &BiomeRegistry,
    ) -> Result<Self, String> {
        let manifest_text = std::fs::read_to_string(manifest_path).map_err(|error| {
            format!(
                "failed to read terrain raster manifest {}: {error}",
                manifest_path.display()
            )
        })?;
        let manifest: RasterManifest = serde_json::from_str(&manifest_text).map_err(|error| {
            format!(
                "failed to parse terrain raster manifest {}: {error}",
                manifest_path.display()
            )
        })?;

        let tile_area = (manifest.tile_size as usize)
            .checked_mul(manifest.tile_size as usize)
            .ok_or_else(|| "tile_size squared overflowed while loading rasters".to_string())?;
        let surface_palette = manifest
            .surface_palette
            .iter()
            .map(|name| block_state_from_name(name))
            .collect::<Result<Vec<_>, _>>()?;
        let biome_palette = manifest
            .biome_palette
            .iter()
            .map(|name| biome_id_from_name(name, biomes))
            .collect::<Result<Vec<_>, _>>()?;
        let default_wilderness_biome = *biome_palette
            .first()
            .ok_or_else(|| "biome palette cannot be empty".to_string())?;
        let forest_wilderness_biome = biome_palette
            .get(7)
            .copied()
            .unwrap_or(default_wilderness_biome);
        let river_wilderness_biome = biome_palette
            .get(8)
            .copied()
            .unwrap_or(default_wilderness_biome);

        let mut tiles = HashMap::with_capacity(manifest.tiles.len());
        for tile in &manifest.tiles {
            let tile_dir = raster_dir.join(&tile.dir);
            let tile_fields = TileFields::load(&tile_dir, &tile.layers, tile_area)?;
            tiles.insert((tile.tile_x, tile.tile_z), tile_fields);
        }

        // --- Narrative / event metadata ---
        let pois = manifest
            .pois
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
            .collect::<Vec<_>>();

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

        // Build indexed decoration palette — expand to `max global_id + 1` so
        // variant lookup is a single Vec::get.
        let max_deco_id = manifest
            .global_decoration_palette
            .iter()
            .map(|d| d.global_id)
            .max()
            .unwrap_or(0);
        let mut decoration_palette: Vec<Option<Decoration>> = vec![None; max_deco_id as usize + 1];
        for raw in manifest.global_decoration_palette {
            let id = raw.global_id as usize;
            if id == 0 || id >= decoration_palette.len() {
                continue;
            }
            decoration_palette[id] = Some(Decoration {
                global_id: raw.global_id,
                profile: raw.profile,
                local_id: raw.local_id,
                name: raw.name,
                kind: raw.kind,
                blocks: raw.blocks,
                size_range: raw.size_range,
                rarity: raw.rarity,
                notes: raw.notes,
            });
        }

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
        })
    }

    /// Zone-scoped POI list from the worldgen blueprint.
    #[allow(dead_code)]
    pub fn pois(&self) -> &[Poi] {
        &self.pois
    }

    /// Look up a decoration by its global id (0 → None).
    #[allow(dead_code)]
    pub fn decoration(&self, global_id: u8) -> Option<&Decoration> {
        self.decoration_palette
            .get(global_id as usize)
            .and_then(|o| o.as_ref())
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
            height: read_f32(&tile.height, index),
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
            cave_mask: read_optional_f32(&tile.cave_mask, index, 0.0),
            ceiling_height: read_optional_f32(&tile.ceiling_height, index, 0.0),
            entrance_mask: read_optional_f32(&tile.entrance_mask, index, 0.0),
            fracture_mask: read_optional_f32(&tile.fracture_mask, index, 0.0),
            neg_pressure: read_optional_f32(&tile.neg_pressure, index, 0.0),
            ruin_density: read_optional_f32(&tile.ruin_density, index, 0.0),
            qi_density: read_optional_f32(&tile.qi_density, index, 0.12),
            mofa_decay: read_optional_f32(&tile.mofa_decay, index, 0.40),
            qi_vein_flow: read_optional_f32(&tile.qi_vein_flow, index, 0.0),
            spirit_eye_candidates: read_optional_u8(&tile.spirit_eye_candidates, index, 0),
            realm_collapse_mask: read_optional_u8(&tile.realm_collapse_mask, index, 0),
            sky_island_mask: read_optional_f32(&tile.sky_island_mask, index, 0.0),
            sky_island_base_y: read_optional_f32(&tile.sky_island_base_y, index, 9999.0),
            sky_island_thickness: read_optional_f32(&tile.sky_island_thickness, index, 0.0),
            underground_tier: read_optional_u8(&tile.underground_tier, index, 0),
            cavern_floor_y: read_optional_f32(&tile.cavern_floor_y, index, 9999.0),
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

    pub fn sample_layer_f32(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        let schema = layer_schema(layer_name)?;
        let fallback = schema.safe_default_f32?;
        let Some((tile, index)) = self.tile_and_index(world_x, world_z) else {
            return Some(fallback);
        };

        Some(match layer_name {
            "height" => read_f32(&tile.height, index),
            "water_level" => read_f32(&tile.water_level, index),
            "feature_mask" => read_f32(&tile.feature_mask, index),
            "boundary_weight" => read_f32(&tile.boundary_weight, index),
            "rift_axis_sdf" => read_optional_f32(&tile.rift_axis_sdf, index, fallback),
            "portal_anchor_sdf" => read_optional_f32(&tile.portal_anchor_sdf, index, fallback),
            "rim_edge_mask" => read_optional_f32(&tile.rim_edge_mask, index, fallback),
            "fracture_mask" => read_optional_f32(&tile.fracture_mask, index, fallback),
            "cave_mask" => read_optional_f32(&tile.cave_mask, index, fallback),
            "ceiling_height" => read_optional_f32(&tile.ceiling_height, index, fallback),
            "entrance_mask" => read_optional_f32(&tile.entrance_mask, index, fallback),
            "neg_pressure" => read_optional_f32(&tile.neg_pressure, index, fallback),
            "ruin_density" => read_optional_f32(&tile.ruin_density, index, fallback),
            "qi_density" => read_optional_f32(&tile.qi_density, index, fallback),
            "mofa_decay" => read_optional_f32(&tile.mofa_decay, index, fallback),
            "qi_vein_flow" => read_optional_f32(&tile.qi_vein_flow, index, fallback),
            "sky_island_mask" => read_optional_f32(&tile.sky_island_mask, index, fallback),
            "sky_island_base_y" => read_optional_f32(&tile.sky_island_base_y, index, fallback),
            "sky_island_thickness" => {
                read_optional_f32(&tile.sky_island_thickness, index, fallback)
            }
            "cavern_floor_y" => read_optional_f32(&tile.cavern_floor_y, index, fallback),
            "flora_density" => read_optional_f32(&tile.flora_density, index, fallback),
            "ground_cover_density" => {
                read_optional_f32(&tile.ground_cover_density, index, fallback)
            }
            "mineral_density" => read_optional_f32(&tile.mineral_density, index, fallback),
            "anomaly_intensity" => read_optional_f32(&tile.anomaly_intensity, index, fallback),
            _ => unreachable!("schema export type should match f32 layer"),
        })
    }

    pub fn sample_layer_u8(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<u8> {
        let schema = layer_schema(layer_name)?;
        let fallback = schema.safe_default_u8?;
        let Some((tile, index)) = self.tile_and_index(world_x, world_z) else {
            return Some(fallback);
        };

        Some(match layer_name {
            "surface_id" => read_u8(&tile.surface_id, index),
            "subsurface_id" => read_u8(&tile.subsurface_id, index),
            "biome_id" => read_u8(&tile.biome_id, index),
            "spirit_eye_candidates" => {
                read_optional_u8(&tile.spirit_eye_candidates, index, fallback)
            }
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
        })
    }

    pub fn sample_layer(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        self.sample_layer_f32(world_x, world_z, layer_name)
            .or_else(|| {
                self.sample_layer_u8(world_x, world_z, layer_name)
                    .map(f32::from)
            })
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

impl TileFields {
    fn load(tile_dir: &Path, layers: &[String], tile_area: usize) -> Result<Self, String> {
        let area4 = tile_area * 4;
        Ok(Self {
            height: map_required_layer(tile_dir, "height.bin", area4)?,
            surface_id: map_required_layer(tile_dir, "surface_id.bin", tile_area)?,
            subsurface_id: map_required_layer(tile_dir, "subsurface_id.bin", tile_area)?,
            biome_id: map_required_layer(tile_dir, "biome_id.bin", tile_area)?,
            water_level: map_required_layer(tile_dir, "water_level.bin", area4)?,
            feature_mask: map_required_layer(tile_dir, "feature_mask.bin", area4)?,
            boundary_weight: map_required_layer(tile_dir, "boundary_weight.bin", area4)?,
            rift_axis_sdf: map_optional_layer(tile_dir, layers, "rift_axis_sdf", area4)?,
            portal_anchor_sdf: map_optional_layer(tile_dir, layers, "portal_anchor_sdf", area4)?,
            rim_edge_mask: map_optional_layer(tile_dir, layers, "rim_edge_mask", area4)?,
            cave_mask: map_optional_layer(tile_dir, layers, "cave_mask", area4)?,
            ceiling_height: map_optional_layer(tile_dir, layers, "ceiling_height", area4)?,
            entrance_mask: map_optional_layer(tile_dir, layers, "entrance_mask", area4)?,
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
            sky_island_base_y: map_optional_layer(tile_dir, layers, "sky_island_base_y", area4)?,
            sky_island_thickness: map_optional_layer(
                tile_dir,
                layers,
                "sky_island_thickness",
                area4,
            )?,
            underground_tier: map_optional_layer(tile_dir, layers, "underground_tier", tile_area)?,
            cavern_floor_y: map_optional_layer(tile_dir, layers, "cavern_floor_y", area4)?,
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

fn block_state_from_name(name: &str) -> Result<BlockState, String> {
    match name {
        "stone" => Ok(BlockState::STONE),
        "smooth_stone" => Ok(BlockState::SMOOTH_STONE),
        "coarse_dirt" => Ok(BlockState::COARSE_DIRT),
        "gravel" => Ok(BlockState::GRAVEL),
        "grass_block" => Ok(BlockState::GRASS_BLOCK),
        "dirt" => Ok(BlockState::DIRT),
        "sand" => Ok(BlockState::SAND),
        "red_sandstone" => Ok(BlockState::RED_SANDSTONE),
        "terracotta" => Ok(BlockState::TERRACOTTA),
        "red_terracotta" => Ok(BlockState::RED_TERRACOTTA),
        "cobblestone" => Ok(BlockState::COBBLESTONE),
        "mossy_cobblestone" => Ok(BlockState::MOSSY_COBBLESTONE),
        "tuff" => Ok(BlockState::TUFF),
        "blackstone" => Ok(BlockState::BLACKSTONE),
        "obsidian" => Ok(BlockState::OBSIDIAN),
        "basalt" => Ok(BlockState::BASALT),
        "magma_block" => Ok(BlockState::MAGMA_BLOCK),
        "crimson_nylium" => Ok(BlockState::CRIMSON_NYLIUM),
        "calcite" => Ok(BlockState::CALCITE),
        "snow_block" => Ok(BlockState::SNOW_BLOCK),
        "packed_ice" => Ok(BlockState::PACKED_ICE),
        "podzol" => Ok(BlockState::PODZOL),
        "rooted_dirt" => Ok(BlockState::ROOTED_DIRT),
        "soul_sand" => Ok(BlockState::SOUL_SAND),
        "soul_soil" => Ok(BlockState::SOUL_SOIL),
        "bone_block" => Ok(BlockState::BONE_BLOCK),
        "mud" => Ok(BlockState::MUD),
        "clay" => Ok(BlockState::CLAY),
        "moss_block" => Ok(BlockState::MOSS_BLOCK),
        "andesite" => Ok(BlockState::ANDESITE),
        "deepslate" => Ok(BlockState::DEEPSLATE),
        "cobbled_deepslate" => Ok(BlockState::COBBLED_DEEPSLATE),
        "deepslate_bricks" => Ok(BlockState::DEEPSLATE_BRICKS),
        "cracked_stone_bricks" => Ok(BlockState::CRACKED_STONE_BRICKS),
        "smooth_quartz" => Ok(BlockState::SMOOTH_QUARTZ),
        "lodestone" => Ok(BlockState::LODESTONE),
        "weathered_copper" => Ok(BlockState::WEATHERED_COPPER),
        "warped_planks" => Ok(BlockState::WARPED_PLANKS),
        "dead_bush" => Ok(BlockState::DEAD_BUSH),
        other => Err(format!("unsupported surface palette block '{other}'")),
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

    fn build_fixture() -> RasterFixture {
        let root = unique_temp_dir();
        let tile_dir = root.join("tile_0_0");
        fs::create_dir_all(&tile_dir).expect("test raster tile dir should be creatable");
        let tile_area = (TILE_SIZE * TILE_SIZE) as usize;

        for (index, schema) in LAYER_SCHEMAS.iter().enumerate() {
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
    }

    #[test]
    fn sample_layer_out_of_tile_bounds_returns_schema_safe_defaults() {
        let fixture = build_fixture();
        let provider = fixture.provider();

        assert_eq!(provider.sample_layer_f32(2, 0, "height"), Some(0.0));
        assert_eq!(provider.sample_layer_u8(2, 0, "surface_id"), Some(0));
    }

    #[test]
    fn sample_layer_compatibility_adapter_exposes_both_export_types() {
        let fixture = build_fixture();
        let provider = fixture.provider();

        let height_index = TerrainProvider::layer_names()
            .iter()
            .position(|schema| schema.name == "height")
            .expect("height schema should exist");
        let surface_index = TerrainProvider::layer_names()
            .iter()
            .position(|schema| schema.name == "surface_id")
            .expect("surface_id schema should exist");

        assert_eq!(
            provider.sample_layer(1, 1, "height"),
            Some(test_f32_value(height_index))
        );
        assert_eq!(
            provider.sample_layer(1, 1, "surface_id"),
            Some(f32::from(test_u8_value(surface_index)))
        );
    }
}
