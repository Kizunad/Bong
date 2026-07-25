package com.bong.client.visual.particle;

import java.util.ArrayList;
import java.util.List;
import java.util.Random;

/**
 * 地面血溅的散布算法（腿伤血渍 {@code bong:combat_leg_wound_decal} 用）。
 *
 * <p><b>为什么单独抽一个类</b>：血渍此前是"一张环形贴图铺一个方块 quad"，
 * 玩家看到的是红色同心圆靶心。血不是这么长的——地上的血是
 * <em>一滩主血泊 + 一圈不规则卫星血点 + 几道甩出去的拖尾</em>，
 * 三者的半径、大小、朝向全都不等。这个类只负责算出这份散布（纯数学、
 * 不碰 MC 运行时对象），渲染交给 {@link LegWoundBloodDecalPlayer}，
 * 这样"不是同心圆"这件事能在无头单测里被指标锁死。
 *
 * <p><b>反同心圆的三条硬性设计</b>：
 * <ol>
 *   <li><b>半径不等距</b>：卫星血点用分层抽样 + 每点独立抖动，半径分布在
 *       {@code 0.12~1.0} 倍飞溅范围上连续铺开，绝不落在若干个固定半径上；</li>
 *   <li><b>角度不均分</b>：先掷一个主飞溅方向 {@code θ0}，血点角度是围绕它的
 *       近正态抖动（三个均匀分布求和），另有小概率的反溅——整体是"有朝向的一片"，
 *       而不是 {@code 2π/n} 均分的一圈；</li>
 *   <li><b>大小/形状不一</b>：血点大小随机且远处的被拉长（长轴沿飞溅径向），
 *       主血泊本身也是椭圆而非正圆。</li>
 * </ol>
 *
 * <p>随机数只用 {@link Random#nextDouble()}，给定 seed 完全确定——测试可复现，
 * 离屏预览脚本也能逐值镜像同一套布局。
 */
public final class BloodSplatterLayout {

    /** 血溅构件类型。三类各自用不同贴图与生命周期，读起来才有层次。 */
    public enum Kind {
        /** 脚下主血泊：最大、最不透明、活得最久。 */
        POOL,
        /** 卫星血点：小、散、半径与角度都不等。 */
        DROPLET,
        /** 甩出去的拖尾血道：细长，长轴沿飞溅径向。 */
        STREAK
    }

    /**
     * 一个血溅构件。
     *
     * @param kind        构件类型
     * @param dx          相对事件原点的 X 偏移（方块）
     * @param dz          相对事件原点的 Z 偏移（方块）
     * @param halfLength  贴地 quad 的本地 X 半长（旋转后指向 {@code rotationRad}）
     * @param halfWidth   贴地 quad 的本地 Z 半宽
     * @param rotationRad 绕 +Y 的旋转角
     * @param alphaScale  相对基准 alpha 的倍率 (0,1]
     * @param ageScale    相对 payload lifetime 的倍率 (0,1]
     */
    public record Splat(
        Kind kind,
        double dx,
        double dz,
        double halfLength,
        double halfWidth,
        double rotationRad,
        float alphaScale,
        double ageScale
    ) {
        /** 距事件原点的水平距离。 */
        public double radius() {
            return Math.hypot(dx, dz);
        }

        /** 相对原点的方位角（{@code atan2(dz, dx)}，与 {@code rotationRad} 同一坐标约定）。 */
        public double bearingRad() {
            return Math.atan2(dz, dx);
        }

        /** 长宽比。1 = 正方，>1 = 沿 {@code rotationRad} 拉长。 */
        public double elongation() {
            return halfWidth <= 0.0 ? 1.0 : halfLength / halfWidth;
        }
    }

    private static final double TAU = Math.PI * 2.0;

    /**
     * 主血泊半尺寸（方块）：strength 0 → 0.24，strength 1 → 0.40。
     *
     * <p>血泊必须明显压过卫星血点，否则整摊读成"红纸屑"而不是"脚下淌了一滩"。
     */
    private static final double POOL_HALF_MIN = 0.24;
    private static final double POOL_HALF_SPAN = 0.16;
    /** 飞溅范围半径（方块）：strength 0 → 0.38，strength 1 → 0.80。 */
    private static final double SPREAD_MIN = 0.38;
    private static final double SPREAD_SPAN = 0.42;

    private static final int DROPLET_MIN = 4;
    private static final int DROPLET_SPAN = 6;
    private static final int STREAK_MIN = 1;
    private static final int STREAK_SPAN = 2;

