package com.bong.client.visual.particle;

/**
 * 环绕轨道的**逐 tick 演化状态** —— plan-skill-anim-fidelity-v1 §P5.1。
 *
 * <p><b>为什么参数化而不是加力</b>：既有 {@link VortexSpiralParticle} 用「每 tick 往速度上叠切向力 +
 * 向心拉力 + 阻尼」模拟涡流，那套适合"乱流"观感，但半径由力平衡涌现、<b>不可控也不可断言</b>。
 * 本类走参数化路线——位置直接由 {@code (圆心 + 半径 × (cos θ, sin θ))} 求出，θ 每 tick 加角速度：
 * 半径<b>由构造恒定</b>，"环绕"不是近似而是不变量，测试可以硬断言（见
 * {@link #horizontalRadius()}）。招式护体环 / 逆流纹要的正是这种规整贴身感。
 *
 * <p>本类**不含任何 MC 类型**，所以纯 JVM 单测能直接驱动 N 个 tick 验证轨道；
 * {@link BongOrbitSpriteParticle} / {@link BongOrbitLineParticle} 只是把它接到 MC 粒子生命周期上。
 */
public final class OrbitPath {
    private final SkillParticleSpawn.OrbitSpec spec;
    private double angle;
    private int elapsedTicks;

    public OrbitPath(SkillParticleSpawn.OrbitSpec spec) {
        this.spec = spec;
        this.angle = spec.startAngle();
        this.elapsedTicks = 0;
    }

    /** 推进一个 tick：角位置前进一个角速度，垂直漂移计数 +1。 */
    public void advance() {
        this.angle += spec.angularVelocity();
        this.elapsedTicks++;
    }

    public double x() {
        return spec.centerX() + Math.cos(angle) * spec.radius();
    }

    /** 垂直漂移相对圆心线性累积——{@code verticalSpeed = 0} 时是严格水平环。 */
    public double y() {
        return spec.centerY() + spec.verticalSpeed() * elapsedTicks;
    }

    public double z() {
        return spec.centerZ() + Math.sin(angle) * spec.radius();
    }

    public double velocityX() {
        return spec.tangentXAt(angle);
    }

    public double velocityY() {
        return spec.verticalSpeed();
    }

    public double velocityZ() {
        return spec.tangentZAt(angle);
    }

    public double angle() {
        return angle;
    }

    public int elapsedTicks() {
        return elapsedTicks;
    }

    /**
     * 当前点到圆心的**水平**距离。环绕不变量：无论推进多少 tick 都恒等于 {@code spec.radius()}。
     *
     * <p>这是区分真环绕与切向抛射的判据：抛射实现下这个值会随 tick 单调增长直到粒子飞出视野。
     */
    public double horizontalRadius() {
        double dx = x() - spec.centerX();
        double dz = z() - spec.centerZ();
        return Math.sqrt(dx * dx + dz * dz);
    }
}
