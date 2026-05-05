use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;
use serde::Deserialize;
use valence::prelude::{BiomeId, BiomeRegistry, BlockState, Ident, Resource};

use super::wilderness;

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
    pub fn is_wilderness_biome(&self) -> bool {
        matches!(self.biome_id, 0 | 7 | 8)
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
    sky_island_mask: Option<Mmap>,
    sky_island_base_y: Option<Mmap>,
    sky_island_thickness: Option<Mmap>,
    underground_tier: Option<Mmap>,
    cavern_floor_y: Option<Mmap>,
    flora_density: Option<Mmap>,
    flora_variant_id: Option<Mmap>,
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
            sky_island_mask: read_optional_f32(&tile.sky_island_mask, index, 0.0),
            sky_island_base_y: read_optional_f32(&tile.sky_island_base_y, index, 9999.0),
            sky_island_thickness: read_optional_f32(&tile.sky_island_thickness, index, 0.0),
            underground_tier: read_optional_u8(&tile.underground_tier, index, 0),
            cavern_floor_y: read_optional_f32(&tile.cavern_floor_y, index, 9999.0),
            flora_density: read_optional_f32(&tile.flora_density, index, 0.0),
            flora_variant_id: read_optional_u8(&tile.flora_variant_id, index, 0),
            fossil_bbox: read_optional_u8(&tile.fossil_bbox, index, 0),
            anomaly_intensity: read_optional_f32(&tile.anomaly_intensity, index, 0.0),
            anomaly_kind: read_optional_u8(&tile.anomaly_kind, index, 0),
            tsy_presence: read_optional_u8(&tile.tsy_presence, index, 0),
            tsy_origin_id: read_optional_u8(&tile.tsy_origin_id, index, 0),
            tsy_depth_tier: read_optional_u8(&tile.tsy_depth_tier, index, 0),
        }
    }

    pub fn sample_layer(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        let (tile, index) = self.tile_and_index(world_x, world_z)?;
        match layer_name {
            "height" => Some(read_f32(&tile.height, index)),
            "water_level" => Some(read_f32(&tile.water_level, index)),
            "feature_mask" => Some(read_f32(&tile.feature_mask, index)),
            "boundary_weight" => Some(read_f32(&tile.boundary_weight, index)),
            "rift_axis_sdf" => read_optional_f32_strict(&tile.rift_axis_sdf, index),
            "portal_anchor_sdf" => read_optional_f32_strict(&tile.portal_anchor_sdf, index),
            "rim_edge_mask" => read_optional_f32_strict(&tile.rim_edge_mask, index),
            "cave_mask" => read_optional_f32_strict(&tile.cave_mask, index),
            "ceiling_height" => read_optional_f32_strict(&tile.ceiling_height, index),
            "entrance_mask" => read_optional_f32_strict(&tile.entrance_mask, index),
            "fracture_mask" => read_optional_f32_strict(&tile.fracture_mask, index),
            "neg_pressure" => read_optional_f32_strict(&tile.neg_pressure, index),
            "ruin_density" => read_optional_f32_strict(&tile.ruin_density, index),
            "qi_density" => read_optional_f32_strict(&tile.qi_density, index),
            "mofa_decay" => read_optional_f32_strict(&tile.mofa_decay, index),
            "qi_vein_flow" => read_optional_f32_strict(&tile.qi_vein_flow, index),
            "sky_island_mask" => read_optional_f32_strict(&tile.sky_island_mask, index),
            "sky_island_base_y" => read_optional_f32_strict(&tile.sky_island_base_y, index),
            "sky_island_thickness" => read_optional_f32_strict(&tile.sky_island_thickness, index),
            "underground_tier" => {
                read_optional_u8_strict(&tile.underground_tier, index).map(f32::from)
            }
            "cavern_floor_y" => read_optional_f32_strict(&tile.cavern_floor_y, index),
            "flora_density" => read_optional_f32_strict(&tile.flora_density, index),
            "flora_variant_id" => {
                read_optional_u8_strict(&tile.flora_variant_id, index).map(f32::from)
            }
            "fossil_bbox" => read_optional_u8_strict(&tile.fossil_bbox, index).map(f32::from),
            "anomaly_intensity" => read_optional_f32_strict(&tile.anomaly_intensity, index),
            "anomaly_kind" => read_optional_u8_strict(&tile.anomaly_kind, index).map(f32::from),
            "tsy_presence" => read_optional_u8_strict(&tile.tsy_presence, index).map(f32::from),
            "tsy_origin_id" => read_optional_u8_strict(&tile.tsy_origin_id, index).map(f32::from),
            "tsy_depth_tier" => read_optional_u8_strict(&tile.tsy_depth_tier, index).map(f32::from),
            _ => None,
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

fn read_optional_f32_strict(bytes: &Option<Mmap>, index: usize) -> Option<f32> {
    bytes.as_ref().map(|mmap| read_f32(mmap, index))
}

fn read_optional_u8_strict(bytes: &Option<Mmap>, index: usize) -> Option<u8> {
    bytes.as_ref().map(|mmap| read_u8(mmap, index))
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
