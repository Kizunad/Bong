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
        "andesite" => BlockState::ANDESITE,
        "polished_diorite" => BlockState::POLISHED_DIORITE,
        "deepslate" => BlockState::DEEPSLATE,
        "cobbled_deepslate" => BlockState::COBBLED_DEEPSLATE,
        "tuff" => BlockState::TUFF,
        "cobblestone" => BlockState::COBBLESTONE,
        "mossy_cobblestone" => BlockState::MOSSY_COBBLESTONE,
        "dead_bush" => BlockState::DEAD_BUSH,
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
        "obsidian" => BlockState::OBSIDIAN,
        "crying_obsidian" => BlockState::CRYING_OBSIDIAN,
        "lodestone" => BlockState::LODESTONE,

        // --- deepslate bricks / stone bricks ---
        "chiseled_deepslate" => BlockState::CHISELED_DEEPSLATE,
        "deepslate_bricks" => BlockState::DEEPSLATE_BRICKS,
        "chiseled_stone_bricks" => BlockState::CHISELED_STONE_BRICKS,
        "cracked_stone_bricks" => BlockState::CRACKED_STONE_BRICKS,
        "mossy_stone_bricks" => BlockState::MOSSY_STONE_BRICKS,
        "warped_planks" => BlockState::WARPED_PLANKS,

        // --- wool / concrete / lights ---
        "white_wool" => BlockState::WHITE_WOOL,
        "red_wool" => BlockState::RED_WOOL,
        "black_wool" => BlockState::BLACK_WOOL,
        "white_concrete" => BlockState::WHITE_CONCRETE,
        "red_concrete" => BlockState::RED_CONCRETE,
        "soul_lantern" => BlockState::SOUL_LANTERN,

        // --- glass ---
        "cyan_stained_glass" => BlockState::CYAN_STAINED_GLASS,
        "light_blue_stained_glass" => BlockState::LIGHT_BLUE_STAINED_GLASS,

        // --- small plants / lichens / stalactites ---
        "glow_lichen" => BlockState::GLOW_LICHEN,
        "dripstone_block" => BlockState::DRIPSTONE_BLOCK,
        "pointed_dripstone" => BlockState::POINTED_DRIPSTONE,
        "lily_pad" => BlockState::LILY_PAD,
        "sugar_cane" => BlockState::SUGAR_CANE,
        "tall_grass" => BlockState::TALL_GRASS,
        "fern" => BlockState::FERN,
        "peony" => BlockState::PEONY,
        "pink_tulip" => BlockState::PINK_TULIP,
        "sweet_berry_bush" => BlockState::SWEET_BERRY_BUSH,
        "prismarine" => BlockState::PRISMARINE,

        // --- signs (碑) ---
        "oak_sign" => BlockState::OAK_SIGN,
        "spruce_sign" => BlockState::SPRUCE_SIGN,
        "birch_sign" => BlockState::BIRCH_SIGN,
        "dark_oak_sign" => BlockState::DARK_OAK_SIGN,

        _ => return None,
    })
}
