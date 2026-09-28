package com.bong.client.entity;

import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.LightmapTextureManager;
import net.minecraft.client.render.model.ModelLoader;
import net.minecraft.client.texture.SpriteAtlasTexture;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.math.RotationAxis;

public final class AlchemyFurnaceRenderer extends BongModeledEntityRenderer {
    public AlchemyFurnaceRenderer(EntityRendererFactory.Context context) {
        super(context, BongEntityModelKind.ALCHEMY_FURNACE);
    }

    @Override
    public void render(BongModeledEntity entity, float yaw, float delta, MatrixStack matrices,
                       VertexConsumerProvider consumers, int light) {
        matrices.push();
        float heat = entity.alchemyHeat();
        double phase = entity.age + delta;
        // 极小震动随火候增强；炉脚仍围绕地面支点，避免丹炉整体后仰。
        matrices.multiply(RotationAxis.POSITIVE_Z.rotationDegrees((float) (Math.sin(phase * 2.7) * heat * .3)));
        super.render(entity, yaw, delta, matrices, consumers, light);
        matrices.pop();
        // 工位已有二维火焰前景，只在真正挂入世界的实体上画三维炉底火焰。
        if (heat <= .01f || entity.getWorld().getEntityById(entity.getId()) != entity) return;
        var sprite = ModelLoader.FIRE_0.getSprite();
        var vertices = consumers.getBuffer(RenderLayer.getEntityCutoutNoCull(SpriteAtlasTexture.BLOCK_ATLAS_TEXTURE));
        for (int plane = 0; plane < 3; plane++) {
            matrices.push();
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(plane * 60));
            var matrix = matrices.peek().getPositionMatrix();
            var normal = matrices.peek().getNormalMatrix();
            float top = .24f + heat * (.48f + (float) Math.sin(phase * .6 + plane) * .04f);
            float width = .19f + heat * .18f;
            int glow = LightmapTextureManager.MAX_LIGHT_COORDINATE;
            vertices.vertex(matrix, -width, .16f, 0).color(255, 255, 255, 255).texture(sprite.getMinU(), sprite.getMaxV())
                .overlay(OverlayTexture.DEFAULT_UV).light(glow).normal(normal, 0, 0, 1).next();
            vertices.vertex(matrix, width, .16f, 0).color(255, 255, 255, 255).texture(sprite.getMaxU(), sprite.getMaxV())
                .overlay(OverlayTexture.DEFAULT_UV).light(glow).normal(normal, 0, 0, 1).next();
            vertices.vertex(matrix, width, top, 0).color(255, 255, 255, 255).texture(sprite.getMaxU(), sprite.getMinV())
                .overlay(OverlayTexture.DEFAULT_UV).light(glow).normal(normal, 0, 0, 1).next();
            vertices.vertex(matrix, -width, top, 0).color(255, 255, 255, 255).texture(sprite.getMinU(), sprite.getMinV())
                .overlay(OverlayTexture.DEFAULT_UV).light(glow).normal(normal, 0, 0, 1).next();
            matrices.pop();
        }
    }
}
