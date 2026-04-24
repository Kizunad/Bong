package com.bong.client.weapon;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;

/**
 * plan-weapon-v1 §5.1 的中间层：Bong {@code template_id} → vanilla {@link Item}。
 *
 * <p>为什么要这个映射：
 * Bong 的武器走 ItemInstance 系统，vanilla {@code PlayerEntity.getMainHandStack()}
 * 始终是 EMPTY。{@link com.bong.client.mixin.MixinHeldItemRenderer} 在 renderItem
 * 的 HEAD 插桩，查 {@link com.bong.client.combat.WeaponEquippedStore} 后，合成一个
 * 对应 vanilla item 的 fake ItemStack 让 vanilla 渲染管线走下去。SML
 * (Special Model Loader) 的 LOAD_SCOPE 已经把 vanilla item model JSON 劫持到 Bong
 * 的 OBJ（见 {@link WeaponRenderBootstrap#VANILLA_ITEM_OVERRIDES}），所以 fake
 * stack 过 renderer 时自然显示 Bong 的 3D 武器模型。
 *
 * <p>具体宿主映射已收口到 {@link BongWeaponModelRegistry}，避免再散落 placeholder/复用注释。
 */
public final class WeaponVanillaIconMap {
    private WeaponVanillaIconMap() {}

    /** 查 template_id 对应的 fake ItemStack；没映射返回 null。
     *
     * 缓存单例：多个 Mixin 调用期间保持同一 ItemStack 实例，便于渲染层的 identity/areEqual 缓存。
     */
    private static final java.util.concurrent.ConcurrentHashMap<String, ItemStack> STACK_CACHE =
            new java.util.concurrent.ConcurrentHashMap<>();

    public static ItemStack createStackFor(String templateId) {
        Item item = BongWeaponModelRegistry.get(templateId)
            .map(BongWeaponModelRegistry.Entry::hostItem)
            .orElse(null);
        if (item == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(item));
    }
}
