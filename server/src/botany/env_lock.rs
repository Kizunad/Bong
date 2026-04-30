use std::collections::HashMap;

use valence::prelude::BlockState;

use super::registry::{BotanyPlantKind, DecorationLock, EnvLock, SkyIsleSurface, WaterPulsePhase};
use crate::world::terrain::{SurfaceProvider, TerrainProvider};
use crate::world::zone::Zone;

pub trait EnvLayerSampler {
    fn env_sample_layer(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32>;
    fn env_query_surface_y(&self, world_x: i32, world_z: i32) -> i32;
    fn env_surface_block(&self, world_x: i32, world_z: i32) -> BlockState;
    fn env_sky_island(&self, world_x: i32, world_z: i32) -> Option<(f32, f32)>;
}

impl EnvLayerSampler for TerrainProvider {
    fn env_sample_layer(&self, world_x: i32, world_z: i32, layer_name: &str) -> Option<f32> {
        self.sample_layer(world_x, world_z, layer_name)
    }

    fn env_query_surface_y(&self, world_x: i32, world_z: i32) -> i32 {
        self.query_surface(world_x, world_z).y
    }

    fn env_surface_block(&self, world_x: i32, world_z: i32) -> BlockState {
        self.sample(world_x, world_z).surface_block
    }

    fn env_sky_island(&self, world_x: i32, world_z: i32) -> Option<(f32, f32)> {
        let sample = self.sample(world_x, world_z);
        Some((sample.sky_island_base_y, sample.sky_island_thickness))
    }
}

#[derive(Debug, Clone, Default)]
pub struct DecorationManifest {
    by_name: HashMap<String, DecorationDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationDescriptor {
    pub name: String,
    pub primary_blocks: Vec<BlockState>,
}

impl DecorationManifest {
    pub fn from_terrain_provider(terrain: &TerrainProvider) -> Self {
        let mut by_name = HashMap::new();
        for decoration in terrain.decorations() {
            let primary_blocks = decoration
                .blocks
                .iter()
                .filter_map(|block| decoration_block_state(block.as_str()))
                .collect::<Vec<_>>();
            by_name.insert(
                decoration.name.clone(),
                DecorationDescriptor {
                    name: decoration.name.clone(),
                    primary_blocks,
                },
            );
        }
        Self { by_name }
    }

