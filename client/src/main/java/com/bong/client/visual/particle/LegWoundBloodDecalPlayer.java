package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import java.util.List;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * 腿伤减速触发时目标脚下的血渍 decal（plan-combat-hit-location-v1 P3）。
 *
 * <p><b>修复记录</b>：本 player 最初复用 {@code lingqi_ripple}（灵气涟漪）贴图——
 * 那是一张<em>同心圆环</em>，染成血红后玩家在地上看到的是"红色靶心"，完全不像血。
 * 现在改成专用血溅贴图（{@code blood_splat_*} 不规则血泊 / {@code blood_drop_*} 卫星血点 /
 * {@code blood_streak_*} 拖尾）
 * 加 {@link BloodSplatterLayout} 的不规则散布：一滩主血泊 + 半径角度都不等的卫星血点 +
 * 几道沿飞溅方向甩出的拖尾，暗红贴地、尾段淡出。
 *
 * <p>server 契约未变：仍消费 {@code bong:combat_leg_wound_decal}，仍只用 payload 的
 * origin / color / strength / duration_ticks（{@code count} 恒为 1，散点数量由 strength
 * 在客户端推导）。
 */
public final class LegWoundBloodDecalPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "combat_leg_wound_decal");

    private static final int FALLBACK_RGB = 0x8C1F1F;

    /** 贴地基准抬升，与其他 decal 一致。 */
    private static final double BASE_Y_LIFT = 0.025;
    /**
     * 逐个错开的抬升步长。一摊血是十几个共面 quad，全用同一个 Y 会互相 z-fighting
     * 闪烁；错开 1.5mm 级别肉眼看不出高度差，却能定死绘制顺序。
     */
    private static final double Y_LIFT_STEP = 0.0015;

    /** 各类构件开始淡出的生命周期比例：血泊留到最后，血点/拖尾先干。 */
    private static final float POOL_FADE_FROM = 0.60f;
    private static final float DROPLET_FADE_FROM = 0.45f;
    private static final float STREAK_FADE_FROM = 0.40f;

    private static final float ALPHA_MIN = 0.45f;
    private static final float ALPHA_MAX = 0.92f;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        double strength = GameplayVfxUtil.strength(payload, 0.7);
        int maxAge = GameplayVfxUtil.duration(payload, 100);
        float baseAlpha = (float) Math.max(ALPHA_MIN, Math.min(ALPHA_MAX, 0.5 + 0.45 * strength));

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        List<BloodSplatterLayout.Splat> splats =
            BloodSplatterLayout.build(strength, world.random.nextLong());
        for (int i = 0; i < splats.size(); i++) {
            BloodSplatterLayout.Splat splat = splats.get(i);
            SpriteProvider provider = spritesFor(splat.kind());
            if (provider == null) {
                continue;
            }
            GameplayVfxUtil.spawnGroundSplat(
                client,
                world,
                provider,
                ox + splat.dx(),
                oy,
                oz + splat.dz(),
                rgb,
                baseAlpha * splat.alphaScale(),
                Math.max(1, (int) Math.round(maxAge * splat.ageScale())),
                splat.halfLength(),
                splat.halfWidth(),
                splat.rotationRad(),
                BASE_Y_LIFT + i * Y_LIFT_STEP,
                fadeFrom(splat.kind())
            );
        }
    }

    private static SpriteProvider spritesFor(BloodSplatterLayout.Kind kind) {
        return switch (kind) {
            case POOL -> BongParticles.bloodSplatSprites;
            case DROPLET -> BongParticles.bloodDropSprites;
            case STREAK -> BongParticles.bloodStreakSprites;
        };
    }

    private static float fadeFrom(BloodSplatterLayout.Kind kind) {
        return switch (kind) {
            case POOL -> POOL_FADE_FROM;
            case DROPLET -> DROPLET_FADE_FROM;
            case STREAK -> STREAK_FADE_FROM;
        };
    }
}
