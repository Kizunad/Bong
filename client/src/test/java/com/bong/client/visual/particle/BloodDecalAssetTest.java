package com.bong.client.visual.particle;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 血渍贴图资产 pin —— 直接对着 PNG 的 alpha 通道量化"这不是同心圆"。
 *
 * <p><b>背景</b>：腿伤血渍最初复用 {@code lingqi_ripple}（灵气涟漪）贴图。那张图的径向
 * alpha 剖面是"亮芯 → 暗谷 → 亮环 → 暗"，染成血红铺在地上就是一圈圈红色同心圆。
 * 单看"用了哪个 sprite 变量名"挡不住这类回归（换一张一样是环的图照样过），所以这里改成
 * <em>量化贴图本身的形状</em>：
 *
 * <ul>
 *   <li><b>径向回弹率</b>（主指标）：alpha 径向剖面越过峰值后是否又显著涨回去。
 *       同心圆环必然回弹（{@code lingqi_ripple} 实测 ≈0.31），一摊血从中心到边缘单调变淡
 *       （血渍贴图实测 ≤0.05）。</li>
 *   <li><b>角向不对称度</b>（辅助指标）：中环带按角度分扇区的 alpha 变异系数。
 *       正圆/圆环各向同性 ≈0.12，不规则血渍 ≥0.21。</li>
 * </ul>
 *
 * <p>{@code lingqi_ripple} 在这两项上的实测值同时作为<b>活对照</b>断言——万一哪天指标算法
 * 被改废了（对什么图都返回 0），这条对照会立刻撞红。
 */
class BloodDecalAssetTest {

    private static final Path RESOURCES = Path.of("src", "main", "resources");
    private static final Path PARTICLES_DIR = RESOURCES.resolve(Path.of("assets", "bong", "particles"));
    private static final Path TEXTURES_DIR =
        RESOURCES.resolve(Path.of("assets", "bong", "textures", "particle"));
    private static final Path PLAYER_SOURCE = Path.of(
        "src", "main", "java", "com", "bong", "client", "visual", "particle",
        "LegWoundBloodDecalPlayer.java");

    /** 主血泊 + 卫星血点：这些是"平铺一张图"的圆形 decal，必须逐张验非同心。 */
    private static final List<String> ROUND_BLOOD_TEXTURES = List.of(
        "blood_splat_0", "blood_splat_1", "blood_splat_2", "blood_splat_3",
        "blood_drop_0", "blood_drop_1", "blood_drop_2");
    private static final List<String> STREAK_TEXTURES = List.of("blood_streak_0", "blood_streak_1");

    private static final double MAX_REBOUND = 0.12;
    private static final double MIN_ANGULAR_CV = 0.18;

    @Test
    void bloodTexturesHaveNoConcentricRingStructure() throws IOException {
        for (String name : ROUND_BLOOD_TEXTURES) {
            double rebound = radialReboundRatio(alpha(name));
            assertTrue(rebound < MAX_REBOUND,
                name + " 的径向 alpha 剖面回弹率 " + rebound + " ≥ " + MAX_REBOUND
                    + "——说明中心外面又出现了一圈更亮的环，血渍会退化成同心圆靶心"
                    + "（lingqi_ripple 正是因此被换掉，实测 ≈0.31）");
        }
    }

    @Test
    void bloodTexturesAreAngularlyIrregular() throws IOException {
        for (String name : ROUND_BLOOD_TEXTURES) {
            double cv = angularAsymmetry(alpha(name));
            assertTrue(cv >= MIN_ANGULAR_CV,
                name + " 的角向不对称度 " + cv + " < " + MIN_ANGULAR_CV
                    + "——各向同性到这个程度就是个规规矩矩的圆盘/圆环，不是血");
        }
    }

