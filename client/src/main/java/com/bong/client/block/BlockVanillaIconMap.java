package com.bong.client.block;

import net.minecraft.block.Block;
import net.minecraft.client.gui.DrawContext;
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

    /** Mirrors server {@code inventory::VANILLA_TEMPLATE_PREFIX}. */
    public static final String VANILLA_TEMPLATE_PREFIX = "vanilla:";

    private BlockVanillaIconMap() {
    }

    public static Optional<ItemStack> createStackFor(String templateId) {
        if (templateId == null) return Optional.empty();
        Entry entry = HOST_ITEMS.get(templateId);
        if (entry != null) {
            return Optional.of(STACK_CACHE.computeIfAbsent(templateId, ignored -> new ItemStack(entry.itemSupplier().get())));
        }
        // BlockPicker `vanilla:<short>` templates render with the matching vanilla item.
        if (templateId.startsWith(VANILLA_TEMPLATE_PREFIX)) {
            return createVanillaBlockStack(templateId.substring(VANILLA_TEMPLATE_PREFIX.length()));
        }
        return Optional.empty();
    }

    public static boolean isKnownBlockItem(String templateId) {
        if (templateId == null) return false;
        if (HOST_ITEMS.containsKey(templateId)) return true;
        // BlockPicker `vanilla:<short>` templates are placeable too: the server
        // resolves them via BlockKind::from_str in block_item_to_state(). The
        // client gate must mirror that, else the place intent is never sent and
        // right-clicking a picked vanilla block silently does nothing.
        if (templateId.startsWith(VANILLA_TEMPLATE_PREFIX)) {
            return createVanillaBlockStack(templateId.substring(VANILLA_TEMPLATE_PREFIX.length())).isPresent();
        }
        return false;
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

    /**
     * Vanilla 模型在 {@link DrawContext#drawItem(ItemStack, int, int)} 下的固定渲染边长（像素）。
     * 缩放到任意槽位大小时以此为基准。
     */
    private static final int VANILLA_ICON_SIZE = 16;

    /**
     * plan-block-placement-ux-v1 P3 — 该 itemId 是否应以 vanilla 原生方块图标渲染
     * （而非扁平 {@code bong-client:textures/gui/items/<id>.png} 占位贴图）。
     *
     * <p>覆盖两类：① HOST_ITEMS 里映射到 vanilla BlockItem 的 Bong 方块（如 {@code earth_crumb}）；
     * ② BlockPicker 给予的 {@code vanilla:<short>} 模板。两者经 {@link #createStackFor} 都能拿到一个
     * 非空的 vanilla {@link ItemStack}。其余物品（功法卷轴、丹药、装备等）仍走扁平贴图，返回 {@code false}。</p>
     *
     * <p>注意：该判断不查 registry——HOST_ITEMS 命中即 true；{@code vanilla:} 前缀也直接 true，
     * 真正的 registry 解析延后到 {@link #drawVanillaIcon} 内 {@link #createStackFor} 时执行，
     * 无 BlockItem 的特殊块在那一步优雅降级（不绘制，调用方按 false 处理回退占位贴图）。</p>
     */
    public static boolean usesVanillaItemIcon(String itemId) {
        if (itemId == null || itemId.isEmpty()) {
            return false;
        }
        return HOST_ITEMS.containsKey(itemId) || itemId.startsWith(VANILLA_TEMPLATE_PREFIX);
    }

    /**
     * plan-block-placement-ux-v1 P3 — 若 {@code itemId} 对应一个 vanilla 方块/物品，则用 MC 原生
     * {@link DrawContext#drawItem(ItemStack, int, int)} 在 {@code (dx, dy)} 处渲染一个 {@code size×size}
     * 的真方块图标（与 BlockPicker 面板 {@code Components.item(stack)} 视觉一致），返回 {@code true}。
     *
     * <p>否则（非 vanilla 物品、未知短名、air/流体/无 BlockItem 的特殊块）<b>不绘制</b>并返回
     * {@code false}，调用方据此回退到扁平贴图路径。这样「无 BlockItem 的特殊块优雅降级」与
     * 「普通物品走扁平贴图」共用同一个 false 分支，不会画出错误的占位图。</p>
     *
     * @param ctx    渲染上下文（{@code OwoUIDrawContext} 是其子类，单格槽位与多格 overlay/HUD 共用）
     * @param itemId Bong template_id 或 {@code vanilla:<short>}
     * @param dx     目标左上角 x
     * @param dy     目标左上角 y
     * @param size   目标边长（像素）；{@code <= 0} 直接返回 false 不绘制
     * @return 是否已用 vanilla 图标渲染（true=已画，调用方勿再画占位贴图）
     */
    public static boolean drawVanillaIcon(DrawContext ctx, String itemId, int dx, int dy, int size) {
        if (ctx == null || size <= 0 || !usesVanillaItemIcon(itemId)) {
            return false;
        }
        Optional<ItemStack> stack = createStackFor(itemId);
        if (stack.isEmpty()) {
            // 无 BlockItem 的特殊块 / 非法短名 → 优雅降级，交给调用方回退占位贴图。
            return false;
        }
        var matrices = ctx.getMatrices();
        matrices.push();
        // drawItem 固定按 16px 渲染；平移到目标点后整体缩放到 size。
        matrices.translate(dx, dy, 0);
        float scale = (float) size / VANILLA_ICON_SIZE;
        matrices.scale(scale, scale, 1.0f);
        ctx.drawItem(stack.get(), 0, 0);
        matrices.pop();
        return true;
    }

    static String vanillaItemIdForTests(String templateId) {
        Entry entry = HOST_ITEMS.get(templateId);
        return entry == null ? "" : entry.vanillaItemId();
    }
}
