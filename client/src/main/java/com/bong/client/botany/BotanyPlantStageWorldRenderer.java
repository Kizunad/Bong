package com.bong.client.botany;

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
import net.minecraft.util.math.MathHelper;
import net.minecraft.util.math.RotationAxis;
import net.minecraft.util.math.Vec3d;
import org.joml.Matrix3f;
import org.joml.Matrix4f;

public final class BotanyPlantStageWorldRenderer {
    private static final double RENDER_DISTANCE_M = 48.0;
    private static final float QUAD_HALF = 0.28f;
    private static final float QUAD_HEIGHT = 0.74f;
    private static final Identifier FALLBACK_STAGE_TEXTURE =
        new Identifier("bong-client", "textures/gui/botany/stages/ning_mai_cao_growing.png");

    private BotanyPlantStageWorldRenderer() {
    }

    public static void register() {
        WorldRenderEvents.AFTER_ENTITIES.register(BotanyPlantStageWorldRenderer::render);
    }

    private static void render(WorldRenderContext context) {
        MinecraftClient client = MinecraftClient.getInstance();
        ClientWorld world = client.world;
        VertexConsumerProvider consumers = context.consumers();
        MatrixStack matrices = context.matrixStack();
        if (world == null || consumers == null || matrices == null) {
            BotanyPlantStageVisualStore.clear();
            return;
        }

        long worldTime = world.getTime();
        BotanyPlantStageVisualStore.clearExpired(worldTime);
        var visuals = BotanyPlantStageVisualStore.snapshot();
        if (visuals.isEmpty()) {
            return;
        }

        Vec3d camPos = context.camera().getPos();
        float cameraYaw = context.camera().getYaw();
        float tickDelta = context.tickDelta();
        double cullSq = RENDER_DISTANCE_M * RENDER_DISTANCE_M;

        for (BotanyPlantStageVisual entry : visuals) {
            double dx = entry.x() - camPos.x;
            double dy = entry.y() - camPos.y;
            double dz = entry.z() - camPos.z;
            if (dx * dx + dy * dy + dz * dz > cullSq) {
                continue;
            }

            BotanyPlantRenderProfile profile = BotanyPlantRenderProfileStore.get(entry.plantId())
                .orElse(BotanyPlantRenderProfile.fallback(entry.plantId()));
            BotanyPlantVisualState visual = BotanyPlantVisualState.forStage(
                entry.stage(),
                entry.tintRgb(),
                (int) worldTime,
                tickDelta
            );
            Identifier texture = textureFor(client, entry, profile);
            BlockPos lightPos = BlockPos.ofFloored(entry.x(), entry.y() + 0.5, entry.z());
            int light = WorldRenderer.getLightmapCoordinates(world, lightPos);

            matrices.push();
            matrices.translate(dx, dy + 0.02, dz);
            matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(180.0f - cameraYaw));
            matrices.scale(visual.scale(), visual.scale(), visual.scale());
            if (visual.swayRadians() != 0.0f) {
                matrices.multiply(RotationAxis.POSITIVE_Z.rotation(visual.swayRadians()));
            }
            drawPlantQuad(consumers, matrices, texture, visual.tintRgb(), light, visual.alpha());
            matrices.pop();
        }
    }

    private static Identifier textureFor(
        MinecraftClient client,
        BotanyPlantStageVisual entry,
        BotanyPlantRenderProfile profile
    ) {
        if (entry.stage() == PlantGrowthStage.SEEDLING || entry.stage() == PlantGrowthStage.GROWING) {
            Identifier stageTexture = new Identifier(
                "bong-client",
                "textures/gui/botany/stages/" + entry.plantId() + "_" + entry.stage().wireName() + ".png"
            );
            if (client.getResourceManager().getResource(stageTexture).isPresent()) {
                return stageTexture;
            }
            return FALLBACK_STAGE_TEXTURE;
        }
        return BotanyPlantEntityRenderer.textureFor(profile.baseMeshRef());
    }

    private static void drawPlantQuad(
        VertexConsumerProvider consumers,
        MatrixStack matrices,
        Identifier texture,
        int tint,
        int light,
        int alpha
    ) {
        VertexConsumer consumer = consumers.getBuffer(RenderLayer.getEntityCutoutNoCull(texture));
        Matrix4f pos = matrices.peek().getPositionMatrix();
        Matrix3f norm = matrices.peek().getNormalMatrix();
        int r = (tint >> 16) & 0xFF;
        int g = (tint >> 8) & 0xFF;
        int b = tint & 0xFF;

        emit(consumer, pos, norm, -QUAD_HALF, 0.0f, 0.0f, 1.0f, r, g, b, alpha, light);
        emit(consumer, pos, norm, QUAD_HALF, 0.0f, 1.0f, 1.0f, r, g, b, alpha, light);
        emit(consumer, pos, norm, QUAD_HALF, QUAD_HEIGHT, 1.0f, 0.0f, r, g, b, alpha, light);
        emit(consumer, pos, norm, -QUAD_HALF, QUAD_HEIGHT, 0.0f, 0.0f, r, g, b, alpha, light);
    }

    private static void emit(
        VertexConsumer consumer,
        Matrix4f pos,
        Matrix3f norm,
        float x,
        float y,
        float u,
        float v,
        int r,
        int g,
        int b,
        int a,
        int light
    ) {
        consumer.vertex(pos, x, y, 0.0f)
            .color(r, g, b, MathHelper.clamp(a, 0, 255))
            .texture(u, v)
            .overlay(OverlayTexture.DEFAULT_UV)
            .light(light)
            .normal(norm, 0.0f, 0.0f, 1.0f)
            .next();
    }
}
