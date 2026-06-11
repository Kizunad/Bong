package com.bong.client.block;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;

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
        Map.entry("window_grate", new Entry("minecraft:iron_bars", () -> Items.IRON_BARS))
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

    static String vanillaItemIdForTests(String templateId) {
        Entry entry = HOST_ITEMS.get(templateId);
        return entry == null ? "" : entry.vanillaItemId();
    }
}