    /**
     * 血点角度抖动幅度。三个 {@code nextDouble()} 求和减 1.5 得到近正态、σ≈0.5 的量，
     * 乘上它 → σ≈0.85 rad。够散（不是一条线），又明显有主方向（不是均分成环）。
     */
    private static final double DROPLET_ANGLE_SPREAD = 1.70;
    /** 反溅概率：少量血点甩到主方向背面，避免整片过于"扇形规整"。 */
    private static final double BACKSPLASH_CHANCE = 0.15;
    /** 拖尾锥角（rad）：拖尾全部集中在主飞溅方向附近。 */
    private static final double STREAK_CONE = 1.10;

    private BloodSplatterLayout() {
    }

    /**
     * 算出一次腿伤血渍的全部构件。
     *
     * @param strength server payload 的 strength（伤势强度），会钳到 [0,1]
     * @param seed     随机种子（运行时取 {@code world.random.nextLong()}）
     * @return 构件列表，首个恒为 {@link Kind#POOL}
     */
    public static List<Splat> build(double strength, long seed) {
        double s = Math.max(0.0, Math.min(1.0, strength));
        Random rnd = new Random(seed);

        double poolHalf = POOL_HALF_MIN + POOL_HALF_SPAN * s;
        double spread = SPREAD_MIN + SPREAD_SPAN * s;
        // 主飞溅方向：整片血溅围绕它偏，这是"有方向的一摊"与"同心圆"的分水岭。
        double dominant = rnd.nextDouble() * TAU;

        List<Splat> out = new ArrayList<>();

        // ① 主血泊：几乎压在脚下，只给一点偏心；长宽略有差别（不是正圆）。
        double poolOffset = 0.10 * spread * rnd.nextDouble();
        double poolAngle = rnd.nextDouble() * TAU;
        double poolLength = poolHalf * (0.88 + 0.26 * rnd.nextDouble());
        double poolWidth = poolLength * (0.80 + 0.28 * rnd.nextDouble());
        out.add(new Splat(
            Kind.POOL,
            poolOffset * Math.cos(poolAngle),
            poolOffset * Math.sin(poolAngle),
            poolLength,
            poolWidth,
            rnd.nextDouble() * TAU,
            1.0f,
            1.0
        ));

        // ② 卫星血点：分层抽样铺半径 + 主方向近正态抖动铺角度。
        int droplets = DROPLET_MIN + (int) Math.round(DROPLET_SPAN * s);
        for (int i = 0; i < droplets; i++) {
            // 分层：第 i 个落在 [i/n, (i+1)/n) 里的随机位置——保证半径连续铺开，
            // 又不会因为纯随机而挤成一撮。
            double stratum = (i + rnd.nextDouble()) / droplets;
            double radial = clamp(
                0.26 + 0.74 * Math.pow(stratum, 0.85) * (0.80 + 0.40 * rnd.nextDouble()),
                0.12,
                1.0
            );
            double radius = spread * radial;
            double jitter =
                (rnd.nextDouble() + rnd.nextDouble() + rnd.nextDouble() - 1.5) * DROPLET_ANGLE_SPREAD;
            double back = rnd.nextDouble() < BACKSPLASH_CHANCE ? Math.PI : 0.0;
            double angle = dominant + jitter + back;

            double size = poolHalf * (0.10 + 0.16 * rnd.nextDouble());
            // 飞得越远的血点被拉得越长（长轴沿飞行方向），这是真实溅射的椭圆化。
            double stretch = 1.0 + 0.9 * radial * rnd.nextDouble();
            out.add(new Splat(
                Kind.DROPLET,
                radius * Math.cos(angle),
                radius * Math.sin(angle),
                size * stretch,
                size,
                angle,
                (float) (0.55 + 0.35 * rnd.nextDouble()),
                0.50 + 0.35 * rnd.nextDouble()
            ));
        }

        // ③ 拖尾血道：主方向锥内，细长，长轴沿径向。
        int streaks = STREAK_MIN + (int) Math.round(STREAK_SPAN * s);
        for (int j = 0; j < streaks; j++) {
            double angle = dominant + (rnd.nextDouble() - 0.5) * STREAK_CONE;
            double radius = spread * (0.30 + 0.42 * rnd.nextDouble());
            double width = poolHalf * (0.07 + 0.05 * rnd.nextDouble());
            double length = width * (3.00 + 2.20 * rnd.nextDouble());
            out.add(new Splat(
                Kind.STREAK,
                radius * Math.cos(angle),
                radius * Math.sin(angle),
                length,
                width,
                angle,
                (float) (0.38 + 0.28 * rnd.nextDouble()),
                0.45 + 0.30 * rnd.nextDouble()
            ));
        }

        return out;
    }

    /** 给定 strength 时血溅可能到达的最远半径（方块）——测试与调参的边界口径。 */
    public static double maxSpread(double strength) {
        double s = Math.max(0.0, Math.min(1.0, strength));
        return SPREAD_MIN + SPREAD_SPAN * s;
    }

    private static double clamp(double v, double lo, double hi) {
        return Math.max(lo, Math.min(hi, v));
    }
}
