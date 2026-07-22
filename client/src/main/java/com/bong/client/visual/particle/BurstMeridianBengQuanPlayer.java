package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;

/** {@code bong:burst_meridian_beng_quan}：崩拳短距古铜色真元爆发。 */
public final class BurstMeridianBengQuanPlayer implements VfxPlayer {
    public static final net.minecraft.util.Identifier EVENT_ID =
        new net.minecraft.util.Identifier("bong", "burst_meridian_beng_quan");

    private static final int FALLBACK_RGB = 0xC58B3F;
    private static final double DEFAULT_STRENGTH = 0.9;
    private static final int DEFAULT_COUNT = 8;
    private static final int MIN_COUNT = 1;
    private static final int MAX_COUNT = 24;
    private static final int DEFAULT_DURATION = 8;
    private static final double[] FALLBACK_DIRECTION = new double[] { 1.0, 0.0, 0.0 };
    private static final float ALPHA_FLOOR = 0.25f;

    /**
     * 「payload -> 归一化渲染标量」纯函数。抽出后所有 clamp / isFinite 守卫都能在无
     * {@link MinecraftClient} 的 JVM 里断言（对齐 {@link OffscreenRelicRevealPlayer#resolveDecalSpec}
     * 的可测性范式，并复用 {@link GameplayVfxUtil} 消除手搓的 rgb/direction/clamp 重复）。
     *
     * <ul>
     *   <li>{@code dir} 经 {@link GameplayVfxUtil#direction} 归一——零向量 / 缺失 / 短数组一律
     *       回落 {@link #FALLBACK_DIRECTION}；</li>
     *   <li>{@code strength} 先 {@link Double#isFinite} 再 clamp[0,1]（NaN / ±Inf 回落
     *       {@link #DEFAULT_STRENGTH}），绝不把 NaN 传进速度 / 线宽 / alpha；</li>
     *   <li>{@code maxAge} 经 {@link GameplayVfxUtil#duration} clamp[1,600]——旧实现完全不 clamp，
     *       负 / 零 / 超大寿命会直穿 {@code setMaxAgePublic}。</li>
     * </ul>
     *
     * <p>对**合法**输入（strength∈[0,1]、durationTicks∈[1,600]、方向非零）本函数逐值等价于旧
     * 内联实现——观感不变，仅非法 / 极端输入被收敛。
     */
    static BurstSpec resolveBurstSpec(VfxEventPayload.SpawnParticle payload) {
        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        double[] dir = GameplayVfxUtil.direction(payload, FALLBACK_DIRECTION);
        int count = GameplayVfxUtil.count(payload, DEFAULT_COUNT, MIN_COUNT, MAX_COUNT);
        int maxAge = GameplayVfxUtil.duration(payload, DEFAULT_DURATION);
        double rawStrength = payload.strength().orElse(DEFAULT_STRENGTH);
        double strength = Double.isFinite(rawStrength)
            ? GameplayVfxUtil.clamp(rawStrength, 0.0, 1.0)
            : DEFAULT_STRENGTH;
        return new BurstSpec(rgb[0], rgb[1], rgb[2], dir[0], dir[1], dir[2], strength, count, maxAge);
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null || payload == null) {
            return;
        }
        double[] origin = payload.origin();
        if (origin == null || origin.length < 3) {
            return;
        }

        BurstSpec spec = resolveBurstSpec(payload);
        double ox = origin[0];
        double oy = origin[1];
        double oz = origin[2];
        double dx = spec.dirX();
        double dy = spec.dirY();
        double dz = spec.dirZ();
        double strength = spec.strength();
        // strength 已 clamp[0,1]，此处等价于旧式 max(0.25, min(1.0, strength))，对合法输入逐值不变。
        float alpha = Math.max(ALPHA_FLOOR, (float) Math.min(1.0, strength));

        for (int i = 0; i < spec.count(); i++) {
            double t = spec.count() == 1 ? 0.0 : (double) i / (spec.count() - 1);
            double spread = (t - 0.5) * 0.6;
            double px = ox + dx * t * 0.7 + spread * dz;
            double py = oy + 0.08 * Math.sin(t * Math.PI);
            double pz = oz + dz * t * 0.7 - spread * dx;

            BongLineParticle particle = new BongLineParticle(
                world,
                px,
                py,
                pz,
                dx * (0.55 + 0.25 * strength),
                dy * 0.25,
                dz * (0.55 + 0.25 * strength)
            );
            particle.setLineShape(1.1, 0.7, 0.22 + 0.18 * strength);
            particle.setColor(spec.red(), spec.green(), spec.blue());
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(spec.maxAge());
            if (BongParticles.qiAuraSprites != null) {
                particle.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** 崩拳爆发的归一化渲染标量（{@link #resolveBurstSpec} 产出，headless 可测）。 */
    record BurstSpec(
        float red,
        float green,
        float blue,
        double dirX,
        double dirY,
        double dirZ,
        double strength,
        int count,
        int maxAge
    ) {
    }
}
