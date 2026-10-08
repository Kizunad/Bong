package com.bong.client.fauna;

import net.minecraft.entity.EntityDimensions;
import net.minecraft.util.Identifier;

public enum FaunaVisualKind {
    DEVOUR_RAT("devour_rat", 126, 0.4f, 0.3f, 0.65f, 0.2f, "devour_rat"),
    // ASH_SPIDER 暂不切 v2：v2 动画文件缺 "fold"（蜘蛛伪装重新折叠用，FaunaPlayback.spiderDisguise
    // 依赖它把 block 置回 true），v1 的 "retreat" 已确认死代码（无调用点，不是阻塞项）。
    // 补上 fold 前切换会让暴起后的蜘蛛永久可见、ambush 隐蔽机制失效，故仍留 v1 资源。
    // 其余 6 只已切到 v2 资源三件套（geo/texture/animation 同名 "<id>_v2"）；
    // entity 注册 id（第一个字段）和 expectedRawId 保持不动，跨端协议契约不受影响。
    ASH_SPIDER("ash_spider", 127, 0.9f, 0.45f, 1.0f, 0.4f, "ash_spider"),
    HYBRID_BEAST("hybrid_beast", 128, 1.2f, 1.4f, 1.0f, 0.6f, "hybrid_beast"),
    VOID_DISTORTED("void_distorted", 129, 1.2f, 1.5f, 1.05f, 0.5f, "void_distorted_v2"),
    DAOXIANG("daoxiang", 130, 0.65f, 1.9f, 0.95f, 0.38f, "daoxiang_v2"),
    ZHINIAN("zhinian", 131, 0.65f, 1.9f, 0.95f, 0.38f, "zhinian_v2"),
    TSY_SENTINEL("tsy_sentinel", 132, 0.85f, 2.1f, 1.05f, 0.45f, "tsy_sentinel_v2"),
    FUYA("fuya", 133, 0.8f, 2.0f, 1.1f, 0.25f, "fuya_v2"),
    SKULL_FIEND("skull_fiend", 134, 1.4f, 1.4f, 1.05f, 0.18f, "skull_fiend_v2"),
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
    HEIWUSHI("heiwushi", 145, 1.2f, 2.8f, 1.0f, 0.5f, "heiwushi"),
    // 追加在原有 168 号之后，不能挤占 modeled entities 的协议 ID。
    DAINU_LION("dainu_lion", 169, 1.2f, 1.3f, 1.0f, 0.6f, "dainu_lion"),
    FUYU_VULTURE("fuyu_vulture", 170, 0.9f, 1.6f, 1.0f, 0.45f, "fuyu_vulture"),
    KEKEDA_GOOSE("kekeda_goose", 171, 0.7f, 1.0f, 1.0f, 0.35f, "kekeda_goose"),
    HORSE("horse", 172, 1.2f, 1.9f, 1.0f, 0.6f, "horse");

    private final String path;
    private final int expectedRawId;
    private final EntityDimensions dimensions;
    private final float renderScale;
    private final float shadowRadius;
    private final String assetPath;

    FaunaVisualKind(
        String path,
        int expectedRawId,
        float width,
        float height,
        float renderScale,
        float shadowRadius,
        String assetPath
    ) {
        this.path = path;
        this.expectedRawId = expectedRawId;
        this.dimensions = EntityDimensions.fixed(width, height);
        this.renderScale = renderScale;
        this.shadowRadius = shadowRadius;
        this.assetPath = assetPath;
    }

    public Identifier entityId() {
        return new Identifier("bong", path);
    }

    /**
     * geo / texture / animation 三件套共用的资源路径（默认形态，未切换子 Profile 时）。
     *
     * <p>与 {@link #path}（entity 注册 id / raw id，跨端协议契约）分离：同一物种升级到
     * 新版模型（如 v2 重做）时只改这里，entity id 和协议 raw id 不受影响。
     */
    public String assetPath() {
        return assetPath == null ? "fauna" : assetPath;
    }

    public Identifier modelId() {
        return new Identifier("bong", "geo/" + assetPath() + ".geo.json");
    }

    public Identifier textureId() {
        return new Identifier("bong", "textures/entity/fauna/" + assetPath() + ".png");
    }

    public Identifier animationId() {
        return new Identifier("bong", "animations/" + assetPath() + ".animation.json");
    }

    public FaunaAnimations.Profile animations() {
        return FaunaAnimations.load(assetPath());
    }

    public String idleAnimationName() {
        return animations().idle().name();
    }

    public String walkAnimationName() {
        var clip = animations().walk();
        return clip == null ? null : clip.name();
    }

    public String runAnimationName() {
        var clip = animations().run();
        return clip == null ? null : clip.name();
    }

    /** Marker 不下发旋转；有移动动画的生物按位移转向。 */
    public boolean facesMovementDirection() {
        return walkAnimationName() != null;
    }

    public boolean deferredRegistration() {
        return expectedRawId >= 169;
    }

    /**
     * 该物种是否挂 emissive 发光层（{@link FaunaEmissiveGlowLayer}）。
     *
     * <p>为 {@code true} 的物种**必须**为其 {@code getTextureResource} 可能返回的**每一张**
     * 底图都备好同名 {@code _glow.png}（见 {@link FaunaModel#glowTextureFor}）——缺一张就会
     * 在该状态下渲染 missing texture（紫黑格）盖住整只怪。
     *
     * <p>目前只有噬元鼠：红眼恒亮 + 尾脊蓝格按吸元档位（q0/q1/q2）递增发光，
     * 让玩家隔着距离就能看出"这只鼠吸饱了"。
     */
    public boolean hasEmissiveGlow() {
        return this == DEVOUR_RAT;
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
