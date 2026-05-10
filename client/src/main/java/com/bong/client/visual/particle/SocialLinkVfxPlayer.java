package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class SocialLinkVfxPlayer implements VfxPlayer {
    public static final Identifier NICHE_ESTABLISH = new Identifier("bong", "social_niche_establish");
    public static final Identifier PACT_LINK = new Identifier("bong", "social_pact_link");
    public static final Identifier FEUD_MARK = new Identifier("bong", "social_feud_mark");

    private final Kind kind;

    public SocialLinkVfxPlayer() {
        this(Kind.NICHE_ESTABLISH);
    }

    public SocialLinkVfxPlayer(Kind kind) {
        this.kind = kind;
    }

    public enum Kind {
        NICHE_ESTABLISH,
        PACT_LINK,
        FEUD_MARK
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        int maxAge = GameplayVfxUtil.duration(payload, 60);
        if (kind == Kind.PACT_LINK) {
            double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
            float[] rgb = GameplayVfxUtil.rgb(payload, 0xC4E0FF);
            GameplayVfxUtil.spawnLine(client, world, BongParticles.qiAuraSprites,
                ox, oy, oz, dir[0], dir[1], dir[2], rgb, 0.75f, maxAge, 0.12);
            return;
        }
        if (kind == Kind.FEUD_MARK) {
            float[] rgb = GameplayVfxUtil.rgb(payload, 0xFF3344);
            int count = GameplayVfxUtil.count(payload, 6, 1, 12);
            for (int i = 0; i < count; i++) {
                GameplayVfxUtil.spawnSprite(client, world, BongParticles.runeCharSprites,
                    ox + (world.random.nextDouble() - 0.5) * 0.25,
                    oy + world.random.nextDouble() * 0.35,
                    oz + (world.random.nextDouble() - 0.5) * 0.25,
                    (world.random.nextDouble() - 0.5) * 0.03,
                    0.02 + world.random.nextDouble() * 0.03,
                    (world.random.nextDouble() - 0.5) * 0.03,
                    rgb, 0.85f, maxAge, 0.20f);
            }
            return;
        }

        float[] rgb = GameplayVfxUtil.rgb(payload, 0xC4E0FF);
        for (int i = 0; i < 3; i++) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
                ox, oy + i * 0.05, oz, rgb, 0.55f, maxAge + i * 8, 0.8 + i * 0.35);
        }
    }
}
