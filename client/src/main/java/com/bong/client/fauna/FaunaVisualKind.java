package com.bong.client.fauna;

import net.minecraft.entity.EntityDimensions;
import net.minecraft.util.Identifier;

public enum FaunaVisualKind {
    DEVOUR_RAT("devour_rat", 126, 0.6f, 0.5f, 0.65f, 0.25f, null),
    ASH_SPIDER("ash_spider", 127, 0.9f, 0.45f, 0.75f, 0.22f, null),
    HYBRID_BEAST("hybrid_beast", 128, 1.2f, 1.4f, 0.95f, 0.45f, null),
    VOID_DISTORTED("void_distorted", 129, 1.2f, 1.5f, 1.05f, 0.5f, null),
    DAOXIANG("daoxiang", 130, 0.65f, 1.9f, 0.95f, 0.38f, null),
    ZHINIAN("zhinian", 131, 0.65f, 1.9f, 0.95f, 0.38f, null),
    TSY_SENTINEL("tsy_sentinel", 132, 0.85f, 2.1f, 1.05f, 0.45f, null),
    FUYA("fuya", 133, 0.8f, 2.0f, 1.1f, 0.25f, null),
    SKULL_FIEND("skull_fiend", 134, 1.4f, 1.4f, 1.05f, 0.18f, null),
    GREEN_SPIDER("green_spider", 135, 0.9f, 0.45f, 0.75f, 0.22f, "green_spider"),
    JUNGLE_SCORPION("jungle_scorpion", 136, 0.8f, 0.5f, 0.7f, 0.25f, "jungle_scorpion"),
    COCKADE_SNAKE("cockade_snake", 137, 0.5f, 0.4f, 0.65f, 0.18f, "cockade_snake"),
    BLUE_SPIDER("blue_spider", 138, 1.0f, 0.55f, 0.8f, 0.28f, "blue_spider"),
    ICE_SCORPION("ice_scorpion", 139, 1.0f, 0.6f, 0.8f, 0.3f, "ice_scorpion"),
    MANDRAKE_SNAKE("mandrake_snake", 140, 0.6f, 0.5f, 0.7f, 0.22f, "mandrake_snake"),
    DARK_TIGER("dark_tiger", 141, 1.4f, 1.2f, 0.9f, 0.4f, "dark_tiger"),
    LIVING_PILLAR("living_pillar", 142, 2.0f, 5.0f, 1.0f, 0.6f, "living_pillar"),
    POISON_DRAGON("poison_dragon", 143, 2.5f, 2.0f, 1.0f, 0.7f, "poison_dragon"),
    BONE_DRAGON("bone_dragon", 144, 2.5f, 2.2f, 1.0f, 0.7f, "bone_dragon"),
    HEIWUSHI("heiwushi", 145, 1.2f, 2.8f, 1.0f, 0.5f, null);

    private final String path;
    private final int expectedRawId;
    private final EntityDimensions dimensions;
    private final float renderScale;
    private final float shadowRadius;
    private final String animPath;

    FaunaVisualKind(
        String path,
        int expectedRawId,
        float width,
        float height,
        float renderScale,
        float shadowRadius,
        String animPath
    ) {
        this.path = path;
        this.expectedRawId = expectedRawId;
        this.dimensions = EntityDimensions.fixed(width, height);
        this.renderScale = renderScale;
        this.shadowRadius = shadowRadius;
        this.animPath = animPath;
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
        if (animPath != null) {
            return new Identifier("bong", "animations/" + animPath + ".animation.json");
        }
        return new Identifier("bong", "animations/fauna.animation.json");
    }

    /**
     * 该物种 idle 动画的 GeckoLib 名称。
     *
     * <p>关键：{@link #animationId()} 对 animPath!=null 的物种只加载**各自**的
     * {@code <animPath>.animation.json}，其中并无通用的 {@code animation.fauna.idle} key。
     * 若 controller 仍硬编码 {@code animation.fauna.idle}，GeckoLib 解析不到 → 实体定格在
     * 绑定姿势（俗称 T-Pose）。故按 animPath 派生与各物种动画文件一致的 idle key：
     * <ul>
     *   <li>animPath==null（通用 fauna 模型）→ {@code animation.fauna.idle}（在 fauna.animation.json 内）</li>
     *   <li>animPath!=null（专属模型）→ {@code animation.bong.<animPath>.idle}（在该物种文件内）</li>
     * </ul>
     * 不变式：每个物种的动画文件**必须**含此 key（由 FaunaVisualKindTest 资源校验锁住）。
     */
    public String idleAnimationName() {
        if (animPath != null) {
            return "animation.bong." + animPath + ".idle";
        }
        return "animation.fauna.idle";
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
