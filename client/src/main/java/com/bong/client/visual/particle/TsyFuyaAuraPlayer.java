package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** 伏牙负压光环：半径内粒子向中心回吸，enrage 时转红。 */
public final class TsyFuyaAuraPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "tsy_fuya_aura");

    private static final int DEFAULT_RGB = 0x220044;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double[] origin = payload.origin();
        int count = clamp(payload.count().orElse(OptionalInt.of(16).getAsInt()), 4, 48);
        int maxAge = clamp(payload.durationTicks().orElse(OptionalInt.of(34).getAsInt()), 12, 100);
        double strength = clamp(payload.strength().orElse(0.6), 0.0, 1.0);
        int rgb = payload.colorRgb().orElse(DEFAULT_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;

        for (int i = 0; i < count; i++) {
            double yaw = world.random.nextDouble() * Math.PI * 2.0;
            double pitch = (world.random.nextDouble() - 0.5) * Math.PI;
            double radius = 2.5 + world.random.nextDouble() * (5.5 + strength * 2.5);
            double cosPitch = Math.cos(pitch);
            double x = origin[0] + Math.cos(yaw) * cosPitch * radius;
            double y = origin[1] + 1.0 + Math.sin(pitch) * Math.min(radius, 4.0);
            double z = origin[2] + Math.sin(yaw) * cosPitch * radius;
            double pull = 0.018 + strength * 0.035;
            BongSpriteParticle mote = new BongSpriteParticle(
                world,
                x,
                y,
                z,
                (origin[0] - x) * pull,
                (origin[1] + 1.0 - y) * pull,
                (origin[2] - z) * pull
            );
            mote.setColor(r, g, b);
            mote.setAlphaPublic((float) (0.32 + strength * 0.42));
            mote.setScalePublic((float) (0.45 + strength * 0.35));
            mote.setMaxAgePublic(maxAge);
            if (BongParticles.qiAuraSprites != null) {
                mote.setSpritePublic(BongParticles.qiAuraSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(mote);
        }
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }
}
