package com.bong.client.mixin;

import com.bong.client.network.ClientRequestSender;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;
import net.minecraft.client.network.ClientPlayerInteractionManager;
import net.minecraft.util.ActionResult;
import net.minecraft.util.Hand;
import net.minecraft.util.hit.BlockHitResult;
import net.minecraft.util.math.BlockPos;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * 神识感知矿脉触发 mixin（plan-exploration-probe-return-v1 P0）。
 * <p>
 * 在 {@code interactBlock} HEAD 注入，当玩家主手右键方块时向服务端发送 {@code mineral_probe} C2S 请求。
 * server 侧 {@code MineralProbeDenialReason::NotMineralOre} 已兜底非矿块，client 不预判矿种。
 * <p>
 * 决议依据：§8.1 #2「矿脉 = use-on-block mixin，镜像 MixinClientPlayerInteractionManagerAlchemy 的
 * interactBlock 钩子，右键矿块且手持空手或神识焦点物时触发」。
 * 此处简化为：主手右键任意方块均发探针（由 server 拒绝非矿块），与 client 不预判矿种的原则一致。
 * <p>
 * 实现镜像 {@link MixinClientPlayerInteractionManagerAlchemy} 的 {@code interactBlock} 钩子签名，
 * 但本 mixin **不取消**交互（不调用 {@code cir.setReturnValue}），允许原生 MC 交互继续进行。
 */
@Mixin(ClientPlayerInteractionManager.class)
public abstract class MixinClientPlayerInteractionManagerMineralProbe {

    @Inject(method = "interactBlock", at = @At("HEAD"), cancellable = true)
    @SuppressWarnings({"unused", "PMD.UnusedPrivateMethod"})
    private void bong$mineralProbeInteractBlock(
        ClientPlayerEntity player,
        Hand hand,
        BlockHitResult hit,
        CallbackInfoReturnable<ActionResult> cir
    ) {
        if (hand != Hand.MAIN_HAND || player == null || hit == null) return;

        MinecraftClient mc = MinecraftClient.getInstance();
        if (mc.world == null) return;

        // 仅空手触发（无持有物品）——避免与其他 use-on-block 动作（炼丹炉放置等）冲突。
        // server 的 MineralProbeDenialReason::NotMineralOre 兜底非矿块，所以 client 这边
        // 不需要判断方块类型，只要是空手右键就发探针请求。
        if (player.getMainHandStack().isEmpty()) {
            BlockPos pos = hit.getBlockPos();
            ClientRequestSender.sendMineralProbe(pos.getX(), pos.getY(), pos.getZ());
            // 不取消（不调 cir.setReturnValue），让 MC 继续处理原生交互。
        }
    }
}
