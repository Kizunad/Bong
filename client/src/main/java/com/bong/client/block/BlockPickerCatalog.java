package com.bong.client.block;

import net.minecraft.block.Block;
import net.minecraft.item.BlockItem;
import net.minecraft.item.Item;
import net.minecraft.registry.Registries;
import net.minecraft.util.Identifier;

import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;

/**
 * plan-worldgen-v4 P5 §8.1#5 — dev-only 方块审阅面板的 vanilla 方块清单来源。
 *
 * <p>枚举所有 {@code minecraft:} namespace 且拥有可给予 {@link BlockItem} 的方块，
 * 产出不含 namespace 的短名（如 {@code "stone_bricks"}）。面板把短名展示为 give-block
 * 条目，点击后通过 {@code block_picker_give} C2S 让 server 拼 {@code "vanilla:<short>"}
 * 查 ItemRegistry 并发物（**非生产 gameplay**，dev/creative 限定）。</p>
 *
 * <p>枚举核心 {@link #filterPlaceableShortNames(List)} 不触碰 Minecraft 注册表，便于
 * headless 单测锁定过滤规则；{@link #vanillaBlockShortNames()} 才走真实
 * {@link Registries#BLOCK}（仅在游戏内调用）。</p>
 */
public final class BlockPickerCatalog {
    private BlockPickerCatalog() {
    }

    /** 注册表条目的可过滤投影：命名空间 + 路径短名 + 是否拥有可给予 BlockItem。 */
    public record Entry(String namespace, String path, boolean hasBlockItem) {
    }

    /**
     * 过滤核心：仅保留 {@code minecraft:} namespace 且拥有可给予 BlockItem 的条目，
     * 输出去重 + 字典序排序的短名清单。registry-free，便于单测。
     *
     * @param entries 注册表投影（顺序无关）
     * @return 去重排序后的 vanilla 方块短名（如 {@code ["dirt", "stone", "stone_bricks"]}）
     */
    public static List<String> filterPlaceableShortNames(List<Entry> entries) {
        TreeSet<String> sorted = new TreeSet<>();
        if (entries == null) {
            return List.of();
        }
        for (Entry entry : entries) {
            if (entry == null) {
                continue;
            }
            if (!"minecraft".equals(entry.namespace())) {
                continue; // dev 方块面板只暴露 vanilla 方块，bong 自有方块走专属物品协议
            }
            if (!entry.hasBlockItem()) {
                continue; // 无 BlockItem 的方块（如 fire/water）无法作为物品给予，跳过
            }
            String path = entry.path();
            if (path == null || path.isBlank()) {
                continue;
            }
            sorted.add(path);
        }
        return List.copyOf(sorted);
    }

    /**
     * 游戏内枚举：遍历 {@link Registries#BLOCK}，投影为 {@link Entry} 后过滤。
     *
     * <p>headless 单测不调用此方法（注册表未 bootstrap）；改测 {@link #filterPlaceableShortNames(List)}。</p>
     *
     * @return 所有可给予的 vanilla 方块短名，去重 + 字典序排序
     */
    public static List<String> vanillaBlockShortNames() {
        List<Entry> entries = new ArrayList<>();
        for (Identifier id : Registries.BLOCK.getIds()) {
            Block block = Registries.BLOCK.get(id);
            Item item = block.asItem();
            boolean hasBlockItem = item instanceof BlockItem;
            entries.add(new Entry(id.getNamespace(), id.getPath(), hasBlockItem));
        }
        return filterPlaceableShortNames(entries);
    }
}
