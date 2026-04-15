package com.bong.client.visual.particle;

import net.minecraft.client.particle.ParticleTextureSheet;
import net.minecraft.client.particle.SpriteBillboardParticle;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.world.ClientWorld;

/**
 * 沿速度方向拉长的线形四边形粒子（plan-particle-system-v1 §1.1）。
 *
 * <p>用途：剑气、刀罡、掌风线条、暗器轨迹。
 *
 * <p>与 vanilla {@link SpriteBillboardParticle} 的差异：
 * <ul>
 *   <li>不做 billboard：quad 长轴永远沿 velocity，宽轴在水平面</li>
 *   <li>长度 = |velocity| × {@link #lengthFactor}，并受 {@link #minLength} 保底</li>
 *   <li>{@link #halfWidth} 独立控制宽度，不走 {@code scale} 通道</li>
 * </ul>
 *
 * <p>发光层由调用方通过 {@link #setAlpha}/{@link #setColor} 组合决定；这里统一走
 * {@link ParticleTextureSheet#PARTICLE_SHEET_TRANSLUCENT} 以获得 alpha blending（plan §1.1 发光层
 * 要求走 {@code RenderLayer.getEntityTranslucentEmissive} 的部分留到 Phase 2 自定义 sheet）。
 */
public class BongLineParticle extends SpriteBillboardParticle {
    /** 长度因子：{@code length = |velocity| * lengthFactor}。默认 1.0。 */
    protected double lengthFactor = 1.0;
    /** 最小长度保底，防止 velocity 极小时 quad 退化成点。 */
    protected double minLength = 0.25;
    /** quad 半宽。 */
    protected double halfWidth = 0.1;

    public BongLineParticle(
        ClientWorld world,
        double x, double y, double z,
        double velocityX, double velocityY, double velocityZ
    ) {
        super(world, x, y, z, velocityX, velocityY, velocityZ);
        // 默认 lifecycle：20 tick（1s），可由子类/factory 覆盖
        this.maxAge = 20;
        // 速度直接采用传入向量，不做 vanilla 的随机扰动——剑气要可控
        this.velocityX = velocityX;
        this.velocityY = velocityY;
        this.velocityZ = velocityZ;
        this.collidesWithWorld = false;
    }

    public BongLineParticle setLineShape(double lengthFactor, double minLength, double halfWidth) {
        this.lengthFactor = lengthFactor;
        this.minLength = minLength;
        this.halfWidth = halfWidth;
        return this;
    }

    /** 暴露 {@link net.minecraft.client.particle.Particle#setAlpha}（受保护）为跨包可调。 */
    public BongLineParticle setAlphaPublic(float alpha) {
        this.setAlpha(alpha);
        return this;
    }

    /** 暴露 maxAge 写入（vanilla 的 setMaxAge 本来就是 public，这里加链式 for 统一风格）。 */
    public BongLineParticle setMaxAgePublic(int maxAge) {
        this.maxAge = maxAge;
        return this;
    }

    @Override
    public ParticleTextureSheet getType() {
        return ParticleTextureSheet.PARTICLE_SHEET_TRANSLUCENT;
    }

    @Override
    public void buildGeometry(VertexConsumer vertexConsumer, Camera camera, float tickDelta) {
        // 相对 camera 的中心点
        net.minecraft.util.math.Vec3d camPos = camera.getPos();
        double cx = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosX, this.x) - camPos.x;
        double cy = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosY, this.y) - camPos.y;
        double cz = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosZ, this.z) - camPos.z;

        float[] quad = BongParticleGeometry.buildLineQuad(
            new double[] { cx, cy, cz },
            new double[] { this.velocityX, this.velocityY, this.velocityZ },
            this.lengthFactor,
            this.minLength,
            this.halfWidth
        );

        float u0 = this.getMinU();
        float u1 = this.getMaxU();
        float v0 = this.getMinV();
        float v1 = this.getMaxV();
        int light = this.getBrightness(tickDelta);

        vertexConsumer.vertex(quad[0],  quad[1],  quad[2]).texture(u1, v1).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[3],  quad[4],  quad[5]).texture(u1, v0).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[6],  quad[7],  quad[8]).texture(u0, v0).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[9],  quad[10], quad[11]).texture(u0, v1).color(this.red, this.green, this.blue, this.alpha).light(light).next();
    }
}
