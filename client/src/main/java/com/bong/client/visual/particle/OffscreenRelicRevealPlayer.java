package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

/**
 * plan-offscreen-war-v1 P3：离屏战场遗物被玩家发现时的地表遗骸贴花（{@code offscreen_relic_reveal}）。
 *
 * <p>server 端 {@code npc::dormant::relic_hydrate::hydrate_pending_dormant_relics_system} 在玩家
 * 靠近战场 zone、把 sqlite pending relic 物化成地面 loot 时 emit 本 VFX（每件遗物一处），用一片
 * 贴地 {@link BongGroundDecalParticle} 标出"此地曾有厮杀、散落遗骸"。
 *
 * <p>视觉规格（plan §145）：
 * <ul>
 *   <li>贴图：复用现成 {@link BongParticles#ningJiaCrustSprites}（凝甲 crust——散碎角片，
 *       视觉上正是"骨片 / 残骸碎屑散落地面"，比 lingqi 涟漪环更贴战场遗骸语义），不新建贴图；</li>
 *   <li>颜色：骨堆灰白（payload 默认 {@code #B8AFA0}），残卷暗黄 {@code #7A6A3C} 留给 server
 *       按 loot 细分时改 color hex（本 player 纯按 payload.color 上色，两种叙事同一套渲染）；</li>
 *   <li>形态：radial 静态**单**贴花（不自转、不漂移），lifetime 拉满 ~ 持续到拾取的感知近似；</li>
 *   <li>spawn 模式：burst-once-on-hydrate（每次 hydrate 物化一处，不连续刷）。</li>
 * </ul>
 */
public final class OffscreenRelicRevealPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "offscreen_relic_reveal");

    /** 骨堆灰白兜底色（与 server RELIC_DECAL_COLOR_BONE 对齐）。 */
    private static final int FALLBACK_RGB = 0xB8AFA0;
    /** decal 渲染参数：半径基数 / 随 strength 增量 / 抬高防 z-fighting / alpha 上下限 / lifetime 上下限。 */
    private static final double HALF_SIZE_BASE = 0.55;
    private static final double HALF_SIZE_PER_STRENGTH = 0.30;
    private static final double Y_LIFT = 0.02;
    private static final float ALPHA_MIN = 0.30f;
    private static final float ALPHA_MAX = 0.80f;
    private static final int MAX_AGE_MIN = 60;
    private static final int MAX_AGE_MAX = 200;
    private static final int DEFAULT_MAX_AGE = 200;
    private static final double DEFAULT_STRENGTH = 0.7;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = payload.strength().orElse(DEFAULT_STRENGTH);
        int maxAge = payload.durationTicks().orElse(DEFAULT_MAX_AGE);
        double halfSize = HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * strength;

        BongGroundDecalParticle decal = new BongGroundDecalParticle(
            world,
            payload.origin()[0],
            payload.origin()[1],
            payload.origin()[2]
        );
        decal.setDecalShape(halfSize, Y_LIFT);
        // 战场遗骸是静止的——不自转（radPerTick=0），随机初始角度避免每处朝向雷同。
        decal.setSpin(world.random.nextDouble() * Math.PI * 2.0, 0.0);
        decal.setColor(r, g, b);
        decal.setAlphaPublic((float) Math.max(ALPHA_MIN, Math.min(ALPHA_MAX, strength)));
        decal.setMaxAgePublic(Math.max(MAX_AGE_MIN, Math.min(MAX_AGE_MAX, maxAge)));
        if (BongParticles.ningJiaCrustSprites != null) {
            decal.setSpritePublic(BongParticles.ningJiaCrustSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(decal);
    }
}