    pub fn contains(&self, lock: DecorationLock) -> bool {
        lock.names()
            .into_iter()
            .any(|name| self.by_name.contains_key(name))
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&DecorationDescriptor> {
        self.by_name.get(name)
    }
}

pub fn check_env_locks(
    kind: &BotanyPlantKind,
    world_x: i32,
    world_z: i32,
    terrain: &TerrainProvider,
    zone: &Zone,
) -> bool {
    let Some(spec) = kind.v2_spec() else {
        return true;
    };
    let manifest = DecorationManifest::from_terrain_provider(terrain);
    spec.env_locks
        .iter()
        .all(|lock| check_env_lock(*lock, world_x, world_z, terrain, zone, &manifest))
}

pub fn check_env_lock(
    lock: EnvLock,
    world_x: i32,
    world_z: i32,
    terrain: &impl EnvLayerSampler,
    zone: &Zone,
    manifest: &DecorationManifest,
) -> bool {
    match lock {
        EnvLock::NegPressure { min } => {
            sample_at_least(terrain, world_x, world_z, "neg_pressure", min)
        }
        EnvLock::QiVeinFlow { min } => {
            sample_at_least(terrain, world_x, world_z, "qi_vein_flow", min)
        }
        EnvLock::FractureMask { min } => {
            sample_at_least(terrain, world_x, world_z, "fracture_mask", min)
        }
        EnvLock::RuinDensity { min } => {
            sample_at_least(terrain, world_x, world_z, "ruin_density", min)
        }
        EnvLock::SkyIslandMask { min, surface } => {
            if !sample_at_least(terrain, world_x, world_z, "sky_island_mask", min) {
                return false;
            }
            let Some((base_y, thickness)) = terrain.env_sky_island(world_x, world_z) else {
                return false;
            };
            match surface {
                SkyIsleSurface::Top => base_y < 9000.0,
                SkyIsleSurface::Bottom => base_y < 9000.0 && thickness > 0.0,
            }
        }
        EnvLock::UndergroundTier { tier } => terrain
            .env_sample_layer(world_x, world_z, "underground_tier")
            .is_some_and(|actual| actual.round() as u8 == tier),
        EnvLock::PortalRiftActive => zone
            .active_events
            .iter()
            .any(|event| event == "portal_rift" || event == "tsy_entry"),
        EnvLock::AdjacentDecoration { kind, .. } => manifest.contains(kind),
        EnvLock::AdjacentLightBlock { .. } => column_has_light_block(terrain, world_x, world_z),
        EnvLock::SnowSurface => {
            terrain.env_query_surface_y(world_x, world_z)
                >= crate::world::terrain::broken_peaks::SNOW_LINE_Y
        }
        EnvLock::TimePhase(phase) => {
            matches!(phase, WaterPulsePhase::Open)
                && zone
                    .active_events
                    .iter()
                    .any(|event| event == "water_pulse_open")
        }
    }
}

fn sample_at_least(
    terrain: &impl EnvLayerSampler,
    world_x: i32,
    world_z: i32,
    layer_name: &str,
    min: f32,
) -> bool {
    terrain
        .env_sample_layer(world_x, world_z, layer_name)
        .is_some_and(|value| value >= min)
}

fn column_has_light_block(terrain: &impl EnvLayerSampler, world_x: i32, world_z: i32) -> bool {
    matches!(
        terrain.env_surface_block(world_x, world_z),
        BlockState::SHROOMLIGHT
            | BlockState::AMETHYST_BLOCK
            | BlockState::AMETHYST_CLUSTER
            | BlockState::GLOW_LICHEN
    )
}

fn decoration_block_state(name: &str) -> Option<BlockState> {
    match name {
        "shroomlight" => Some(BlockState::SHROOMLIGHT),
        "amethyst_block" => Some(BlockState::AMETHYST_BLOCK),
        "amethyst_cluster" => Some(BlockState::AMETHYST_CLUSTER),
        "glow_lichen" => Some(BlockState::GLOW_LICHEN),
        "bone_block" => Some(BlockState::BONE_BLOCK),
        "packed_ice" => Some(BlockState::PACKED_ICE),
        "snow_block" => Some(BlockState::SNOW_BLOCK),
        "magma_block" => Some(BlockState::MAGMA_BLOCK),
        "blackstone" => Some(BlockState::BLACKSTONE),
        "deepslate" => Some(BlockState::DEEPSLATE),
        "andesite" => Some(BlockState::ANDESITE),
        "moss_block" => Some(BlockState::MOSS_BLOCK),
        "mud" => Some(BlockState::MUD),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::registry::{BotanyKindRegistry, BotanyPlantId};
    use crate::world::dimension::DimensionKind;
    use valence::prelude::Position;

    struct MockTerrain {
        layers: HashMap<&'static str, f32>,
        surface_y: i32,
        surface_block: BlockState,
        sky_island: Option<(f32, f32)>,
    }

    impl EnvLayerSampler for MockTerrain {
        fn env_sample_layer(&self, _world_x: i32, _world_z: i32, layer_name: &str) -> Option<f32> {
            self.layers.get(layer_name).copied()
        }

        fn env_query_surface_y(&self, _world_x: i32, _world_z: i32) -> i32 {
            self.surface_y
        }

        fn env_surface_block(&self, _world_x: i32, _world_z: i32) -> BlockState {
            self.surface_block
        }

        fn env_sky_island(&self, _world_x: i32, _world_z: i32) -> Option<(f32, f32)> {
            self.sky_island
        }
    }

    fn mock_terrain(layer_name: &'static str, value: f32) -> MockTerrain {
        MockTerrain {
            layers: HashMap::from([(layer_name, value)]),
            surface_y: 80,
            surface_block: BlockState::STONE,
            sky_island: None,
        }
    }

    fn zone(events: &[&str]) -> Zone {
        Zone {
            name: "test".to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (
                Position::new([0.0, 0.0, 0.0]).get(),
                Position::new([16.0, 320.0, 16.0]).get(),
            ),
            spirit_qi: -0.3,
            danger_level: 1,
            active_events: events.iter().map(|event| event.to_string()).collect(),
            patrol_anchors: vec![],
            blocked_tiles: vec![],
        }
    }

    #[test]
    fn portal_rift_lock_reads_zone_active_events() {
        let terrain = mock_terrain("neg_pressure", 0.0);
        let manifest = DecorationManifest::default();
        assert!(check_env_lock(
            EnvLock::PortalRiftActive,
            0,
            0,
            &terrain,
            &zone(&["portal_rift"]),
            &manifest
        ));
        assert!(!check_env_lock(
            EnvLock::PortalRiftActive,
            0,
            0,
            &terrain,
            &zone(&[]),
            &manifest
        ));
    }

    #[test]
    fn bai_yan_peng_has_empty_env_locks() {
        let registry = BotanyKindRegistry::default();
        let kind = registry
            .get(BotanyPlantId::BaiYanPeng)
            .expect("bai_yan_peng should be registered");
        assert!(kind.v2_spec().unwrap().env_locks.is_empty());
    }

    #[test]
    fn neg_pressure_lock_reads_strict_layer_value() {
        let manifest = DecorationManifest::default();
        assert!(check_env_lock(
            EnvLock::NegPressure { min: 0.3 },
            0,
            0,
            &mock_terrain("neg_pressure", 0.31),
            &zone(&[]),
            &manifest
        ));
        assert!(!check_env_lock(
            EnvLock::NegPressure { min: 0.3 },
            0,
            0,
            &mock_terrain("neg_pressure", 0.29),
            &zone(&[]),
            &manifest
        ));
    }

    #[test]
    fn adjacent_light_lock_accepts_glow_lichen_surface() {
        let terrain = MockTerrain {
            layers: HashMap::new(),
            surface_y: 80,
            surface_block: BlockState::GLOW_LICHEN,
            sky_island: None,
        };
        assert!(check_env_lock(
            EnvLock::AdjacentLightBlock { radius: 2 },
            0,
            0,
            &terrain,
            &zone(&[]),
            &DecorationManifest::default()
        ));
    }
}
