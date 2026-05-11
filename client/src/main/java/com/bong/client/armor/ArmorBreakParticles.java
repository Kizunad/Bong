package com.bong.client.armor;

import com.bong.client.visual.particle.BongParticles;
import net.minecraft.client.MinecraftClient;

/**
 * plan-armor-visual-v1：盔甲破碎时的本地碎片粒子。
 */
public final class ArmorBreakParticles {
    public static final int SHARD_COUNT = 4;

    private ArmorBreakParticles() {
    }

    public static boolean spawnLocalShards() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.world == null || client.player == null) {
            return false;
        }

        double x = client.player.getX();
        double y = client.player.getY() + 1.0;
        double z = client.player.getZ();
        for (int i = 0; i < SHARD_COUNT; i++) {
            double angle = (Math.PI * 2.0 * i) / SHARD_COUNT;
            double vx = Math.cos(angle) * 0.08;
            double vz = Math.sin(angle) * 0.08;
            client.world.addParticle(
                BongParticles.ENLIGHTENMENT_DUST,
                x,
                y,
                z,
                vx,
                0.06,
                vz
            );
        }
        return true;
    }
}
