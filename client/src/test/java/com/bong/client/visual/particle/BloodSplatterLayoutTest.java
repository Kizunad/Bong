package com.bong.client.visual.particle;

import com.bong.client.visual.particle.BloodSplatterLayout.Kind;
import com.bong.client.visual.particle.BloodSplatterLayout.Splat;
import java.util.List;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 地面血溅散布的分布特征 pin。
 *
 * <p><b>本测试存在的理由</b>：腿伤血渍曾经是"一张 lingqi_ripple 环形贴图铺一个 quad"，
 * 玩家在地上看到的是<em>红色同心圆</em>。修法是把血渍改成不规则散布，那么"不规则"就必须
 * 被量化锁死，否则下次谁再顺手换成规整图形没人拦得住。三条指标（半径离散度 / 角度非均分 /
 * 主方向偏置）都拿"等距圆环"当对照：圆环在这三项上分别是 0、0、0，而本布局全部远离 0。
 */
class BloodSplatterLayoutTest {

    private static final int SEEDS = 200;
    private static final double[] STRENGTHS = { 0.0, 0.3, 0.5, 0.8, 1.0 };

    // ---------- 确定性 ----------

    @Test
    void sameSeedProducesIdenticalLayout() {
        assertEquals(
            BloodSplatterLayout.build(0.7, 4242L),
            BloodSplatterLayout.build(0.7, 4242L),
            "同 seed 必须给出逐字段一致的布局，否则离屏预览与实机、测试与实现全对不上"
        );
    }

    @Test
    void differentSeedsProduceDifferentLayouts() {
        assertNotEquals(
            BloodSplatterLayout.build(0.7, 1L),
            BloodSplatterLayout.build(0.7, 2L),
            "不同 seed 应给出不同的一摊血——每次受伤都长一样会立刻被看穿是贴图"
        );
    }

    // ---------- 结构 ----------

