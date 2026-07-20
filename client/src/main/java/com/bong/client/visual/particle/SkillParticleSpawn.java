package com.bong.client.visual.particle;

import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.texture.Sprite;
import net.minecraft.client.world.ClientWorld;

/**
 * 招式粒子的**纯数据发射描述符** —— plan-skill-anim-fidelity-v1 §P5.1（review r1 #3 引入）。
 *
 * <p><b>为什么存在</b>：P5 首版把 spawn 逻辑直接写在 {@code play(MinecraftClient, ...)} 里，
 * 而 {@code MinecraftClient} / {@code ClientWorld} 在纯 JVM 单测里造不出来——于是测试只能退化成
 * 「断言 {@code Form} 枚举的 id→形态映射」，<b>从未观察过一颗真实粒子的参数</b>。后果是 review r1
 * 抓到的两处实现偏离（穴位点 lifetime 擅自 +6、「切向环绕」实为切向抛射）在全绿门禁下溜了过去。
 *
 * <p><b>修法</b>：把「发射什么粒子」与「怎么把粒子塞进 particleManager」切成两层——
 * 三个 player 的 {@code plan(payload)} 是<b>不依赖任何 MC 运行时的纯函数</b>，返回本描述符列表；
 * {@link SkillParticleSpawner} 才碰 {@code MinecraftClient}。测试直接消费 {@code plan()} 的输出，
 * 逐颗断言基类 / 数量 / 初始位置 / 速度向量 / lifetime / 颜色 / 贴图 provider。
 *
 * <p><b>为什么是 sealed + 各变体自带 {@link #createParticle}</b>：新增一种粒子形态时，
 * 不实现工厂方法就<b>编译不过</b>——「加了描述符却忘了接实例化」这类静默缺口由编译器兜住，
 * 不必靠测试。（Java 17 的 switch 类型模式还是预览特性，用不了，故取多态而非集中 switch。）
 *
 * <p><b>纯度边界</b>：{@link #createParticle} 是唯一触碰 MC 类型的成员，且只在渲染线程被调用；
 * 描述符本身的全部字段仍是纯数据，单测无需 MC 运行时就能构造与逐格断言——这正是本层的目的。
 *
 * <p>坐标语义：{@link #x()} / {@link #y()} / {@link #z()} 是<b>生成瞬间</b>的世界坐标，
 * {@link #velocityX()} 等同理。环绕形态（{@link OrbitPoint} / {@link OrbitPulse}）的这两组值
 * 由 {@link OrbitSpec} 在 {@code startAngle} 处求出，逐 tick 演化归 {@link OrbitPath}。
 */
