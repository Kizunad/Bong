package com.bong.client.mixin;

import com.bong.client.social.SocialStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** plan-social-v1 Phase 0：远端玩家默认匿名，只有 server_data 暴露后才显示名牌。 */
@Mixin(EntityRenderer.class)
public abstract class MixinEntityRenderer<T extends Entity> {

    @Inject(method = "renderLabelIfPresent", at = @At("HEAD"), cancellable = true)
    private void bong$hideAnonymousPlayerLabel(
        T entity,
        Text text,
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        CallbackInfo ci
    ) {
        if (!(entity instanceof PlayerEntity player)) return;
        MinecraftClient client = MinecraftClient.getInstance();
        if (client.player == player) return;

        String playerName = player.getGameProfile() == null ? player.getName().getString() : player.getGameProfile().getName();
        if (!SocialStateStore.shouldShowRemoteNameTag(player.getUuidAsString(), playerName)) {
            ci.cancel();
        }
    }
}
