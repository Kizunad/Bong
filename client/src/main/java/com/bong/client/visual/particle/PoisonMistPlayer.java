package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class PoisonMistPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "poison_mist");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, 0x44AA44);
        int count = GameplayVfxUtil.count(payload, 6, 1, 16);
        int maxAge = GameplayVfxUtil.duration(payload, 60);
        for (int i = 0; i < count; i++) {
            GameplayVfxUtil.spawnSprite(client, world, BongParticles.qiAuraSprites,
                ox + (world.random.nextDouble() - 0.5) * 0.8,
                oy + world.random.nextDouble() * 0.8,
                oz + (world.random.nextDouble() - 0.5) * 0.8,
                (world.random.nextDouble() - 0.5) * 0.03,
                0.01 + world.random.nextDouble() * 0.02,
                (world.random.nextDouble() - 0.5) * 0.03,
                rgb, 0.45f, maxAge, 0.14f);
        }
    }
}