    /** 活对照：老贴图必须在两项指标上都判"是同心圆"，否则指标本身失效了。 */
    @Test
    void theOldRippleTextureStillScoresAsAConcentricRing() throws IOException {
        int[][] ripple = alpha("lingqi_ripple");
        double rebound = radialReboundRatio(ripple);
        double cv = angularAsymmetry(ripple);
        assertTrue(rebound >= 0.25,
            "lingqi_ripple 的径向回弹率应 ≈0.31（同心圆环特征），实测 " + rebound
                + "——若连它都判成非同心，说明回弹率指标已经失效，血渍 pin 形同虚设");
        assertTrue(cv < MIN_ANGULAR_CV,
            "lingqi_ripple 是各向同性圆环，角向不对称度应低于阈值 " + MIN_ANGULAR_CV
                + "，实测 " + cv);
    }

    @Test
    void streakTexturesAreElongatedAlongTheirLongAxis() throws IOException {
        for (String name : STREAK_TEXTURES) {
            int[][] a = alpha(name);
            int minX = Integer.MAX_VALUE;
            int maxX = Integer.MIN_VALUE;
            int minY = Integer.MAX_VALUE;
            int maxY = Integer.MIN_VALUE;
            for (int y = 0; y < a.length; y++) {
                for (int x = 0; x < a[y].length; x++) {
                    if (a[y][x] > 32) {
                        minX = Math.min(minX, x);
                        maxX = Math.max(maxX, x);
                        minY = Math.min(minY, y);
                        maxY = Math.max(maxY, y);
                    }
                }
            }
            double aspect = (double) (maxX - minX + 1) / (maxY - minY + 1);
            assertTrue(aspect >= 3.0,
                name + " 的可见形状长宽比 " + aspect + " < 3——拖尾血道必须沿贴图 X 轴拉长，"
                    + "否则贴到被拉长的 quad 上会糊成一条粗棍");
        }
    }

    @Test
    void bloodParticleDescriptorsListMultipleVariantsWithCommittedPngs() throws IOException {
        assertVariants("blood_splat.json", 4);
        assertVariants("blood_drop.json", 3);
        assertVariants("blood_streak.json", 2);
    }

    /**
     * 回归 pin：血渍 player 不得再引用环形贴图 sprite。
     *
     * <p>这条是对着源码文本断言的——它锁的正是当初那行"复用 lingqiRippleSprites"的接线，
     * 属于最直接的反悔守卫。
     */
    @Test
    void legWoundPlayerNoLongerReusesTheRingSprite() throws IOException {
        String source = Files.readString(PLAYER_SOURCE, StandardCharsets.UTF_8);
        assertFalse(source.contains("lingqiRippleSprites"),
            "LegWoundBloodDecalPlayer 又引用了 lingqiRippleSprites（同心圆环贴图）——"
                + "血渍必须走 bloodSplat / bloodDrop / bloodStreak");
        assertTrue(source.contains("bloodSplatSprites")
                && source.contains("bloodDropSprites")
                && source.contains("bloodStreakSprites"),
            "血渍三类构件（血泊 / 血点 / 拖尾）必须各自接到自己的贴图 provider");
        assertTrue(source.contains("BloodSplatterLayout"),
            "血渍必须按 BloodSplatterLayout 散布，不能退回单个居中 quad");
    }

    // ---------- 指标实现 ----------

    /**
     * 径向 alpha 剖面越过峰值之后的最大回弹幅度（归一化到峰值）。
     *
     * <p>同心圆环 = 亮芯之后先塌下去再涨起来 → 回弹显著；一摊血 = 单调变淡 → 回弹≈0。
     */
    static double radialReboundRatio(int[][] alpha) {
        double[] profile = radialProfile(alpha, 16);
        double peak = 0.0;
        int peakIdx = 0;
        for (int i = 0; i < profile.length; i++) {
            if (profile[i] > peak) {
                peak = profile[i];
                peakIdx = i;
            }
        }
        if (peak <= 0.0) {
            return 0.0;
        }
        double minSoFar = Double.MAX_VALUE;
        double rebound = 0.0;
        for (int i = peakIdx; i < profile.length; i++) {
            minSoFar = Math.min(minSoFar, profile[i]);
            rebound = Math.max(rebound, profile[i] - minSoFar);
        }
        return rebound / peak;
    }

