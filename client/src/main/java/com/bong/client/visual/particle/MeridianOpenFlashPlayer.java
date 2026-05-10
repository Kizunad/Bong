package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class MeridianOpenFlashPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "meridian_open");

    private static final int FALLBACK_RGB = 0x22FFAA;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double[] dir = GameplayVfxUtil.direction(payload, new double[] { 0.0, 1.0, 0.0 });
        float[] rgb = GameplayVfxUtil.rgb(payload, FALLBACK_RGB);
        int count = GameplayVfxUtil.count(payload, 3, 1, 8);
        int maxAge = GameplayVfxUtil.duration(payload, 20);

        for (int i = 0; i < count; i++) {
            double spread = (i - (count - 1) * 0.5) * 0.12;
            GameplayVfxUtil.spawnLine(
                client,
                world,
                BongParticles.qiAuraSprites,
                ox + spread,
                oy + 0.2,
                oz,
                dir[0] * 0.8,
                dir[1] * 0.8,
                dir[2] * 0.8,
                rgb,
                0.85f,
                maxAge,
                0.10
            );
        }
    }
}
