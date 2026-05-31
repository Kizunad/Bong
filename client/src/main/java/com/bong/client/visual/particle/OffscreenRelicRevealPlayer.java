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
 *   <li>颜色：由 server 按 loot_seed 奇偶发来——骨堆灰白 {@code #B8AFA0}（宗门信物叙事）/
 *       残卷暗黄 {@code #7A6A3C}（散修残经叙事），贴花色与 zone narration 同源讲同一故事。
 *       本 player 纯按 {@code payload.color} 上色（兜底 {@code #B8AFA0}），两种叙事共用一套渲染；</li>
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

        // 全部「服务端 payload -> 渲染标量」的规范化 / clamp 算术抽到纯 {@link #resolveDecalSpec}
        // （headless 可测：strength NaN/负/超大/边界、durationTicks 越界、缺省兜底都在那里锁住）。
        // play() 只剩需要 MinecraftClient/ClientWorld 的渲染副作用（建粒子、随机朝向、选 sprite、
        // 提交 particleManager）—— 这部分留在人工 runClient checklist（headless JVM 不可达）。
        DecalSpec spec = resolveDecalSpec(payload);

        BongGroundDecalParticle decal = new BongGroundDecalParticle(
            world,
            payload.origin()[0],
            payload.origin()[1],
            payload.origin()[2]
        );
        decal.setDecalShape(spec.halfSize(), Y_LIFT);
        // 战场遗骸是静止的——不自转（radPerTick=0），随机初始角度避免每处朝向雷同。
        decal.setSpin(world.random.nextDouble() * Math.PI * 2.0, 0.0);
        decal.setColor(spec.red(), spec.green(), spec.blue());
        decal.setAlphaPublic(spec.alpha());
        decal.setMaxAgePublic(spec.maxAge());
        if (BongParticles.ningJiaCrustSprites != null) {
            decal.setSpritePublic(BongParticles.ningJiaCrustSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(decal);
    }

    /**
     * 纯函数：把服务端 {@link VfxEventPayload.SpawnParticle} 规范化成贴花的最终渲染标量。
     * 不解引用 {@code MinecraftClient} / {@code ClientWorld}，可在 headless 单测里直接断言
     * （与 sibling {@link VortexSpiralPlayer#effectSpec} 同款抽法，把回归面从 {@code play()}
     * 里挪出来）。
     *
     * <p>规范化规则（CodeRabbit 加固点都在这里锁死）：
     * <ul>
     *   <li>颜色：缺省兜底 {@link #FALLBACK_RGB}（骨堆灰白）；payload.color 原样拆 RGB；</li>
     *   <li>strength：先 {@link Double#isFinite}——NaN / ±Inf 一律退回 {@link #DEFAULT_STRENGTH}
     *       （否则 NaN 会污染 halfSize/alpha、Inf 会算出超大贴花），有限值再 clamp 到 {@code [0,1]}；
     *       规范化后的 strength 同时驱动 halfSize 与 alpha；</li>
     *   <li>halfSize：{@code BASE + PER_STRENGTH * strength}，因 strength∈[0,1] 故恒在
     *       {@code [BASE, BASE+PER_STRENGTH]}，永不为负 / 超大；</li>
     *   <li>alpha：把规范化 strength clamp 到 {@code [ALPHA_MIN, ALPHA_MAX]}；</li>
     *   <li>maxAge：缺省兜底 {@link #DEFAULT_MAX_AGE}，再 clamp 到 {@code [MAX_AGE_MIN, MAX_AGE_MAX]}
     *       （负 / 0 / 超大 lifetime 都被收口）。</li>
     * </ul>
     */
    static DecalSpec resolveDecalSpec(VfxEventPayload.SpawnParticle payload) {
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        double rawStrength = payload.strength().orElse(DEFAULT_STRENGTH);
        double strength = Double.isFinite(rawStrength)
            ? Math.max(0.0, Math.min(1.0, rawStrength))
            : DEFAULT_STRENGTH;

        double halfSize = HALF_SIZE_BASE + HALF_SIZE_PER_STRENGTH * strength;
        float alpha = (float) Math.max(ALPHA_MIN, Math.min(ALPHA_MAX, strength));

        int rawMaxAge = payload.durationTicks().orElse(DEFAULT_MAX_AGE);
        int maxAge = Math.max(MAX_AGE_MIN, Math.min(MAX_AGE_MAX, rawMaxAge));

        return new DecalSpec(r, g, b, halfSize, alpha, maxAge);
    }

    /** {@link #resolveDecalSpec} 的输出：贴花最终渲染标量（已规范化 / clamp，无需再处理）。 */
    record DecalSpec(float red, float green, float blue, double halfSize, float alpha, int maxAge) {}
}
