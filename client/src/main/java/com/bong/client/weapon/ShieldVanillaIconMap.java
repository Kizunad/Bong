package com.bong.client.weapon;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;

/**
 * #3 手持盾无盾模型：Bong 盾牌 {@code template_id} → vanilla 宿主 {@link Item} 的 fake stack。
 *
 * <p>盾牌状态走 {@link com.bong.client.combat.EquippedShieldStore}（语义非武器，不进
 * {@link com.bong.client.combat.WeaponEquippedStore}）。但「手持物 3D 模型」的渲染链与武器
 * 共用——{@link com.bong.client.mixin.MixinHeldItemRenderer}（FPV）与
 * {@link com.bong.client.mixin.MixinPlayerEntityHeldItem}（TPV）发现 off_hand 真 stack 为空、
 * 且盾槽有盾时，合成对应宿主 vanilla item 的 fake {@link ItemStack}；该宿主 item 的 model JSON
 * 已被 SML 劫持到 Bong 盾牌 OBJ（见 {@link WeaponRenderBootstrap} +
 * {@link BongWeaponModelRegistry#SHIELD_TEMPLATE_IDS}），于是 fake stack 过渲染管线时显示
 * Bong 的 3D 盾牌模型。
 *
 * <p>映射数据统一收口在 {@link BongWeaponModelRegistry}（单一 SML scope 来源，杜绝孤岛），
 * 本类只做盾牌语义入口 + identity 缓存的薄封装，与 {@link WeaponVanillaIconMap} 对称。
 */
public final class ShieldVanillaIconMap {
    private ShieldVanillaIconMap() {}

    /**
     * 缓存单例：多个 Mixin 调用期间保持同一 ItemStack 实例，便于渲染层的 identity/areEqual 缓存。
     */
    private static final java.util.concurrent.ConcurrentHashMap<String, ItemStack> STACK_CACHE =
            new java.util.concurrent.ConcurrentHashMap<>();

    /** 查盾牌 template_id 对应的 fake ItemStack；非盾牌模板或无映射返回 null。 */
    public static ItemStack createStackFor(String templateId) {
        if (templateId == null || !BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS.contains(templateId)) {
            return null;
        }
        Item item = BongWeaponModelRegistry.get(templateId)
            .map(BongWeaponModelRegistry.Entry::hostItem)
            .orElse(null);
        if (item == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(item));
    }
}
