package com.bong.client.visual;

import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Locale;

/**
 * plan-combat-skill-feedback-bridges-v1 P3 fix — per-entity 虚蚀模型半透明渲染。
 *
 * <p>每帧读取 {@link VoidErosionVisualStore} 中与当前被渲染实体匹配的 {@code modelAlpha}，
 * 并以半透明方式叠加一层玩家模型，实现虚蚀阶段 4 = 0.4 透明度渐变。
 *
 * <p><b>多人串扰修复（per-entity 过滤）：</b> 旧版直接读全局单槽快照，导致一个玩家的
 * erosion alpha 套到所有玩家模型。本版通过 {@code entity.getEntityName()} 转为
 * wire entityId 格式（"offline:{username}" 或直接 username 小写）后查 per-entity map，
 * 只在匹配当前被渲染实体时应用 alpha，其他实体不受影响。
 *
 * <p><b>注意（fix5 透明方式说明）：</b> 本 renderer 使用 {@code FeatureRenderer} 叠加
 * translucent ghost 层，而非修改基础模型本身的 alpha。这意味着原始皮肤层保持不透明，
 * ghost 层叠在上面以 alpha 值渲染。真正的基础模型透明需要 Mixin LivingEntityRenderer，
 * 属于重构工程，延后至后续 plan。当前方案在反馈桥语义下（玩家能看到随 erosion 变化的
 * 可见半透明效果）已满足 P3 目标。
 *
 * <p>接入方式：{@link VoidErosionRenderBootstrap} 通过
 * {@code LivingEntityFeatureRendererRegistrationCallback} 在所有
 * {@link net.minecraft.client.render.entity.PlayerEntityRenderer} 上注册本 renderer，
 * 保证每帧调用。
 *
 * <p>当 {@code modelAlpha >= 1.0f}（无虚蚀或阶段 0）时跳过渲染，性能零开销。
 */
public final class VoidErosionModelAlphaRenderer
        extends FeatureRenderer<AbstractClientPlayerEntity, PlayerEntityModel<AbstractClientPlayerEntity>> {

    private static final Logger LOGGER = LoggerFactory.getLogger("bong/void_erosion/alpha_renderer");

    /** 低于此值才启用半透明渲染（1.0 = 完全不透明，不需要渲染）。 */
    private static final float ALPHA_THRESHOLD = 0.999f;

    public VoidErosionModelAlphaRenderer(
            FeatureRendererContext<AbstractClientPlayerEntity,
                    PlayerEntityModel<AbstractClientPlayerEntity>> context) {
        super(context);
    }

    /**
     * 每帧由 {@link net.minecraft.client.render.entity.PlayerEntityRenderer} 的
     * feature 渲染循环调用。查询 {@link VoidErosionVisualStore} 的 per-entity map，
     * 匹配当前被渲染实体；若 modelAlpha < 1.0 则绘制半透明 ghost 层。
     */
    @Override
    public void render(
            MatrixStack matrices,
            VertexConsumerProvider vertexConsumers,
            int light,
            AbstractClientPlayerEntity entity,
            float limbAngle,
            float limbDistance,
            float tickDelta,
            float animationProgress,
            float headYaw,
            float headPitch
    ) {
        // 解析 entity → wire entityId：server 端 wire 格式为 "offline:{username}"。
        // 先直接按 "offline:{name}" 查，找不到再按小写 name 查（容错）。
        String entityName = entity.getEntityName();
        String wireId = resolveWireId(entityName);

        VoidErosionVisualStore.State state = VoidErosionVisualStore.snapshotForEntity(wireId);
        if (state == null) {
            // 当前实体无虚蚀 payload — 不渲染
            return;
        }
        if (state.modelAlpha() >= ALPHA_THRESHOLD) {
            // 阶段 0 / 完全不透明 — 跳过渲染，性能零开销
            return;
        }

        float alpha = Math.max(0.0f, Math.min(1.0f, state.modelAlpha()));
        LOGGER.trace("void_erosion_alpha: entity={} wireId={} stage={} alpha={}",
                entityName, wireId, state.stage(), alpha);

        // 使用玩家皮肤纹理 + EntityTranslucent 渲染半透明叠加层（ghost 层）
        // MC 1.20.1 yarn API: getSkinTexture() 直接返回 Identifier
        Identifier skinTexture = entity.getSkinTexture();
        RenderLayer renderLayer = RenderLayer.getEntityTranslucent(skinTexture);
        VertexConsumer buffer = vertexConsumers.getBuffer(renderLayer);

        // 轻微放大（1.002f）防止 z-fighting（与原始皮肤层重叠）
        matrices.push();
        matrices.scale(1.002f, 1.002f, 1.002f);
        // RGBA: r=1, g=1, b=1, a=alpha（lerp 到目标透明度）
        this.getContextModel().render(
                matrices, buffer, light, OverlayTexture.DEFAULT_UV,
                1.0f, 1.0f, 1.0f, alpha);
        matrices.pop();
    }

    /**
     * 将 MC entity 名转为 server wire entityId 格式。
     *
     * <p>Server 端 offline mode 的 wire 格式为 {@code "offline:{username}"}（原始 case）
     * 或直接 {@code "char:{bits}"}（char id）。本方法按以下策略匹配：
     * <ol>
     *   <li>先试 {@code "offline:{entityName}"} — 覆盖最常见的 offline mode 场景；</li>
     *   <li>若 map 中无此键，再试 {@code entityName.toLowerCase()} — 容错小写；</li>
     *   <li>仍无则返回 {@code "offline:{entityName}"}（caller 侧 snapshotForEntity 返回 null，跳过渲染）。</li>
     * </ol>
     *
     * @param entityName MC entity getEntityName() 返回值
     * @return server wire entityId（优先匹配 map 中已有的键）
     */
    static String resolveWireId(String entityName) {
        if (entityName == null || entityName.isEmpty()) {
            return "offline:";
        }
        // 优先 "offline:{name}" 精确匹配（最常见 offline mode 格式）
        String offlineKey = "offline:" + entityName;
        if (VoidErosionVisualStore.snapshotForEntity(offlineKey) != null) {
            return offlineKey;
        }
        // 容错：小写名
        String lowerKey = entityName.toLowerCase(Locale.ROOT);
        if (VoidErosionVisualStore.snapshotForEntity(lowerKey) != null) {
            return lowerKey;
        }
        // 容错：直接返回原始名（若 payload 恰好用 entityName 作键）
        if (VoidErosionVisualStore.snapshotForEntity(entityName) != null) {
            return entityName;
        }
        // 默认：offline:{name}（未收到 payload 时 snapshotForEntity 返回 null，跳过渲染）
        return offlineKey;
    }
}
