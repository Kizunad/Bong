//! Name → `BlockState` translation used by the flora decoration pipeline.
//!
//! Every block name that appears in a `DecorationSpec.blocks` entry of any
//! worldgen profile must resolve here; otherwise the flora placer silently
//! skips that block slot. Keep this file in sync with
//! `worldgen/scripts/terrain_gen/profiles/*.py`.
//!
//! Returning `Option<BlockState>` instead of `Result` lets callers degrade
//! gracefully when a manifest references a block the server doesn't yet know
//! how to spawn (e.g. future stained glass variants): the decoration just
//! won't render that block, rather than panicking the chunk generator.

use valence::prelude::BlockState;

/// Resolve a Minecraft block name (matching the palette strings authored in
/// Python) into a Valence `BlockState`. Returns `None` for unknown names.
pub fn block_from_name(name: &str) -> Option<BlockState> {
    Some(match name {
        // --- natural / surface (mirrors raster::block_state_from_name) ---
        "stone" => BlockState::STONE,
        "smooth_stone" => BlockState::SMOOTH_STONE,
        "coarse_dirt" => BlockState::COARSE_DIRT,
        "gravel" => BlockState::GRAVEL,
        "grass_block" => BlockState::GRASS_BLOCK,
        "dirt" => BlockState::DIRT,
        "sand" => BlockState::SAND,
        "sandstone" => BlockState::SANDSTONE,
        "red_sandstone" => BlockState::RED_SANDSTONE,
        "red_sand" => BlockState::RED_SAND,
        "chiseled_red_sandstone" => BlockState::CHISELED_RED_SANDSTONE,
        "terracotta" => BlockState::TERRACOTTA,
        "red_terracotta" => BlockState::RED_TERRACOTTA,
        "blackstone" => BlockState::BLACKSTONE,
        "basalt" => BlockState::BASALT,
        "polished_basalt" => BlockState::POLISHED_BASALT,
        "magma_block" => BlockState::MAGMA_BLOCK,
        "crimson_nylium" => BlockState::CRIMSON_NYLIUM,
        "calcite" => BlockState::CALCITE,
        "snow_block" => BlockState::SNOW_BLOCK,
        "packed_ice" => BlockState::PACKED_ICE,
        "blue_ice" => BlockState::BLUE_ICE,
        "podzol" => BlockState::PODZOL,
        "rooted_dirt" => BlockState::ROOTED_DIRT,
        "soul_sand" => BlockState::SOUL_SAND,
        "soul_soil" => BlockState::SOUL_SOIL,
        "bone_block" => BlockState::BONE_BLOCK,
        "mud" => BlockState::MUD,
        "clay" => BlockState::CLAY,
        "moss_block" => BlockState::MOSS_BLOCK,
        "moss_carpet" => BlockState::MOSS_CARPET,
        "andesite" => BlockState::ANDESITE,
        "polished_andesite" => BlockState::POLISHED_ANDESITE,
        "polished_diorite" => BlockState::POLISHED_DIORITE,
        "deepslate" => BlockState::DEEPSLATE,
        "cobbled_deepslate" => BlockState::COBBLED_DEEPSLATE,
        "tuff" => BlockState::TUFF,
        "cobblestone" => BlockState::COBBLESTONE,
        "mossy_cobblestone" => BlockState::MOSSY_COBBLESTONE,
        "dead_bush" => BlockState::DEAD_BUSH,
        "grass" => BlockState::GRASS,
        "cobweb" => BlockState::COBWEB,
        "gray_concrete_powder" => BlockState::GRAY_CONCRETE_POWDER,
        "cobblestone_wall" => BlockState::COBBLESTONE_WALL,
        "torch" => BlockState::TORCH,
        "stone_button" => BlockState::STONE_BUTTON,
        "muddy_mangrove_roots" => BlockState::MUDDY_MANGROVE_ROOTS,

        // --- logs & leaves ---
        "oak_log" => BlockState::OAK_LOG,
        "stripped_oak_log" => BlockState::STRIPPED_OAK_LOG,
        "oak_leaves" => BlockState::OAK_LEAVES,
        "birch_log" => BlockState::BIRCH_LOG,
        "birch_leaves" => BlockState::BIRCH_LEAVES,
        "stripped_birch_log" => BlockState::STRIPPED_BIRCH_LOG,
        "spruce_log" => BlockState::SPRUCE_LOG,
        "spruce_leaves" => BlockState::SPRUCE_LEAVES,
        "stripped_spruce_log" => BlockState::STRIPPED_SPRUCE_LOG,
        "jungle_log" => BlockState::JUNGLE_LOG,
        "azalea_leaves" => BlockState::AZALEA_LEAVES,
        "flowering_azalea_leaves" => BlockState::FLOWERING_AZALEA_LEAVES,
        "mangrove_log" => BlockState::MANGROVE_LOG,
        "mangrove_leaves" => BlockState::MANGROVE_LEAVES,
        "mangrove_roots" => BlockState::MANGROVE_ROOTS,

        // --- bamboo ---
        "bamboo_block" => BlockState::BAMBOO_BLOCK,
        "stripped_bamboo_block" => BlockState::STRIPPED_BAMBOO_BLOCK,

        // --- nether ---
        "crimson_stem" => BlockState::CRIMSON_STEM,
        "crimson_hyphae" => BlockState::CRIMSON_HYPHAE,
        "crimson_roots" => BlockState::CRIMSON_ROOTS,
        "warped_wart_block" => BlockState::WARPED_WART_BLOCK,
        "nether_wart_block" => BlockState::NETHER_WART_BLOCK,
        "weeping_vines" => BlockState::WEEPING_VINES,
        "red_mushroom_block" => BlockState::RED_MUSHROOM_BLOCK,
        "brown_mushroom_block" => BlockState::BROWN_MUSHROOM_BLOCK,
        "mushroom_stem" => BlockState::MUSHROOM_STEM,
        "shroomlight" => BlockState::SHROOMLIGHT,

        // --- metals / crystals / quartz ---
        "iron_block" => BlockState::IRON_BLOCK,
        "copper_block" => BlockState::COPPER_BLOCK,
        "weathered_copper" => BlockState::WEATHERED_COPPER,
        "diamond_block" => BlockState::DIAMOND_BLOCK,
        "emerald_block" => BlockState::EMERALD_BLOCK,
        "quartz_block" => BlockState::QUARTZ_BLOCK,
        "smooth_quartz" => BlockState::SMOOTH_QUARTZ,
        "quartz_stairs" => BlockState::QUARTZ_STAIRS,
        "amethyst_block" => BlockState::AMETHYST_BLOCK,
        "amethyst_cluster" => BlockState::AMETHYST_CLUSTER,
        "budding_amethyst" => BlockState::BUDDING_AMETHYST,
        "emerald_ore" => BlockState::EMERALD_ORE,
        "deepslate_emerald_ore" => BlockState::DEEPSLATE_EMERALD_ORE,
        "obsidian" => BlockState::OBSIDIAN,
        "crying_obsidian" => BlockState::CRYING_OBSIDIAN,
        "lodestone" => BlockState::LODESTONE,

        // --- deepslate bricks / stone bricks ---
        "chiseled_deepslate" => BlockState::CHISELED_DEEPSLATE,
        "deepslate_bricks" => BlockState::DEEPSLATE_BRICKS,
        "stone_bricks" => BlockState::STONE_BRICKS,
        "chiseled_stone_bricks" => BlockState::CHISELED_STONE_BRICKS,
        "cracked_stone_bricks" => BlockState::CRACKED_STONE_BRICKS,
        "mossy_stone_bricks" => BlockState::MOSSY_STONE_BRICKS,
        "smooth_stone_slab" => BlockState::SMOOTH_STONE_SLAB,
        "stone_brick_slab" => BlockState::STONE_BRICK_SLAB,
        "stone_brick_stairs" => BlockState::STONE_BRICK_STAIRS,
        "warped_planks" => BlockState::WARPED_PLANKS,
        "end_stone" => BlockState::END_STONE,

        // --- wool / concrete / lights ---
        "white_wool" => BlockState::WHITE_WOOL,
        "red_wool" => BlockState::RED_WOOL,
        "black_wool" => BlockState::BLACK_WOOL,
        "white_concrete" => BlockState::WHITE_CONCRETE,
        "black_concrete" => BlockState::BLACK_CONCRETE,
        "red_concrete" => BlockState::RED_CONCRETE,
        "light_blue_concrete" => BlockState::LIGHT_BLUE_CONCRETE,
        "soul_lantern" => BlockState::SOUL_LANTERN,
        "redstone_lamp" => BlockState::REDSTONE_LAMP,
        "verdant_froglight" => BlockState::VERDANT_FROGLIGHT,

        // --- glass ---
        "cyan_stained_glass" => BlockState::CYAN_STAINED_GLASS,
        "light_blue_stained_glass" => BlockState::LIGHT_BLUE_STAINED_GLASS,
        "blue_stained_glass" => BlockState::BLUE_STAINED_GLASS,
        "glass" => BlockState::GLASS,
        "tinted_glass" => BlockState::TINTED_GLASS,

        // --- small plants / lichens / stalactites ---
        "glow_lichen" => BlockState::GLOW_LICHEN,
        "sculk" => BlockState::SCULK,
        "sculk_vein" => BlockState::SCULK_VEIN,
        "dripstone_block" => BlockState::DRIPSTONE_BLOCK,
        "pointed_dripstone" => BlockState::POINTED_DRIPSTONE,
        "lily_pad" => BlockState::LILY_PAD,
        "sugar_cane" => BlockState::SUGAR_CANE,
        "tall_grass" => BlockState::TALL_GRASS,
        "fern" => BlockState::FERN,
        "dandelion" => BlockState::DANDELION,
        "poppy" => BlockState::POPPY,
        "peony" => BlockState::PEONY,
        "pink_tulip" => BlockState::PINK_TULIP,
        "pink_petals" => BlockState::PINK_PETALS,
        "lily_of_the_valley" => BlockState::LILY_OF_THE_VALLEY,
        "sweet_berry_bush" => BlockState::SWEET_BERRY_BUSH,
        "prismarine" => BlockState::PRISMARINE,
        "tube_coral_block" => BlockState::TUBE_CORAL_BLOCK,

        // --- signs (碑) ---
        "oak_sign" => BlockState::OAK_SIGN,
        "spruce_sign" => BlockState::SPRUCE_SIGN,
        "birch_sign" => BlockState::BIRCH_SIGN,
        "dark_oak_sign" => BlockState::DARK_OAK_SIGN,

        // --- agriculture / worldgen placement ---
        "farmland" => BlockState::FARMLAND,
        "wheat" => BlockState::WHEAT,
        "potatoes" => BlockState::POTATOES,
        "carrots" => BlockState::CARROTS,
        "beetroots" => BlockState::BEETROOTS,

        // --- misc structure blocks ---
        "chiseled_polished_blackstone" => BlockState::CHISELED_POLISHED_BLACKSTONE,
        "polished_blackstone" => BlockState::POLISHED_BLACKSTONE,
        "polished_blackstone_bricks" => BlockState::POLISHED_BLACKSTONE_BRICKS,
        "cracked_polished_blackstone_bricks" => BlockState::CRACKED_POLISHED_BLACKSTONE_BRICKS,
        "nether_bricks" => BlockState::NETHER_BRICKS,
        "red_nether_bricks" => BlockState::RED_NETHER_BRICKS,
        "end_stone_bricks" => BlockState::END_STONE_BRICKS,
        "purpur_block" => BlockState::PURPUR_BLOCK,
        "lectern" => BlockState::LECTERN,
        "wither_skeleton_skull" => BlockState::WITHER_SKELETON_SKULL,
        "skeleton_skull" => BlockState::SKELETON_SKULL,
        "barrel" => BlockState::BARREL,
        "crafting_table" => BlockState::CRAFTING_TABLE,
        "furnace" => BlockState::FURNACE,
        "brewing_stand" => BlockState::BREWING_STAND,
        "cauldron" => BlockState::CAULDRON,
        "bookshelf" => BlockState::BOOKSHELF,
        "chiseled_bookshelf" => BlockState::CHISELED_BOOKSHELF,
        "chest" => BlockState::CHEST,
        "trapped_chest" => BlockState::TRAPPED_CHEST,
        "iron_bars" => BlockState::IRON_BARS,
        "raw_iron_block" => BlockState::RAW_IRON_BLOCK,
        "chain" => BlockState::CHAIN,
        "lantern" => BlockState::LANTERN,
        "end_rod" => BlockState::END_ROD,
        "lightning_rod" => BlockState::LIGHTNING_ROD,
        "sea_lantern" => BlockState::SEA_LANTERN,
        "glowstone" => BlockState::GLOWSTONE,
        "glowshroom" => BlockState::SHROOMLIGHT,
        "air" => BlockState::AIR,
        "cave_air" => BlockState::CAVE_AIR,

        // --- NBT structure blocks (dan_zong / wangyintai palette) ---
        "smooth_basalt" => BlockState::SMOOTH_BASALT,
        "polished_deepslate" => BlockState::POLISHED_DEEPSLATE,
        "polished_deepslate_slab" => BlockState::POLISHED_DEEPSLATE_SLAB,
        "deepslate_brick_slab" => BlockState::DEEPSLATE_BRICK_SLAB,
        "cracked_deepslate_bricks" => BlockState::CRACKED_DEEPSLATE_BRICKS,
        "water" => BlockState::WATER,
        "vine" => BlockState::VINE,
        "dark_oak_planks" => BlockState::DARK_OAK_PLANKS,
        "oak_planks" => BlockState::OAK_PLANKS,
        "spruce_planks" => BlockState::SPRUCE_PLANKS,
        "dark_oak_slab" => BlockState::DARK_OAK_SLAB,
        "dark_oak_stairs" => BlockState::DARK_OAK_STAIRS,
        "polished_blackstone_slab" => BlockState::POLISHED_BLACKSTONE_SLAB,
        "polished_blackstone_stairs" => BlockState::POLISHED_BLACKSTONE_STAIRS,
        "polished_blackstone_wall" => BlockState::POLISHED_BLACKSTONE_WALL,
        "purple_terracotta" => BlockState::PURPLE_TERRACOTTA,
        "purple_glazed_terracotta" => BlockState::PURPLE_GLAZED_TERRACOTTA,
        "purple_stained_glass" => BlockState::PURPLE_STAINED_GLASS,
        "purple_stained_glass_pane" => BlockState::PURPLE_STAINED_GLASS_PANE,
        "campfire" => BlockState::CAMPFIRE,
        "soul_campfire" => BlockState::SOUL_CAMPFIRE,
        "candle" => BlockState::CANDLE,
        "coal_block" => BlockState::COAL_BLOCK,
        "coal_ore" => BlockState::COAL_ORE,
        "cut_copper" => BlockState::CUT_COPPER,
        "raw_copper_block" => BlockState::RAW_COPPER_BLOCK,
        "flower_pot" => BlockState::FLOWER_POT,
        "birch_pressure_plate" => BlockState::BIRCH_PRESSURE_PLATE,
        "oak_fence" => BlockState::OAK_FENCE,
        "red_mushroom" => BlockState::RED_MUSHROOM,
        "brown_mushroom" => BlockState::BROWN_MUSHROOM,
        "warped_roots" => BlockState::WARPED_ROOTS,
        "twisting_vines" => BlockState::TWISTING_VINES,
        "bamboo" => BlockState::BAMBOO,
        "small_amethyst_bud" => BlockState::SMALL_AMETHYST_BUD,
        "white_banner" => BlockState::WHITE_BANNER,
        // iron_nugget is an item, not a placeable block in MC 1.20.1;
        // map to air so the slot is not silently dropped.
        "iron_nugget" => BlockState::AIR,

        _ => return None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::block_from_name;

    /// Full list of distinct block names authored in dan_zong and wangyintai NBT
    /// structures (generated by scripts/nbt dump; updated whenever structures change).
    ///
    /// Any block added to the NBT palette must be added here AND resolved in
    /// `block_from_name` above; this test is the dual-pin contract.
    pub const AUTHORED_STRUCTURE_BLOCKS: &[&str] = &[
        "amethyst_block",
        "amethyst_cluster",
        "andesite",
        "birch_pressure_plate",
        "blackstone",
        "bone_block",
        "bookshelf",
        "calcite",
        "campfire",
        "candle",
        "cauldron",
        "chain",
        "chiseled_deepslate",
        "chiseled_polished_blackstone",
        "chiseled_stone_bricks",
        "coal_block",
        "coal_ore",
        "coarse_dirt",
        "cobblestone",
        "cobblestone_wall",
        "cobweb",
        "cracked_deepslate_bricks",
        "cracked_polished_blackstone_bricks",
        "cracked_stone_bricks",
        "dark_oak_planks",
        "dark_oak_slab",
        "dark_oak_stairs",
        "dead_bush",
        "deepslate_brick_slab",
        "deepslate_bricks",
        "flower_pot",
        "gravel",
        "iron_nugget",
        "lectern",
        "mossy_cobblestone",
        "mossy_stone_bricks",
        "oak_fence",
        "oak_log",
        "podzol",
        "polished_blackstone",
        "polished_blackstone_bricks",
        "polished_blackstone_slab",
        "polished_blackstone_stairs",
        "polished_blackstone_wall",
        "polished_deepslate",
        "polished_deepslate_slab",
        "purple_glazed_terracotta",
        "purple_stained_glass",
        "purple_stained_glass_pane",
        "purple_terracotta",
        "red_mushroom",
        "skeleton_skull",
        "smooth_basalt",
        "soul_campfire",
        "soul_lantern",
        "soul_sand",
        "soul_soil",
        "stone_brick_slab",
        "stone_brick_stairs",
        "stone_bricks",
        "vine",
        "water",
        "white_banner",
    ];

    /// Every block name in the authored NBT palette must resolve in `block_from_name`.
    ///
    /// When a new structure is authored:
    ///
    /// 1. Run the dump script (`cd <repo> && python3 -c "..."`) to get the new
    ///    distinct block list.
    /// 2. Add missing names to AUTHORED_STRUCTURE_BLOCKS above.
    /// 3. Add missing names to `block_from_name` above.
    ///
    /// This test will then confirm both sides are aligned.
    #[test]
    fn all_authored_structure_blocks_resolve() {
        let mut failures: Vec<&str> = Vec::new();
        for name in AUTHORED_STRUCTURE_BLOCKS {
            if block_from_name(name).is_none() {
                failures.push(name);
            }
        }
        assert!(
            failures.is_empty(),
            "The following authored NBT block names are missing from block_from_name: {:?}\n\
             Add each name to both AUTHORED_STRUCTURE_BLOCKS (test fixture) and block_from_name \
             (runtime resolver) to prevent silent structure holes.",
            failures
        );
    }

    /// Smoke-test: known blocks resolve to Some; non-existent name resolves to None.
    #[test]
    fn block_from_name_known_and_unknown() {
        assert!(block_from_name("stone").is_some(), "stone must resolve");
        assert!(
            block_from_name("smooth_basalt").is_some(),
            "smooth_basalt must resolve (NBT palette block)"
        );
        assert!(
            block_from_name("polished_deepslate").is_some(),
            "polished_deepslate must resolve (NBT palette block)"
        );
        assert!(
            block_from_name("water").is_some(),
            "water must resolve (NBT palette block)"
        );
        assert!(
            block_from_name("vine").is_some(),
            "vine must resolve (NBT palette block)"
        );
        assert!(
            block_from_name("nonexistent_fantasy_block").is_none(),
            "unknown name must return None"
        );
    }
}
