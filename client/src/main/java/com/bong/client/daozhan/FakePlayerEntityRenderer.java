package com.bong.client.daozhan;

import com.bong.client.fauna.FaunaEntity;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;

/**
 * plan-daozhan-v1 P1 — 道伥伪装玩家模型渲染辅助类（P3 扩展点，P1 为占位）。
 *
 * <p>P1 实现：道伥伪装通过 {@link com.bong.client.fauna.FaunaModel#getTextureResource}
 * 返回 Steve 皮肤纹理实现，不需要完整玩家模型渲染。
 *
 * <p>此类定义 P3 扩展接口：当 P3 实装完整 FakePlayerEntity 渲染时，
 * {@link FakePlayerRendererMixin} 将调用 {@link #renderDisguisedDaoZhan}
 * 以真实玩家比例渲染道伥。
 *
 * <p>P1 约束：
 * <ul>
 *   <li>禁止 vanilla MC armor stand / invisible player mob hack（见 CLAUDE.md 红线）。
 *   <li>皮肤 P1 使用默认 Steve（{@link #DEFAULT_SKIN_TEXTURE}），P3 替换为随机死亡修士皮肤。
 *   <li>计时用游戏 tick（{@code ClientTickEvents}），非渲染帧。
 * </ul>
 */
public final class FakePlayerEntityRenderer {

    /**
     * 默认皮肤纹理（Steve）——P1 占位，P3 替换为随机死亡修士皮肤。
     */
    public static final Identifier DEFAULT_SKIN_TEXTURE =
        new Identifier("minecraft", "textures/entity/player/wide/steve.png");

    private FakePlayerEntityRenderer() {
    }

    /**
     * P3 扩展点：渲染处于 Mimicry 伪装状态的道伥为无名玩家外形（真实玩家比例）。
     *
     * <p>P1 此方法为 no-op 占位；P1 伪装通过 {@link com.bong.client.fauna.FaunaModel#getTextureResource}
     * 的纹理替换实现（不调用此方法）。
     *
     * <p>P3 实现时：
     * <ol>
     *   <li>激活 {@link FakePlayerRendererMixin} 的 {@code @Mixin(EntityRenderDispatcher.class)} 注入。
     *   <li>在此方法内调用 {@code PlayerEntityRenderer.render(fakeState, matrices, vertexConsumers, light)}。
     *   <li>通过 server 下发的 UUID 拉取 Mineskin 皮肤替换 {@link #DEFAULT_SKIN_TEXTURE}。
     * </ol>
     *
     * @param entity        FaunaEntity（道伥本体）
     * @param x             渲染坐标偏移 X
     * @param y             渲染坐标偏移 Y
     * @param z             渲染坐标偏移 Z
     * @param yaw           偏航角（度）
     * @param tickDelta     帧间插值 delta
     * @param matrices      MatrixStack
     * @param vertexConsumers VertexConsumerProvider
     * @param light         光照值
     */
    public static void renderDisguisedDaoZhan(
        FaunaEntity entity,
        double x,
        double y,
        double z,
        float yaw,
        float tickDelta,
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light
    ) {
        // P1 no-op：伪装通过 FaunaModel.getTextureResource 的 Steve 皮肤纹理替换实现。
        // P3 在此处实装完整 PlayerEntityRenderer 渲染路径。
    }
}
