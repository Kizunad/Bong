//! 普通方块掉落系统：BlockState → 物品掉落。
//!
//! 当玩家破坏一个"普通"方块（不属于矿脉 / 灵木 / 灵龛的方块）时，本系统给予
//! 对应的基础材料物品。
//!
//! 跳过条件（由其他模块专门处理）：
//!   - `MineralOreIndex` 命中 → mineral break_handler 接管
//!   - 灵木巨树 OAK_LOG → spiritwood 模块接管
//!   - 灵龛保护 → social 模块接管
//!
//! 掉落映射：
//!   - OAK_LOG / BIRCH_LOG / STRIPPED_OAK_LOG / STRIPPED_BIRCH_LOG → crude_wood ×1-2
//!   - STONE / COBBLESTONE / ANDESITE / DIORITE / GRANITE → stone_chunk ×1
//!   - IRON_ORE / DEEPSLATE_IRON_ORE → iron_ore ×1
//!   - GRASS_BLOCK → grass_fiber ×0-1 (50%)

use valence::prelude::{
    App, BlockPos, BlockState, Client, DiggingEvent, DiggingState, Entity, EventReader, GameMode,
    IntoSystemConfigs, Query, Res, ResMut, Update, With,
};

use crate::inventory::{
    add_item_to_player_inventory, InventoryInstanceIdAllocator, ItemRegistry, PlayerInventory,
};
use crate::mineral::components::MineralOreIndex;
use crate::player::gameplay::GameplayTick;
use crate::social::{block_break_is_protected_by_registered_spirit_niche, SpiritNicheRegistry};
use crate::world::dimension::{CurrentDimension, DimensionKind, DimensionLayers};
use crate::world::terrain::TerrainProviders;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDropEntry {
    pub template_id: &'static str,
    pub min_count: u32,
    pub max_count: u32,
}

pub fn block_drop_for(block: BlockState) -> Option<BlockDropEntry> {
    match block {
        BlockState::OAK_LOG
        | BlockState::BIRCH_LOG
        | BlockState::STRIPPED_OAK_LOG
        | BlockState::STRIPPED_BIRCH_LOG => Some(BlockDropEntry {
            template_id: "crude_wood",
            min_count: 1,
            max_count: 2,
        }),
        BlockState::STONE
        | BlockState::COBBLESTONE
        | BlockState::ANDESITE
        | BlockState::DIORITE
        | BlockState::GRANITE => Some(BlockDropEntry {
            template_id: "stone_chunk",
            min_count: 1,
            max_count: 1,
        }),
        BlockState::IRON_ORE | BlockState::DEEPSLATE_IRON_ORE => Some(BlockDropEntry {
            template_id: "iron_ore",
            min_count: 1,
            max_count: 1,
        }),
        BlockState::GRASS_BLOCK => Some(BlockDropEntry {
            template_id: "grass_fiber",
            min_count: 0,
            max_count: 1,
        }),
        _ => None,
    }
}

fn should_drop(state: DiggingState, mode: GameMode) -> bool {
    matches!((state, mode), (DiggingState::Stop, GameMode::Survival))
}

