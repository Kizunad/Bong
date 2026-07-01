package com.bong.client.mixin;

import com.bong.client.weapon.HeldItemStackResolver;

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
 *
 * <p>F8：weapon → shield(仅 off_hand) → block → hoe 的 fallback 优先级链此前在本类与
 * {@link MixinHeldItemRenderer}（FPV）各自重复实现了一份（含逐字相同的私有
 * {@code bong$selectedBlockStack()}），现统一委托给非-mixin 包的
 * {@link HeldItemStackResolver}，两处调用同一份实现，语义不变。
 */
@Mixin(LivingEntity.class)
public abstract class MixinPlayerEntityHeldItem {
    private static final Logger LOGGER = LoggerFactory.getLogger("bong-mixin-livingstack");
    private static int mainHandOverrideCount = 0;
    private static int offHandOverrideCount = 0;

    @Inject(method = "getMainHandStack", at = @At("RETURN"), cancellable = true)
    private void bong$overrideMainHand(CallbackInfoReturnable<ItemStack> cir) {
        if (!eligibleForOverride(cir)) return;
        HeldItemStackResolver.resolveMainHand().ifPresent(fake -> {
            cir.setReturnValue(fake);
            if (mainHandOverrideCount++ < 3) {
                LOGGER.info("getMainHandStack #{} override → {}", mainHandOverrideCount, fake.getItem());
            }
        });
    }

    @Inject(method = "getOffHandStack", at = @At("RETURN"), cancellable = true)
    private void bong$overrideOffHand(CallbackInfoReturnable<ItemStack> cir) {
        if (!eligibleForOverride(cir)) return;
        HeldItemStackResolver.resolveOffHand().ifPresent(fake -> {
            cir.setReturnValue(fake);
            if (offHandOverrideCount++ < 3) {
                LOGGER.info("getOffHandStack #{} override → {}", offHandOverrideCount, fake.getItem());
            }
        });
    }

    /**
     * 共用前置校验：vanilla 返回值非空则不介入；只在客户端世界 + 本地玩家自身时生效
     * （不影响其他玩家的渲染，也不在服务端触发）。
     */
    private boolean eligibleForOverride(CallbackInfoReturnable<ItemStack> cir) {
        ItemStack real = cir.getReturnValue();
        if (real != null && !real.isEmpty()) return false;

        LivingEntity self = (LivingEntity) (Object) this;
        if (!(self instanceof PlayerEntity)) return false;
        World world = self.getWorld();
        if (world == null || !world.isClient) return false;
        return MinecraftClient.getInstance().player == self;
    }
}
