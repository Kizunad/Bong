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
 *
 * <p><b>plan-race-system-v1 P0 review r3 (major x3 收口)</b> -- slot positioning now
 * reads {@link MutationSlotLayoutRegistry} (backed by the shared
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} contract, pinned
 * against the server's {@code humanoid.json mutation_slot_mapping}) instead of the
 * previous private hardcoded table ({@code MutationKind.defaultBodySlot}, which was
 * unused dead weight) and the ordinal-based z-fight scale hack. An unknown mutation
 * kind or a {@code body_slot} with no layout mapping is an explicit "do not render"
 * decision ({@link #resolveRenderable}), logged at debug level -- never a silent
 * guessed fallback position.
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

        MutationSlotLayout layout = MutationSlotLayoutRegistry.get();
        for (MutationVisualState.MutationSlotEntry slot : slots) {
            RenderableSlot renderable = resolveRenderable(slot, layout);
            if (renderable == null) {
                continue;
            }
            renderMutationOverlay(
                matrices, vertexConsumers, light,
                renderable.kind().textureId(), renderable, slot.level()
            );
        }
    }

    /**
     * Pure decision logic (no GL calls) -- resolves which mutation kind + layout
     * anchor a wire slot entry should render with, or {@code null} if it should be
     * explicitly skipped. Package-private + static so it is unit-testable without a
     * real MC render context (see {@code MutationFeatureRendererTest}).
     */
    static RenderableSlot resolveRenderable(MutationVisualState.MutationSlotEntry slot, MutationSlotLayout layout) {
        MutationKind kind = MutationKind.fromServerName(slot.kind());
        if (kind == null) {
            LOGGER.debug("Unknown mutation kind '{}', skipping render", slot.kind());
            return null;
        }
        MutationSlotLayout.SlotEntry layoutEntry = layout.forBodySlot(slot.bodySlot());
        if (layoutEntry == null) {
            LOGGER.debug(
                "No mutation slot layout mapping for body_slot '{}' (kind={}), skipping render "
                    + "-- explicit fallback: unknown/unmapped slots render nothing rather than "
                    + "guessing a position",
                slot.bodySlot(), slot.kind()
            );
            return null;
        }
        return new RenderableSlot(kind, layoutEntry);
    }

    private void renderMutationOverlay(
        MatrixStack matrices,
        VertexConsumerProvider vertexConsumers,
        int light,
        Identifier textureId,
        RenderableSlot renderable,
        int level
    ) {
        // Render the mutation texture as a translucent overlay on the player model.
        // The alpha is scaled by mutation level (higher level = more opaque).
        float alpha = 0.5f + 0.15f * Math.min(level, 3);
        RenderLayer renderLayer = RenderLayer.getEntityTranslucent(textureId);
        VertexConsumer buffer = vertexConsumers.getBuffer(renderLayer);

        MutationSlotLayout.Anchor anchor = renderable.layoutEntry().anchor();
        matrices.push();
        matrices.translate(anchor.offsetX(), anchor.offsetY(), anchor.offsetZ());
        // Base scale comes from the shared per-part anchor contract; a tiny
        // per-mutation-kind epsilon is layered on top (not the sole source of
        // offset any more) so two different mutation kinds that happen to share
        // the same BodySlot (e.g. ToughSkin + BodyEnlarge both attach to Torso)
        // still avoid z-fighting when both are simultaneously active.
        float scale = anchor.scale() + 0.001f * renderable.kind().ordinal();
        matrices.scale(scale, scale, scale);
        this.getContextModel().render(matrices, buffer, light, OverlayTexture.DEFAULT_UV, 1.0f, 1.0f, 1.0f, alpha);
        matrices.pop();
    }

    /** A wire slot entry resolved to a renderable (kind, layout anchor) pair. */
    record RenderableSlot(MutationKind kind, MutationSlotLayout.SlotEntry layoutEntry) {}
}
