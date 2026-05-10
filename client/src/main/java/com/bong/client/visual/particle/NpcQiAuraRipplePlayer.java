package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/**
 * Subtle ground ripple used by high-realm hydrated NPCs.
 */
public final class NpcQiAuraRipplePlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "npc_qi_aura_ripple");

    private static final int FALLBACK_RGB = 0x8FE6B8;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        double strength = Math.max(0.1, Math.min(1.0, payload.strength().orElse(0.35)));

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(35).getAsInt());

        BongGroundDecalParticle ripple = new BongGroundDecalParticle(world, ox, oy, oz);
        ripple.setDecalShape(0.9 + strength * 1.4, 0.025);
        ripple.setSpin(world.random.nextDouble() * Math.PI * 2, 0.025);
        ripple.setColor(r, g, b);
        ripple.setAlphaPublic((float) (0.22 + strength * 0.22));
        ripple.setMaxAgePublic(maxAge);
        if (BongParticles.lingqiRippleSprites != null) {
            ripple.setSpritePublic(BongParticles.lingqiRippleSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(ripple);
    }
}
