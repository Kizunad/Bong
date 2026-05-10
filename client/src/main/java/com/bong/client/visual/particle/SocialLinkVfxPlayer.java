package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class SocialLinkVfxPlayer implements VfxPlayer {
    public static final Identifier NICHE_ESTABLISH = new Identifier("bong", "social_niche_establish");

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, 0xC4E0FF);
        int maxAge = GameplayVfxUtil.duration(payload, 60);
        for (int i = 0; i < 3; i++) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
                ox, oy + i * 0.05, oz, rgb, 0.55f, maxAge + i * 8, 0.8 + i * 0.35);
        }
    }
}
