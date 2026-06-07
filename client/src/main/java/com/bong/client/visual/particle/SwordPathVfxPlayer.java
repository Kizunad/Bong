package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.particle.SpriteProvider;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

public final class SwordPathVfxPlayer implements VfxPlayer {
    public static final Identifier SWORD_BOND_FORM = id("sword_bond_form");
    public static final Identifier SWORD_SHATTER = id("sword_shatter");
    public static final Identifier SWORD_CONDENSE_EDGE = id("sword_condense_edge");
    public static final Identifier SWORD_CONDENSE_HIT = id("sword_condense_hit");
    public static final Identifier SWORD_QI_SLASH_PATH = id("sword_qi_slash_path");
    public static final Identifier SWORD_RESONANCE = id("sword_resonance");
    public static final Identifier SWORD_MANIFEST_SUMMON = id("sword_manifest_summon");
    public static final Identifier SWORD_MANIFEST_STRIKE = id("sword_manifest_strike");
    public static final Identifier HEAVEN_GATE_CHARGE_0S = id("heaven_gate_charge_0s");
    public static final Identifier HEAVEN_GATE_CHARGE_1S = id("heaven_gate_charge_1s");
    public static final Identifier HEAVEN_GATE_CHARGE_2S = id("heaven_gate_charge_2s");
    public static final Identifier HEAVEN_GATE_FLASH = id("heaven_gate_flash");
    public static final Identifier HEAVEN_GATE_RELEASE = id("heaven_gate_release");
    public static final Identifier HEAVEN_GATE_SHOCKWAVE = id("heaven_gate_shockwave");
    public static final Identifier HEIWUSHI_MELEE_SLASH = id("heiwushi_melee_slash");
    public static final Identifier HEIWUSHI_DARK_BARRAGE = id("heiwushi_dark_barrage");
    public static final Identifier HEIWUSHI_DARK_VORTEX = id("heiwushi_dark_vortex");
    public static final Identifier HEIWUSHI_TRANSFORM = id("heiwushi_transform");
    public static final Identifier HEIWUSHI_DEATH = id("heiwushi_death");
    public static final Identifier SWORD_SCROLL_READ = id("sword_scroll_read");

    public static final Identifier[] EVENT_IDS = {
        SWORD_BOND_FORM,
        SWORD_SHATTER,
        SWORD_CONDENSE_EDGE,
        SWORD_CONDENSE_HIT,
        SWORD_QI_SLASH_PATH,
        SWORD_RESONANCE,
        SWORD_MANIFEST_SUMMON,
        SWORD_MANIFEST_STRIKE,
        HEAVEN_GATE_CHARGE_0S,
        HEAVEN_GATE_CHARGE_1S,
        HEAVEN_GATE_CHARGE_2S,
        HEAVEN_GATE_FLASH,
        HEAVEN_GATE_RELEASE,
        HEAVEN_GATE_SHOCKWAVE,
        HEIWUSHI_MELEE_SLASH,
        HEIWUSHI_DARK_BARRAGE,
        HEIWUSHI_DARK_VORTEX,
        HEIWUSHI_TRANSFORM,
        HEIWUSHI_DEATH,
        SWORD_SCROLL_READ
    };

    private static Identifier id(String path) {
        return new Identifier("bong", path);
    }

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = GameplayVfxUtil.world(client);
        if (world == null) {
            return;
        }

        Identifier eventId = payload.eventId();
        double ox = payload.origin()[0];
        double oy = payload.origin()[1];
        double oz = payload.origin()[2];
        float[] rgb = GameplayVfxUtil.rgb(payload, fallbackRgb(eventId));
        int count = GameplayVfxUtil.count(payload, fallbackCount(eventId), 1, 48);
        int duration = GameplayVfxUtil.duration(payload, fallbackDuration(eventId));

        if (eventId.equals(SWORD_RESONANCE) || eventId.equals(HEAVEN_GATE_SHOCKWAVE)
            || eventId.equals(HEAVEN_GATE_RELEASE) || eventId.equals(HEIWUSHI_DARK_VORTEX)) {
            GameplayVfxUtil.spawnDecal(client, world, BongParticles.lingqiRippleSprites,
                ox, oy + 0.03, oz, rgb, 0.72f, duration, radiusFor(eventId));
            return;
        }

