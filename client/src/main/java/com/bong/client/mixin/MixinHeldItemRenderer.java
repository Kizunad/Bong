package com.bong.client.mixin;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lingtian.HoeVanillaIconMap;
import com.bong.client.weapon.WeaponVanillaIconMap;

import net.minecraft.client.render.item.HeldItemRenderer;
import net.minecraft.item.ItemStack;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * plan-weapon-v1 §5.1：把 Bong 武器 (ItemInstance / {@link EquippedWeapon}) 注入
 * vanilla 持握渲染管线。
 *
 * <p>链路：server 推 {@code WeaponEquippedV1} → {@link WeaponEquippedStore}。玩家真正
 * 的 vanilla {@code PlayerEntity.getMainHandStack()} 是 EMPTY（Bong 不同步到 vanilla
 * inventory）。vanilla {@link HeldItemRenderer} 每 tick 从 player 拉 stack 缓存到
 * {@code mainHand} / {@code offHand} 字段；FPV 渲染直接读这俩字段,如果是 EMPTY 就画
 * 空手动画,不走通用 {@code renderItem} overload。
 *
 * <p>所以 target 选 {@link HeldItemRenderer#updateHeldItems()}：每 tick TAIL 后,如果
 * {@link WeaponEquippedStore} 有 Bong 武器而 vanilla 字段为空,直接改写 {@code mainHand}
 * 为 {@link WeaponVanillaIconMap} 合成的 fake {@code ItemStack}。后续 vanilla 渲染读到
 * 的就是非空 stack,走正常 item 渲染路径 → SML 劫持（见
 * {@link com.bong.client.weapon.WeaponRenderBootstrap}）→ Bong OBJ 模型。
 *
 * <p>副作用说明：attack / damage 等 gameplay 逻辑不读 {@code HeldItemRenderer} 字段,
 * 走 {@code player.getMainHandStack()},所以本 Mixin 只影响视觉,不干扰战斗数值。
 */
@Mixin(HeldItemRenderer.class)
public abstract class MixinHeldItemRenderer {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-mixin-helditem");
    private static boolean loggedFirstInject = false;

    @Shadow private ItemStack mainHand;
    @Shadow private ItemStack offHand;

    @Inject(method = "updateHeldItems", at = @At("TAIL"))
    private void bong$overrideHeldItemsForBongWeapons(CallbackInfo ci) {
        EquippedWeapon bongMain = WeaponEquippedStore.get("main_hand");
        if (bongMain != null && this.mainHand.isEmpty()) {
            ItemStack fake = WeaponVanillaIconMap.createStackFor(bongMain.templateId());
            if (fake != null) {
                this.mainHand = fake;
                if (!loggedFirstInject) {
                    LOGGER.info("注入 fake stack for main_hand template={} → {}",
                            bongMain.templateId(), fake.getItem());
                    loggedFirstInject = true;
                }
            }
        }

        EquippedWeapon bongOff = WeaponEquippedStore.get("off_hand");
        if (bongOff != null && this.offHand.isEmpty()) {
            ItemStack fake = WeaponVanillaIconMap.createStackFor(bongOff.templateId());
            if (fake != null) this.offHand = fake;
        }

        // plan-lingtian-v1 §1.2.1 — 无 Bong 武器 + 主手装备槽是 Bong 锄头时，合成 fake
        // vanilla HOE stack 让 HeldItemRenderer 画原生锄头 FP（三档材质区分铁/灵铁/玄铁）。
        if (bongMain == null && this.mainHand.isEmpty()) {
            InventoryItem main = InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND);
            if (main != null && !main.isEmpty() && HoeVanillaIconMap.isHoe(main.itemId())) {
                ItemStack fake = HoeVanillaIconMap.createStackFor(main.itemId());
                if (fake != null) this.mainHand = fake;
            }
        }
    }
}
