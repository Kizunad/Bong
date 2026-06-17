package com.bong.client.visual.particle;

import com.bong.client.network.VfxEventPayload;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.world.ClientWorld;
import net.minecraft.util.Identifier;

import java.util.OptionalInt;

/** Spawns several inward ribbon trails around a woliu-v2 low-pressure point. */
public final class VortexSpiralPlayer implements VfxPlayer {
    public static final Identifier EVENT_ID = new Identifier("bong", "vortex_spiral");
    public static final Identifier VACUUM_PALM = new Identifier("bong", "woliu_vacuum_palm_spiral");
    public static final Identifier VORTEX_SHIELD = new Identifier("bong", "woliu_vortex_shield_sphere");
    public static final Identifier VACUUM_LOCK = new Identifier("bong", "woliu_vacuum_lock_cage");
    public static final Identifier VORTEX_RESONANCE = new Identifier("bong", "woliu_vortex_resonance_field");
    public static final Identifier TURBULENCE_BURST = new Identifier("bong", "woliu_turbulence_burst_wave");
    // plan-woliu-path-v1：虚蚀路径 5 招式 — particle IDs must match visual_for() in server skills.rs exactly
    public static final Identifier VORTEX_AMBIENT = new Identifier("bong", "vortex_ambient");
    public static final Identifier VOID_SPHERE = new Identifier("bong", "woliu_void_sphere");
    public static final Identifier SWALLOWING_SPIRAL = new Identifier("bong", "woliu_swallowing_spiral");
    public static final Identifier ECHO_RIPPLE = new Identifier("bong", "woliu_echo_ripple");
    public static final Identifier VOID_CORE_COLLAPSE = new Identifier("bong", "woliu_void_core_collapse");

    private static final int DEFAULT_COUNT = 10;
    private static final int FALLBACK_RGB = 0x201832;

