package com.bong.client.mixin;

import com.bong.client.inventory.model.MorphEntry;
import com.bong.client.inventory.state.MorphStateStore;
import com.bong.client.morph.MorphModelRegistry;
import com.bong.client.morph.MorphRenderProxy;
import com.bong.client.whale.WhaleEntity;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.Optional;

/**
 * plan-race-system-v1 PR-5b — 易形（形态变换）玩家渲染 Mixin。
 *
 * <p>{@link MorphStateStore} 命中当前实体（且 {@link MorphModelRegistry} 有对应模型）时，
 * 取消原本的 vanilla 玩家渲染，改为借用 {@link com.bong.client.whale.WhaleRenderer}
 * （复用现有 {@code bong:whale} EntityType 的 GeckoLib 模型/贴图/动画）画一个位置/朝向
 * 完全对齐的「代理」{@link WhaleEntity}（{@link MorphRenderProxy}）。
 *
 * <p>拦截点选在 {@link EntityRenderDispatcher#render} 而非
 * {@code PlayerEntityRenderer.render}——后者因 {@code LivingEntityRenderer}/
 * {@code EntityRenderer} 泛型继承链产生多个桥接方法签名，注入目标易产生歧义；
 * 前者是单一非重载方法，且已经完成 x/y/z 插值定位，代理渲染直接复用同一份
 * x/y/z/yaw/tickDelta/matrices/vertexConsumers/light 递归调用
 * {@code dispatcher.render(proxy, ...)} 即可获得与原 vanilla 渲染完全一致的定位/
 * 阴影/朝向，不需要自己手搓 {@code matrices.translate}。
 *
 * <p>**Scope（决议）**：仅 TPV（第三人称）。本地玩家在第一人称视角时 vanilla 本就
 * 不会走 {@code EntityRenderDispatcher.render}（{@code WorldRenderer} 对
 * focused entity 在非-third-person 时跳过），故本 mixin 天然不影响 FPV——不需要
 * 额外判断。第一人称"兽形手臂"是明确排除的范围（留给后续批次）。
 *
 * <p>队友标识/名牌不受影响——本 mixin 只拦截模型/贴图渲染这一层，不触碰
 * {@code EntityRenderer#renderLabelIfPresent}（见 {@code MixinEntityRenderer}），
 * 递归调用 dispatcher.render 时 vanilla 名牌渲染逻辑仍会对 proxy 走一遍
 * （{@link WhaleEntity} 无自定义名牌，视觉上不产生额外标签；玩家名牌本身在
 * cancel 之前从未被跳过的名牌渲染路径覆盖）。
 */
@Mixin(EntityRenderDispatcher.class)
public abstract class MixinMorphedPlayerRenderer {

    @Inject(method = "render", at = @At("HEAD"), cancellable = true)
    private <E extends Entity> void bong$renderMorphedPlayerAsWhale(
        E entity,
        double x,
        double y,
        double z,
        float yaw,
        float tickDelta,
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        CallbackInfo ci
    ) {
        if (!(entity instanceof PlayerEntity player)) return;

        Optional<MorphEntry> morph = MorphStateStore.morphOf(player.getId());
        if (morph.isEmpty() || !MorphModelRegistry.hasModel(morph.get().formRaceId())) return;

        WhaleEntity proxy = MorphRenderProxy.whaleFor(player);
        ci.cancel();

        EntityRenderDispatcher self = (EntityRenderDispatcher) (Object) this;
        self.render(proxy, x, y, z, yaw, tickDelta, matrices, vertexConsumers, light);
    }
}
