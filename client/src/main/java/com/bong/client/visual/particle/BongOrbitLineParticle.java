package com.bong.client.visual.particle;

import net.minecraft.client.world.ClientWorld;

/**
 * 绕固定圆心真环绕的线脉冲（真脉多点连环的腰高环）—— plan-skill-anim-fidelity-v1 §P5.1。
 *
 * <p>与 {@link BongOrbitSpriteParticle} 同构（轨道数学全在 {@link OrbitPath}），额外一点：
 * {@link BongLineParticle} 的 quad 长轴<b>沿速度方向</b>，而本类每 tick 把速度同步成当前切向量，
 * 于是脉冲条自然始终切于圆周——线粒子朝向与运动方向自洽，不会出现"横着飞"的错位。
 */
public final class BongOrbitLineParticle extends BongLineParticle {
    private final OrbitPath path;

    public BongOrbitLineParticle(ClientWorld world, SkillParticleSpawn.OrbitSpec orbit) {
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