    @Override
    public void play(MinecraftClient client, VfxEventPayload.SpawnParticle payload) {
        ClientWorld world = client.world;
        if (world == null) return;
        EffectSpec spec = effectSpec(payload);

        if (spec.route() == Route.RESONANCE_FIELD) {
            playResonanceField(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.TURBULENCE_BURST) {
            playTurbulenceBurst(client, world, payload, spec);
            return;
        }
        // plan-woliu-path-v1：虚蚀路径路由
        if (spec.route() == Route.VORTEX_AMBIENT) {
            playVortexAmbient(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.VOID_SPHERE) {
            playVoidSphere(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.SWALLOWING_SPIRAL) {
            playSwallowingSpiral(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.ECHO_RIPPLE) {
            playEchoRipple(client, world, payload, spec);
            return;
        }
        if (spec.route() == Route.VOID_CORE_COLLAPSE) {
            playVoidCoreCollapse(client, world, payload, spec);
            return;
        }

        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            double angle = (Math.PI * 2.0 * i / spec.count()) + world.random.nextDouble() * 0.35;
            double radius = 0.35 + world.random.nextDouble() * 0.65;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.45;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * 0.035,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * 0.035,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.055 + spec.strength() * 0.08);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge());
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playResonanceField(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.95;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 3;
            double ringRatio = 0.34 + ring * 0.28;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.22;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.35;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 2.0 + ring) * 0.32 + (world.random.nextDouble() - 0.5) * 0.18;
            double tangent = 0.055 + spec.strength() * 0.045 + ring * 0.012;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.cos(angle) * tangent,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.09 + spec.strength() * 0.09 + ring * 0.015);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static void playTurbulenceBurst(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.75;
        double oz = payload.origin()[2];
        float[] color = rgb(payload);

        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.16;
            double x = ox + Math.cos(angle) * spec.radius();
            double z = oz + Math.sin(angle) * spec.radius();
            double y = oy + (world.random.nextDouble() - 0.5) * 0.5;
            double speed = 0.10 + spec.strength() * 0.08 + world.random.nextDouble() * 0.04;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world,
                x,
                y,
                z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.025,
                Math.sin(angle) * speed,
                ox,
                oy,
                oz
            );
            particle.setAngularVelocity(0.02 + spec.strength() * 0.04);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** AmbientVortex：持续低频旋涡，深紫慢速收缩，粒子绕轴慢转。 */
    private static void playVortexAmbient(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.8;
        double oz = payload.origin()[2];
        // fallback: deep purple if no color in payload
        int rgb = payload.colorRgb().orElse(0x2A0A3C);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = (Math.PI * 2.0 * i / spec.count()) + world.random.nextDouble() * 0.5;
            double radius = 0.4 + world.random.nextDouble() * 0.5;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.6;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * 0.018,
                (world.random.nextDouble() - 0.5) * 0.008,
                Math.cos(angle) * 0.018,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.025 + spec.strength() * 0.03);
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() + world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VoidVortex：虚空球体，多环密排粒子，偏蓝灰。 */
    private static void playVoidSphere(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.0;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x1C2B38);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 4;
            double ringRatio = 0.25 + ring * 0.22;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.18;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.25;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + Math.sin(angle * 2.0 + ring * 0.7) * 0.45 + (world.random.nextDouble() - 0.5) * 0.2;
            double tangent = 0.04 + spec.strength() * 0.035 + ring * 0.008;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                -Math.sin(angle) * tangent,
                (world.random.nextDouble() - 0.5) * 0.010,
                Math.cos(angle) * tangent,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.07 + spec.strength() * 0.06 + ring * 0.010);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 5)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** SwallowingVortex：吞噬螺旋，大半径向心拉扯，偏暗红。 */
    private static void playSwallowingSpiral(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.9;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x3D0A0A);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            int ring = i % 3;
            double ringRatio = 0.30 + ring * 0.32;
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.25;
            double radius = spec.radius() * ringRatio + (world.random.nextDouble() - 0.5) * 0.4;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.55;
            // inward velocity — swallowing effect
            double inward = -(0.06 + spec.strength() * 0.05 + ring * 0.01);
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * inward,
                (world.random.nextDouble() - 0.5) * 0.012,
                Math.sin(angle) * inward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.08 + spec.strength() * 0.08 + ring * 0.012);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 4)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VortexEcho：余震涟漪，轻量环形脉冲，偏冷青。 */
    private static void playEchoRipple(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 1.05;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x0A2830);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.20;
            double radius = spec.radius() + (world.random.nextDouble() - 0.5) * 0.3;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            // flat ripple — minimal vertical spread
            double y = oy + (world.random.nextDouble() - 0.5) * 0.15;
            double outward = 0.03 + spec.strength() * 0.025;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * outward,
                (world.random.nextDouble() - 0.5) * 0.006,
                Math.sin(angle) * outward,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.04 + spec.strength() * 0.03);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    /** VoidCore：虚核崩塌，高密度中心内爆，偏墨色。 */
    private static void playVoidCoreCollapse(
        MinecraftClient client,
        ClientWorld world,
        VfxEventPayload.SpawnParticle payload,
        EffectSpec spec
    ) {
        double ox = payload.origin()[0];
        double oy = payload.origin()[1] + 0.85;
        double oz = payload.origin()[2];
        int rgb = payload.colorRgb().orElse(0x0A0A12);
        float[] color = new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
        for (int i = 0; i < spec.count(); i++) {
            double angle = Math.PI * 2.0 * i / spec.count() + world.random.nextDouble() * 0.12;
            double radius = spec.radius() + (world.random.nextDouble() - 0.5) * 0.2;
            double x = ox + Math.cos(angle) * radius;
            double z = oz + Math.sin(angle) * radius;
            double y = oy + (world.random.nextDouble() - 0.5) * 0.4;
            double speed = 0.12 + spec.strength() * 0.10 + world.random.nextDouble() * 0.05;
            VortexSpiralParticle particle = new VortexSpiralParticle(
                world, x, y, z,
                Math.cos(angle) * speed,
                (world.random.nextDouble() - 0.2) * 0.030,
                Math.sin(angle) * speed,
                ox, oy, oz
            );
            particle.setAngularVelocity(0.03 + spec.strength() * 0.05);
            particle.setRibbonWidth(spec.ribbonWidth(), spec.ribbonEndWidth());
            particle.setColor(color[0], color[1], color[2]);
            particle.setAlphaPublic((float) spec.alpha());
            particle.setMaxAgePublic(spec.maxAge() - world.random.nextInt(Math.max(1, spec.maxAge() / 3)));
            if (BongParticles.vortexSpiralSprites != null) {
                particle.setSpritePublic(BongParticles.vortexSpiralSprites.getSprite(world.random));
            }
            client.particleManager.addParticle(particle);
        }
    }

    private static float[] rgb(VfxEventPayload.SpawnParticle payload) {
        int rgb = payload.colorRgb().orElse(FALLBACK_RGB);
        return new float[] {
            ((rgb >> 16) & 0xFF) / 255f,
            ((rgb >> 8) & 0xFF) / 255f,
            (rgb & 0xFF) / 255f
        };
    }

    static EffectSpec effectSpec(VfxEventPayload.SpawnParticle payload) {
        if (VORTEX_RESONANCE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.8));
            return new EffectSpec(
                Route.RESONANCE_FIELD,
                clamp(payload.count().orElse(48), 24, 96),
                clamp(payload.durationTicks().orElse(80), 30, 120),
                strength,
                2.2 + strength * 3.8,
                Math.min(0.9, 0.48 + strength * 0.34),
                0.12 + strength * 0.05,
                0.018
            );
        }
        if (TURBULENCE_BURST.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.9));
            return new EffectSpec(
                Route.TURBULENCE_BURST,
                clamp(payload.count().orElse(64), 24, 96),
                clamp(payload.durationTicks().orElse(44), 18, 80),
                strength,
                0.6 + strength * 0.7,
                Math.min(0.92, 0.55 + strength * 0.32),
                0.14 + strength * 0.04,
                0.02
            );
        }
        // plan-woliu-path-v1：虚蚀路径 5 招式 effectSpec（色相/形状各异化）
        // AmbientVortex：持续低频旋涡，颜色偏深紫，慢速收缩
        if (VORTEX_AMBIENT.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.6));
            return new EffectSpec(
                Route.VORTEX_AMBIENT,
                clamp(payload.count().orElse(16), 8, 48),
                clamp(payload.durationTicks().orElse(60), 30, 120),
                strength,
                0.0,
                Math.min(0.75, 0.38 + strength * 0.32),
                0.0,
                0.0
            );
        }
        // VoidVortex：虚空球体，较高粒子密度，偏蓝灰色
        if (VOID_SPHERE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.85));
            return new EffectSpec(
                Route.VOID_SPHERE,
                clamp(payload.count().orElse(36), 16, 72),
                clamp(payload.durationTicks().orElse(55), 20, 100),
                strength,
                1.8 + strength * 2.0,
                Math.min(0.88, 0.52 + strength * 0.3),
                0.10 + strength * 0.04,
                0.016
            );
        }
        // SwallowingVortex：吞噬螺旋，半径大，向心拉扯感强，偏暗红
        if (SWALLOWING_SPIRAL.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.9));
            return new EffectSpec(
                Route.SWALLOWING_SPIRAL,
                clamp(payload.count().orElse(52), 24, 96),
                clamp(payload.durationTicks().orElse(70), 30, 120),
                strength,
                3.0 + strength * 2.5,
                Math.min(0.90, 0.50 + strength * 0.34),
                0.13 + strength * 0.05,
                0.019
            );
        }
        // VortexEcho：余震涟漪，轻量环形脉冲，偏冷青色
        if (ECHO_RIPPLE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(0.7));
            return new EffectSpec(
                Route.ECHO_RIPPLE,
                clamp(payload.count().orElse(28), 12, 56),
                clamp(payload.durationTicks().orElse(38), 15, 80),
                strength,
                1.5 + strength * 1.8,
                Math.min(0.82, 0.44 + strength * 0.32),
                0.09 + strength * 0.03,
                0.014
            );
        }
        // VoidCore：虚核崩塌，最高强度，致密中心球爆炸，偏墨色
        if (VOID_CORE_COLLAPSE.equals(payload.eventId())) {
            double strength = clamp01(payload.strength().orElse(1.0));
            return new EffectSpec(
                Route.VOID_CORE_COLLAPSE,
                clamp(payload.count().orElse(72), 32, 96),
                clamp(payload.durationTicks().orElse(50), 20, 90),
                strength,
                0.8 + strength * 0.9,
                Math.min(0.95, 0.60 + strength * 0.30),
                0.16 + strength * 0.04,
                0.022
            );
        }
        double strength = clamp01(payload.strength().orElse(0.75));
        return new EffectSpec(
            Route.SPIRAL,
            clamp(payload.count().orElse(OptionalInt.of(DEFAULT_COUNT).getAsInt()), 1, 64),
            clamp(payload.durationTicks().orElse(42), 1, 120),
            strength,
            0.0,
            Math.max(0.35, Math.min(0.95, 0.45 + strength * 0.5)),
            0.0,
            0.0
        );
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private static double clamp01(double value) {
        return Math.max(0.0, Math.min(1.0, value));
    }

    enum Route {
        SPIRAL,
        RESONANCE_FIELD,
        TURBULENCE_BURST,
        // plan-woliu-path-v1：虚蚀路径 5 招式路由
        VORTEX_AMBIENT,
        VOID_SPHERE,
        SWALLOWING_SPIRAL,
        ECHO_RIPPLE,
        VOID_CORE_COLLAPSE
    }

    record EffectSpec(
        Route route,
        int count,
        int maxAge,
        double strength,
        double radius,
        double alpha,
        double ribbonWidth,
        double ribbonEndWidth
    ) {}
}
