package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;

import java.util.OptionalInt;

/**
 * {@code bong:sword_qi_slash} 的播放器（plan-particle-system-v1 §4.4 首批事件）。
 *
 * <p>Phase 1 最小实现：沿 {@code direction} 生成 N 个 {@link BongLineParticle}（默认 N=4），
 * 每个相位错开，形成一条"斩击弧"的视觉印象。颜色取 payload.colorRgb；strength 调制 alpha 与
 * 半宽。
 *
 * <p>真正成品的"剑气斩击弧" Phase 2 会改用 {@link BongRibbonParticle} 做曲线轨迹
 * + 贴图资源（见 plan §4.1 表）。本类此时是占位实现，目标是 §5.2 要求的"最小链路打通"。
 */
public final class SwordQiSlashPlayer implements VfxPlayer {
    /** 粒子 id（plan §4.4）。 */
    public static final net.minecraft.util.Identifier EVENT_ID =
        new net.minecraft.util.Identifier("bong", "sword_qi_slash");

    /** 沿方向采样的默认粒子数，用于编织出一段弧。 */
    private static final int DEFAULT_FANOUT = 4;
    /** 单个 Line 粒子的默认长度因子。 */
    private static final double LENGTH_FACTOR = 0.6;
    private static final double MIN_LENGTH = 0.6;
    /** 默认颜色：淡青。payload.color 缺省时使用。 */
    private static final int FALLBACK_RGB = 0x88CCFF;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) {
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];

        double[] dir = payload.direction().orElse(new double[] { 1.0, 0.0, 0.0 });
        // 归一化。dir 长度 0 时已被 server 端拒绝（ParticleVectorNotFinite）——这里保底再查一次。
        double len = Math.sqrt(dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
        if (len <= 1e-6) {
            dir = new double[] { 1.0, 0.0, 0.0 };
            len = 1.0;
        }
        double dx = dir[0] / len;
        double dy = dir[1] / len;
        double dz = dir[2] / len;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        double strength = payload.strength().orElse(0.8);
        int fanout = clamp(payload.count().orElse(OptionalInt.of(DEFAULT_FANOUT).getAsInt()), 1, 16);
        float alpha = (float) Math.max(0.1, Math.min(1.0, strength));
        double halfWidth = 0.05 + 0.15 * strength;

        int maxAge = payload.durationTicks().orElse(OptionalInt.of(16).getAsInt());

        // 沿 direction 均匀分布若干条 Line；彼此位置微错开（±0.15 垂直方向）以体现弧感。
        // "弧"的垂直偏移轴：取 direction × +Y（若平行则退回 +X）
        double[] side = cross(new double[] { dx, dy, dz }, new double[] { 0, 1, 0 });
        double sideLen = Math.sqrt(side[0]*side[0] + side[1]*side[1] + side[2]*side[2]);
        if (sideLen < 1e-6) {
            side = new double[] { 0, 0, 1 };
            sideLen = 1.0;
        }
        side[0] /= sideLen; side[1] /= sideLen; side[2] /= sideLen;

        for (int i = 0; i < fanout; i++) {
            double t = fanout == 1 ? 0.0 : ((double) i / (fanout - 1)) * 2 - 1; // [-1, 1]
            double jitter = t * 0.2;
            double px = ox + side[0] * jitter;
            double py = oy + side[1] * jitter;
            double pz = oz + side[2] * jitter;

            // velocity 与 direction 同向，模长选择与 halfWidth 匹配；factor 内部乘
            double speed = 0.8 + 0.3 * strength;
            BongLineParticle particle = new BongLineParticle(
                world, px, py, pz,
                dx * speed, dy * speed, dz * speed
            );
            particle.setLineShape(LENGTH_FACTOR, MIN_LENGTH, halfWidth);
            particle.setColor(r, g, b);
            particle.setAlphaPublic(alpha);
            particle.setMaxAgePublic(maxAge);
            if (BongParticles.swordQiTrailSprites != null) {
                particle.setSpritePublic(
                    BongParticles.swordQiTrailSprites.getSprite(world.random)
                );
            }

            client.particleManager.addParticle(particle);
        }
    }

    private static double[] cross(double[] a, double[] b) {
        return new double[] {
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        };
    }

    private static int clamp(int v, int lo, int hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
