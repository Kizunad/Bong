package com.bong.client.visual.particle;

import net.minecraft.client.particle.ParticleTextureSheet;
import net.minecraft.client.particle.SpriteBillboardParticle;
import net.minecraft.client.texture.Sprite;
import net.minecraft.client.world.ClientWorld;

/**
 * 标准 billboard Sprite 粒子（qi_aura / rune_char / enlightenment_dust）。
 *
 * <p>存在的唯一目的是把 {@link SpriteBillboardParticle} 的 protected setter 暴露为 public
 * 以便跨包 VfxPlayer 调用；几何和 tick 行为完全交给父类。
 */
public class BongSpriteParticle extends SpriteBillboardParticle {
    public BongSpriteParticle(
        ClientWorld world,
        double x, double y, double z,
        double vx, double vy, double vz
    ) {
        super(world, x, y, z, vx, vy, vz);
        this.collidesWithWorld = false;
        this.velocityX = vx;
        this.velocityY = vy;
        this.velocityZ = vz;
    }

    @Override
    public ParticleTextureSheet getType() {
        return ParticleTextureSheet.PARTICLE_SHEET_TRANSLUCENT;
    }

    public BongSpriteParticle setSpritePublic(Sprite sprite) {
        if (sprite != null) {
            this.setSprite(sprite);
        }
        return this;
    }

    public BongSpriteParticle setAlphaPublic(float alpha) {
        this.setAlpha(alpha);
        return this;
    }

    public BongSpriteParticle setMaxAgePublic(int maxAge) {
        this.maxAge = maxAge;
        return this;
    }

    public BongSpriteParticle setScalePublic(float scale) {
        this.scale(scale);
        return this;
    }
}
