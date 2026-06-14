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

    // ─── plan-block-placement-ux-v1 P3 — usesVanillaItemIcon：哪些 itemId 应走原生方块图标 ───
    //
    // 该谓词不查 registry（HOST_ITEMS 命中即 true / vanilla: 前缀即 true），是渲染路径分流的
    // 唯一判据：true → 调用方走 DrawContext.drawItem(真方块 stack)；false → 走扁平占位贴图。
    // 真正的 ItemStack 解析（registry / drawItem）延后到 drawVanillaIcon，需 MC Bootstrap，由
    // e2e / 真游戏覆盖；这里完整锁住分流谓词的全部分支，让任何回归（漏判 vanilla / 误判普通物品）
    // 立刻撞红。

    @Test
    void usesVanillaItemIconTrueForMappedBongBlocks() {
        // HOST_ITEMS 里映射到 vanilla BlockItem 的 Bong 方块都应走原生图标。
        for (String id : new String[] {
            "earth_crumb", "hardened_soil", "barren_sand", "weathered_stone",
            "raw_clay_lump", "obsidian_shard", "workbench_item", "torch_item",
            "lantern_item", "door_bolt", "window_grate", "simple_bed",
            "meditation_mat", "moisture_base", "spirit_stone_rack"
        }) {
            assertTrue(
                BlockVanillaIconMap.usesVanillaItemIcon(id),
                "HOST_ITEMS 映射方块 `" + id + "` 应走 vanilla 原生图标（usesVanillaItemIcon=true）"
            );
        }
    }

    @Test
    void usesVanillaItemIconTrueForVanillaPrefixTemplates() {
        // BlockPicker 给予的 vanilla:<short> 模板：无论短名是否真存在，前缀即判 true；
        // 真伪由 drawVanillaIcon 内 createStackFor 解析时优雅降级处理。
        assertTrue(BlockVanillaIconMap.usesVanillaItemIcon("vanilla:stone_bricks"),
            "vanilla:stone_bricks 前缀应判走原生图标");
        assertTrue(BlockVanillaIconMap.usesVanillaItemIcon("vanilla:oak_planks"),
            "vanilla:oak_planks 前缀应判走原生图标");
        assertTrue(BlockVanillaIconMap.usesVanillaItemIcon("vanilla:"),
            "退化的 vanilla: 空短名仍命中前缀分支（drawVanillaIcon 内再降级）");
    }

    @Test
    void usesVanillaItemIconFalseForNonBlockAndNullEmpty() {
        // 功法卷轴 / 丹药 / 装备等普通物品：仍走扁平贴图，绝不误判成方块图标。
        assertFalse(BlockVanillaIconMap.usesVanillaItemIcon("guyuan_pill"),
            "丹药 guyuan_pill 不是方块，应走扁平贴图（usesVanillaItemIcon=false）");
        assertFalse(BlockVanillaIconMap.usesVanillaItemIcon("skill_scroll_fireball"),
            "功法卷轴不是方块，应走扁平贴图");
        assertFalse(BlockVanillaIconMap.usesVanillaItemIcon("missing_block"),
            "未在 HOST_ITEMS 且无 vanilla: 前缀的 id 应走扁平贴图");
        assertFalse(BlockVanillaIconMap.usesVanillaItemIcon(null),
            "null itemId 不得 NPE，应返回 false");
        assertFalse(BlockVanillaIconMap.usesVanillaItemIcon(""),
            "空 itemId 应返回 false");
    }

    // ─── plan-block-placement-ux-v1 P3 — drawVanillaIcon 早退分支（不触 registry / DrawContext）───
    //
    // 当 ctx==null / size<=0 / 非 vanilla itemId 时，drawVanillaIcon 必须在调用 createStackFor 与
    // ctx.drawItem 之前安全返回 false（调用方据此回退占位贴图）。这些路径不需要 MC Bootstrap，
    // 锁住「渲染分流稳定、绝不 NPE、非 vanilla 一律交还占位路径」契约。真正的 vanilla 绘制
    // （createStackFor 命中 + ctx.drawItem）由 e2e / 真游戏覆盖。

    @Test
    void drawVanillaIconReturnsFalseForNullContext() {
        // ctx==null（理论不应发生，但渲染入口必须健壮）→ 返回 false，不 NPE。
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, "vanilla:stone_bricks", 0, 0, 16),
            "ctx==null 必须返回 false 且不 NPE"
        );
    }

    @Test
    void drawVanillaIconReturnsFalseForNonPositiveSize() {
        // size<=0 → 无可绘区域，返回 false（在触 createStackFor 之前短路，不触 registry）。
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, "vanilla:stone_bricks", 0, 0, 0),
            "size==0 应返回 false"
        );
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, "vanilla:stone_bricks", 0, 0, -4),
            "size<0 应返回 false"
        );
    }

    @Test
    void drawVanillaIconReturnsFalseForNonVanillaItem() {
        // 非 vanilla 物品在 usesVanillaItemIcon 处即判 false → 不触 createStackFor / drawItem，
        // 返回 false 交还占位贴图路径。ctx 传 null 也安全（usesVanillaItemIcon 先短路）。
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, "guyuan_pill", 0, 0, 16),
            "非方块物品必须返回 false 走占位贴图，且不因 ctx==null NPE"
        );
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, "missing_block", 0, 0, 16),
            "未知 id 必须返回 false 走占位贴图"
        );
        assertFalse(
            BlockVanillaIconMap.drawVanillaIcon(null, null, 0, 0, 16),
            "null id 必须返回 false 且不 NPE"
        );
    }
}
