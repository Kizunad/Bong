package com.bong.client.visual.particle;

import net.minecraft.client.world.ClientWorld;

/** Ribbon particle with a light centripetal drift for woliu-v2 vortex tails. */
public final class VortexSpiralParticle extends BongRibbonParticle {
    private final double centerX;
    private final double centerY;
    private final double centerZ;
    private double angularVelocity = 0.12;

    public VortexSpiralParticle(
        ClientWorld world,
        double x,
        double y,
        double z,
        double velocityX,
        double velocityY,
        double velocityZ,
        double centerX,
        double centerY,
        double centerZ
    ) {
        super(world, x, y, z, velocityX, velocityY, velocityZ, 20);
        this.centerX = centerX;
        this.centerY = centerY;
        this.centerZ = centerZ;
        this.maxAge = 36;
        this.setRibbonWidth(0.10, 0.015);
        this.setAlpha(0.82f);
    }

    public VortexSpiralParticle setAngularVelocity(double angularVelocity) {
        this.angularVelocity = angularVelocity;
        return this;
    }

    @Override
    public void tick() {
        double dx = this.x - centerX;
        double dz = this.z - centerZ;
        double radius = Math.max(0.05, Math.sqrt(dx * dx + dz * dz));
        double tangentX = -dz / radius;
        double tangentZ = dx / radius;
        double pull = 0.018;
        this.velocityX += tangentX * angularVelocity - dx * pull;
        this.velocityZ += tangentZ * angularVelocity - dz * pull;
        this.velocityY += (centerY + 0.25 - this.y) * 0.006;
        this.velocityX *= 0.88;
        this.velocityY *= 0.90;
        this.velocityZ *= 0.88;
        super.tick();
    }
}