public sealed interface SkillParticleSpawn
    permits SkillParticleSpawn.Pulse,
    SkillParticleSpawn.Point,
    SkillParticleSpawn.Ribbon,
    SkillParticleSpawn.Decal,
    SkillParticleSpawn.OrbitPoint,
    SkillParticleSpawn.OrbitPulse {

    /**
     * 贴图来源。**只列既有 provider**——P5 全域硬约束是「贴图零新增」（plan §P5.1 全域共同约束），
     * 枚举在这里封顶，新增贴图必须先改 plan 再改这里，挡住"顺手加一张 png"。
     */
    enum Sheet {
        /** {@code BongParticles.qiAuraSprites}。 */
        QI_AURA,
        /** {@code BongParticles.lingqiRippleSprites}。 */
        LINGQI_RIPPLE;

        /** provider 尚未注册（资源包未加载完）时返回 {@code null}，各 setter 自行忽略。 */
        Sprite pick(ClientWorld world) {
            SpriteProvider provider = switch (this) {
                case QI_AURA -> BongParticles.qiAuraSprites;
                case LINGQI_RIPPLE -> BongParticles.lingqiRippleSprites;
            };
            return provider == null ? null : provider.getSprite(world.random);
        }
    }

    /** 0xRRGGBB。 */
    int rgb();

    float alpha();

    /** 粒子存活 tick 数（= {@code Particle.maxAge}）。 */
    int maxAge();

    Sheet sheet();

    double x();

    double y();

    double z();

    double velocityX();

    double velocityY();

    double velocityZ();

    /**
     * 按本描述符造出真粒子（几何 / alpha / sprite 各形态自持）。
     *
     * <p>颜色与 {@code maxAge} 不在这里设——它们在 {@code Particle} 基类上就是 public 且对所有
     * 形态同构，统一由 {@link SkillParticleSpawner} 施加，避免 6 个变体各抄一遍。
     */
    Particle createParticle(ClientWorld world);

    /**
     * 环绕轨道参数 —— review r1 #2 的核心修复。
     *
     * <p><b>为什么不是「初速度设成切线方向」就完了</b>：只给切向初速度的粒子沿切线<b>直线飞离</b>，
     * 到中心的距离随 tick 线性增长，几个 tick 后就散成一圈外扩的点——那是切向抛射，不是环绕。
     * 真环绕必须持有圆心并逐 tick 更新<b>角</b>位置，半径由构造保证恒定（见 {@link OrbitPath}）。
     *
     * @param centerX         圆心 X（世界坐标）
     * @param centerY         圆心 Y——垂直漂移的**基准**，粒子 Y = centerY + verticalSpeed × 已过 tick
     * @param centerZ         圆心 Z
     * @param radius          环绕半径（格），必须为正且有限——环绕不变量，全程恒定
     * @param startAngle      起始角（rad），逐颗错开以铺满整圈
     * @param tangentialSpeed 线速度（格/tick），plan §P5.1 spec 表直读的那个数；
     *                        角速度 = 线速度 / 半径，正值 = 俯视（自 +Y 向下看）顺时针 +x→+z
     * @param verticalSpeed   垂直漂移（格/tick），0 = 严格水平环
     */
    record OrbitSpec(
        double centerX,
        double centerY,
        double centerZ,
        double radius,
        double startAngle,
        double tangentialSpeed,
        double verticalSpeed
    ) {
        public OrbitSpec {
            if (!(radius > 0.0) || !Double.isFinite(radius)) {
                throw new IllegalArgumentException(
                    "环绕半径必须为正且有限，否则轨道退化成一点（粒子全叠在圆心）；实际 radius=" + radius);
            }
            if (!Double.isFinite(startAngle) || !Double.isFinite(tangentialSpeed)
                || !Double.isFinite(verticalSpeed)) {
                throw new IllegalArgumentException(
                    "轨道参数必须有限，NaN/Inf 会让粒子坐标变 NaN 而静默消失；实际 startAngle="
                        + startAngle + " tangentialSpeed=" + tangentialSpeed
                        + " verticalSpeed=" + verticalSpeed);
            }
        }

        /** 角速度（rad/tick）= 线速度 / 半径。 */
        public double angularVelocity() {
            return tangentialSpeed / radius;
        }

        public double startX() {
            return centerX + Math.cos(startAngle) * radius;
        }

        public double startY() {
            return centerY;
        }

        public double startZ() {
            return centerZ + Math.sin(startAngle) * radius;
        }

        /** {@code angle} 处的切向速度 X 分量（沿角增大方向）。 */
        public double tangentXAt(double angle) {
            return -Math.sin(angle) * tangentialSpeed;
        }

        /** {@code angle} 处的切向速度 Z 分量（沿角增大方向）。 */
        public double tangentZAt(double angle) {
            return Math.cos(angle) * tangentialSpeed;
        }
    }

    /** {@link BongLineParticle} 直线脉冲（剑气 / 经脉短脉冲 / 疾行尘）。 */
    record Pulse(
        double x,
        double y,
        double z,
        double velocityX,
        double velocityY,
        double velocityZ,
        double lengthFactor,
        double minLength,
        double halfWidth,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public Particle createParticle(ClientWorld world) {
            BongLineParticle particle =
                new BongLineParticle(world, x, y, z, velocityX, velocityY, velocityZ);
            particle.setLineShape(lengthFactor, minLength, halfWidth);
            particle.setAlphaPublic(alpha);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }

    /** {@link BongSpriteParticle} 点粒子（穴位点 / 光点柱）。 */
    record Point(
        double x,
        double y,
        double z,
        double velocityX,
        double velocityY,
        double velocityZ,
        float scale,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public Particle createParticle(ClientWorld world) {
            BongSpriteParticle particle =
                new BongSpriteParticle(world, x, y, z, velocityX, velocityY, velocityZ);
            particle.setAlphaPublic(alpha);
            particle.setScalePublic(scale);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }

    /** {@link BongRibbonParticle} 拖尾（步法残影）。 */
    record Ribbon(
        double x,
        double y,
        double z,
        double velocityX,
        double velocityY,
        double velocityZ,
        double headHalfWidth,
        double tailHalfWidth,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public Particle createParticle(ClientWorld world) {
            BongRibbonParticle particle =
                new BongRibbonParticle(world, x, y, z, velocityX, velocityY, velocityZ);
            particle.setRibbonWidth(headHalfWidth, tailHalfWidth);
            particle.setAlphaPublic(alpha);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }

    /** {@link BongGroundDecalParticle} 贴地符圈（撞击冲击环）。贴地静止，故速度恒为 0。 */
    record Decal(
        double x,
        double y,
        double z,
        double halfSize,
        double yLift,
        double spinRad,
        double spinRadPerTick,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public double velocityX() {
            return 0.0;
        }

        @Override
        public double velocityY() {
            return 0.0;
        }

        @Override
        public double velocityZ() {
            return 0.0;
        }

        @Override
        public Particle createParticle(ClientWorld world) {
            BongGroundDecalParticle particle = new BongGroundDecalParticle(world, x, y, z);
            particle.setDecalShape(halfSize, yLift);
            particle.setSpin(spinRad, spinRadPerTick);
            particle.setAlphaPublic(alpha);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }

    /** 环绕的 {@link BongSpriteParticle} 点粒子（体表逆流纹 / 护体环）。 */
    record OrbitPoint(
        OrbitSpec orbit,
        float scale,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public double x() {
            return orbit.startX();
        }

        @Override
        public double y() {
            return orbit.startY();
        }

        @Override
        public double z() {
            return orbit.startZ();
        }

        @Override
        public double velocityX() {
            return orbit.tangentXAt(orbit.startAngle());
        }

        @Override
        public double velocityY() {
            return orbit.verticalSpeed();
        }

        @Override
        public double velocityZ() {
            return orbit.tangentZAt(orbit.startAngle());
        }

        @Override
        public Particle createParticle(ClientWorld world) {
            BongOrbitSpriteParticle particle = new BongOrbitSpriteParticle(world, orbit);
            particle.setAlphaPublic(alpha);
            particle.setScalePublic(scale);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }

    /** 环绕的 {@link BongLineParticle} 脉冲（多点连环的腰高环）。线粒子按速度定向，故切向速度同时是朝向。 */
    record OrbitPulse(
        OrbitSpec orbit,
        double lengthFactor,
        double minLength,
        double halfWidth,
        int rgb,
        float alpha,
        int maxAge,
        Sheet sheet
    ) implements SkillParticleSpawn {
        @Override
        public double x() {
            return orbit.startX();
        }

        @Override
        public double y() {
            return orbit.startY();
        }

        @Override
        public double z() {
            return orbit.startZ();
        }

        @Override
        public double velocityX() {
            return orbit.tangentXAt(orbit.startAngle());
        }

        @Override
        public double velocityY() {
            return orbit.verticalSpeed();
        }

        @Override
        public double velocityZ() {
            return orbit.tangentZAt(orbit.startAngle());
        }

        @Override
        public Particle createParticle(ClientWorld world) {
            BongOrbitLineParticle particle = new BongOrbitLineParticle(world, orbit);
            particle.setLineShape(lengthFactor, minLength, halfWidth);
            particle.setAlphaPublic(alpha);
            particle.setSpritePublic(sheet.pick(world));
            return particle;
        }
    }
}