    /** 中环带按角度分 24 扇区的 alpha 变异系数：各向同性 → 0。 */
    static double angularAsymmetry(int[][] alpha) {
        int h = alpha.length;
        int w = alpha[0].length;
        double cy = (h - 1) / 2.0;
        double cx = (w - 1) / 2.0;
        double rMax = Math.min(h, w) / 2.0;
        int sectors = 24;
        double[] sum = new double[sectors];
        int[] hits = new int[sectors];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                double r = Math.hypot(y - cy, x - cx);
                if (r < 0.25 * rMax || r >= 0.60 * rMax) {
                    continue;
                }
                double theta = Math.atan2(y - cy, x - cx);
                int k = (int) ((theta + Math.PI) / (Math.PI * 2.0) * sectors);
                k = Math.min(sectors - 1, Math.max(0, k));
                sum[k] += alpha[y][x];
                hits[k]++;
            }
        }
        double[] means = new double[sectors];
        double mean = 0.0;
        for (int k = 0; k < sectors; k++) {
            means[k] = hits[k] == 0 ? 0.0 : sum[k] / hits[k];
            mean += means[k];
        }
        mean /= sectors;
        if (mean <= 0.0) {
            return 0.0;
        }
        double var = 0.0;
        for (double v : means) {
            var += (v - mean) * (v - mean);
        }
        return Math.sqrt(var / sectors) / mean;
    }

    private static double[] radialProfile(int[][] alpha, int bands) {
        int h = alpha.length;
        int w = alpha[0].length;
        double cy = (h - 1) / 2.0;
        double cx = (w - 1) / 2.0;
        double rMax = Math.min(h, w) / 2.0;
        double[] sum = new double[bands];
        int[] hits = new int[bands];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                double r = Math.hypot(y - cy, x - cx);
                int band = (int) (r / rMax * bands);
                if (band < 0 || band >= bands) {
                    continue;
                }
                sum[band] += alpha[y][x];
                hits[band]++;
            }
        }
        double[] profile = new double[bands];
        for (int i = 0; i < bands; i++) {
            profile[i] = hits[i] == 0 ? 0.0 : sum[i] / hits[i];
        }
        return profile;
    }

    private static int[][] alpha(String textureName) throws IOException {
        Path png = TEXTURES_DIR.resolve(textureName + ".png");
        assertTrue(Files.isRegularFile(png), "缺少贴图 " + png);
        BufferedImage image = ImageIO.read(png.toFile());
        int[][] out = new int[image.getHeight()][image.getWidth()];
        for (int y = 0; y < image.getHeight(); y++) {
            for (int x = 0; x < image.getWidth(); x++) {
                out[y][x] = (image.getRGB(x, y) >>> 24) & 0xFF;
            }
        }
        return out;
    }

    private static void assertVariants(String descriptor, int expected) throws IOException {
        Path json = PARTICLES_DIR.resolve(descriptor);
        assertTrue(Files.isRegularFile(json), "缺少粒子描述文件 " + json);
        JsonObject root = JsonParser.parseReader(
            new InputStreamReader(Files.newInputStream(json), StandardCharsets.UTF_8)).getAsJsonObject();
        List<String> textures = new ArrayList<>();
        for (JsonElement element : root.getAsJsonArray("textures")) {
            textures.add(element.getAsString());
        }
        assertEquals(expected, textures.size(),
            descriptor + " 的 variant 数变了——血渍靠多 variant 随机取形状，"
                + "只剩一张时同一摊血里每颗血点长得一模一样");
        for (String textureId : textures) {
            assertTrue(textureId.startsWith("bong:particle/"), "贴图 id 约定 bong:particle/<name>：" + textureId);
            String name = textureId.substring("bong:particle/".length());
            assertTrue(Files.isRegularFile(TEXTURES_DIR.resolve(name + ".png")),
                descriptor + " 引用的 " + textureId + " 没有提交对应 PNG");
        }
    }
}
