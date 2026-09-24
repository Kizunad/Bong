package com.bong.client.visual.particle;

import net.minecraft.client.world.ClientWorld;

/**
 * 绕固定圆心真环绕的点粒子（体表逆流纹 / 护体环）—— plan-skill-anim-fidelity-v1 §P5.1。
 *
 * <p>位置完全由 {@link OrbitPath} 参数化给出，<b>不走父类的速度积分</b>——速度字段仍逐 tick 同步成
 * 当前切向量，供渲染/下游读取，但它不再决定位置，所以半径恒定（对比：只设切向初速度的实现会
 * 沿切线直线飞离）。
 *
 * <p>{@link BongOrbitLineParticle} 是本类的线粒子孪生：Java 单继承使两者无法共用父类，
 * 但**轨道数学全在 {@link OrbitPath} 里只有一份**，重复的仅是 6 行生命周期委托。
 */
public final class BongOrbitSpriteParticle extends BongSpriteParticle {
    private final OrbitPath path;

    public BongOrbitSpriteParticle(ClientWorld world, SkillParticleSpawn.OrbitSpec orbit) {
        super(
            world,
            orbit.startX(), orbit.startY(), orbit.startZ(),
            orbit.tangentXAt(orbit.startAngle()),
            orbit.verticalSpeed(),
            orbit.tangentZAt(orbit.startAngle())
        );
        this.path = new OrbitPath(orbit);
    }

    @Override
    public void tick() {
        this.prevPosX = this.x;
        this.prevPosY = this.y;
        this.prevPosZ = this.z;
        if (this.age++ >= this.maxAge) {
            this.markDead();
            return;
        }
        path.advance();
        this.setPos(path.x(), path.y(), path.z());
        this.velocityX = path.velocityX();
        this.velocityY = path.velocityY();
        this.velocityZ = path.velocityZ();
    }
}
