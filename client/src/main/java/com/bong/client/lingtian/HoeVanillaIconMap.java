package com.bong.client.lingtian;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;

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
 *
 * <p>F8 附带修复：{@code MAP} 值原为 {@code Items.IRON_HOE} 等直接引用——静态字段初始化
 * 在类加载时立即求值，意味着任何一次 {@link #isHoe} / {@link #createStackFor} 调用
 * （哪怕传非锄头 id）都会触发 {@code Items} 类初始化，而 {@code Items} 需要 MC Bootstrap
 * （headless 单测环境没有，见 {@code ShieldVanillaIconMapTest} / {@code BlockVanillaIconMapTest}
 * 的同类注释）。改成 {@code Supplier<Item>}（与 {@link com.bong.client.weapon.WeaponVanillaIconMap}
 * / {@code BongWeaponModelRegistry} / {@code BlockVanillaIconMap} 同款惰性求值模式）后，
 * {@code isHoe} 纯查 key、不触发 {@code Items} 求值；{@code createStackFor} 仅在命中已知
 * 锄头 id 时才求值 {@code Items}——这是从 {@code HeldItemStackResolver}（F8 抽出的共享
 * fallback resolver）单测里发现的：resolver 需要对"任意非空主手装备物"调用 {@code isHoe}
 * 判断是否落 hoe tier，此前任何非锄头 id 都会连带炸类初始化，导致 hoe/block fallback 链
 * 完全无法被单测覆盖。纯行为保持：公开 API 签名/语义不变。
 */
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
