package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** 灵田地块灵纹 decal，复用同一个 player 承接开垦/种植/补灵/收获/吸灵事件。 */
public final class LingtianPlotRunePlayer implements VfxPlayer {
    public static final Identifier TILL = new Identifier("bong", "lingtian_till");
    public static final Identifier PLANT = new Identifier("bong", "lingtian_plant");
    public static final Identifier REPLENISH = new Identifier("bong", "lingtian_replenish");
    public static final Identifier HARVEST = new Identifier("bong", "lingtian_harvest");
    public static final Identifier DRAIN = new Identifier("bong", "lingtian_drain");

    private static final int FALLBACK_RGB = 0x44CCCC;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;

        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        float r = ((rgb >> 16) & 0xFF) / 255f;
        float g = ((rgb >> 8) & 0xFF) / 255f;
        float b = (rgb & 0xFF) / 255f;
        double strength = payload.strength().orElse(0.7);
        int maxAge = payload.durationTicks().orElse(OptionalInt.of(80).getAsInt());
        double halfSize = 0.42 + 0.28 * strength;

        BongGroundDecalParticle decal = new BongGroundDecalParticle(
            world,
            payload.origin()[0],
            payload.origin()[1],
            payload.origin()[2]
        );
        decal.setDecalShape(halfSize, 0.025);
        decal.setSpin(world.random.nextDouble() * Math.PI * 2.0, spinFor(payload.eventId()));
        decal.setColor(r, g, b);
        decal.setAlphaPublic((float) Math.max(0.32, Math.min(0.86, strength)));
        decal.setMaxAgePublic(Math.max(30, Math.min(140, maxAge)));
        if (BongParticles.runeCharSprites != null) {
            decal.setSpritePublic(BongParticles.runeCharSprites.getSprite(world.random));
        }
        client.particleManager.addParticle(decal);
    }

    private static double spinFor(Identifier eventId) {
        if (DRAIN.equals(eventId)) {
            return -0.035;
        }
        if (REPLENISH.equals(eventId)) {
            return 0.055;
        }
        return 0.025;
    }
}
