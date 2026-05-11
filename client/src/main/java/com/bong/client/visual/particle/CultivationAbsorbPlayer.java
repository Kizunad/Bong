package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import com.bong.client.season.SeasonBreakthroughOverlay;
import com.bong.client.state.SeasonStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class CultivationAbsorbPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "cultivation_absorb");

    private static final int FALLBACK_RGB = 0x66FFCC;

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
        float alpha = (float) (0.25 + GameplayVfxUtil.strength(payload, 0.6) * 0.45);
        int count = GameplayVfxUtil.count(payload, 8, 1, 24);
        int maxAge = GameplayVfxUtil.duration(payload, 30);
        SeasonBreakthroughOverlay.MeditationProfile profile =
            SeasonBreakthroughOverlay.meditationAbsorbProfile(SeasonStateStore.snapshot(), world.getTime());
        count = Math.max(1, Math.min(32, (int) Math.round(count * profile.densityMultiplier())));

        for (int i = 0; i < count; i++) {
            double theta = world.random.nextDouble() * Math.PI * 2.0;
            double radius = 1.2 + world.random.nextDouble() * 1.8;
            double px = ox + Math.cos(theta) * radius;
            double py = oy + (world.random.nextDouble() - 0.5) * 0.8;
            double pz = oz + Math.sin(theta) * radius;
            double speed = 0.04 * profile.velocityMultiplier();
            if (profile.allowsReverseBounce() && i % 5 == 0) {
                speed = -speed;
            }
            GameplayVfxUtil.spawnSprite(
                client,
                world,
                BongParticles.lingqiRippleSprites,
                px,
                py,
                pz,
                (ox - px) * speed,
                (oy - py) * speed,
                (oz - pz) * speed,
                rgb,
                alpha,
                maxAge,
                0.12f
            );
        }
    }
}
