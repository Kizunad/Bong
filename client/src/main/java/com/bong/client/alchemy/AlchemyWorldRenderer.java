package com.bong.client.alchemy;

import com.bong.client.render.ItemTextureResolver;
import net.fabricmc.fabric.api.client.rendering.v1.WorldRenderContext;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.WorldRenderer;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;

import java.util.List;

/** 投料先越过炉盖再没入炉腹；收取则升起、转一周并显示权威物品名称。 */
final class AlchemyWorldRenderer {
    private AlchemyWorldRenderer() {}

    static void render(WorldRenderContext context, ClientWorld world, List<AlchemyWorldEffects.ActionEffect> effects) {
        var client = MinecraftClient.getInstance();
        var matrices = context.matrixStack();
        var consumers = context.consumers();
        if (world == null || client.world != world || matrices == null || consumers == null) return;
        for (var effect : effects) {
            if (!effect.animated) continue;
            var payload = effect.payload;
            boolean feed = payload.action().equals("feed");
            if ((!feed && !payload.action().equals("collect")) || payload.item().isBlank()) continue;
            float age = effect.age(context.tickDelta());
            // 等炉盖先抬起；材料沉入炉腹后停止渲染，深度测试负责炉壁遮挡。
            if (age < 3 || (feed && age > 15)) continue;
            var center = Vec3d.ofBottomCenter(payload.position());
            var camera = context.camera().getPos();
            if (center.squaredDistanceTo(camera) > 48 * 48) continue;
            double progress = feed ? Math.min(1, (age - 3) / 12) : Math.min(1, (age - 3) / 24);
            double height = feed ? 1.85 - progress * progress * 1.25 : 1.05 + progress * .9;
            int light = WorldRenderer.getLightmapCoordinates(world, payload.position().up());
            matrices.push();
            matrices.translate(center.x - camera.x, center.y + height - camera.y, center.z - camera.z);
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees((float) (progress * 360)));
            if (effect.model != null) {
                var bounds = effect.model.bounds();
                double size = Math.max(.01, Math.max(bounds.maxX - bounds.minX,
                    Math.max(bounds.maxY - bounds.minY, bounds.maxZ - bounds.minZ)));
                float scale = (float) (.38 / size);
                matrices.scale(scale, effect.model.yUp() ? scale : -scale, scale);
                matrices.translate(-bounds.getCenter().x, -bounds.getCenter().y, -bounds.getCenter().z);
                effect.model.render(matrices, consumers);
            } else {
                var texture = ItemTextureResolver.forItemId(payload.item());
                if (texture != null) {
                    var vertices = consumers.getBuffer(RenderLayer.getEntityCutoutNoCull(texture));
                    var matrix = matrices.peek().getPositionMatrix();
                    var normal = matrices.peek().getNormalMatrix();
                    vertices.vertex(matrix, -.18f, -.18f, 0).color(255, 255, 255, 255).texture(0, 1)
                        .overlay(OverlayTexture.DEFAULT_UV).light(light).normal(normal, 0, 0, 1).next();
                    vertices.vertex(matrix, .18f, -.18f, 0).color(255, 255, 255, 255).texture(1, 1)
                        .overlay(OverlayTexture.DEFAULT_UV).light(light).normal(normal, 0, 0, 1).next();
                    vertices.vertex(matrix, .18f, .18f, 0).color(255, 255, 255, 255).texture(1, 0)
                        .overlay(OverlayTexture.DEFAULT_UV).light(light).normal(normal, 0, 0, 1).next();
                    vertices.vertex(matrix, -.18f, .18f, 0).color(255, 255, 255, 255).texture(0, 0)
                        .overlay(OverlayTexture.DEFAULT_UV).light(light).normal(normal, 0, 0, 1).next();
                }
            }
            matrices.pop();
            if (!feed && progress >= .9 && center.squaredDistanceTo(camera) < 12 * 12) {
                matrices.push();
                matrices.translate(center.x - camera.x, center.y + 2.3 - camera.y, center.z - camera.z);
                matrices.multiply(context.camera().getRotation());
                matrices.scale(-.018f, -.018f, .018f);
                client.textRenderer.draw(payload.name(), -client.textRenderer.getWidth(payload.name()) / 2f, 0,
                    0xFFF3D5, false, matrices.peek().getPositionMatrix(), consumers,
                    TextRenderer.TextLayerType.NORMAL, 0x55000000, light);
                matrices.pop();
            }
        }
    }
}
