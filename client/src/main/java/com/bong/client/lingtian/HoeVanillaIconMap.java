package com.bong.client.lingtian;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;

/**
 * plan-lingtian-v1 §1.2.1 — Bong 锄头 template_id → vanilla {@link Item}（用作 fake
 * ItemStack 走 vanilla HeldItemRenderer 管线，让主手 FP 视觉立刻出现锄头）。
 *
 * <p>复用 {@link com.bong.client.weapon.WeaponVanillaIconMap} 思路，但锄头走的是
 * vanilla 原生锄头 mesh（无 SML override），三档 vanilla 材质区分档位：
 * <ul>
 *   <li>{@code hoe_iron}    → {@link Items#IRON_HOE}</li>
 *   <li>{@code hoe_lingtie} → {@link Items#DIAMOND_HOE}（蓝白调符合灵铁）</li>
 *   <li>{@code hoe_xuantie} → {@link Items#NETHERITE_HOE}（深色符合玄铁）</li>
 * </ul>
 *
 * <p>不走 {@code WeaponEquippedStore}：锄头不是 {@code Weapon} Component。客户端由
 * {@link com.bong.client.inventory.state.InventoryStateStore} 读主手槽 itemId 就能决
 * 定显示哪档锄。
 */
public final class HoeVanillaIconMap {
    private HoeVanillaIconMap() {}

    private static final Map<String, Item> MAP = Map.of(
            "hoe_iron",    Items.IRON_HOE,
            "hoe_lingtie", Items.DIAMOND_HOE,
            "hoe_xuantie", Items.NETHERITE_HOE
    );

    private static final ConcurrentHashMap<String, ItemStack> STACK_CACHE = new ConcurrentHashMap<>();

    /** 查 Bong 锄头 template_id → vanilla fake ItemStack；非锄头返回 null。 */
    public static ItemStack createStackFor(String templateId) {
        if (templateId == null) return null;
        Item item = MAP.get(templateId);
        if (item == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(item));
    }

    /** 判定是否 Bong 锄头 template_id。 */
    public static boolean isHoe(String templateId) {
        return templateId != null && MAP.containsKey(templateId);
    }
}