fn roll_count(entry: &BlockDropEntry, pos: BlockPos, player: Entity, tick: u64) -> u32 {
    if entry.min_count == entry.max_count {
        return entry.min_count;
    }
    let mut hash = tick
        ^ player.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (pos.x as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (pos.y as i64 as u64).rotate_left(17)
        ^ (pos.z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    let range = entry.max_count - entry.min_count + 1;
    entry.min_count + (hash % range as u64) as u32
}

fn is_spiritwood_position(
    pos: BlockPos,
    dimension: DimensionKind,
    providers: Option<&TerrainProviders>,
) -> bool {
    if dimension != DimensionKind::Overworld {
        return false;
    }
    providers.is_some_and(|providers| {
        crate::world::terrain::mega_tree::is_spiritwood_log_at(pos, &providers.overworld)
    })
}

#[allow(clippy::too_many_arguments)]
pub fn apply_block_drops(
    mut digs: EventReader<DiggingEvent>,
    gameplay_tick: Option<Res<GameplayTick>>,
    mineral_index: Option<Res<MineralOreIndex>>,
    spirit_niches: Option<Res<SpiritNicheRegistry>>,
    providers: Option<Res<TerrainProviders>>,
    dimension_layers: Option<Res<DimensionLayers>>,
    item_registry: Res<ItemRegistry>,
    mut allocator: ResMut<InventoryInstanceIdAllocator>,
    game_modes: Query<&GameMode, With<Client>>,
    dimensions: Query<&CurrentDimension, With<Client>>,
    layers: Query<&valence::prelude::ChunkLayer>,
    mut inventories: Query<&mut PlayerInventory, With<Client>>,
) {
    let now_tick = gameplay_tick
        .as_deref()
        .map(GameplayTick::current_tick)
        .unwrap_or(0);

    for event in digs.read() {
        let mode = game_modes.get(event.client).copied().unwrap_or_default();
        if !should_drop(event.state, mode) {
            continue;
        }

        let dimension = dimensions
            .get(event.client)
            .map(|d| d.0)
            .unwrap_or(DimensionKind::Overworld);

        if mineral_index
            .as_deref()
            .and_then(|idx| idx.lookup(dimension, event.position))
            .is_some()
        {
            continue;
        }

        if spirit_niches.as_deref().is_some_and(|registry| {
            block_break_is_protected_by_registered_spirit_niche(
                None,
                [event.position.x, event.position.y, event.position.z],
                registry,
            )
        }) {
            continue;
        }

        let block_state = dimension_layers.as_deref().and_then(|dl| {
            layers
                .get(dl.entity_for(dimension))
                .ok()
                .and_then(|layer| layer.block(event.position).map(|b| b.state))
        });
        let Some(block_state) = block_state else {
            continue;
        };

        if (block_state == BlockState::OAK_LOG || block_state == BlockState::STRIPPED_OAK_LOG)
            && is_spiritwood_position(event.position, dimension, providers.as_deref())
        {
            continue;
        }

        let Some(entry) = block_drop_for(block_state) else {
            continue;
        };

        let count = roll_count(&entry, event.position, event.client, now_tick);
        if count == 0 {
            continue;
        }

        let Ok(mut inventory) = inventories.get_mut(event.client) else {
            continue;
        };
        match add_item_to_player_inventory(
            &mut inventory,
            &item_registry,
            &mut allocator,
            entry.template_id,
            count,
        ) {
            Ok(receipt) => {
                tracing::debug!(
                    target: "bong::block_drop",
                    "dropped {} ×{} to player {:?} (rev={})",
                    receipt.template_id,
                    receipt.stack_count,
                    event.client,
                    receipt.revision.0
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "bong::block_drop",
                    "failed to grant {} ×{} to player {:?}: {error}",
                    entry.template_id,
                    count,
                    event.client
                );
            }
        }
    }
}

pub fn register(app: &mut App) {
    // 必须在 block_break（set AIR）之前运行，否则读到的 block_state 已是 AIR。
    app.add_systems(
        Update,
        apply_block_drops.before(super::block_break::apply_default_block_break),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oak_log_drops_crude_wood() {
        let entry = block_drop_for(BlockState::OAK_LOG).expect("oak_log should have a drop");
        assert_eq!(entry.template_id, "crude_wood");
        assert_eq!(entry.min_count, 1);
        assert_eq!(entry.max_count, 2);
    }

    #[test]
    fn birch_log_drops_crude_wood() {
        let entry = block_drop_for(BlockState::BIRCH_LOG).expect("birch_log should have a drop");
        assert_eq!(entry.template_id, "crude_wood");
    }

    #[test]
    fn stripped_variants_also_drop() {
        assert!(block_drop_for(BlockState::STRIPPED_OAK_LOG).is_some());
        assert!(block_drop_for(BlockState::STRIPPED_BIRCH_LOG).is_some());
    }

    #[test]
    fn stone_variants_drop_stone_chunk() {
        for block in [
            BlockState::STONE,
            BlockState::COBBLESTONE,
            BlockState::ANDESITE,
            BlockState::DIORITE,
            BlockState::GRANITE,
        ] {
            let entry = block_drop_for(block).expect("stone variant should have a drop");
            assert_eq!(entry.template_id, "stone_chunk", "block {block:?}");
            assert_eq!(entry.min_count, 1);
            assert_eq!(entry.max_count, 1);
        }
    }

    #[test]
    fn iron_ore_drops_iron_ore_item() {
        for block in [BlockState::IRON_ORE, BlockState::DEEPSLATE_IRON_ORE] {
            let entry = block_drop_for(block).expect("iron_ore should have a drop");
            assert_eq!(entry.template_id, "iron_ore");
            assert_eq!(entry.min_count, 1);
            assert_eq!(entry.max_count, 1);
        }
    }

    #[test]
    fn grass_block_drops_grass_fiber_probabilistic() {
        let entry = block_drop_for(BlockState::GRASS_BLOCK).expect("grass should have a drop");
        assert_eq!(entry.template_id, "grass_fiber");
        assert_eq!(entry.min_count, 0);
        assert_eq!(entry.max_count, 1);
    }

    #[test]
    fn air_and_unrecognized_blocks_have_no_drop() {
        assert!(block_drop_for(BlockState::AIR).is_none());
        assert!(block_drop_for(BlockState::DIRT).is_none());
        assert!(block_drop_for(BlockState::BEDROCK).is_none());
        assert!(block_drop_for(BlockState::SAND).is_none());
    }

    #[test]
    fn should_drop_truth_table() {
        assert!(should_drop(DiggingState::Stop, GameMode::Survival));
        assert!(!should_drop(DiggingState::Start, GameMode::Survival));
        assert!(!should_drop(DiggingState::Abort, GameMode::Survival));
        assert!(!should_drop(DiggingState::Start, GameMode::Creative));
        assert!(!should_drop(DiggingState::Stop, GameMode::Creative));
        assert!(!should_drop(DiggingState::Stop, GameMode::Adventure));
        assert!(!should_drop(DiggingState::Stop, GameMode::Spectator));
    }

    #[test]
    fn roll_count_stays_in_range() {
        let entry = BlockDropEntry {
            template_id: "crude_wood",
            min_count: 1,
            max_count: 2,
        };
        for tick in 0..200 {
            let count = roll_count(&entry, BlockPos::new(5, 64, 5), Entity::from_raw(1), tick);
            assert!(
                (entry.min_count..=entry.max_count).contains(&count),
                "tick {tick}: count {count} out of [{}, {}]",
                entry.min_count,
                entry.max_count
            );
        }
    }

    #[test]
    fn roll_count_fixed_when_min_equals_max() {
        let entry = BlockDropEntry {
            template_id: "stone_chunk",
            min_count: 1,
            max_count: 1,
        };
        for tick in 0..50 {
            assert_eq!(
                roll_count(&entry, BlockPos::new(0, 0, 0), Entity::from_raw(0), tick),
                1,
            );
        }
    }

    #[test]
    fn roll_count_grass_can_be_zero() {
        let entry = BlockDropEntry {
            template_id: "grass_fiber",
            min_count: 0,
            max_count: 1,
        };
        let mut saw_zero = false;
        let mut saw_one = false;
        for tick in 0..500 {
            let count = roll_count(&entry, BlockPos::new(3, 64, 7), Entity::from_raw(2), tick);
            match count {
                0 => saw_zero = true,
                1 => saw_one = true,
                _ => panic!("unexpected count {count}"),
            }
            if saw_zero && saw_one {
                break;
            }
        }
        assert!(saw_zero, "grass_fiber should sometimes produce 0");
        assert!(saw_one, "grass_fiber should sometimes produce 1");
    }
}
