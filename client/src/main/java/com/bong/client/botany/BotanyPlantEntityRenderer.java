package com.bong.client.botany;

import net.minecraft.block.Block;
import net.minecraft.block.Blocks;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.render.entity.EntityRendererFactory;
import net.minecraft.client.texture.SpriteAtlasTexture;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.util.Identifier;
import net.minecraft.util.math.MathHelper;
import net.minecraft.util.math.RotationAxis;
import org.joml.Matrix3f;
import org.joml.Matrix4f;

import java.util.Locale;

public final class BotanyPlantEntityRenderer extends EntityRenderer<BotanyPlantV2Entity> {
    private static final Identifier BLOCK_ATLAS = SpriteAtlasTexture.BLOCK_ATLAS_TEXTURE;
    private static final float QUAD_HALF = 0.28f;
    private static final float QUAD_HEIGHT = 0.74f;

    public BotanyPlantEntityRenderer(EntityRendererFactory.Context context) {
        super(context);
        this.shadowRadius = 0.15f;
    }

    @Override
    public void render(
        BotanyPlantV2Entity entity,
        float yaw,
        float tickDelta,
        MatrixStack matrices,
        VertexConsumerProvider consumers,
        int light
    ) {
        BotanyPlantRenderProfile profile = BotanyPlantRenderProfileStore.get(entity.plantId())
            .orElse(BotanyPlantRenderProfile.fallback(entity.plantId()));
        BotanyPlantVisualState visual = BotanyPlantVisualState.forStage(
            entity.growthStage(),
            profile.tintAt(entity.getWorld().getTime()),
            entity.age,
            tickDelta
        );

        matrices.push();
        matrices.multiply(RotationAxis.POSITIVE_Y.rotationDegrees(180.0f - dispatcher.camera.getYaw()));
        matrices.translate(0.0, 0.02, 0.0);
        matrices.scale(visual.scale(), visual.scale(), visual.scale());
        if (visual.swayRadians() != 0.0f) {
            matrices.multiply(RotationAxis.POSITIVE_Z.rotation(visual.swayRadians()));
        }
        drawPlantQuad(
            consumers,
            matrices,
            textureFor(profile.baseMeshRef()),
            visual.tintRgb(),
            light,
            visual.alpha()
        );
        if (
            entity.growthStage() != PlantGrowthStage.WILTED
                && profile.overlay() == BotanyPlantRenderProfile.ModelOverlay.EMISSIVE
        ) {
            drawPlantQuad(consumers, matrices, textureFor(profile.baseMeshRef()), visual.tintRgb(), 0x00F000F0, 96);
        }
        matrices.pop();

        super.render(entity, yaw, tickDelta, matrices, consumers, light);
    }

    @Override
    public Identifier getTexture(BotanyPlantV2Entity entity) {
        return BLOCK_ATLAS;
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

    static Identifier textureFor(String baseMeshRef) {
        Block block = blockFor(baseMeshRef);
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.getBlockRenderManager() != null) {
            Identifier spriteId = client
                .getBlockRenderManager()
                .getModel(block.getDefaultState())
                .getParticleSprite()
                .getContents()
                .getId();
            if (spriteId != null) {
                return spriteId;
            }
        }
        return new Identifier("minecraft", "textures/block/grass.png");
    }

    private static Block blockFor(String baseMeshRef) {
        String normalized = baseMeshRef == null ? "" : baseMeshRef.trim().toLowerCase(Locale.ROOT);
        return switch (normalized) {
            case "large_fern" -> Blocks.LARGE_FERN;
            case "dead_bush" -> Blocks.DEAD_BUSH;
            case "sweet_berry_bush" -> Blocks.SWEET_BERRY_BUSH;
            case "tall_grass" -> Blocks.TALL_GRASS;
            case "lily_of_the_valley" -> Blocks.LILY_OF_THE_VALLEY;
            case "vine" -> Blocks.VINE;
            case "red_mushroom" -> Blocks.RED_MUSHROOM;
            case "moss_carpet" -> Blocks.MOSS_CARPET;
            case "seagrass" -> Blocks.SEAGRASS;
            case "weeping_vines" -> Blocks.WEEPING_VINES;
            case "glow_lichen" -> Blocks.GLOW_LICHEN;
            case "brown_mushroom" -> Blocks.BROWN_MUSHROOM;
            case "twisting_vines" -> Blocks.TWISTING_VINES;
            case "wheat" -> Blocks.WHEAT;
            default -> Blocks.GRASS;
        };
    }
}
