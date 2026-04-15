package com.bong.client.visual.particle;

import net.minecraft.client.particle.ParticleTextureSheet;
import net.minecraft.client.particle.SpriteBillboardParticle;
import net.minecraft.client.render.Camera;
import net.minecraft.client.render.VertexConsumer;
import net.minecraft.client.world.ClientWorld;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * 带状多段粒子（plan-particle-system-v1 §1.2）。
 *
 * <p>用途：飞剑拖尾、雷电、丝带法宝。
 *
 * <p>维护一个<strong>位置环形缓冲</strong>（默认 16 帧），每帧把当前 {@code (x, y, z)} 压入队尾，
 * 超出 {@link #maxHistory} 丢掉队首。渲染时按相邻两节构造一个 {@link BongParticleGeometry#buildRibbonSegment}。
 *
 * <p>宽度随位置在 ribbon 上的归一化 t 变化：头端（新采样）最粗，尾端（最老采样）最细，
 * 以达到 plan §1.2 "头尾 alpha 渐隐"的视觉效果。进一步的回调（"开放 ribbon 宽度随生命周期变化"）
 * 留给子类覆盖 {@link #widthAt(double)}。
 */
public class BongRibbonParticle extends SpriteBillboardParticle {
    /** 默认历史缓冲长度（tick）。 */
    public static final int DEFAULT_MAX_HISTORY = 16;

    protected final int maxHistory;
    protected double headHalfWidth = 0.12;
    protected double tailHalfWidth = 0.02;

    /** 位置历史：队尾 = 最新。 */
    protected final Deque<double[]> history;

    public BongRibbonParticle(
        ClientWorld world,
        double x, double y, double z,
        double velocityX, double velocityY, double velocityZ
    ) {
        this(world, x, y, z, velocityX, velocityY, velocityZ, DEFAULT_MAX_HISTORY);
    }

    public BongRibbonParticle(
        ClientWorld world,
        double x, double y, double z,
        double velocityX, double velocityY, double velocityZ,
        int maxHistory
    ) {
        super(world, x, y, z, velocityX, velocityY, velocityZ);
        if (maxHistory < 2) {
            throw new IllegalArgumentException("maxHistory must be >= 2, got " + maxHistory);
        }
        this.maxHistory = maxHistory;
        this.history = new ArrayDeque<>(maxHistory);
        this.history.addLast(new double[] { x, y, z });
        this.maxAge = 40;
        this.velocityX = velocityX;
        this.velocityY = velocityY;
        this.velocityZ = velocityZ;
        this.collidesWithWorld = false;
    }

    public BongRibbonParticle setRibbonWidth(double head, double tail) {
        this.headHalfWidth = head;
        this.tailHalfWidth = tail;
        return this;
    }

    @Override
    public void tick() {
        super.tick();
        if (!this.dead) {
            // 压入当前位置；tick() 已经把 velocity 应用到 x/y/z 上。
            // 超出容量丢最老的一节。
            history.addLast(new double[] { this.x, this.y, this.z });
            while (history.size() > maxHistory) {
                history.pollFirst();
            }
        }
    }

    /**
     * 子类钩子：返回位置 t ∈ [0, 1] 处的半宽（0 = 尾端老采样，1 = 头端新采样）。
     * 默认线性插值 {@link #tailHalfWidth} → {@link #headHalfWidth}。
     */
    protected double widthAt(double t) {
        return tailHalfWidth + (headHalfWidth - tailHalfWidth) * t;
    }

    @Override
    public ParticleTextureSheet getType() {
        return ParticleTextureSheet.PARTICLE_SHEET_TRANSLUCENT;
    }

    @Override
    public void buildGeometry(VertexConsumer vertexConsumer, Camera camera, float tickDelta) {
        if (history.size() < 2) {
            return;
        }
        net.minecraft.util.math.Vec3d camPos = camera.getPos();

        // 把历史缓冲转成 camera-relative 坐标序列
        double[][] nodes = new double[history.size()][3];
        int i = 0;
        for (double[] pt : history) {
            nodes[i][0] = pt[0] - camPos.x;
            nodes[i][1] = pt[1] - camPos.y;
            nodes[i][2] = pt[2] - camPos.z;
            i++;
        }

        int segCount = nodes.length - 1;
        float u0 = this.getMinU();
        float u1 = this.getMaxU();
        float v0 = this.getMinV();
        float v1 = this.getMaxV();
        int light = this.getBrightness(tickDelta);

        for (int s = 0; s < segCount; s++) {
            double[] prev = nodes[s];
            double[] curr = nodes[s + 1];
            // t 衡量这条段在 ribbon 上的位置（0 尾端，1 头端）
            double tPrev = (double) s / segCount;
            double tCurr = (double) (s + 1) / segCount;
            // 单段宽度用两端平均，保持相邻段连续
            double halfWidth = (widthAt(tPrev) + widthAt(tCurr)) * 0.5;

            float[] quad = BongParticleGeometry.buildRibbonSegment(prev, curr, curr, halfWidth);

            // 头尾 alpha 渐隐：尾端（t=0）alpha × 0，头端 × 1。相邻段接缝处两端 alpha 相同。
            float alphaPrev = this.alpha * (float) tPrev;
            float alphaCurr = this.alpha * (float) tCurr;

            // UV 沿 ribbon 长度流动：prev 在 u0 侧，curr 在 u1 侧（plan §1.2）
            float uPrev = u0 + (u1 - u0) * (float) tPrev;
            float uCurr = u0 + (u1 - u0) * (float) tCurr;

            vertexConsumer.vertex(quad[0],  quad[1],  quad[2]).texture(uPrev, v1).color(this.red, this.green, this.blue, alphaPrev).light(light).next();
            vertexConsumer.vertex(quad[3],  quad[4],  quad[5]).texture(uPrev, v0).color(this.red, this.green, this.blue, alphaPrev).light(light).next();
            vertexConsumer.vertex(quad[6],  quad[7],  quad[8]).texture(uCurr, v0).color(this.red, this.green, this.blue, alphaCurr).light(light).next();
            vertexConsumer.vertex(quad[9],  quad[10], quad[11]).texture(uCurr, v1).color(this.red, this.green, this.blue, alphaCurr).light(light).next();
        }
    }
}
