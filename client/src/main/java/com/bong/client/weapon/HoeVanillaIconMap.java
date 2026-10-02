package com.bong.client.weapon;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;

/** 采药锄头使用原版模型；惰性解析物品，避免无游戏环境时触发注册表初始化。 */
public final class HoeVanillaIconMap {
    private HoeVanillaIconMap() {}

    private static final Map<String, Supplier<Item>> MAP = Map.of(
            "hoe_iron",    () -> Items.IRON_HOE,
            "hoe_lingtie", () -> Items.DIAMOND_HOE,
            "hoe_xuantie", () -> Items.NETHERITE_HOE
    );

    private static final ConcurrentHashMap<String, ItemStack> STACK_CACHE = new ConcurrentHashMap<>();

    /** 查 Bong 锄头 template_id → vanilla fake ItemStack；非锄头返回 null。 */
    public static ItemStack createStackFor(String templateId) {
        if (templateId == null) return null;
        Supplier<Item> supplier = MAP.get(templateId);
        if (supplier == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(supplier.get()));
    }

    /** 判定是否 Bong 锄头 template_id。仅查 key，不触发 vanilla {@link Items} 求值。 */
    public static boolean isHoe(String templateId) {
        return templateId != null && MAP.containsKey(templateId);
    }
}
