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
 *   <li>长宽可分离（{@link #setDecalShape(double, double, double)}）—— 血溅拖尾等要沿朝向拉长</li>
 *   <li>可选尾段淡出（{@link #setFadeOutFrom}）—— 默认关闭，既有 decal 表现不变</li>
 * </ul>
 *
 * <p>构建几何时会采样当前位置与下方方块的 collision / outline shape，将 decal 中心 Y 吸附到
 * 可见顶面，再叠加 {@link #yLift} 防止 z-fighting。
 */
public class BongGroundDecalParticle extends SpriteBillboardParticle {
    protected double halfSize = 0.5;
    /** 本地 X 轴半长。默认与 {@link #halfSize} 相等（方形）；拉长的血道等用它单独放大。 */
    protected double halfLength = 0.5;
    protected double rotationRad = 0.0;
    /** 符阵自转角速度（rad / tick）。正值 CCW。 */
    protected double rotationVelocity = 0.0;
    protected double yLift = 0.02;
    /** 生命周期进度超过此比例后线性淡出；≥1 表示不淡出（默认，保持既有 decal 表现）。 */
    protected float fadeOutFrom = 1.0f;
    /** 淡出基准 alpha —— 记录 {@link #setAlphaPublic} 传入的初始值。 */
    protected float baseAlpha = 1.0f;

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
        return setDecalShape(halfSize, halfSize, yLift);
    }

    /** 长宽分离版：{@code halfLength} 沿本地 +X（即 {@link #rotationRad} 指向），{@code halfWidth} 沿本地 +Z。 */
    public BongGroundDecalParticle setDecalShape(double halfLength, double halfWidth, double yLift) {
        this.halfLength = halfLength;
        this.halfSize = halfWidth;
        this.yLift = yLift;
        return this;
    }

    /**
     * 开启尾段淡出。
     *
     * <p>MC 的 {@code SpriteBillboardParticle} 默认 alpha 恒定到死——decal 会"啪"地消失。
     * 血渍需要慢慢渗掉，所以在 {@code age / maxAge > fadeOutFrom} 之后线性把 alpha 收到 0。
     * 默认 1.0（不淡出），既有符阵 / 涟漪 decal 表现不受影响。
     *
     * @param fadeOutFrom 开始淡出的生命周期比例，钳到 [0, 1]
     */
    public BongGroundDecalParticle setFadeOutFrom(float fadeOutFrom) {
        this.fadeOutFrom = Math.max(0.0f, Math.min(1.0f, fadeOutFrom));
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
        this.baseAlpha = alpha;
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
        this.setAlpha(fadedAlpha(this.age, this.maxAge, this.baseAlpha, this.fadeOutFrom));
    }

    /**
     * 尾段线性淡出后的 alpha（纯函数，便于单测）。
     *
     * <p>{@code fadeOutFrom >= 1} 或 {@code maxAge <= 0} 时原样返回 {@code baseAlpha}。
     */
    static float fadedAlpha(int age, int maxAge, float baseAlpha, float fadeOutFrom) {
        if (fadeOutFrom >= 1.0f || maxAge <= 0) {
            return baseAlpha;
        }
        float progress = Math.min(1.0f, Math.max(0.0f, (float) age / (float) maxAge));
        if (progress <= fadeOutFrom) {
            return baseAlpha;
        }
        float span = 1.0f - fadeOutFrom;
        float remaining = span <= 0.0f ? 0.0f : (1.0f - progress) / span;
        return baseAlpha * Math.max(0.0f, Math.min(1.0f, remaining));
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
            this.halfLength,
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
