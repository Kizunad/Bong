package com.bong.client.weapon;

import java.util.Map;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;

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
 * <p>映射规则：每把武器挑一个 <strong>不太常用的 vanilla item</strong> 作 override
 * 宿主，避免玩家捡到的正常 vanilla 物品也被 override。挑选偏好：
 * <ul>
 *   <li>相同 weapon_kind 的 vanilla sword → iron_sword / netherite_sword 等</li>
 *   <li>非武器类 vanilla item 也可以借用作容器（flint / nether_star / totem 等）</li>
 * </ul>
 *
 * <p><strong>新增武器的流程</strong>：
 * <ol>
 *   <li>跑 {@code client/tools/tripo_to_sml.py --override minecraft:&lt;vanilla_item&gt;}
 *       产出 OBJ + mtl + texture + vanilla item model JSON</li>
 *   <li>把 {@code item/&lt;vanilla_item&gt;} 加到 {@link WeaponRenderBootstrap#VANILLA_ITEM_OVERRIDES}</li>
 *   <li>把 {@code template_id → Items.&lt;vanilla&gt;} 加到本文件的 {@link #MAP}</li>
 * </ol>
 */
public final class WeaponVanillaIconMap {
    private WeaponVanillaIconMap() {}

    /**
     * template_id → vanilla {@link Item}（用来合成 fake ItemStack）。
     *
     * <p>未列入表的 template_id 会让 {@link com.bong.client.mixin.MixinHeldItemRenderer}
     * 放弃 override，玩家手上什么都不画（明确的视觉信号：mesh 没接入）。
     */
    private static final Map<String, Item> MAP = Map.of(
            // plan §10 tier 0
            "iron_sword", Items.IRON_SWORD,            // 唐刀 mesh (placeholder_sword)
            "bronze_saber", Items.GOLDEN_SWORD,        // 青铜刀 tier 0 (cracked_sword 铜色 mesh)
            "bone_dagger", Items.BONE,                 // 骨刀 tier 0 (dagger mesh)
            "hand_wrap", Items.LEATHER,                // 拳套 tier 0 (armored_gauntlets mesh)
            // plan §10 tier 2
            "flying_sword_feixuan", Items.DIAMOND_SWORD // 飞玄剑 tier 2 (medieval_dagger mesh)
            // 待补:
            // "wooden_staff", Items.END_ROD
            // "spirit_sword", Items.NETHERITE_SWORD  (需要真灵器蓝光 mesh)
    );

    /** 查 template_id 对应的 fake ItemStack；没映射返回 null。
     *
     * 缓存单例：多个 Mixin 调用期间保持同一 ItemStack 实例，便于渲染层的 identity/areEqual 缓存。
     */
    private static final java.util.concurrent.ConcurrentHashMap<String, ItemStack> STACK_CACHE =
            new java.util.concurrent.ConcurrentHashMap<>();

    public static ItemStack createStackFor(String templateId) {
        Item item = MAP.get(templateId);
        if (item == null) return null;
        return STACK_CACHE.computeIfAbsent(templateId, k -> new ItemStack(item));
    }
}
