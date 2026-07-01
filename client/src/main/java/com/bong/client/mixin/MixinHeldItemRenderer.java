package com.bong.client.mixin;

import com.bong.client.weapon.HeldItemStackResolver;

import net.minecraft.client.MinecraftClient;
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
 * plan-weapon-v1 §5.1：把 Bong 武器 (ItemInstance / {@link com.bong.client.combat.EquippedWeapon})
 * 注入 vanilla 持握渲染管线。
 *
 * <p>链路：server 推 {@code WeaponEquippedV1} → {@code WeaponEquippedStore}。玩家真正
 * 的 vanilla {@code PlayerEntity.getMainHandStack()} 是 EMPTY（Bong 不同步到 vanilla
 * inventory）。vanilla {@link HeldItemRenderer} 每 tick 从 player 拉 stack 缓存到
 * {@code mainHand} / {@code offHand} 字段；FPV 渲染直接读这俩字段,如果是 EMPTY 就画
 * 空手动画,不走通用 {@code renderItem} overload。
 *
 * <p>所以 target 选 {@link HeldItemRenderer#updateHeldItems()}：每 tick TAIL 后,如果
 * 字段为空则用 {@link HeldItemStackResolver} 算出的 fallback fake stack 覆盖。后续
 * vanilla 渲染读到的就是非空 stack,走正常 item 渲染路径 → SML 劫持（见
 * {@link com.bong.client.weapon.WeaponRenderBootstrap}）→ Bong OBJ 模型。
 *
 * <p>F8：weapon → shield(仅 off_hand) → block → hoe 的 fallback 优先级链此前在本类与
 * {@link MixinPlayerEntityHeldItem}（TPV）各自重复实现了一份，历史上因此漏同步过两次
 * （盾 off_hand 只接了这里、锄头 TPV 缺失）。现统一委托给 {@link HeldItemStackResolver}
 * （非-mixin 包，两处调用同一份实现，语义不变）。
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
        if (MinecraftClient.getInstance().player == null) return;

        if (this.mainHand.isEmpty()) {
            HeldItemStackResolver.resolveMainHand().ifPresent(fake -> {
                this.mainHand = fake;
                if (!loggedFirstInject) {
                    LOGGER.info("注入 fake stack for main_hand/two_hand → {}", fake.getItem());
                    loggedFirstInject = true;
                }
            });
        }

        if (this.offHand.isEmpty()) {
            HeldItemStackResolver.resolveOffHand().ifPresent(fake -> this.offHand = fake);
        }
    }
}
