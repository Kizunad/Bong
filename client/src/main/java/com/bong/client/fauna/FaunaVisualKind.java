package com.bong.client.fauna;

import net.minecraft.entity.EntityDimensions;
import net.minecraft.util.Identifier;

public enum FaunaVisualKind {
    DEVOUR_RAT("devour_rat", 126, 0.6f, 0.5f, 0.65f, 0.25f),
    ASH_SPIDER("ash_spider", 127, 0.9f, 0.45f, 0.75f, 0.22f),
    HYBRID_BEAST("hybrid_beast", 128, 1.2f, 1.4f, 0.95f, 0.45f),
    VOID_DISTORTED("void_distorted", 129, 1.2f, 1.5f, 1.05f, 0.5f),
    DAOXIANG("daoxiang", 130, 0.65f, 1.9f, 0.95f, 0.38f),
    ZHINIAN("zhinian", 131, 0.65f, 1.9f, 0.95f, 0.38f),
    TSY_SENTINEL("tsy_sentinel", 132, 0.85f, 2.1f, 1.05f, 0.45f),
    FUYA("fuya", 133, 0.8f, 2.0f, 1.1f, 0.25f);

    private final String path;
    private final int expectedRawId;
    private final EntityDimensions dimensions;
    private final float renderScale;
    private final float shadowRadius;

    FaunaVisualKind(
        String path,
        int expectedRawId,
        float width,
        float height,
        float renderScale,
        float shadowRadius
    ) {
        this.path = path;
        this.expectedRawId = expectedRawId;
        this.dimensions = EntityDimensions.fixed(width, height);
        this.renderScale = renderScale;
        this.shadowRadius = shadowRadius;
    }

    public Identifier entityId() {
        return new Identifier("bong", path);
    }

    public Identifier modelId() {
        return new Identifier("bong", "geo/" + path + ".geo.json");
    }

    public Identifier textureId() {
        return new Identifier("bong", "textures/entity/fauna/" + path + ".png");
    }

    public Identifier animationId() {
        return new Identifier("bong", "animations/fauna.animation.json");
    }

    public int expectedRawId() {
        return expectedRawId;
    }

    public EntityDimensions dimensions() {
        return dimensions;
    }

    public float renderScale() {
        return renderScale;
    }

    public float shadowRadius() {
        return shadowRadius;
    }

    public String path() {
        return path;
    }
}
