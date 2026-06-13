package com.bong.client.block;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BlockVanillaIconMapTest {

    @Test
    void createStackForMapsKnownBongBlocksToVanillaBlockItems() {
        assertEquals("minecraft:dirt", BlockVanillaIconMap.vanillaItemIdForTests("earth_crumb"));
        assertEquals("minecraft:coarse_dirt", BlockVanillaIconMap.vanillaItemIdForTests("hardened_soil"));
        assertEquals("minecraft:sand", BlockVanillaIconMap.vanillaItemIdForTests("barren_sand"));
        assertEquals("minecraft:gravel", BlockVanillaIconMap.vanillaItemIdForTests("weathered_stone"));
        assertEquals("minecraft:clay", BlockVanillaIconMap.vanillaItemIdForTests("raw_clay_lump"));
        assertEquals("minecraft:obsidian", BlockVanillaIconMap.vanillaItemIdForTests("obsidian_shard"));
        assertEquals("minecraft:crafting_table", BlockVanillaIconMap.vanillaItemIdForTests("workbench_item"));
        assertEquals("minecraft:torch", BlockVanillaIconMap.vanillaItemIdForTests("torch_item"));
        assertEquals("minecraft:lantern", BlockVanillaIconMap.vanillaItemIdForTests("lantern_item"));
        assertEquals("minecraft:iron_door", BlockVanillaIconMap.vanillaItemIdForTests("door_bolt"));
        assertEquals("minecraft:iron_bars", BlockVanillaIconMap.vanillaItemIdForTests("window_grate"));
        assertEquals("minecraft:brown_bed", BlockVanillaIconMap.vanillaItemIdForTests("simple_bed"));
        assertEquals("minecraft:brown_carpet", BlockVanillaIconMap.vanillaItemIdForTests("meditation_mat"));
        assertEquals("minecraft:stone_slab", BlockVanillaIconMap.vanillaItemIdForTests("moisture_base"));
        assertEquals(
            "minecraft:chiseled_stone_bricks",
            BlockVanillaIconMap.vanillaItemIdForTests("spirit_stone_rack")
        );
    }

    @Test
    void recognizesKnownBlocksAndReturnsEmptyForUnknown() {
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("earth_crumb"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("workbench_item"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("torch_item"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("window_grate"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("simple_bed"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("meditation_mat"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("moisture_base"));
        assertTrue(BlockVanillaIconMap.isKnownBlockItem("spirit_stone_rack"));
        assertFalse(BlockVanillaIconMap.isKnownBlockItem("missing_block"));
        assertTrue(BlockVanillaIconMap.createStackFor("missing_block").isEmpty());
    }

    // ─── plan-worldgen-v4 P5 §8.1#5 — createVanillaBlockStack 输入校验分支 ───
    //
    // 该方法的 Registries.BLOCK 查询路径（有 BlockItem→stack / air/流体/未知→empty）需要原版
    // 注册表 bootstrap；本 client 测试环境无 MC Bootstrap 基础设施（SharedConstants/Bootstrap
    // 在此抛 IllegalAccessError，全仓无任何 client 测试访问 Registries），故 registry 查询路径
    // 由 e2e / 真游戏覆盖。这里完整锁住**不触 registry 的早退分支**：null/blank/非法 Identifier
    // 都必须在查 registry 之前安全返回 empty，绝不 NPE 或让异常逃逸——这是面板渲染稳定性的契约。

    @Test
    void createVanillaBlockStackReturnsEmptyForNull() {
        assertTrue(
            BlockVanillaIconMap.createVanillaBlockStack(null).isEmpty(),
            "null 短名必须在查 registry 前返回 empty，不得 NPE"
        );
    }

    @Test
    void createVanillaBlockStackReturnsEmptyForBlankInput() {
        assertTrue(
            BlockVanillaIconMap.createVanillaBlockStack("").isEmpty(),
            "空串短名必须返回 empty"
        );
        assertTrue(
            BlockVanillaIconMap.createVanillaBlockStack("   ").isEmpty(),
            "纯空白短名必须返回 empty"
        );
        assertTrue(
            BlockVanillaIconMap.createVanillaBlockStack("\t\n").isEmpty(),
            "制表/换行等空白短名必须返回 empty"
        );
    }

    @Test
    void createVanillaBlockStackReturnsEmptyForInvalidIdentifier() {
        // Identifier path 不接受大写字母与空格 → new Identifier(..) 抛 InvalidIdentifierException，
        // 必须被 catch 返回 empty，不得让异常逃逸到面板渲染。
        for (String invalid : new String[] {"Stone Bricks", "STONE", "a b", "минск"}) {
            assertTrue(
                BlockVanillaIconMap.createVanillaBlockStack(invalid).isEmpty(),
                "非法 Identifier `" + invalid + "` 必须被捕获返回 empty，不得抛异常逃逸"
            );
        }
    }
}
