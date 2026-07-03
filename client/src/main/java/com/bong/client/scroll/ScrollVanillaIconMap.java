package com.bong.client.scroll;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;

/**
 * plan-scroll-reading-v1 P2 — 可阅读残卷 template_id → vanilla {@link Item}（白嫖
 * {@link Items#PAPER} 走 vanilla HeldItemRenderer 管线，不做 OBJ 模型）。
 *
 * <p>照抄 {@link com.bong.client.lingtian.HoeVanillaIconMap} 的惰性
 * {@code Supplier<Item>} 模式：headless 单测环境没有 MC Bootstrap，{@code Items}
 * 类字段若在静态初始化时直接求值会 panic。{@link #isReadableScroll} 只查 key、不触发
 * {@code Items} 求值；{@link #createStackFor} 仅在命中已知残卷 id 时才求值。
 *
 * <p>纳入 {@link com.bong.client.weapon.HeldItemStackResolver#resolveMainHand()}
 * fallback 链第五级（weapon → block → hoe → 之后），FPV+TPV 双入口自动同步。
 *
 * <p>后续新增可阅读残卷（丹方预览/黑武士遗书/图书馆残页等，见 plan §8.1 #6 复用清单）
 * 只需在 {@link #MAP} 里追加一行 template_id，无需改 resolver 分支逻辑。
 */
public final class ScrollVanillaIconMap {
    private ScrollVanillaIconMap() {}

    private static final Map<String, Supplier<Item>> MAP = Map.of(
            "scroll_meridian_primer", () -> Items.PAPER
    );

    private static final ConcurrentHashMap<String, ItemStack> STACK_CACHE = new ConcurrentHashMap<>();

    /** 查 Bong 可阅读残卷 template_id → vanilla fake ItemStack；非残卷返回 null。 */
    public static ItemStack createStackFor(String templateId) {
        if (templateId == null) return null;
        Supplier<Item> supplier = MAP.get(templateId);
        if (supplier == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(supplier.get()));
    }

    /** 判定是否已知可阅读残卷 template_id。仅查 key，不触发 vanilla {@link Items} 求值。 */
    public static boolean isReadableScroll(String templateId) {
        return templateId != null && MAP.containsKey(templateId);
    }
}
