package com.bong.client.block;

import net.minecraft.block.Block;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.registry.Registries;
import net.minecraft.util.Identifier;

import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

/**
 * Bong 方块 template_id → vanilla BlockItem fake stack。
 *
 * <p>Bong 方块仍走自有 ItemInstance/方块协议；这里仅给客户端空手渲染与快捷栏选中态
 * 提供一个稳定的 vanilla 宿主物品，未知 template_id 返回 {@link ItemStack#EMPTY}。
 */
public final class BlockVanillaIconMap {
    private record Entry(String vanillaItemId, Supplier<Item> itemSupplier) {
    }

    private static final Map<String, Entry> HOST_ITEMS = Map.ofEntries(
        Map.entry("earth_crumb", new Entry("minecraft:dirt", () -> Items.DIRT)),
        Map.entry("hardened_soil", new Entry("minecraft:coarse_dirt", () -> Items.COARSE_DIRT)),
        Map.entry("barren_sand", new Entry("minecraft:sand", () -> Items.SAND)),
        Map.entry("weathered_stone", new Entry("minecraft:gravel", () -> Items.GRAVEL)),
        Map.entry("raw_clay_lump", new Entry("minecraft:clay", () -> Items.CLAY)),
        Map.entry("obsidian_shard", new Entry("minecraft:obsidian", () -> Items.OBSIDIAN)),
        Map.entry("workbench_item", new Entry("minecraft:crafting_table", () -> Items.CRAFTING_TABLE)),
        Map.entry("torch_item", new Entry("minecraft:torch", () -> Items.TORCH)),
        Map.entry("lantern_item", new Entry("minecraft:lantern", () -> Items.LANTERN)),
        Map.entry("door_bolt", new Entry("minecraft:iron_door", () -> Items.IRON_DOOR)),
        Map.entry("window_grate", new Entry("minecraft:iron_bars", () -> Items.IRON_BARS)),
        Map.entry("simple_bed", new Entry("minecraft:brown_bed", () -> Items.BROWN_BED)),
        Map.entry("meditation_mat", new Entry("minecraft:brown_carpet", () -> Items.BROWN_CARPET)),
        Map.entry("moisture_base", new Entry("minecraft:stone_slab", () -> Items.STONE_SLAB)),
        Map.entry("spirit_stone_rack", new Entry("minecraft:chiseled_stone_bricks", () -> Items.CHISELED_STONE_BRICKS))
    );
    private static final ConcurrentHashMap<String, ItemStack> STACK_CACHE = new ConcurrentHashMap<>();

    private BlockVanillaIconMap() {
    }

    public static Optional<ItemStack> createStackFor(String templateId) {
        Entry entry = HOST_ITEMS.get(templateId);
        if (entry == null) return Optional.empty();
        return Optional.of(STACK_CACHE.computeIfAbsent(templateId, ignored -> new ItemStack(entry.itemSupplier().get())));
    }

    public static boolean isKnownBlockItem(String templateId) {
        return HOST_ITEMS.containsKey(templateId);
    }

    /**
     * plan-worldgen-v4 P5 §8.1#5 — 按 vanilla 方块短名（如 {@code "stone_bricks"}）取渲染用
     * {@link ItemStack}，供 dev-only 方块审阅面板展示图标。
     *
     * <p>查 {@link Registries#BLOCK}，方块必须拥有可给予的 {@link BlockItem} 才返回 stack；
     * 否则（air / 无 item / 短名非法）返回 {@link Optional#empty()}。该路径与
     * {@link BlockPickerCatalog#vanillaBlockShortNames()} 同源，保证面板每个条目可渲染。</p>
     *
     * @param blockShortId 不含 namespace 的 vanilla 方块短名
     */
    public static Optional<ItemStack> createVanillaBlockStack(String blockShortId) {
        if (blockShortId == null || blockShortId.isBlank()) {
            return Optional.empty();
        }
        Identifier id;
        try {
            id = new Identifier("minecraft", blockShortId);
        } catch (RuntimeException invalidId) {
            return Optional.empty();
        }
        if (!Registries.BLOCK.containsId(id)) {
            return Optional.empty();
        }
        Block block = Registries.BLOCK.get(id);
        Item item = block.asItem();
        if (!(item instanceof BlockItem)) {
            return Optional.empty();
        }
        return Optional.of(new ItemStack(item));
    }

    static String vanillaItemIdForTests(String templateId) {
        Entry entry = HOST_ITEMS.get(templateId);
        return entry == null ? "" : entry.vanillaItemId();
    }
}
