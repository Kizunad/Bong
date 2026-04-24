package com.bong.client.mixin;

import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.weapon.WeaponVanillaIconMap;

import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.world.World;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * plan-weapon-v1 §5.1：最终版 hand-stack override。
 *
 * <p>Mixin {@link LivingEntity#getMainHandStack()} 与 {@link LivingEntity#getOffHandStack()}
 * —— 这俩是 renderer/feature 最常调的入口 ({@link net.minecraft.client.render.item.HeldItemRenderer#updateHeldItems}
 * 直接调 {@code player.getMainHandStack()}; TP 的 {@code HeldItemFeatureRenderer}
 * 也调这俩).  前面试过 {@link PlayerEntity#getEquippedStack} 的 @RETURN 抢替但不知为
 * 何对 ClientPlayerEntity 的渲染路径不生效（可能是 ItemStack identity 缓存或
 * virtual dispatch 的 dev-env 问题）,换到最上层 getter 强制替换.
 *
 * <p>只在 client 线程 + 玩家 + stack 为 EMPTY 时介入。副作用受限:
 * <ul>
 *   <li>vanilla attack input 会认为 "拿着 iron_sword",但 Bong 的攻击走 combat/weapon 组件,
 *       不读 vanilla stack;</li>
 *   <li>HUD tooltip 等视觉都会显示 iron_sword,这正是我们要的；</li>
 *   <li>Server 端 LivingEntity 不走本客户端 Mixin,不影响战斗数值。</li>
 * </ul>
 */
@Mixin(LivingEntity.class)
public abstract class MixinPlayerEntityHeldItem {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-mixin-livingstack");
    private static int mainHandOverrideCount = 0;
    private static int offHandOverrideCount = 0;

    @Inject(method = "getMainHandStack", at = @At("RETURN"), cancellable = true)
    private void bong$overrideMainHand(CallbackInfoReturnable<ItemStack> cir) {
        overrideMainHandIfEmpty(cir);
    }

    @Inject(method = "getOffHandStack", at = @At("RETURN"), cancellable = true)
    private void bong$overrideOffHand(CallbackInfoReturnable<ItemStack> cir) {
        overrideIfEmpty(cir, "off_hand", false);
    }

    private void overrideIfEmpty(CallbackInfoReturnable<ItemStack> cir, String slot, boolean isMain) {
        ItemStack real = cir.getReturnValue();
        if (real != null && !real.isEmpty()) return;

        LivingEntity self = (LivingEntity) (Object) this;
        if (!(self instanceof PlayerEntity)) return;
        World world = self.getWorld();
        if (world == null || !world.isClient) return;
        if (MinecraftClient.getInstance().player != self) return;

        EquippedWeapon bong = WeaponEquippedStore.get(slot);
        if (bong == null) return;

        ItemStack fake = WeaponVanillaIconMap.createStackFor(bong.templateId());
        if (fake == null) return;

        cir.setReturnValue(fake);
        if (isMain && mainHandOverrideCount++ < 3) {
            LOGGER.info("getMainHandStack #{} override → {} (template={})",
                    mainHandOverrideCount, fake.getItem(), bong.templateId());
        } else if (!isMain && offHandOverrideCount++ < 3) {
            LOGGER.info("getOffHandStack #{} override → {} (template={})",
                    offHandOverrideCount, fake.getItem(), bong.templateId());
        }
    }

    private void overrideMainHandIfEmpty(CallbackInfoReturnable<ItemStack> cir) {
        ItemStack real = cir.getReturnValue();
        if (real != null && !real.isEmpty()) return;

        LivingEntity self = (LivingEntity) (Object) this;
        if (!(self instanceof PlayerEntity)) return;
        World world = self.getWorld();
        if (world == null || !world.isClient) return;
        if (MinecraftClient.getInstance().player != self) return;

        EquippedWeapon bong = WeaponEquippedStore.mainHandRenderWeapon();
        if (bong == null) return;

        ItemStack fake = WeaponVanillaIconMap.createStackFor(bong.templateId());
        if (fake == null) return;

        cir.setReturnValue(fake);
        if (mainHandOverrideCount++ < 3) {
            LOGGER.info("getMainHandStack #{} override → {} (template={})",
                mainHandOverrideCount, fake.getItem(), bong.templateId());
        }
    }
}
