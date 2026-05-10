package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class BreakthroughFailPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "breakthrough_fail");

    private static final int FALLBACK_RGB = 0xFF3344;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        int count = GameplayVfxUtil.count(payload, 16, 4, 48);
        int maxAge = GameplayVfxUtil.duration(payload, 60);
        float alpha = (float) (0.45 + GameplayVfxUtil.strength(payload, 0.8) * 0.4);

        GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
            ox, oy, oz, rgb, alpha, maxAge, 1.2);
        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double speed = 0.08 + world.random.nextDouble() * 0.12;
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.tribulationSparkSprites,
                ox,
                oy + 0.4,
                oz,
                Math.cos(theta) * speed,
                0.03 + world.random.nextDouble() * 0.08,
                Math.sin(theta) * speed,
                rgb,
                alpha,
                maxAge / 2,
                0.12f
            );
        }
    }
}
