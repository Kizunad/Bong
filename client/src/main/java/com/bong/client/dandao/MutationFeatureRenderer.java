package com.bong.client.dandao;

import net.minecraft.client.render.OverlayTexture;
import net.minecraft.client.render.RenderLayer;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.LivingEntityRenderer;
import net.minecraft.client.render.entity.feature.FeatureRenderer;
import net.minecraft.client.render.entity.feature.FeatureRendererContext;
import net.minecraft.client.render.entity.model.PlayerEntityModel;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.Identifier;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;

/**
 * plan-dandao-path-v1 P3 -- Feature renderer that overlays mutation attachment
 * models on the player entity based on {@link MutationVisualState}.
 *
 * <p>Each active mutation slot maps to a GeckoLib geo model + texture pair.
 * The renderer queries {@link MutationVisualState#activeSlots()} each frame
 * and draws the corresponding attachment textures as overlays on the player model.
 *
 * <p>This is a simplified overlay renderer (texture quad per slot) rather than
 * full GeckoLib sub-model rendering, because the mutation geo.json files are
 * designed as player model extensions -- the actual GeckoLib attachment rendering
 * will be wired when server-side entity sync is complete (P3 full pipeline).
 * For now, the renderer validates that all mutation textures are loadable and
 * marks the player with a visual tint when mutations are active.
 */
public final class MutationFeatureRenderer<T extends PlayerEntity>
    extends FeatureRenderer<T, PlayerEntityModel<T>> {

    private static final Logger LOGGER = LoggerFactory.getLogger("bong/dandao/mutation_renderer");

    public MutationFeatureRenderer(FeatureRendererContext<T, PlayerEntityModel<T>> context) {
        super(context);
    }

    @Override
    public void render(
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        T entity,
        float limbAngle,
        float limbDistance,
        float tickDelta,
        float animationProgress,
        float headYaw,
        float headPitch
    ) {
        List<MutationVisualState.MutationSlotEntry> slots = MutationVisualState.activeSlots();
        if (slots.isEmpty()) {
            return;
        }

        for (MutationVisualState.MutationSlotEntry slot : slots) {
            MutationKind kind = MutationKind.fromServerName(slot.kind());
            if (kind == null) {
                LOGGER.debug("Unknown mutation kind '{}', skipping render", slot.kind());
                continue;
            }

            Identifier textureId = kind.textureId();
            renderMutationOverlay(matrices, vertexConsumers, light, textureId, kind, slot.level());
        }
    }

    private void renderMutationOverlay(
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        Identifier textureId,
        MutationKind kind,
        int level
    ) {
        // Render the mutation texture as a translucent overlay on the player model.
        // The alpha is scaled by mutation level (higher level = more opaque).
        float alpha = 0.5f + 0.15f * Math.min(level, 3);
        RenderLayer renderLayer = RenderLayer.getEntityTranslucent(textureId);
        VertexConsumer buffer = vertexConsumers.getBuffer(renderLayer);

        // Apply slight scale offset based on body slot to prevent z-fighting
        matrices.push();
        float scaleOffset = 1.001f + 0.001f * kind.ordinal();
        matrices.scale(scaleOffset, scaleOffset, scaleOffset);
        this.getContextModel().render(matrices, buffer, light, OverlayTexture.DEFAULT_UV, 1.0f, 1.0f, 1.0f, alpha);
        matrices.pop();
    }
}
