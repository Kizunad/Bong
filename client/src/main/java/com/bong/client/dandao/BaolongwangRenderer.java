package com.bong.client.dandao;

import net.minecraft.client.render.entity.EntityRendererFactory;
import software.bernie.geckolib.renderer.GeoEntityRenderer;

import java.util.Map;

/**
 * plan-dandao-path-v1 P4 -- GeckoLib renderer for Baolongwang BOSS.
 *
 * <p>Body size: 4x4x6 blocks. §8.1 #6 decision: bone alias HashMap
 * in the renderer (not modifying the source geo.json).
 *
 * <p>Alias mapping:
 * <pre>
 *   bdk_body -> body
 *   bdk_rl   -> right_leg
 *   bdk_ll   -> left_leg
 *   bdk_ra   -> right_arm
 *   bdk_la   -> left_arm
 *   bdk_rw   -> right_wing
 *   bdk_lw   -> left_wing
 *   bone / bone2 / group -> discarded (decorative, no animation binding)
 * </pre>
 */
public final class BaolongwangRenderer extends GeoEntityRenderer<BaolongwangEntity> {
    /** Render scale = 1.0 (geo.json dimensions already define 4x4x6 block body). */
    private static final float RENDER_SCALE = 1.0f;

    /**
     * Bone alias map -- §8.1 #6. Bedrock custom names -> semantic names.
     * GeckoLib reads bone names from the geo.json directly, so these aliases
     * are exposed for programmatic bone queries (e.g. animation targeting).
     * The actual geo.json bone names are preserved; aliases are only used
     * when this renderer needs to reference bones by semantic name.
     */
    static final Map<String, String> BONE_ALIASES = Map.of(
        "bdk_body", "body",
        "bdk_rl", "right_leg",
        "bdk_ll", "left_leg",
        "bdk_ra", "right_arm",
        "bdk_la", "left_arm",
        "bdk_rw", "right_wing",
        "bdk_lw", "left_wing"
    );

    /** Reverse map for querying by semantic name. */
    static final Map<String, String> REVERSE_ALIASES = Map.of(
        "body", "bdk_body",
        "right_leg", "bdk_rl",
        "left_leg", "bdk_ll",
        "right_arm", "bdk_ra",
        "left_arm", "bdk_la",
        "right_wing", "bdk_rw",
        "left_wing", "bdk_lw"
    );

    public BaolongwangRenderer(EntityRendererFactory.Context ctx) {
        super(ctx, new BaolongwangModel());
        this.withScale(RENDER_SCALE);
        this.shadowRadius = 2.0f;
    }

    /**
     * Resolves a Bedrock bone name to its semantic alias, or returns the
     * original name if no alias exists.
     */
    public static String resolveAlias(String bedrockBoneName) {
        return BONE_ALIASES.getOrDefault(bedrockBoneName, bedrockBoneName);
    }

    /**
     * Resolves a semantic bone name back to the Bedrock name used in the geo.json.
     */
    public static String resolveReverse(String semanticName) {
        return REVERSE_ALIASES.getOrDefault(semanticName, semanticName);
    }
}
