package com.bong.client.visual.particle;

import net.minecraft.client.particle.ParticleTextureSheet;
import net.minecraft.client.particle.SpriteBillboardParticle;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.util.shape.VoxelShape;

/**
 * 水平贴地的符圈粒子（plan-particle-system-v1 §1.3）。
 *
 * <p>用途：脚下符圈、血迹、脚印、结界投影。
 *
 * <p>特性：
 * <ul>
 *   <li>法线锁定 +Y（quad 永远水平）</li>
 *   <li>绕 +Y 旋转（{@link #rotationRad}）—— 符阵自转</li>
 *   <li>{@link #yLift} 微抬防止 z-fighting（默认 0.02）</li>
 * </ul>
 *
 * <p>构建几何时会采样当前位置与下方方块的 collision / outline shape，将 decal 中心 Y 吸附到
 * 可见顶面，再叠加 {@link #yLift} 防止 z-fighting。
 */
public class BongGroundDecalParticle extends SpriteBillboardParticle {
    protected double halfSize = 0.5;
    protected double rotationRad = 0.0;
    /** 符阵自转角速度（rad / tick）。正值 CCW。 */
    protected double rotationVelocity = 0.0;
    protected double yLift = 0.02;

    public BongGroundDecalParticle(
        ClientWorld world,
        double x, double y, double z
    ) {
        super(world, x, y, z, 0.0, 0.0, 0.0);
        // decal 典型生命周期 2s（40 tick），子类可覆盖
        this.maxAge = 40;
        this.velocityX = 0;
        this.velocityY = 0;
        this.velocityZ = 0;
        this.collidesWithWorld = false;
    }

    public BongGroundDecalParticle setDecalShape(double halfSize, double yLift) {
        this.halfSize = halfSize;
        this.yLift = yLift;
        return this;
    }

    public BongGroundDecalParticle setSpin(double initialRad, double radPerTick) {
        this.rotationRad = initialRad;
        this.rotationVelocity = radPerTick;
        return this;
    }

    public BongGroundDecalParticle setSpritePublic(net.minecraft.client.texture.Sprite sprite) {
        if (sprite != null) {
            this.setSprite(sprite);
        }
        return this;
    }

    public BongGroundDecalParticle setAlphaPublic(float alpha) {
        this.setAlpha(alpha);
        return this;
    }

    public BongGroundDecalParticle setMaxAgePublic(int maxAge) {
        this.maxAge = maxAge;
        return this;
    }

    @Override
    public void tick() {
        super.tick();
        // 自转累加。Math.IEEEremainder 取模到 (-π, π]，防止数值漂移。
        this.rotationRad = Math.IEEEremainder(
            this.rotationRad + this.rotationVelocity,
            Math.PI * 2.0
        );
    }

    @Override
    public ParticleTextureSheet getType() {
        return ParticleTextureSheet.PARTICLE_SHEET_TRANSLUCENT;
    }

    @Override
    public void buildGeometry(VertexConsumer vertexConsumer, Camera camera, float tickDelta) {
        net.minecraft.util.math.Vec3d camPos = camera.getPos();
        double cx = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosX, this.x) - camPos.x;
        double worldY = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosY, this.y);
        double cy = terrainFittedCenterY(worldY) - camPos.y;
        double cz = net.minecraft.util.math.MathHelper.lerp((double) tickDelta, this.prevPosZ, this.z) - camPos.z;

        float[] quad = BongParticleGeometry.buildGroundDecalQuad(
            new double[] { cx, cy, cz },
            this.halfSize,
            this.rotationRad,
            this.yLift
        );

        float u0 = this.getMinU();
        float u1 = this.getMaxU();
        float v0 = this.getMinV();
        float v1 = this.getMaxV();
        int light = this.getBrightness(tickDelta);

        // UV 按 decal local 坐标 (west, north, east, south) 贴图。
        vertexConsumer.vertex(quad[0],  quad[1],  quad[2]).texture(u0, v1).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[3],  quad[4],  quad[5]).texture(u0, v0).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[6],  quad[7],  quad[8]).texture(u1, v0).color(this.red, this.green, this.blue, this.alpha).light(light).next();
        vertexConsumer.vertex(quad[9],  quad[10], quad[11]).texture(u1, v1).color(this.red, this.green, this.blue, this.alpha).light(light).next();
    }

    private double terrainFittedCenterY(double worldY) {
        BlockPos base = BlockPos.ofFloored(this.x, worldY, this.z);
        double[] candidates = {
            supportTopY(base),
            supportTopY(base.down()),
            supportTopY(base.down(2)),
        };
        return BongParticleGeometry.fitGroundDecalY(worldY, candidates);
    }

    private double supportTopY(BlockPos pos) {
        net.minecraft.block.BlockState state = this.world.getBlockState(pos);
        VoxelShape shape = state.getCollisionShape(this.world, pos);
        if (shape.isEmpty()) {
            shape = state.getOutlineShape(this.world, pos);
        }
        if (shape.isEmpty()) {
            return Double.NaN;
        }
        return pos.getY() + shape.getMax(Direction.Axis.Y);
    }
}
