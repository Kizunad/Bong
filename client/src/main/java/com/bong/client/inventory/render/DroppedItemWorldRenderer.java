package com.bong.client.inventory.render;

import com.bong.client.inventory.component.GridSlotComponent;
import com.bong.client.inventory.AncientRelicGlowRenderer;
import com.bong.client.inventory.state.DroppedItemStore;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix3f;
import org.joml.Matrix4f;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * 地面 dropped loot 的世界空间 billboard 渲染。
 *
 * <p>"甲" 风格（参见 plan-inventory-v1.md §0.1）：
 * <ul>
 *   <li>yaw-only semi-billboard：quad 正面永远朝向相机 yaw，pitch 锁竖直（玩家抬头低头 quad 不歪）</li>
 *   <li>悬浮 + sine 上下浮动</li>
 *   <li>贴图复用 {@code textures/gui/items/{item_id}.png}，不加新资产</li>
 *   <li>走 lightmap 采样，夜晚会变暗（沉浸感）</li>
 *   <li>距离剔除省性能；远处/遮挡时 HUD marker 仍作方向指示</li>
 * </ul>
 * 纯 client-only：世界坐标来自 {@link DroppedItemStore}，不 spawn entity、不改 server。</p>
 */
public final class DroppedItemWorldRenderer {

    /** 剔除距离（m）。超过此距离的 entry 不渲染。 */
    private static final double RENDER_DISTANCE_M = 32.0;
    /** Quad 半宽/半高（世界单位=方块）。0.22 ≈ 总尺寸 0.44 m，约 MC 原版 item entity 视觉量级。 */
    private static final float QUAD_HALF = 0.22f;
    /** 悬浮基础高度（在 worldPosY 之上）。 */
    private static final float HOVER_HEIGHT = 0.45f;
    /** 上下浮动振幅（世界单位）。 */
    private static final float BOB_AMPLITUDE = 0.06f;
    /** 上下浮动周期（tick，20 tick = 1 s）。2 秒一圈。 */
    private static final float BOB_PERIOD_TICKS = 40.0f;

    /** itemId → texture Identifier 缓存，避免每帧 GC。 */
    private static final Map<String, Identifier> TEXTURE_CACHE = new ConcurrentHashMap<>();

    private DroppedItemWorldRenderer() {}

    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(DroppedItemWorldRenderer::render);
    }

    private static void render(WorldRenderContext context) {
        var entries = DroppedItemStore.snapshot();
        if (entries.isEmpty()) return;

        ClientWorld world = MinecraftClient.getInstance().world;
        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (world == null || consumers == null || matrices == null) return;

        Vec3d camPos = context.camera().getPos();
        float cameraYaw = context.camera().getYaw();
        float tickDelta = context.tickDelta();
        // 用世界 time 作相位源，保证所有 client 同相位（将来若接入多人观察时保持一致）。
        double phaseTicks = world.getTime() + tickDelta;
        float bob = (float) Math.sin(phaseTicks * (2.0 * Math.PI / BOB_PERIOD_TICKS)) * BOB_AMPLITUDE;

        double cullSq = RENDER_DISTANCE_M * RENDER_DISTANCE_M;

        for (var entry : entries) {
            if (entry == null || entry.item() == null) continue;

            // WorldRenderContext.matrixStack() 已应用相机偏移，translate 用 world-cam 差量。
            double dx = entry.worldPosX() - camPos.x;
            double dy = entry.worldPosY() - camPos.y;
            double dz = entry.worldPosZ() - camPos.z;
            if (dx * dx + dy * dy + dz * dz > cullSq) continue;

            BlockPos lightPos = BlockPos.ofFloored(
                entry.worldPosX(), entry.worldPosY() + 0.5, entry.worldPosZ()
            );
            int light = WorldRenderer.getLightmapCoordinates(world, lightPos);

            matrices.push();
            matrices.translate(dx, dy + HOVER_HEIGHT + bob, dz);
            // yaw-only semi-billboard: MC 中 camera yaw=0 时面朝 -Z，quad 默认正面 +Z；
            // 旋转 (180 - yaw) 让 quad 正面永远朝向相机 yaw 方向。
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(180.0f - cameraYaw));

            Identifier texture = textureFor(entry.item().itemId());
            Matrix4f pos = matrices.peek().getPositionMatrix();
            Matrix3f norm = matrices.peek().getNormalMatrix();

            if (AncientRelicGlowRenderer.shouldGlow(entry.item())) {
                VertexConsumer glow = consumers.getBuffer(RenderLayer.getEntityTranslucent(texture));
                int color = AncientRelicGlowRenderer.pulseColor((long) (phaseTicks * 50.0));
                int alpha = (color >>> 24) & 0xFF;
                int red = (color >>> 16) & 0xFF;
                int green = (color >>> 8) & 0xFF;
                int blue = color & 0xFF;
                emitVertex(glow, pos, norm, -0.31f, -0.31f, 0.0f, 1.0f, light, red, green, blue, alpha);
                emitVertex(glow, pos, norm,  0.31f, -0.31f, 1.0f, 1.0f, light, red, green, blue, alpha);
                emitVertex(glow, pos, norm,  0.31f,  0.31f, 1.0f, 0.0f, light, red, green, blue, alpha);
                emitVertex(glow, pos, norm, -0.31f,  0.31f, 0.0f, 0.0f, light, red, green, blue, alpha);
            }

            // Quad（CCW，正面法线 +Z）：bottom-left → bottom-right → top-right → top-left
            VertexConsumer consumer = consumers.getBuffer(RenderLayer.getEntityCutoutNoCull(texture));
            emitVertex(consumer, pos, norm, -QUAD_HALF, -QUAD_HALF, 0.0f, 1.0f, light);
            emitVertex(consumer, pos, norm,  QUAD_HALF, -QUAD_HALF, 1.0f, 1.0f, light);
            emitVertex(consumer, pos, norm,  QUAD_HALF,  QUAD_HALF, 1.0f, 0.0f, light);
            emitVertex(consumer, pos, norm, -QUAD_HALF,  QUAD_HALF, 0.0f, 0.0f, light);

            matrices.pop();
        }
    }

    private static void emitVertex(
        VertexConsumer consumer, Matrix4f pos, Matrix3f norm,
        float x, float y, float u, float v, int light
    ) {
        emitVertex(consumer, pos, norm, x, y, u, v, light, 255, 255, 255, 255);
    }

    private static void emitVertex(
        VertexConsumer consumer, Matrix4f pos, Matrix3f norm,
        float x, float y, float u, float v, int light,
        int red, int green, int blue, int alpha
    ) {
        consumer.vertex(pos, x, y, 0.0f)
            .color(red, green, blue, alpha)
            .texture(u, v)
            .overlay(OverlayTexture.DEFAULT_UV)
            .light(light)
            .normal(norm, 0.0f, 0.0f, 1.0f)
            .next();
    }

    private static Identifier textureFor(String itemId) {
        return TEXTURE_CACHE.computeIfAbsent(itemId,
            GridSlotComponent::textureIdForItemId);
    }
}