        if (eventId.equals(SWORD_QI_SLASH_PATH) || eventId.equals(HEIWUSHI_MELEE_SLASH)) {
            double[] dir = GameplayVfxUtil.direction(payload, new double[] { 1.0, 0.0, 0.0 });
            GameplayVfxUtil.spawnLine(client, world, BongParticles.swordSlashArcSprites,
                ox, oy, oz, dir[0] * 1.1, dir[1] * 1.1, dir[2] * 1.1,
                rgb, 0.85f, duration, 0.16);
            return;
        }

        SpriteProvider provider = spriteProviderFor(eventId);
        for (int i = 0; i < count; i++) {
            double vx = (world.random.nextDouble() - 0.5) * velocityScale(eventId);
            double vy = world.random.nextDouble() * velocityScale(eventId) + yLift(eventId);
            double vz = (world.random.nextDouble() - 0.5) * velocityScale(eventId);
            GameplayVfxUtil.spawnSprite(client, world, provider,
                ox + (world.random.nextDouble() - 0.5) * 0.35,
                oy + world.random.nextDouble() * 0.7,
                oz + (world.random.nextDouble() - 0.5) * 0.35,
                vx, vy, vz, rgb, alphaFor(eventId), duration, scaleFor(eventId));
        }
    }

    private static SpriteProvider spriteProviderFor(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_DARK_BARRAGE) || eventId.equals(HEIWUSHI_TRANSFORM)
            || eventId.equals(HEIWUSHI_DEATH) || eventId.equals(HEAVEN_GATE_FLASH)) {
            return BongParticles.tribulationSparkSprites;
        }
        if (eventId.equals(SWORD_MANIFEST_SUMMON) || eventId.equals(SWORD_MANIFEST_STRIKE)) {
            return BongParticles.flyingSwordTrailSprites;
        }
        return BongParticles.swordQiTrailSprites;
    }

    private static int fallbackRgb(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_DARK_BARRAGE) || eventId.equals(HEIWUSHI_DARK_VORTEX)
            || eventId.equals(HEIWUSHI_TRANSFORM)) {
            return 0x2A0033;
        }
        if (eventId.equals(HEIWUSHI_DEATH)) {
            return 0x334455;
        }
        if (eventId.equals(HEAVEN_GATE_FLASH) || eventId.equals(HEAVEN_GATE_RELEASE)
            || eventId.equals(HEAVEN_GATE_SHOCKWAVE)) {
            return 0xE8F0FF;
        }
        if (eventId.equals(SWORD_SCROLL_READ)) {
            return 0xAABBCC;
        }
        return 0xC8D8E8;
    }

    private static int fallbackCount(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_TRANSFORM)) {
            return 32;
        }
        if (eventId.equals(HEIWUSHI_DEATH)) {
            return 48;
        }
        if (eventId.equals(SWORD_SCROLL_READ)) {
            return 6;
        }
        if (eventId.equals(SWORD_SHATTER)) {
            return 18;
        }
        return 10;
    }

    private static int fallbackDuration(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_DEATH)) {
            return 60;
        }
        if (eventId.equals(HEAVEN_GATE_SHOCKWAVE) || eventId.equals(HEIWUSHI_DARK_VORTEX)) {
            return 30;
        }
        return 20;
    }

    private static double radiusFor(Identifier eventId) {
        if (eventId.equals(HEAVEN_GATE_RELEASE) || eventId.equals(HEAVEN_GATE_SHOCKWAVE)) {
            return 3.0;
        }
        if (eventId.equals(HEIWUSHI_DARK_VORTEX)) {
            return 2.4;
        }
        return 1.2;
    }

    private static double velocityScale(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_TRANSFORM) || eventId.equals(SWORD_SHATTER)) {
            return 0.28;
        }
        if (eventId.equals(HEIWUSHI_DARK_BARRAGE)) {
            return 0.10;
        }
        return 0.14;
    }

    private static double yLift(Identifier eventId) {
        if (eventId.equals(HEIWUSHI_DEATH)) {
            return 0.03;
        }
        if (eventId.equals(SWORD_SCROLL_READ)) {
            return 0.08;
        }
        return 0.01;
    }

    private static float alphaFor(Identifier eventId) {
        if (eventId.equals(HEAVEN_GATE_FLASH)) {
            return 0.95f;
        }
        return 0.78f;
    }

    private static float scaleFor(Identifier eventId) {
        if (eventId.equals(HEAVEN_GATE_FLASH)) {
            return 0.16f;
        }
        return 0.10f;
    }
}
