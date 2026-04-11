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
    pub rim_edge_mask: f32,
    pub cave_mask: f32,
    pub ceiling_height: f32,
    pub entrance_mask: f32,
    pub fracture_mask: f32,
    pub neg_pressure: f32,
    pub ruin_density: f32,
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
}

impl Resource for TerrainProvider {}

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
    rim_edge_mask: Option<Mmap>,
    cave_mask: Option<Mmap>,
    ceiling_height: Option<Mmap>,
    entrance_mask: Option<Mmap>,
    fracture_mask: Option<Mmap>,
    neg_pressure: Option<Mmap>,
    ruin_density: Option<Mmap>,
}

#[derive(Debug, Deserialize)]
struct RasterManifest {
    tile_size: i32,
    world_bounds: ManifestBounds,
    surface_palette: Vec<String>,
    biome_palette: Vec<String>,
    tiles: Vec<ManifestTile>,
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

impl TerrainProvider {
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

        let mut tiles = HashMap::with_capacity(manifest.tiles.len());
        for tile in manifest.tiles {
            let tile_dir = raster_dir.join(&tile.dir);
            let tile_fields = TileFields::load(&tile_dir, &tile.layers, tile_area)?;
            tiles.insert((tile.tile_x, tile.tile_z), tile_fields);
        }

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
        })
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn sample(&self, world_x: i32, world_z: i32) -> ColumnSample {
        let tile_x = world_x.div_euclid(self.tile_size);
        let tile_z = world_z.div_euclid(self.tile_size);

        let Some(tile) = self.tiles.get(&(tile_x, tile_z)) else {
            return wilderness::sample(world_x, world_z, self.default_wilderness_biome);
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
            rim_edge_mask: read_optional_f32(&tile.rim_edge_mask, index, 0.0),
            cave_mask: read_optional_f32(&tile.cave_mask, index, 0.0),
            ceiling_height: read_optional_f32(&tile.ceiling_height, index, 0.0),
            entrance_mask: read_optional_f32(&tile.entrance_mask, index, 0.0),
            fracture_mask: read_optional_f32(&tile.fracture_mask, index, 0.0),
            neg_pressure: read_optional_f32(&tile.neg_pressure, index, 0.0),
            ruin_density: read_optional_f32(&tile.ruin_density, index, 0.0),
        }
    }
}

impl TileFields {
    fn load(tile_dir: &Path, layers: &[String], tile_area: usize) -> Result<Self, String> {
        Ok(Self {
            height: map_required_layer(tile_dir, "height.bin", tile_area * 4)?,
            surface_id: map_required_layer(tile_dir, "surface_id.bin", tile_area)?,
            subsurface_id: map_required_layer(tile_dir, "subsurface_id.bin", tile_area)?,
            biome_id: map_required_layer(tile_dir, "biome_id.bin", tile_area)?,
            water_level: map_required_layer(tile_dir, "water_level.bin", tile_area * 4)?,
            feature_mask: map_required_layer(tile_dir, "feature_mask.bin", tile_area * 4)?,
            boundary_weight: map_required_layer(tile_dir, "boundary_weight.bin", tile_area * 4)?,
            rift_axis_sdf: map_optional_layer(tile_dir, layers, "rift_axis_sdf", tile_area * 4)?,
            rim_edge_mask: map_optional_layer(tile_dir, layers, "rim_edge_mask", tile_area * 4)?,
            cave_mask: map_optional_layer(tile_dir, layers, "cave_mask", tile_area * 4)?,
            ceiling_height: map_optional_layer(tile_dir, layers, "ceiling_height", tile_area * 4)?,
            entrance_mask: map_optional_layer(tile_dir, layers, "entrance_mask", tile_area * 4)?,
            fracture_mask: map_optional_layer(tile_dir, layers, "fracture_mask", tile_area * 4)?,
            neg_pressure: map_optional_layer(tile_dir, layers, "neg_pressure", tile_area * 4)?,
            ruin_density: map_optional_layer(tile_dir, layers, "ruin_density", tile_area * 4)?,
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

fn block_state_from_name(name: &str) -> Result<BlockState, String> {
    match name {
        "stone" => Ok(BlockState::STONE),
        "coarse_dirt" => Ok(BlockState::COARSE_DIRT),
        "gravel" => Ok(BlockState::GRAVEL),
        "grass_block" => Ok(BlockState::GRASS_BLOCK),
        "dirt" => Ok(BlockState::DIRT),
        "sand" => Ok(BlockState::SAND),
        "red_sandstone" => Ok(BlockState::RED_SANDSTONE),
        "terracotta" => Ok(BlockState::TERRACOTTA),
        "blackstone" => Ok(BlockState::BLACKSTONE),
        "basalt" => Ok(BlockState::BASALT),
        "magma_block" => Ok(BlockState::MAGMA_BLOCK),
        "crimson_nylium" => Ok(BlockState::CRIMSON_NYLIUM),
        "calcite" => Ok(BlockState::CALCITE),
        "snow_block" => Ok(BlockState::SNOW_BLOCK),
        "packed_ice" => Ok(BlockState::PACKED_ICE),
        "podzol" => Ok(BlockState::PODZOL),
        "rooted_dirt" => Ok(BlockState::ROOTED_DIRT),
        "soul_sand" => Ok(BlockState::SOUL_SAND),
        "bone_block" => Ok(BlockState::BONE_BLOCK),
        "mud" => Ok(BlockState::MUD),
        "clay" => Ok(BlockState::CLAY),
        "moss_block" => Ok(BlockState::MOSS_BLOCK),
        "andesite" => Ok(BlockState::ANDESITE),
        "deepslate" => Ok(BlockState::DEEPSLATE),
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