    @Test
    void everyLayoutHasOnePoolPlusDropletsPlusStreaks() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                List<Splat> splats = BloodSplatterLayout.build(strength, seed);
                assertEquals(Kind.POOL, splats.get(0).kind(),
                    "首个构件必须是主血泊（strength=" + strength + " seed=" + seed + "）");
                assertEquals(1, count(splats, Kind.POOL), "主血泊有且只有一摊");
                assertTrue(count(splats, Kind.DROPLET) >= 4,
                    "卫星血点至少 4 颗，否则散不开——实际 " + count(splats, Kind.DROPLET));
                assertTrue(count(splats, Kind.STREAK) >= 1,
                    "至少一道拖尾，血要有甩出去的方向感");
            }
        }
    }

    @Test
    void strongerWoundsSplashWiderAndDenser() {
        int weakDroplets = count(BloodSplatterLayout.build(0.0, 7L), Kind.DROPLET);
        int strongDroplets = count(BloodSplatterLayout.build(1.0, 7L), Kind.DROPLET);
        assertTrue(strongDroplets > weakDroplets,
            "重伤应溅出更多血点：轻伤 " + weakDroplets + " vs 重伤 " + strongDroplets);
        assertTrue(count(BloodSplatterLayout.build(1.0, 7L), Kind.STREAK)
                > count(BloodSplatterLayout.build(0.0, 7L), Kind.STREAK),
            "重伤应甩出更多拖尾");
        assertTrue(BloodSplatterLayout.maxSpread(1.0) > BloodSplatterLayout.maxSpread(0.0),
            "重伤的飞溅范围应更远");

        double weakMean = meanDropletRadius(0.0);
        double strongMean = meanDropletRadius(1.0);
        assertTrue(strongMean > weakMean,
            "重伤血点的平均飞溅距离应更远：" + weakMean + " → " + strongMean);
    }

    @Test
    void strengthIsClampedAtBothEnds() {
        assertEquals(BloodSplatterLayout.build(0.0, 99L), BloodSplatterLayout.build(-5.0, 99L),
            "strength < 0 必须钳到 0（server payload 已钳，但客户端不能假设）");
        assertEquals(BloodSplatterLayout.build(1.0, 99L), BloodSplatterLayout.build(9.0, 99L),
            "strength > 1 必须钳到 1");
        assertEquals(BloodSplatterLayout.maxSpread(0.0), BloodSplatterLayout.maxSpread(-1.0));
        assertEquals(BloodSplatterLayout.maxSpread(1.0), BloodSplatterLayout.maxSpread(4.0));
    }

    // ---------- 反同心圆：半径 ----------

    @Test
    void dropletRadiiSpreadOutInsteadOfSittingOnRings() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                double[] radii = BloodSplatterLayout.build(strength, seed).stream()
                    .filter(s -> s.kind() == Kind.DROPLET)
                    .mapToDouble(Splat::radius)
                    .toArray();
                double cv = coefficientOfVariation(radii);
                double ratio = max(radii) / min(radii);
                String where = " (strength=" + strength + " seed=" + seed + ")";
                assertTrue(cv >= 0.12,
                    "血点半径离散度 " + cv + " 过低" + where
                        + "——同心圆环的离散度是 0，血溅必须明显偏离；实测样本最低约 0.17");
                assertTrue(ratio >= 1.3,
                    "最远/最近血点半径比 " + ratio + " 过接近 1" + where
                        + "——等于所有血点落在同一个圆上，正是要修掉的同心圆观感");
            }
        }
    }

    // ---------- 反同心圆：角度 ----------

    @Test
    void dropletAnglesAreNotEvenlySpaced() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                double[] gaps = sortedAngularGaps(BloodSplatterLayout.build(strength, seed));
                double cv = coefficientOfVariation(gaps);
                assertTrue(cv >= 0.08,
                    "相邻血点夹角的离散度 " + cv + " 过低 (strength=" + strength + " seed=" + seed
                        + ")——等分成环时该值为 0；实测样本最低约 0.13");
            }
        }
    }

    @Test
    void dropletsClusterTowardOneSplashDirection() {
        // 角度合矢量长度：等距圆环 = 0，纯均匀随机（n=8）≈ 0.32，本布局因为围绕主飞溅
        // 方向偏置，应显著高于均匀随机——"血是朝一边溅的"而不是"绕一圈铺满"。
        double total = 0.0;
        int samples = 0;
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                total += angularResultantLength(BloodSplatterLayout.build(strength, seed));
                samples++;
            }
        }
        double mean = total / samples;
        assertTrue(mean >= 0.45,
            "血点角度的平均合矢量长度 " + mean + " 偏低——贴近均匀随机基线(≈0.32)就说明"
                + "飞溅方向感丢了、退回了绕中心铺一圈；实测约 0.53~0.61");
    }

    // ---------- 尺寸 / 形状 ----------

    @Test
    void dropletsAreClearlySmallerThanThePool() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                List<Splat> splats = BloodSplatterLayout.build(strength, seed);
                double poolWidth = splats.get(0).halfWidth();
                for (Splat splat : splats) {
                    if (splat.kind() != Kind.DROPLET) {
                        continue;
                    }
                    assertTrue(splat.halfWidth() < poolWidth * 0.6,
                        "血点 " + splat.halfWidth() + " 必须明显小于主血泊 " + poolWidth
                            + "——血点和血泊一样大时整摊读成红纸屑而不是一滩血");
                }
            }
        }
    }

    @Test
    void dropletSizesVaryWithinOneSplatter() {
        for (long seed = 0; seed < SEEDS; seed++) {
            double[] sizes = BloodSplatterLayout.build(0.8, seed).stream()
                .filter(s -> s.kind() == Kind.DROPLET)
                .mapToDouble(Splat::halfWidth)
                .toArray();
            assertTrue(max(sizes) / min(sizes) >= 1.2,
                "同一摊血里血点大小应有明显差异（seed=" + seed + "），一样大会读成规则图案");
        }
    }

    @Test
    void dropletsAreStretchedAlongTheirFlightPath() {
        boolean sawStretched = false;
        for (long seed = 0; seed < SEEDS; seed++) {
            for (Splat splat : BloodSplatterLayout.build(1.0, seed)) {
                if (splat.kind() != Kind.DROPLET) {
                    continue;
                }
                assertTrue(splat.elongation() >= 1.0,
                    "血点只允许沿飞行方向拉长，不允许被压扁：" + splat.elongation());
                assertEquals(1.0, Math.cos(splat.rotationRad() - splat.bearingRad()), 1e-9,
                    "血点长轴朝向必须等于它相对原点的方位角（沿飞溅径向）");
                sawStretched |= splat.elongation() > 1.2;
            }
        }
        assertTrue(sawStretched, "远处的血点应出现明显椭圆化（>1.2），否则拉长逻辑没生效");
    }

    @Test
    void poolSitsUnderTheVictimAndIsNotAPerfectCircle() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                Splat pool = BloodSplatterLayout.build(strength, seed).get(0);
                assertTrue(pool.radius() <= 0.10 * BloodSplatterLayout.maxSpread(strength) + 1e-9,
                    "主血泊必须压在脚下（偏心 ≤ 10% 飞溅半径），实测 " + pool.radius());
                assertTrue(pool.elongation() >= 0.90 && pool.elongation() <= 1.30,
                    "主血泊是略带椭圆的一摊而不是正圆/长条，实测长宽比 " + pool.elongation());
            }
        }
    }

    @Test
    void streaksAreElongatedAndPointOutward() {
        for (double strength : STRENGTHS) {
            for (long seed = 0; seed < SEEDS; seed++) {
                for (Splat splat : BloodSplatterLayout.build(strength, seed)) {
                    if (splat.kind() != Kind.STREAK) {
                        continue;
                    }
                    assertTrue(splat.elongation() >= 2.5,
                        "拖尾必须细长（长宽比 ≥ 2.5），实测 " + splat.elongation());
                    assertEquals(1.0, Math.cos(splat.rotationRad() - splat.bearingRad()), 1e-9,
                        "拖尾长轴必须沿飞溅径向，否则读成随便撒的小棍");
                    assertTrue(splat.radius() > 0.0, "拖尾不应叠在原点上");
                }
            }
        }
    }

    // ---------- 边界 / 数值健壮 ----------

    @Test
    void allSplatsStayInsideSpreadBoundsWithFiniteExtents() {
        for (double strength : STRENGTHS) {
            double bound = BloodSplatterLayout.maxSpread(strength);
            for (long seed = 0; seed < SEEDS; seed++) {
                for (Splat splat : BloodSplatterLayout.build(strength, seed)) {
                    assertTrue(splat.radius() <= bound + 1e-9,
                        "构件飞出了 strength 对应的飞溅半径 " + bound + "：" + splat);
                    assertTrue(splat.halfLength() > 0.0 && splat.halfWidth() > 0.0,
                        "退化成零面积的 quad 会渲染出空洞：" + splat);
                    assertTrue(Double.isFinite(splat.dx()) && Double.isFinite(splat.dz()),
                        "偏移必须有限：" + splat);
                    assertTrue(splat.alphaScale() > 0.0f && splat.alphaScale() <= 1.0f,
                        "alpha 倍率越界：" + splat.alphaScale());
                    assertTrue(splat.ageScale() > 0.0 && splat.ageScale() <= 1.0,
                        "lifetime 倍率越界：" + splat.ageScale());
                }
            }
        }
    }

    @Test
    void poolIsTheLongestLivedAndMostOpaqueComponent() {
        for (long seed = 0; seed < SEEDS; seed++) {
            List<Splat> splats = BloodSplatterLayout.build(0.9, seed);
            Splat pool = splats.get(0);
            for (Splat splat : splats) {
                if (splat.kind() == Kind.POOL) {
                    continue;
                }
                assertTrue(splat.alphaScale() <= pool.alphaScale(),
                    "血点/拖尾不应比主血泊更浓");
                assertTrue(splat.ageScale() <= pool.ageScale(),
                    "血点/拖尾应比主血泊先干");
            }
        }
    }

    @Test
    void extremeSeedsDoNotBreakTheLayout() {
        long[] seeds = { Long.MIN_VALUE, -1L, 0L, 1L, Long.MAX_VALUE };
        for (long seed : seeds) {
            for (double strength : new double[] { 0.0, 1.0 }) {
                List<Splat> splats = BloodSplatterLayout.build(strength, seed);
                assertFalse(splats.isEmpty(), "seed=" + seed + " 不应产出空布局");
                for (Splat splat : splats) {
                    assertTrue(Double.isFinite(splat.rotationRad()), "旋转角必须有限：" + splat);
                }
            }
        }
    }

    // ---------- helpers ----------

    private static int count(List<Splat> splats, Kind kind) {
        return (int) splats.stream().filter(s -> s.kind() == kind).count();
    }

    private static double meanDropletRadius(double strength) {
        double sum = 0.0;
        int n = 0;
        for (long seed = 0; seed < SEEDS; seed++) {
            for (Splat splat : BloodSplatterLayout.build(strength, seed)) {
                if (splat.kind() == Kind.DROPLET) {
                    sum += splat.radius();
                    n++;
                }
            }
        }
        return sum / n;
    }

    /** 相邻血点方位角之差（升序绕一圈），等分成环时全部相等。 */
    private static double[] sortedAngularGaps(List<Splat> splats) {
        double[] angles = splats.stream()
            .filter(s -> s.kind() == Kind.DROPLET)
            .mapToDouble(s -> (s.bearingRad() + Math.PI * 2.0) % (Math.PI * 2.0))
            .sorted()
            .toArray();
        double[] gaps = new double[angles.length];
        for (int i = 0; i < angles.length; i++) {
            double next = i + 1 < angles.length ? angles[i + 1] : angles[0] + Math.PI * 2.0;
            gaps[i] = next - angles[i];
        }
        return gaps;
    }

    /** 单位矢量平均长度：均匀铺一圈 → 0，全挤在一个方向 → 1。 */
    private static double angularResultantLength(List<Splat> splats) {
        double sx = 0.0;
        double sz = 0.0;
        int n = 0;
        for (Splat splat : splats) {
            if (splat.kind() != Kind.DROPLET) {
                continue;
            }
            sx += Math.cos(splat.bearingRad());
            sz += Math.sin(splat.bearingRad());
            n++;
        }
        return Math.hypot(sx, sz) / n;
    }

    private static double coefficientOfVariation(double[] values) {
        double mean = 0.0;
        for (double v : values) {
            mean += v;
        }
        mean /= values.length;
        double var = 0.0;
        for (double v : values) {
            var += (v - mean) * (v - mean);
        }
        return Math.sqrt(var / values.length) / mean;
    }

    private static double max(double[] values) {
        double out = Double.NEGATIVE_INFINITY;
        for (double v : values) {
            out = Math.max(out, v);
        }
        return out;
    }

    private static double min(double[] values) {
        double out = Double.POSITIVE_INFINITY;
        for (double v : values) {
            out = Math.min(out, v);
        }
        return out;
    }
}
